//! New versions of Fogata from friends and group members, with no server (`fogata-update` for
//! the release format and its rules, `fogata_net::updates` for the protocol).
//!
//! A device asks each friend and member it connects to that takes the protocol whether it
//! holds a newer release than the build it runs or has. One that does sends the manifest; it
//! is checked (key, signature, fields, a newer build) and kept, and the package for this
//! platform is fetched from whoever has it, chunk by chunk, each chunk checked as it comes.
//! A peer that sent a damaged chunk is not asked again for that release. Once the whole
//! package hashes right it is ready to install, and this device tells its friends and members
//! it has the release, so it spreads from friend to friend. It gives on the packages it has,
//! and fetches the package of another platform too when a friend or member asks it for one
//! it does not have, so a friend on another system behind it gets it as well: one such
//! package at a time, let go once nobody has asked for it in two days (the publisher's own
//! copy keeps every platform's).
//!
//! Nothing is installed by itself: [`crate::Core::install_update`] does it when asked.

mod install;
pub(crate) mod store;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

use anyhow::Result;
use fogata_crypto::DevicePublicKey;
use fogata_net::{
    NodeHandle, PeerId,
    updates::{self as wire, FetchError, Heard, Held, Misbehave, Offer, Shelf, UPDATE_PROTOCOL},
};
use fogata_update::{KeyRole, Manifest, Refusal, ReleaseKey, Signed};
use tokio::sync::mpsc;

pub use install::{Installer, installer};
pub use store::publish;

use crate::CoreEvent;

/// How often the updater looks at who is connected.
const TICK: Duration = Duration::from_secs(5);
/// A friend or member is asked again after this long on the same connection.
const ASK_AGAIN: Duration = Duration::from_secs(20 * 60);
/// A peer that is neither friend nor member yet is looked at again after this long.
const NOT_YET: Duration = Duration::from_secs(20);
/// A peer whose download failed is tried again after this long.
const RETRY: Duration = Duration::from_secs(45);
/// A peer that offered a release that was refused, or sent a damaged chunk, is left alone
/// for this long.
const SHUNNED: Duration = Duration::from_secs(60 * 60);
/// A peer with the release but not the package wanted is asked for it at most this often,
/// which makes it fetch that package itself.
const NUDGE: Duration = Duration::from_secs(60);
/// A peer that said it was asked too soon is asked again after this long.
const RETRY_ASK: Duration = Duration::from_secs(5);
/// A peer's word that it has a release makes it asked again, at most this often.
const HAVE_AGAIN: Duration = Duration::from_secs(30);
/// Notes kept for the command line.
const NOTES: usize = 64;
/// A package of another platform, kept for friends and members on that system, goes once
/// none has asked for it for this long.
const OTHER_IDLE: Duration = Duration::from_secs(48 * 3600);
/// A package of another platform makes way for one wanted now once none has asked for it
/// for this long.
const OTHER_YIELDS: Duration = Duration::from_secs(60 * 60);
/// The most bytes the packages kept may take together: a package of another platform is
/// fetched only while this platform's and it stay under this.
const DISK_CAP: u64 = 600 * 1024 * 1024;

/// How the updater runs.
#[derive(Clone, Debug)]
pub struct UpdateOptions {
    /// The release keys trusted ([`fogata_update::compiled_keys`] in the application).
    pub keys: Vec<ReleaseKey>,
    /// The build this copy runs ([`fogata_update::THIS_BUILD`]).
    pub build: u64,
    pub version: String,
    /// The platform whose package this copy fetches and installs.
    pub platform: Option<String>,
    /// Network lab only: the server misbehaves, and the release on disk is given on without
    /// checking it ([`UpdateOptions::lab`]).
    misbehave: Misbehave,
    unchecked: bool,
}

impl UpdateOptions {
    /// The application's own: the keys, build and platform compiled in.
    pub fn compiled_in() -> UpdateOptions {
        UpdateOptions {
            keys: fogata_update::compiled_keys(),
            build: fogata_update::THIS_BUILD,
            version: fogata_update::THIS_VERSION.to_string(),
            platform: fogata_update::this_platform().map(str::to_string),
            misbehave: Misbehave::default(),
            unchecked: false,
        }
    }

    /// Network lab only: the server misbehaves in these ways, and the release on disk is
    /// given on without checking it. Only the command-line program is built with this.
    #[cfg(feature = "lab")]
    pub fn lab(&mut self, misbehave: Misbehave, unchecked: bool) {
        self.misbehave = misbehave;
        self.unchecked = unchecked;
    }
}

/// Where updates stand, for the screens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateStatus {
    pub this_build: u64,
    pub this_version: String,
    pub state: UpdateState,
    /// The newest release known here.
    pub release: Option<ReleaseInfo>,
    /// This copy is older than the newest release's oldest compatible build: it must update
    /// to keep talking to copies of that release.
    pub required: bool,
    /// The friend or member this platform's package comes from, while it downloads.
    pub from: Option<DevicePublicKey>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub build: u64,
    pub version: String,
    pub notes: String,
    pub min_build: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateState {
    /// Nothing newer than this copy is known.
    UpToDate,
    /// Asking friends and members.
    Checking,
    /// The package is on its way: `done` of `total` bytes.
    Downloading {
        done: u64,
        total: u64,
    },
    /// The package is being checked whole.
    Verifying,
    /// A newer release is here, checked, ready to install.
    Ready,
    /// The install is under way; the application quits next.
    Installing,
    Failed {
        kind: FailKind,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailKind {
    /// The newest release has no package for this platform.
    NoPackage,
    /// The package cannot be written or read here.
    Disk,
    /// This copy cannot install itself (not where installs go, or not a release build).
    CannotInstall,
    /// The install did not go through.
    Install,
    /// An install was started, and this copy is still the older build.
    DidNotTake,
}

/// The updater of a running core.
pub(crate) struct Updater {
    /// `updates/` in the data folder.
    dir: PathBuf,
    data: PathBuf,
    node: NodeHandle,
    shelf: Shelf,
    options: UpdateOptions,
    /// Shared with the downloads' tasks, for their progress.
    phase: Arc<Mutex<Phase>>,
    notes: Mutex<VecDeque<String>>,
    events: crate::events::Sender,
    commands: mpsc::UnboundedSender<Command>,
    /// The release held was published from this data folder: every package stays.
    publisher: std::sync::atomic::AtomicBool,
}

impl std::fmt::Debug for Updater {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Updater")
            .field("build", &self.options.build)
            .finish_non_exhaustive()
    }
}

/// What the updater is doing, behind the screens' [`UpdateStatus`].
#[derive(Clone, Debug, Default)]
struct Phase {
    /// The newest release known, checked.
    release: Option<Manifest>,
    /// The package for this platform is complete and checked.
    ready: bool,
    asking: usize,
    /// Bytes of this platform's package there, while it comes, and from whom.
    downloading: Option<(u64, u64)>,
    from: Option<DevicePublicKey>,
    verifying: bool,
    installing: bool,
    failed: Option<(FailKind, String)>,
}

enum Command {
    CheckNow,
}

/// What the tasks the updater starts give back.
enum Done {
    Asked(PeerId, Result<Option<Offer>, FetchError>),
    Fetched {
        peer: PeerId,
        build: u64,
        platform: String,
        result: Result<(), FetchError>,
    },
    Told,
}

impl Updater {
    /// Opens what `data/updates/` holds and starts giving and fetching releases.
    pub(crate) fn start(
        data: &Path,
        node: NodeHandle,
        options: UpdateOptions,
        events: crate::events::Sender,
    ) -> Result<Arc<Updater>> {
        tracing::info!(
            "build {} ({}), {} {}",
            options.build,
            options.version,
            fogata_update::build_marker(),
            fogata_update::commit_marker()
        );
        let dir = store::dir(data);
        let shelf = Shelf::default();
        let (heard_tx, heard_rx) = mpsc::unbounded_channel();
        wire::serve(node.clone(), shelf.clone(), heard_tx, options.misbehave)?;
        let (commands, commands_rx) = mpsc::unbounded_channel();
        let updater = Arc::new(Updater {
            dir,
            data: data.to_path_buf(),
            node,
            shelf,
            options,
            phase: Arc::default(),
            notes: Mutex::new(VecDeque::new()),
            events,
            commands,
            publisher: std::sync::atomic::AtomicBool::new(false),
        });
        let weak = Arc::downgrade(&updater);
        tokio::spawn(async move {
            if let Some(updater) = weak.upgrade() {
                updater.load().await;
            }
            run(weak, heard_rx, commands_rx).await;
        });
        Ok(updater)
    }

    pub(crate) fn status(&self) -> UpdateStatus {
        let phase = self.lock().clone();
        let build = self.options.build;
        let state = if phase.installing {
            UpdateState::Installing
        } else if let Some((kind, message)) = phase.failed {
            UpdateState::Failed { kind, message }
        } else if phase.verifying {
            UpdateState::Verifying
        } else if let Some((done, total)) = phase.downloading {
            UpdateState::Downloading { done, total }
        } else if phase.ready && phase.release.as_ref().is_some_and(|r| r.build > build) {
            UpdateState::Ready
        } else if phase.asking > 0 {
            UpdateState::Checking
        } else {
            UpdateState::UpToDate
        };
        UpdateStatus {
            this_build: build,
            this_version: self.options.version.clone(),
            state,
            required: phase.release.as_ref().is_some_and(|r| r.min_build > build),
            from: phase.downloading.and(phase.from),
            release: phase.release.map(|m| ReleaseInfo {
                build: m.build,
                version: m.version,
                notes: m.notes,
                min_build: m.min_build,
            }),
        }
    }

    /// Asks every friend and member connected again now.
    pub(crate) fn check_now(&self) {
        let _ = self.commands.send(Command::CheckNow);
    }

    /// Notes for the command line: releases refused and why, packages fetched.
    pub(crate) fn notes(&self) -> Vec<String> {
        locked(&self.notes).iter().cloned().collect()
    }

    /// Installs the release that is ready: see [`install::install`].
    pub(crate) async fn install(&self) -> Result<()> {
        let release = {
            let mut phase = self.lock();
            let Some(release) = phase
                .release
                .clone()
                .filter(|r| r.build > self.options.build)
            else {
                anyhow::bail!("no newer release is here");
            };
            if !phase.ready {
                anyhow::bail!("the new release is not here whole yet");
            }
            if phase.installing {
                anyhow::bail!("the install is under way");
            }
            phase.installing = true;
            phase.failed = None;
            release
        };
        self.changed();
        let result = install::install(&self.data, &release, self.platform()).await;
        if let Err(e) = &result {
            let mut phase = self.lock();
            phase.installing = false;
            let kind = match e.downcast_ref::<install::CannotInstall>() {
                Some(_) => FailKind::CannotInstall,
                None => FailKind::Install,
            };
            phase.failed = Some((kind, format!("{e:#}")));
        }
        self.changed();
        result
    }

    fn platform(&self) -> &str {
        self.options.platform.as_deref().unwrap_or("")
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Phase> {
        locked(&self.phase)
    }

    fn changed(&self) {
        let _ = self.events.send(CoreEvent::UpdateChanged);
    }

    fn note(&self, text: String) {
        tracing::info!("update: {text}");
        let mut notes = locked(&self.notes);
        if notes.len() == NOTES {
            notes.pop_front();
        }
        notes.push_back(format!("{} {text}", crate::now_ms()));
    }

    /// The newest build this copy has or runs.
    fn best(&self) -> u64 {
        let held = self.lock().release.as_ref().map(|r| r.build).unwrap_or(0);
        held.max(self.options.build)
    }

    /// Reads the release kept on disk, checks it again with its packages, and holds it.
    async fn load(&self) {
        let dir = self.dir.clone();
        let options = self.options.clone();
        let platform = self.platform().to_string();
        self.lock().verifying = true;
        self.changed();
        let loaded = tokio::task::spawn_blocking(move || load(&dir, &options, &platform))
            .await
            .unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
        {
            let mut phase = self.lock();
            phase.verifying = false;
            match loaded {
                Ok(Some(loaded)) => {
                    phase.ready = loaded.held.packages.contains_key(self.platform());
                    phase.release = Some(loaded.held.manifest.clone());
                    phase.failed = loaded.failed;
                    self.publisher
                        .store(loaded.publisher, std::sync::atomic::Ordering::Relaxed);
                    self.shelf.set(Some(loaded.held));
                }
                Ok(None) => {}
                Err(e) => phase.failed = Some((FailKind::Disk, format!("{e:#}"))),
            }
        }
        self.changed();
    }

    /// Takes a release offered by `peer` if it is good and newer; returns whether it is the
    /// one held now (taken now or before).
    async fn offered(&self, peer: PeerId, from: &str, offer: &Offer) -> bool {
        // A copy that gives what it holds unchecked (the network lab's) takes nothing.
        if self.options.unchecked {
            return false;
        }
        let checked = offer.signed.verify(&self.options.keys);
        let manifest = match checked {
            Ok(manifest) => manifest,
            Err(refusal) => {
                self.note(format!("refused a release from {from}: {refusal}"));
                return false;
            }
        };
        let held = self.lock().release.clone();
        if held.as_ref().map(|r| r.build) == Some(manifest.build) {
            return true;
        }
        if let Err(refusal) = manifest.current(now_secs()) {
            self.note(format!(
                "refused build {} from {from}: {refusal}",
                manifest.build
            ));
            return false;
        }
        let role = offer
            .signed
            .role(&self.options.keys)
            .unwrap_or(KeyRole::Release);
        if let Err(refusal) = manifest.taken_by(role, self.options.build, held.as_ref()) {
            self.note(format!(
                "refused build {} from {from}: {refusal}",
                manifest.build
            ));
            return false;
        }
        let (dir, signed) = (self.dir.clone(), offer.signed.clone());
        let kept = tokio::task::spawn_blocking(move || store::keep(&dir, &signed))
            .await
            .unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
        if let Err(e) = kept {
            self.lock().failed = Some((FailKind::Disk, format!("{e:#}")));
            self.changed();
            return false;
        }
        self.note(format!(
            "build {} ({}) from {from}, through {}",
            manifest.build,
            manifest.version,
            fogata_net::short_peer(&peer)
        ));
        self.publisher
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.shelf.set(Some(Held {
            signed: offer.signed.clone(),
            manifest: manifest.clone(),
            packages: HashMap::new(),
        }));
        {
            let mut phase = self.lock();
            phase.ready = false;
            phase.downloading = None;
            phase.failed = (!manifest.artifacts.contains_key(self.platform())).then(|| {
                (
                    FailKind::NoPackage,
                    format!("build {} has no package for this system", manifest.build),
                )
            });
            phase.release = Some(manifest);
        }
        self.changed();
        true
    }
}

/// What [`load`] found.
struct Loaded {
    held: Held,
    failed: Option<(FailKind, String)>,
    /// The release was published from this data folder.
    publisher: bool,
}

/// Reads the release in `dir`: checked with the keys, newer than or the same as this
/// build, each package hashed again. Anything else goes.
fn load(dir: &Path, options: &UpdateOptions, platform: &str) -> Result<Option<Loaded>> {
    let installed = store::installed(dir);
    let Some(signed) = store::read_signed(dir)? else {
        return Ok(None);
    };
    let checked = if options.unchecked {
        serde_json::from_slice::<Manifest>(&signed.body)
            .map_err(|e| Refusal::Malformed(e.to_string()))
    } else {
        signed.verify(&options.keys)
    };
    let role = signed.role(&options.keys).unwrap_or(KeyRole::Release);
    let manifest = match checked {
        Ok(manifest)
            if options.unchecked
                || (manifest.build >= options.build
                    && manifest.within_reach(role, options.build).is_ok()
                    && manifest.current(now_secs()).is_ok()) =>
        {
            manifest
        }
        Ok(_) | Err(_) => {
            // An older release than this copy, one too far above it (a build that went
            // back), or one the keys no longer take.
            store::forget(dir)?;
            store::forget_installed(dir);
            return Ok(None);
        }
    };
    store::remove_packages(dir, Some(&manifest))?;
    let publisher = store::published(dir);
    let mut packages = HashMap::new();
    for (name, artifact) in &manifest.artifacts {
        let path = store::package_path(dir, name);
        if !path.exists() {
            continue;
        }
        if options.unchecked || wire::check_package(&path, artifact).unwrap_or(false) {
            packages.insert(name.clone(), path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
    // Of other platforms' packages a copy keeps one, the newest; the publisher's keeps all.
    if !publisher {
        let mut others: Vec<(String, PathBuf)> = packages
            .iter()
            .filter(|(name, _)| name.as_str() != platform)
            .map(|(name, path)| (name.clone(), path.clone()))
            .collect();
        others.sort_by_key(|(_, path)| std::fs::metadata(path).and_then(|m| m.modified()).ok());
        while others.len() > 1 {
            let (name, path) = others.remove(0);
            let _ = std::fs::remove_file(&path);
            packages.remove(&name);
        }
    }
    let failed = match installed {
        Some(note) if note.build > options.build && note.build == manifest.build => Some((
            FailKind::DidNotTake,
            format!(
                "build {} was installed, and this copy is still build {}",
                note.build, options.build
            ),
        )),
        Some(_) => {
            store::forget_installed(dir);
            None
        }
        None => (!manifest.artifacts.contains_key(platform)).then(|| {
            (
                FailKind::NoPackage,
                format!("build {} has no package for this system", manifest.build),
            )
        }),
    }
    .filter(|_| manifest.build > options.build);
    Ok(Some(Loaded {
        held: Held {
            signed,
            manifest,
            packages,
        },
        failed,
        publisher,
    }))
}

/// Who to ask, who has what, what is on its way: the updater task's own.
#[derive(Default)]
struct Book {
    /// When each peer was last asked.
    asked: HashMap<PeerId, Instant>,
    /// Peers not yet known as friends or members, until when not to look again.
    not_yet: HashMap<PeerId, Instant>,
    /// Peers left alone until then.
    shunned: HashMap<PeerId, Instant>,
    /// For the release held: who has it, and which packages.
    holders: HashMap<PeerId, HashSet<String>>,
    /// When each peer's word that it has a release last made it asked again.
    heard: HashMap<PeerId, Instant>,
    /// Who is a friend or member, by peer, for the notes.
    devices: HashMap<PeerId, DevicePublicKey>,
    /// Packages on their way, by platform, and from whom.
    fetching: HashMap<String, PeerId>,
    /// Packages of other platforms friends or members asked for.
    wanted: HashSet<String>,
    /// When a friend or member last asked for each other platform's package.
    others: HashMap<String, Instant>,
    /// When a peer last failed a download, to wait before the next.
    failed: HashMap<PeerId, Instant>,
    last_nudge: Option<Instant>,
}

async fn run(
    weak: Weak<Updater>,
    mut heard: mpsc::UnboundedReceiver<Heard>,
    mut commands: mpsc::UnboundedReceiver<Command>,
) {
    let (done_tx, mut done_rx) = mpsc::unbounded_channel::<Done>();
    let mut book = Book::default();
    let mut tick = tokio::time::interval(TICK);
    loop {
        tokio::select! {
            _ = tick.tick() => {}
            Some(h) = heard.recv() => {
                let Some(updater) = weak.upgrade() else { return };
                match h {
                    Heard::Have { peer, build } => {
                        let held = updater.shelf.get().map(|h| h.manifest.build);
                        if build > updater.best() || Some(build) == held {
                            // Asked again: a new release, or a package it has now. A word of a
                            // peer is no release (only the manifest's signature counts), and
                            // it makes the peer asked at most every half minute.
                            let now = Instant::now();
                            let wait = book
                                .heard
                                .get(&peer)
                                .map(|at| HAVE_AGAIN.saturating_sub(now.duration_since(*at)))
                                .unwrap_or_default();
                            if wait.is_zero() {
                                book.asked.remove(&peer);
                                book.heard.insert(peer, now);
                            } else if let Some(at) = now.checked_sub(ASK_AGAIN - wait) {
                                book.asked.insert(peer, at);
                            }
                        }
                    }
                    Heard::Wanted { platform, build } => {
                        let held = updater.shelf.get();
                        if held.is_some_and(|h| h.manifest.build == build) {
                            book.others.insert(platform.clone(), Instant::now());
                            book.wanted.insert(platform);
                        }
                    }
                    Heard::Given { platform, .. } => {
                        book.others.insert(platform, Instant::now());
                    }
                }
            }
            Some(Command::CheckNow) = commands.recv() => book.asked.clear(),
            Some(done) = done_rx.recv() => {
                let Some(updater) = weak.upgrade() else { return };
                updater.done(&mut book, done, &done_tx).await;
            }
        }
        let Some(updater) = weak.upgrade() else {
            return;
        };
        updater.step(&mut book, &done_tx).await;
    }
}

impl Updater {
    /// Asks whom it is time to ask, and starts the downloads that can start.
    async fn step(&self, book: &mut Book, done: &mpsc::UnboundedSender<Done>) {
        let now = Instant::now();
        let connected: HashSet<PeerId> = self
            .node
            .peers_taking(&UPDATE_PROTOCOL)
            .into_iter()
            .collect();
        book.asked
            .retain(|p, at| connected.contains(p) && now.duration_since(*at) < ASK_AGAIN);
        book.not_yet
            .retain(|p, until| connected.contains(p) && *until > now);
        book.shunned.retain(|_, until| *until > now);
        book.holders.retain(|p, _| connected.contains(p));
        book.devices.retain(|p, _| connected.contains(p));
        book.failed.retain(|_, at| now.duration_since(*at) < RETRY);
        book.heard
            .retain(|p, at| connected.contains(p) && now.duration_since(*at) < HAVE_AGAIN);

        let mut asking = Vec::new();
        for peer in &connected {
            if book.asked.contains_key(peer)
                || book.not_yet.contains_key(peer)
                || book.shunned.contains_key(peer)
            {
                continue;
            }
            match self.node.trusted_device(*peer).await {
                Some(device) => {
                    book.devices.insert(*peer, device);
                    book.asked.insert(*peer, now);
                    asking.push(*peer);
                }
                None => {
                    book.not_yet.insert(*peer, now + NOT_YET);
                }
            }
        }
        if !asking.is_empty() {
            self.lock().asking += asking.len();
            self.changed();
        }
        let have = self.best();
        for peer in asking {
            let (node, done, platform) =
                (self.node.clone(), done.clone(), self.platform().to_string());
            tokio::spawn(async move {
                let result = wire::ask(&node, peer, have, &platform).await;
                let _ = done.send(Done::Asked(peer, result));
            });
        }
        self.tidy_others(book, now);
        self.start_fetches(book, done, now);
    }

    fn is_publisher(&self) -> bool {
        self.publisher.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Lets go of a package of another platform nobody has asked for in [`OTHER_IDLE`].
    fn tidy_others(&self, book: &mut Book, now: Instant) {
        if self.is_publisher() {
            return;
        }
        let Some(held) = self.shelf.get() else {
            return;
        };
        for (platform, path) in &held.packages {
            if platform == self.platform() {
                continue;
            }
            let last = *book.others.entry(platform.clone()).or_insert(now);
            if now.duration_since(last) > OTHER_IDLE {
                self.let_go(book, &held, platform, path);
                self.note(format!(
                    "let go of the package {platform} of build {}: nobody asked for it in days",
                    held.manifest.build
                ));
            }
        }
    }

    /// Removes the package of another platform.
    fn let_go(&self, book: &mut Book, held: &Held, platform: &str, path: &Path) {
        let _ = std::fs::remove_file(path);
        if self
            .shelf
            .get()
            .is_some_and(|h| h.manifest.build == held.manifest.build)
        {
            self.shelf.remove_package(platform);
        }
        book.others.remove(platform);
        book.wanted.remove(platform);
    }

    /// Whether the package of `other` may be fetched for friends and members on that system:
    /// one package of another platform at a time (one unasked for [`OTHER_YIELDS`] makes
    /// way), and all packages under [`DISK_CAP`] together.
    fn room_for(&self, book: &mut Book, held: &Held, other: &str, now: Instant) -> bool {
        if self.is_publisher() {
            return true;
        }
        let own = self.platform();
        let size = |p: &str| held.manifest.artifacts.get(p).map(|a| a.size).unwrap_or(0);
        let own_size = if held.packages.contains_key(own) {
            size(own)
        } else {
            0
        };
        if own_size.saturating_add(size(other)) > DISK_CAP {
            return false;
        }
        let kept: Vec<(String, PathBuf)> = held
            .packages
            .iter()
            .filter(|(p, _)| p.as_str() != own && p.as_str() != other)
            .map(|(p, path)| (p.clone(), path.clone()))
            .collect();
        for (platform, path) in kept {
            let last = book.others.get(&platform).copied().unwrap_or(now);
            if now.duration_since(last) < OTHER_YIELDS {
                return false;
            }
            self.let_go(book, held, &platform, &path);
            self.note(format!(
                "let go of the package {platform} of build {} to make way for {other}",
                held.manifest.build
            ));
        }
        true
    }

    /// Starts fetching this platform's package of the release held, if it is missing, and
    /// one package another device wanted.
    fn start_fetches(&self, book: &mut Book, done: &mpsc::UnboundedSender<Done>, now: Instant) {
        let Some(held) = self.shelf.get() else {
            return;
        };
        let own = self.platform().to_string();
        let mut wanted: Vec<String> = Vec::new();
        if held.manifest.artifacts.contains_key(&own) && !held.packages.contains_key(&own) {
            wanted.push(own.clone());
        }
        // Another platform's, one at a time, once this one's is here, while there is room.
        if wanted.is_empty()
            && book.fetching.keys().all(|p| *p == own)
            && let Some(other) = book
                .wanted
                .iter()
                .find(|p| {
                    !held.packages.contains_key(*p) && held.manifest.artifacts.contains_key(*p)
                })
                .cloned()
            && self.room_for(book, &held, &other, now)
        {
            wanted.push(other);
        }
        for platform in wanted {
            if book.fetching.contains_key(&platform) {
                continue;
            }
            let usable = |p: &PeerId| !book.shunned.contains_key(p) && !book.failed.contains_key(p);
            let source = book
                .holders
                .iter()
                .find(|(p, platforms)| platforms.contains(&platform) && usable(p))
                .map(|(p, _)| *p);
            let source = match source {
                Some(peer) => peer,
                // Nobody connected has it: asking one with the release for it makes that one
                // fetch it.
                None if book
                    .last_nudge
                    .is_none_or(|t| now.duration_since(t) > NUDGE) =>
                {
                    match book.holders.keys().find(|p| usable(p)) {
                        Some(peer) => {
                            book.last_nudge = Some(now);
                            *peer
                        }
                        None => continue,
                    }
                }
                None => continue,
            };
            let Some(artifact) = held.manifest.artifacts.get(&platform).cloned() else {
                continue;
            };
            book.fetching.insert(platform.clone(), source);
            let part = store::part_path(&self.dir, &platform);
            let build = held.manifest.build;
            let (node, done) = (self.node.clone(), done.clone());
            let progress = (platform == own).then(|| (self.events.clone(), self.phase.clone()));
            if let Some((_, phase)) = &progress {
                let mut phase = locked(phase);
                phase.downloading = Some((0, artifact.size));
                phase.from = book.devices.get(&source).copied();
            }
            let dir = self.dir.clone();
            tokio::spawn(async move {
                if let Err(e) = tokio::fs::create_dir_all(&dir).await {
                    let _ = done.send(Done::Fetched {
                        peer: source,
                        build,
                        platform,
                        result: Err(FetchError::Io(e)),
                    });
                    return;
                }
                let mut last = Instant::now();
                let result =
                    wire::fetch(&node, source, build, &platform, &artifact, &part, |got| {
                        if let Some((events, phase)) = &progress
                            && (last.elapsed() > Duration::from_millis(500) || got == artifact.size)
                        {
                            last = Instant::now();
                            locked(phase).downloading = Some((got, artifact.size));
                            let _ = events.send(CoreEvent::UpdateChanged);
                        }
                    })
                    .await;
                let _ = done.send(Done::Fetched {
                    peer: source,
                    build,
                    platform,
                    result,
                });
            });
        }
    }

    async fn done(&self, book: &mut Book, done: Done, tasks: &mpsc::UnboundedSender<Done>) {
        match done {
            Done::Asked(peer, result) => {
                {
                    let mut phase = self.lock();
                    phase.asking = phase.asking.saturating_sub(1);
                }
                let from = self.who(book, &peer);
                match result {
                    Ok(Some(offer)) => {
                        if self.offered(peer, &from, &offer).await {
                            book.holders
                                .insert(peer, offer.platforms.into_iter().collect());
                            // A new release: those who had the old one are asked again.
                            book.holders
                                .retain(|p, _| *p == peer || book.asked.contains_key(p));
                        } else {
                            book.shunned.insert(peer, Instant::now() + SHUNNED);
                        }
                    }
                    Ok(None) => {}
                    // Asked too soon after another ask: again in a moment.
                    Err(FetchError::Busy) => {
                        if let Some(at) = Instant::now().checked_sub(ASK_AGAIN - RETRY_ASK) {
                            book.asked.insert(peer, at);
                        }
                    }
                    Err(FetchError::Refused) => {
                        tracing::debug!("{from} does not give releases");
                    }
                    Err(e) => tracing::debug!("asking {from} for releases: {e}"),
                }
                self.changed();
            }
            Done::Fetched {
                peer,
                build,
                platform,
                result,
            } => {
                book.fetching.remove(&platform);
                let own = platform == self.platform();
                if own {
                    let mut phase = self.lock();
                    phase.downloading = None;
                    phase.from = None;
                }
                let current = self.shelf.get().is_some_and(|h| h.manifest.build == build);
                let from = self.who(book, &peer);
                match result {
                    Ok(()) if current => {
                        let (part, path) = (
                            store::part_path(&self.dir, &platform),
                            store::package_path(&self.dir, &platform),
                        );
                        if let Err(e) = tokio::fs::rename(&part, &path).await {
                            self.lock().failed = Some((FailKind::Disk, e.to_string()));
                        } else {
                            self.shelf.add_package(build, &platform, path);
                            book.wanted.remove(&platform);
                            self.note(format!("package {platform} of build {build} from {from}"));
                            if own {
                                self.lock().ready = true;
                            }
                            self.tell_everyone(book, build, tasks);
                        }
                    }
                    Ok(()) => {}
                    Err(FetchError::Damaged) => {
                        self.note(format!(
                            "a chunk of {platform} from {from} arrived damaged; not asking it again"
                        ));
                        book.shunned.insert(peer, Instant::now() + SHUNNED);
                        book.holders.remove(&peer);
                    }
                    Err(FetchError::NotHere) => {
                        if let Some(platforms) = book.holders.get_mut(&peer) {
                            platforms.remove(&platform);
                        }
                        book.failed.insert(peer, Instant::now());
                    }
                    Err(FetchError::Io(e))
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::StorageFull
                                | std::io::ErrorKind::PermissionDenied
                                | std::io::ErrorKind::ReadOnlyFilesystem
                        ) =>
                    {
                        self.lock().failed = Some((FailKind::Disk, e.to_string()));
                    }
                    Err(e) => {
                        tracing::debug!("fetching {platform} from {from}: {e}");
                        book.failed.insert(peer, Instant::now());
                    }
                }
                self.changed();
            }
            Done::Told => {}
        }
    }

    /// Tells every friend and member connected that this device has `build`.
    fn tell_everyone(&self, book: &Book, build: u64, tasks: &mpsc::UnboundedSender<Done>) {
        for peer in book.devices.keys().copied() {
            let (node, tasks) = (self.node.clone(), tasks.clone());
            tokio::spawn(async move {
                let _ = wire::tell(&node, peer, build).await;
                let _ = tasks.send(Done::Told);
            });
        }
    }

    fn who(&self, book: &Book, peer: &PeerId) -> String {
        match book.devices.get(peer) {
            Some(device) => device.fingerprint(),
            None => fogata_net::short_peer(peer),
        }
    }
}

fn now_secs() -> u64 {
    crate::now_ms() / 1000
}

/// Locks `mutex` even when a panic left it poisoned: what it holds is changed one whole step at
/// a time.
fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Installs a release outside a running core, from data folder `data`: the command line's
/// `fogata update install`. The release kept there must be newer than `options.build` and
/// its package for this platform complete.
pub async fn install_offline(data: &Path, options: &UpdateOptions) -> Result<Manifest> {
    let dir = store::dir(data);
    let platform = options.platform.clone().unwrap_or_default();
    let (options2, dir2, platform2) = (options.clone(), dir.clone(), platform.clone());
    let loaded = tokio::task::spawn_blocking(move || load(&dir2, &options2, &platform2))
        .await??
        .ok_or_else(|| anyhow::anyhow!("no release is kept in {}", dir.display()))?;
    let manifest = loaded.held.manifest;
    manifest.newer_than(options.build)?;
    if !loaded.held.packages.contains_key(&platform) {
        anyhow::bail!("the package for {platform} is not here whole yet");
    }
    install::install(data, &manifest, &platform).await?;
    Ok(manifest)
}

/// Where updates stand in data folder `data`, without starting anything: for `fogata update
/// status`.
pub fn status_offline(data: &Path, options: &UpdateOptions) -> Result<UpdateStatus> {
    let dir = store::dir(data);
    let loaded = load(&dir, options, options.platform.as_deref().unwrap_or(""))?;
    let mut status = UpdateStatus {
        this_build: options.build,
        this_version: options.version.clone(),
        state: UpdateState::UpToDate,
        release: None,
        required: false,
        from: None,
    };
    if let Some(loaded) = loaded {
        let m = &loaded.held.manifest;
        status.required = m.min_build > options.build;
        status.state = match loaded.failed {
            Some((kind, message)) => UpdateState::Failed { kind, message },
            None if m.build > options.build
                && loaded
                    .held
                    .packages
                    .contains_key(options.platform.as_deref().unwrap_or("")) =>
            {
                UpdateState::Ready
            }
            None if m.build > options.build => {
                let part = store::part_path(&dir, options.platform.as_deref().unwrap_or(""));
                let done = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
                let total = m
                    .artifacts
                    .get(options.platform.as_deref().unwrap_or(""))
                    .map(|a| a.size)
                    .unwrap_or(0);
                UpdateState::Downloading { done, total }
            }
            None => UpdateState::UpToDate,
        };
        status.release = Some(ReleaseInfo {
            build: m.build,
            version: m.version.clone(),
            notes: m.notes.clone(),
            min_build: m.min_build,
        });
    }
    Ok(status)
}

/// The verified manifest of a release folder, for `fogata release verify`.
pub fn verify_release(from: &Path, keys: &[ReleaseKey]) -> Result<(Signed, Manifest)> {
    let signed = store::read_signed(from)?
        .ok_or_else(|| anyhow::anyhow!("no release.json and release.sig in {}", from.display()))?;
    let manifest = signed.verify(keys)?;
    Ok((signed, manifest))
}

/// Network lab only: puts the release in `from` into data folder `data` as it is, checking
/// nothing, for a copy run with [`UpdateOptions::lab`] to offer to others (bad signatures,
/// older builds, oversized packages), to see them refused.
#[cfg(feature = "lab")]
pub fn plant_unchecked(data: &Path, from: &Path) -> Result<()> {
    let dir = store::dir(data);
    std::fs::create_dir_all(&dir)?;
    store::forget(&dir)?;
    let signed = store::read_signed(from)?
        .ok_or_else(|| anyhow::anyhow!("no release.json and release.sig in {}", from.display()))?;
    store::keep(&dir, &signed)?;
    if let Ok(manifest) = serde_json::from_slice::<Manifest>(&signed.body) {
        for (platform, artifact) in &manifest.artifacts {
            let source = from.join(&artifact.file);
            if source.is_file() {
                std::fs::copy(&source, store::package_path(&dir, platform))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use fogata_crypto::DeviceIdentity;
    use fogata_update::{Artifact, CHANNEL, MAX_BUILD_STEP, PRODUCT, manifest::FORMAT};

    use super::*;

    const PLATFORM: &str = "linux-flatpak";

    fn offer(key: &DeviceIdentity, build: u64, published: u64) -> Offer {
        let manifest = Manifest {
            format: FORMAT,
            product: PRODUCT.into(),
            channel: CHANNEL.into(),
            build,
            version: format!("0.{build}"),
            min_build: 1,
            published,
            expires: published + fogata_update::manifest::DEFAULT_LIFETIME,
            notes: String::new(),
            artifacts: BTreeMap::from([(
                PLATFORM.to_string(),
                Artifact::of("Fogata.pack", format!("build {build}").as_bytes()),
            )]),
        };
        Offer {
            signed: Signed::sign(&manifest, key).unwrap(),
            platforms: vec![PLATFORM.into()],
        }
    }

    /// The updater itself, as friends offer it releases: one further ahead of the build it
    /// runs than a release key may go, and one made before the release it holds, are refused
    /// and noted; the rescue key's go through.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_updater_refuses_a_release_too_far_ahead_or_made_before_the_one_it_holds() {
        let data =
            std::env::temp_dir().join(format!("fogata-updater-offered-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        std::fs::create_dir_all(&data).unwrap();
        let (node, _node_events) = fogata_net::start(fogata_net::NodeConfig {
            identity: Arc::new(DeviceIdentity::generate().unwrap()),
            groups: Vec::new(),
            friends: None,
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap();
        let (key, rescue) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        let mut options = UpdateOptions::compiled_in();
        options.keys = vec![
            ReleaseKey::release(*key.public_key().as_bytes()),
            ReleaseKey::rescue(*rescue.public_key().as_bytes()),
        ];
        options.build = 5;
        options.platform = Some(PLATFORM.into());
        let (events, _events) = crate::events::queue(None);
        let updater = Updater::start(&data, node.clone(), options, events).unwrap();
        let peer = node.local_peer();
        let now = now_secs();
        let held = || updater.lock().release.as_ref().map(|r| r.build);
        let last_note = || updater.notes().last().cloned().unwrap_or_default();

        let far = offer(&key, 5 + MAX_BUILD_STEP + 1, now);
        assert!(!updater.offered(peer, "Bea", &far).await);
        assert!(last_note().contains("further ahead"), "{}", last_note());
        assert_eq!(held(), None);

        assert!(updater.offered(peer, "Bea", &offer(&key, 7, now)).await);
        assert_eq!(held(), Some(7));

        let older_made = offer(&key, 8, now - 3600);
        assert!(!updater.offered(peer, "Bea", &older_made).await);
        assert!(last_note().contains("made before"), "{}", last_note());
        assert_eq!(held(), Some(7));
        // What is kept on disk is still build 7.
        let kept = store::read_signed(&store::dir(&data)).unwrap().unwrap();
        assert_eq!(kept.verify(&updater.options.keys).unwrap().build, 7);

        // The rescue key's release goes further and is not held to the time.
        let rescued = offer(&rescue, 5 + MAX_BUILD_STEP + 1, now - 7200);
        assert!(updater.offered(peer, "Bea", &rescued).await);
        assert_eq!(held(), Some(5 + MAX_BUILD_STEP + 1));

        node.shutdown().await.unwrap();
        let _ = std::fs::remove_dir_all(&data);
    }
}
