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
//! A copy also reads the project's releases page every few hours ([`web`]), and takes a
//! release it finds there by the same rules, its package fetched from there; from then on
//! it gives it on like one from a friend.
//!
//! Nothing is installed by itself: [`crate::Core::install_update`] does it when asked.

mod install;
pub(crate) mod store;
pub mod web;

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
pub use web::WebSource;

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
/// The releases page is read this often, and once soon after the start.
const WEB_EVERY: Duration = Duration::from_secs(6 * 3600);
/// A releases page that could not be read is tried again after this long.
const WEB_RETRY: Duration = Duration::from_secs(15 * 60);
/// A package from the releases page that stopped is tried again after this long.
const WEB_FETCH_RETRY: Duration = Duration::from_secs(2 * 60);

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
    /// The releases page read besides friends and members, if any.
    pub web: Option<WebSource>,
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
            web: Some(WebSource::compiled_in()),
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
    /// The releases page, when it is read.
    web: Option<web::Web>,
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
    /// What is kept on disk was read once, as the updater started.
    loaded: bool,
    installing: bool,
    failed: Option<(FailKind, String)>,
}

enum Command {
    CheckNow,
}

/// What the tasks the updater starts give back.
enum Done {
    Asked(PeerId, Result<Option<Offer>, FetchError>),
    /// The releases page was read.
    Listed(Result<Vec<web::WebOffer>, FetchError>),
    Fetched {
        source: Source,
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
        // A copy that trusts no key could take nothing from the page, and the network lab's
        // copies that give unchecked releases take nothing at all.
        let web = match &options.web {
            Some(source) if !options.keys.is_empty() && !options.unchecked => {
                match web::Web::new(source.clone(), &options.version) {
                    Ok(web) => Some(web),
                    Err(e) => {
                        tracing::warn!("the releases page is not read: {e:#}");
                        None
                    }
                }
            }
            _ => None,
        };
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
            web,
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
        } else if let Some(size) = phase
            .release
            .as_ref()
            .filter(|r| r.build > build)
            .and_then(|r| r.artifacts.get(self.platform()))
            .map(|a| a.size)
        {
            // A newer release is held and its package is not here yet: it is on its way,
            // even in the moment before its download starts. Saying "up to date" then made
            // the splash stop waiting and open the old version.
            UpdateState::Downloading {
                done: 0,
                total: size,
            }
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
            phase.loaded = true;
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

    /// Takes a release offered by `from` (a friend or member, through `via`, or the releases
    /// page) if it is good and newer; returns whether it is the one held now (taken now or
    /// before).
    async fn offered(&self, via: &str, from: &str, signed: &Signed) -> bool {
        // A copy that gives what it holds unchecked (the network lab's) takes nothing.
        if self.options.unchecked {
            return false;
        }
        let checked = signed.verify(&self.options.keys);
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
        let role = signed.role(&self.options.keys).unwrap_or(KeyRole::Release);
        if let Err(refusal) = manifest.taken_by(role, self.options.build, held.as_ref()) {
            self.note(format!(
                "refused build {} from {from}: {refusal}",
                manifest.build
            ));
            return false;
        }
        let (dir, to_keep) = (self.dir.clone(), signed.clone());
        let kept = tokio::task::spawn_blocking(move || store::keep(&dir, &to_keep))
            .await
            .unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")));
        if let Err(e) = kept {
            self.lock().failed = Some((FailKind::Disk, format!("{e:#}")));
            self.changed();
            return false;
        }
        self.note(format!(
            "build {} ({}) from {from}{via}",
            manifest.build, manifest.version
        ));
        self.publisher
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.shelf.set(Some(Held {
            signed: signed.clone(),
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
    /// Packages on their way, by platform, and from where.
    fetching: HashMap<String, Source>,
    /// Packages of other platforms friends or members asked for.
    wanted: HashSet<String>,
    /// When a friend or member last asked for each other platform's package.
    others: HashMap<String, Instant>,
    /// When a peer last failed a download, to wait before the next.
    failed: HashMap<PeerId, Instant>,
    last_nudge: Option<Instant>,
    /// The releases page: when it was last read (or tried), whether it is being read, and
    /// where the packages of the release held are on it, by file name.
    web_read: Option<Instant>,
    web_reading: bool,
    web_packages: Option<(u64, HashMap<String, String>)>,
    /// When a package from the page last stopped, and whether one arrived damaged (the
    /// page is then not used for that release).
    web_failed: Option<Instant>,
    web_damaged: Option<u64>,
}

/// Where a package comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Peer(PeerId),
    Web,
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
            Some(Command::CheckNow) = commands.recv() => {
                book.asked.clear();
                book.web_read = None;
            }
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
        self.read_web(book, done, now);
        self.tidy_others(book, now);
        self.start_fetches(book, done, now);
    }

    /// Reads the releases page when it is time.
    fn read_web(&self, book: &mut Book, done: &mpsc::UnboundedSender<Done>, now: Instant) {
        let Some(web) = self.web.clone() else {
            return;
        };
        if book.web_reading
            || book
                .web_read
                .is_some_and(|at| now.duration_since(at) < WEB_EVERY)
        {
            return;
        }
        book.web_reading = true;
        book.web_read = Some(now);
        self.lock().asking += 1;
        self.changed();
        let done = done.clone();
        tokio::spawn(async move {
            let _ = done.send(Done::Listed(web.releases().await));
        });
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
            let Some(artifact) = held.manifest.artifacts.get(&platform).cloned() else {
                continue;
            };
            // The releases page first, when it has the package: it spares friends' uploads.
            let web_url =
                self.web
                    .as_ref()
                    .and(book.web_packages.as_ref())
                    .and_then(|(build, packages)| {
                        (*build == held.manifest.build
                            && book.web_damaged != Some(*build)
                            && book
                                .web_failed
                                .is_none_or(|at| now.duration_since(at) > WEB_FETCH_RETRY))
                        .then(|| packages.get(&artifact.file).cloned())
                        .flatten()
                    });
            if let (Some(url), Some(web)) = (web_url, self.web.clone()) {
                self.fetch(
                    book,
                    done,
                    &held,
                    &platform,
                    artifact,
                    Source::Web,
                    move |part, artifact, progress| async move {
                        web.fetch(&url, &artifact, &part, progress).await
                    },
                );
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
            let (node, build, name) = (self.node.clone(), held.manifest.build, platform.clone());
            self.fetch(
                book,
                done,
                &held,
                &platform,
                artifact,
                Source::Peer(source),
                move |part, artifact, progress| async move {
                    wire::fetch(&node, source, build, &name, &artifact, &part, progress).await
                },
            );
        }
    }

    /// Starts fetching the package of `platform` with `get`, which writes it into the part
    /// file it is given, calling the progress function it is given.
    #[allow(clippy::too_many_arguments)]
    fn fetch<F, Fut>(
        &self,
        book: &mut Book,
        done: &mpsc::UnboundedSender<Done>,
        held: &Held,
        platform: &str,
        artifact: fogata_update::Artifact,
        source: Source,
        get: F,
    ) where
        F: FnOnce(PathBuf, fogata_update::Artifact, Box<dyn FnMut(u64) + Send>) -> Fut
            + Send
            + 'static,
        Fut: std::future::Future<Output = Result<(), FetchError>> + Send,
    {
        book.fetching.insert(platform.to_string(), source);
        let part = store::part_path(&self.dir, platform);
        let build = held.manifest.build;
        let done = done.clone();
        let progress =
            (platform == self.platform()).then(|| (self.events.clone(), self.phase.clone()));
        if let Some((_, phase)) = &progress {
            let mut phase = locked(phase);
            phase.downloading = Some((0, artifact.size));
            phase.from = match source {
                Source::Peer(peer) => book.devices.get(&peer).copied(),
                Source::Web => None,
            };
        }
        let dir = self.dir.clone();
        let platform = platform.to_string();
        tokio::spawn(async move {
            if let Err(e) = tokio::fs::create_dir_all(&dir).await {
                let _ = done.send(Done::Fetched {
                    source,
                    build,
                    platform,
                    result: Err(FetchError::Io(e)),
                });
                return;
            }
            let size = artifact.size;
            let mut last = Instant::now();
            let report: Box<dyn FnMut(u64) + Send> = Box::new(move |got| {
                if let Some((events, phase)) = &progress
                    && (last.elapsed() > Duration::from_millis(500) || got == size)
                {
                    last = Instant::now();
                    locked(phase).downloading = Some((got, size));
                    let _ = events.send(CoreEvent::UpdateChanged);
                }
            });
            let result = get(part, artifact, report).await;
            let _ = done.send(Done::Fetched {
                source,
                build,
                platform,
                result,
            });
        });
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
                        let via = format!(", through {}", fogata_net::short_peer(&peer));
                        if self.offered(&via, &from, &offer.signed).await {
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
            Done::Listed(result) => {
                {
                    let mut phase = self.lock();
                    phase.asking = phase.asking.saturating_sub(1);
                }
                book.web_reading = false;
                match result {
                    Ok(offers) => self.listed(book, offers).await,
                    Err(e) => {
                        tracing::debug!("reading the releases page: {e}");
                        // Tried again sooner than the next regular read.
                        book.web_read = Instant::now().checked_sub(WEB_EVERY - WEB_RETRY);
                    }
                }
                self.changed();
            }
            Done::Fetched {
                source,
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
                let peer = match source {
                    Source::Peer(peer) => peer,
                    Source::Web => {
                        self.fetched_from_web(book, build, &platform, result, current, tasks)
                            .await;
                        self.changed();
                        return;
                    }
                };
                let from = self.who(book, &peer);
                match result {
                    Ok(()) if current => {
                        self.arrived(book, build, &platform, &from, tasks).await;
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

    /// The package of `platform` in release `build` is whole in its part file: it becomes
    /// the package, and friends and members hear of it.
    async fn arrived(
        &self,
        book: &mut Book,
        build: u64,
        platform: &str,
        from: &str,
        tasks: &mpsc::UnboundedSender<Done>,
    ) {
        let (part, path) = (
            store::part_path(&self.dir, platform),
            store::package_path(&self.dir, platform),
        );
        if let Err(e) = tokio::fs::rename(&part, &path).await {
            self.lock().failed = Some((FailKind::Disk, e.to_string()));
            return;
        }
        self.shelf.add_package(build, platform, path);
        book.wanted.remove(platform);
        self.note(format!("package {platform} of build {build} from {from}"));
        if platform == self.platform() {
            self.lock().ready = true;
        }
        self.tell_everyone(book, build, tasks);
    }

    /// A package from the releases page came, or stopped.
    async fn fetched_from_web(
        &self,
        book: &mut Book,
        build: u64,
        platform: &str,
        result: Result<(), FetchError>,
        current: bool,
        tasks: &mpsc::UnboundedSender<Done>,
    ) {
        match result {
            Ok(()) if current => {
                self.arrived(book, build, platform, "the releases page", tasks)
                    .await;
            }
            Ok(()) => {}
            Err(FetchError::Damaged) => {
                self.note(format!(
                    "a chunk of {platform} from the releases page arrived damaged; \
                     fetching it from friends and members instead"
                ));
                book.web_damaged = Some(build);
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
                tracing::debug!("fetching {platform} from the releases page: {e}");
                book.web_failed = Some(Instant::now());
            }
        }
    }

    /// Takes the newest good release the page lists that is newer than what is here, and
    /// notes where its packages are.
    async fn listed(&self, book: &mut Book, offers: Vec<web::WebOffer>) {
        let mut offers: Vec<(u64, web::WebOffer)> = offers
            .into_iter()
            .filter_map(|offer| {
                let build = offer.signed.verify(&self.options.keys).ok()?.build;
                Some((build, offer))
            })
            .collect();
        offers.sort_by_key(|(build, _)| std::cmp::Reverse(*build));
        let held = self.shelf.get().map(|h| h.manifest.build);
        for (build, offer) in offers {
            // The release held already: only where its packages are is new.
            if Some(build) == held {
                book.web_packages = Some((build, offer.packages));
                return;
            }
            if build <= self.best() {
                return;
            }
            if self.offered("", "the releases page", &offer.signed).await {
                book.web_packages = Some((build, offer.packages));
                book.web_failed = None;
                // A new release: those who had the old one are asked again.
                book.holders.clear();
                return;
            }
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
        options.web = None;
        let (events, _events) = crate::events::queue(None);
        let updater = Updater::start(&data, node.clone(), options, events).unwrap();
        // What is kept on disk is read first, and the status says Verifying meanwhile; the
        // reading may not even have started yet on a busy computer.
        for _ in 0..400 {
            if updater.lock().loaded {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(updater.lock().loaded, "what is kept on disk was never read");
        let now = now_secs();
        let held = || updater.lock().release.as_ref().map(|r| r.build);
        let last_note = || updater.notes().last().cloned().unwrap_or_default();

        let far = offer(&key, 5 + MAX_BUILD_STEP + 1, now);
        assert!(!updater.offered("", "Bea", &far.signed).await);
        assert!(last_note().contains("further ahead"), "{}", last_note());
        assert_eq!(held(), None);

        assert!(
            updater
                .offered("", "Bea", &offer(&key, 7, now).signed)
                .await
        );
        assert_eq!(held(), Some(7));
        // Held, its package not here and no download started yet: the screens hear that it is
        // on its way, never that this copy is up to date.
        assert!(
            matches!(
                updater.status().state,
                UpdateState::Downloading { done: 0, .. }
            ),
            "{:?}",
            updater.status().state
        );

        let older_made = offer(&key, 8, now - 3600);
        assert!(!updater.offered("", "Bea", &older_made.signed).await);
        assert!(last_note().contains("made before"), "{}", last_note());
        assert_eq!(held(), Some(7));
        // What is kept on disk is still build 7.
        let kept = store::read_signed(&store::dir(&data)).unwrap().unwrap();
        assert_eq!(kept.verify(&updater.options.keys).unwrap().build, 7);

        // The rescue key's release goes further and is not held to the time.
        let rescued = offer(&rescue, 5 + MAX_BUILD_STEP + 1, now - 7200);
        assert!(updater.offered("", "Bea", &rescued.signed).await);
        assert_eq!(held(), Some(5 + MAX_BUILD_STEP + 1));

        node.shutdown().await.unwrap();
        let _ = std::fs::remove_dir_all(&data);
    }

    /// A releases page on this computer, like GitHub's: the list of releases, and each
    /// release's files, with ranges. `damaged` flips a byte of the package past its first
    /// chunk.
    async fn releases_page(
        releases: Vec<(Signed, &'static str, Vec<u8>)>,
        damaged: bool,
    ) -> String {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let mut files: HashMap<String, Vec<u8>> = HashMap::new();
        let mut list = Vec::new();
        for (n, (signed, name, mut package)) in releases.into_iter().enumerate() {
            if damaged {
                package[fogata_update::manifest::CHUNK as usize + 10] ^= 1;
            }
            files.insert(format!("/d/{n}/release.json"), signed.body.clone());
            files.insert(
                format!("/d/{n}/release.sig"),
                signed.signature_file().into_bytes(),
            );
            files.insert(format!("/d/{n}/{name}"), package);
            let asset = |f: &str| serde_json::json!({"name": f, "browser_download_url": format!("{base}/d/{n}/{f}")});
            list.push(serde_json::json!({
                "draft": false,
                "prerelease": true,
                "assets": [asset("release.json"), asset("release.sig"), asset(name)],
            }));
        }
        list.insert(0, serde_json::json!({"draft": true, "assets": []}));
        files.insert(
            "/repos/holypandadev/Fogata/releases".into(),
            serde_json::to_vec(&list).unwrap(),
        );
        let files = Arc::new(files);
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let files = files.clone();
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buf = [0u8; 1024];
                    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                        match socket.read(&mut buf).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => request.extend_from_slice(&buf[..n]),
                        }
                    }
                    let request = String::from_utf8_lossy(&request).to_string();
                    let path = request.split(' ').nth(1).unwrap_or("").to_string();
                    let path = path.split('?').next().unwrap_or("").to_string();
                    let from = request
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("range: bytes=")
                                .map(str::to_string)
                        })
                        .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok());
                    let response = match (files.get(&path), from) {
                        (Some(body), Some(from)) => {
                            let part = &body[from.min(body.len())..];
                            let mut out = format!(
                                "HTTP/1.1 206 Partial Content\r\ncontent-length: {}\r\ncontent-range: bytes {from}-{}/{}\r\nconnection: close\r\n\r\n",
                                part.len(),
                                body.len().saturating_sub(1),
                                body.len()
                            )
                            .into_bytes();
                            out.extend_from_slice(part);
                            out
                        }
                        (Some(body), None) => {
                            let mut out = format!(
                                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                                body.len()
                            )
                            .into_bytes();
                            out.extend_from_slice(body);
                            out
                        }
                        (None, _) => b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_vec(),
                    };
                    let _ = socket.write_all(&response).await;
                    let _ = socket.shutdown().await;
                });
            }
        });
        base
    }

    /// A release of `size` bytes of package, signed by `key`.
    fn web_release(
        key: &DeviceIdentity,
        build: u64,
        size: usize,
    ) -> (Signed, &'static str, Vec<u8>) {
        let package: Vec<u8> = (0..size).map(|i| (i * 7 % 251) as u8).collect();
        let now = now_secs();
        let manifest = Manifest {
            format: FORMAT,
            product: PRODUCT.into(),
            channel: CHANNEL.into(),
            build,
            version: format!("0.{build}"),
            min_build: 1,
            published: now,
            expires: now + fogata_update::manifest::DEFAULT_LIFETIME,
            notes: String::new(),
            artifacts: BTreeMap::from([(
                PLATFORM.to_string(),
                Artifact::of("Fogata-linux.pack", &package),
            )]),
        };
        (
            Signed::sign(&manifest, key).unwrap(),
            "Fogata-linux.pack",
            package,
        )
    }

    /// An updater with no friends, reading the page at `base`, as build 5.
    async fn web_updater(
        name: &str,
        key: &DeviceIdentity,
        base: &str,
    ) -> (Arc<Updater>, NodeHandle, PathBuf) {
        let data = std::env::temp_dir().join(format!("fogata-web-{name}-{}", std::process::id()));
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
        let mut options = UpdateOptions::compiled_in();
        options.keys = vec![
            ReleaseKey::release(*key.public_key().as_bytes()),
            ReleaseKey::rescue(*DeviceIdentity::generate().unwrap().public_key().as_bytes()),
        ];
        options.build = 5;
        options.platform = Some(PLATFORM.into());
        options.web = Some(WebSource {
            api: base.to_string(),
            repo: "holypandadev/Fogata".into(),
        });
        let (events, _events) = crate::events::queue(None);
        let updater = Updater::start(&data, node.clone(), options, events).unwrap();
        (updater, node, data)
    }

    async fn until(what: impl Fn() -> bool) -> bool {
        for _ in 0..200 {
            if what() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }

    /// The newest release the page has that the key signed is taken, a draft and one signed
    /// by another key are not, and its package comes from the page, ready to install.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_release_on_the_releases_page_is_taken_and_its_package_fetched() {
        let key = DeviceIdentity::generate().unwrap();
        let stranger = DeviceIdentity::generate().unwrap();
        let size = 2 * fogata_update::manifest::CHUNK as usize + 1234;
        let good = web_release(&key, 7, size);
        let package = good.2.clone();
        let base = releases_page(
            vec![
                web_release(&stranger, 9, 100),
                good,
                web_release(&key, 6, 100),
            ],
            false,
        )
        .await;
        let (updater, node, data) = web_updater("good", &key, &base).await;
        assert!(
            until(|| updater.status().state == UpdateState::Ready).await,
            "{:?} {:?}",
            updater.status(),
            updater.notes()
        );
        let status = updater.status();
        assert_eq!(status.release.unwrap().build, 7);
        assert_eq!(status.from, None);
        let kept = std::fs::read(store::package_path(&store::dir(&data), PLATFORM)).unwrap();
        assert_eq!(kept, package);
        assert!(
            updater
                .notes()
                .iter()
                .any(|n| n.contains("from the releases page")),
            "{:?}",
            updater.notes()
        );
        node.shutdown().await.unwrap();
        let _ = std::fs::remove_dir_all(&data);
    }

    /// A package from the page that does not match the signed manifest is refused at the
    /// damaged chunk, keeping the good chunks before it, and the page is not used again for
    /// that release.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_damaged_package_on_the_releases_page_is_refused() {
        let key = DeviceIdentity::generate().unwrap();
        let size = 3 * fogata_update::manifest::CHUNK as usize;
        let base = releases_page(vec![web_release(&key, 7, size)], true).await;
        let (updater, node, data) = web_updater("damaged", &key, &base).await;
        assert!(
            until(|| updater
                .notes()
                .iter()
                .any(|n| n.contains("arrived damaged")))
            .await,
            "{:?}",
            updater.notes()
        );
        let status = updater.status();
        assert_eq!(status.release.unwrap().build, 7);
        assert_ne!(status.state, UpdateState::Ready);
        let dir = store::dir(&data);
        assert!(!store::package_path(&dir, PLATFORM).exists());
        let part = std::fs::metadata(store::part_path(&dir, PLATFORM)).unwrap();
        assert_eq!(part.len(), fogata_update::manifest::CHUNK);
        node.shutdown().await.unwrap();
        let _ = std::fs::remove_dir_all(&data);
    }

    /// A download from the page that stopped goes on from its last whole chunk, asking
    /// the page for the rest only.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_download_from_the_releases_page_goes_on_where_it_stopped() {
        let key = DeviceIdentity::generate().unwrap();
        let size = 3 * fogata_update::manifest::CHUNK as usize + 99;
        let (signed, name, package) = web_release(&key, 7, size);
        let artifact = signed
            .verify(&[ReleaseKey::release(*key.public_key().as_bytes())])
            .unwrap()
            .artifacts[PLATFORM]
            .clone();
        let base = releases_page(vec![(signed, name, package.clone())], false).await;
        let dir = std::env::temp_dir().join(format!("fogata-web-resume-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("p.part");
        // Two whole chunks and a bit of the third were here.
        let chunk = fogata_update::manifest::CHUNK as usize;
        std::fs::write(&part, &package[..2 * chunk + 500]).unwrap();
        let web = web::Web::new(
            WebSource {
                api: base.clone(),
                repo: "holypandadev/Fogata".into(),
            },
            "test",
        )
        .unwrap();
        let mut first = None;
        web.fetch(&format!("{base}/d/0/{name}"), &artifact, &part, |got| {
            first.get_or_insert(got);
        })
        .await
        .unwrap();
        assert_eq!(first, Some(2 * chunk as u64));
        assert_eq!(std::fs::read(&part).unwrap(), package);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
