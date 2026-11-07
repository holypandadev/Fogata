//! The log kept in the data folder for problem reports: what Fogata's own crates say at
//! `info` and above and the warnings of the rest, one file per day (UTC) in
//! `diagnostics/fogata-YYYY-MM-DD.log`. The files of today and the two days before stay;
//! older ones go when the first line of a day is written. A day's file stops growing at
//! [`DAY_CAP`]. Nothing personal reaches these files: [`ScrubFields`] formats every message
//! and field through [`scrub`] before the line is written, and the folder and its files are
//! its user's only. Nothing here leaves the computer by itself: the application reads these
//! files for a report, scrubs them again, and the person sends it or not.

mod scrub;

use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tracing::field::{Field, Visit};
use tracing_subscriber::{
    EnvFilter, Layer,
    field::RecordFields,
    fmt::{FormatFields, MakeWriter, format::Writer},
    registry::LookupSpan,
};

pub(crate) use scrub::Known;

/// The folder in the data folder.
pub(crate) const DIR: &str = "diagnostics";

/// What the file takes: the core's steps, and only the warnings of the libraries.
pub(crate) const LEVELS: &str = "warn,quinn_udp=error,fogata_core=info,fogata_net=info,\
                                 fogata_ffi=info,fogata_media=info,fogata_update=info";

/// The most one day's file grows to.
const DAY_CAP: u64 = 8 * 1024 * 1024;

/// Days kept before today's.
const KEEP_DAYS: i64 = 2;

// ----- What is known here -----

/// The names and folders the log must not carry ([`Known`]), for every line formatted.
static KNOWN: RwLock<Option<Arc<Known>>> = RwLock::new(None);

fn known() -> Arc<Known> {
    KNOWN
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default()
}

/// Learns the folders of this computer that a log must not show (the data folder, the home
/// folder) and the names of the computer and its user. Called before the first line.
pub(crate) fn know_computer(data: &Path) {
    let mut paths = vec![data.display().to_string()];
    if let Ok(full) = data.canonicalize() {
        paths.push(full.display().to_string());
    }
    for var in ["HOME", "USERPROFILE"] {
        if let Some(home) = std::env::var_os(var).filter(|h| !h.is_empty()) {
            paths.push(PathBuf::from(home).display().to_string());
        }
    }
    // As written with the other slash, and as Rust prints paths in quotes on Windows.
    let variants: Vec<String> = paths
        .iter()
        .flat_map(|p| [p.replace('\\', "/"), p.replace('\\', "\\\\")])
        .collect();
    paths.extend(variants);
    let mut names: Vec<String> = ["USER", "USERNAME", "LOGNAME", "HOSTNAME", "COMPUTERNAME"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .collect();
    names.extend(host_name());
    let mut known = Known::new(names, paths);
    if let Some(old) = KNOWN.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        known.add_names(old.names());
    }
    *KNOWN.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(known));
}

/// Adds names of people, groups and rooms to those the log takes out.
pub(crate) fn know_names(names: impl IntoIterator<Item = String>) {
    let mut slot = KNOWN.write().unwrap_or_else(|e| e.into_inner());
    let mut known = slot.as_deref().cloned().unwrap_or_default();
    known.add_names(names);
    *slot = Some(Arc::new(known));
}

/// This computer's name, as the system says it.
#[cfg(unix)]
#[allow(unsafe_code)]
fn host_name() -> Option<String> {
    let mut name = [0u8; 256];
    // SAFETY: gethostname writes at most the length given into the buffer, which lives
    // across the call.
    let ok = unsafe { libc::gethostname(name.as_mut_ptr().cast(), name.len()) } == 0;
    let end = name.iter().position(|b| *b == 0)?;
    let name = String::from_utf8_lossy(&name[..end]).into_owned();
    // Without its domain (`Martas-Laptop.local` as `Martas-Laptop`); the scrubber takes
    // out local host names whole.
    ok.then(|| name.split('.').next().unwrap_or(&name).to_owned())
}

#[cfg(not(unix))]
fn host_name() -> Option<String> {
    None
}

/// Wakes [`watch_names`]: something that may have changed a name happened.
static NAMES_CHANGED: tokio::sync::Notify = tokio::sync::Notify::const_new();

fn names_may_change() {
    NAMES_CHANGED.notify_one();
}

/// Wakes [`watch_names`] after an event of the core that may bring a name.
pub(crate) fn after_event(event: &fogata_core::CoreEvent) {
    use fogata_core::CoreEvent as E;
    if matches!(
        event,
        E::GroupsChanged
            | E::FriendsChanged
            | E::JoinAsked { .. }
            | E::JoinAsksChanged
            | E::MemberConnected { .. }
            | E::FriendRequest { .. }
            | E::FriendRequestSent { .. }
            | E::FriendAdded { .. }
            | E::Joined { .. }
            | E::GroupInvited { .. }
    ) {
        names_may_change();
    }
}

/// Keeps the names known here in [`KNOWN`]: those of the profile, friends and their
/// requests, groups, their rooms and members, invitations and people asking to join, read
/// again a moment after an event of the core that may bring one ([`after_event`]). Started
/// once; a core started again is read at once.
pub(crate) fn watch_names(runtime: &tokio::runtime::Runtime) {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        runtime.spawn(read_names());
    });
    names_may_change();
}

async fn read_names() {
    use crate::api::app;
    loop {
        let mut names = Vec::new();
        if let Ok(me) = app::my_profile().await {
            names.push(me.person.name);
        }
        if let Ok(friends) = app::friends().await {
            names.extend(friends.into_iter().map(|f| f.person.name));
        }
        if let Ok(requests) = app::requests().await {
            let all = requests.incoming.into_iter().chain(requests.outgoing);
            names.extend(all.map(|r| r.person.name));
        }
        if let Ok(groups) = app::groups().await {
            for g in groups {
                if let Ok(members) = app::members(g.id.clone()).await {
                    names.extend(members.into_iter().map(|m| m.person.name));
                }
                names.extend(g.voice_rooms.into_iter().map(|v| v.name));
                names.extend([g.name, g.local_name]);
            }
        }
        if let Ok(invites) = app::group_invitations().await {
            names.extend(invites.into_iter().flat_map(|i| [i.name, i.by.name]));
        }
        if let Ok(asks) = app::join_asks().await {
            names.extend(asks.into_iter().flat_map(|a| [a.name, a.group_name]));
        }
        know_names(names);
        NAMES_CHANGED.notified().await;
        // Events come in bursts: one reading for each.
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

// ----- Standard error -----

/// The variable that asks, in a debug build only, for standard error to carry the values as
/// they are, where it never leaves the computer's terminal.
const RAW_VARIABLE: &str = "FOGATA_RAW_LOGS";

/// What standard error takes: what `RUST_LOG` asks for, or the warnings. Standard error may
/// end in the system's journal on a disk, so it is scrubbed like the days' log whatever the
/// level; only a debug build with [`RAW_VARIABLE`] set leaves the values as they are.
pub(crate) fn stderr_layer<S>() -> Box<dyn Layer<S> + Send + Sync>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    let raw = cfg!(debug_assertions) && std::env::var_os(RAW_VARIABLE).is_some();
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,quinn_udp=error"));
    scrubbed_layer(io::stderr, filter, raw)
}

fn scrubbed_layer<S, W>(writer: W, filter: EnvFilter, raw: bool) -> Box<dyn Layer<S> + Send + Sync>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    W: for<'w> MakeWriter<'w> + Send + Sync + 'static,
{
    let layer = tracing_subscriber::fmt::layer().with_writer(writer);
    if raw {
        layer.with_filter(filter).boxed()
    } else {
        layer.fmt_fields(ScrubFields).with_filter(filter).boxed()
    }
}

// ----- Formatting -----

/// Formats the message and the fields of an event or a span with what is personal taken
/// out ([`scrub`]); the rest of a line (time, level, target, the names of spans) is fixed in
/// the code.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ScrubFields;

impl<'w> FormatFields<'w> for ScrubFields {
    fn format_fields<R: RecordFields>(&self, mut writer: Writer<'w>, fields: R) -> fmt::Result {
        let mut visitor = Scrubbing {
            out: String::new(),
            known: known(),
        };
        fields.record(&mut visitor);
        writer.write_str(&visitor.out)
    }
}

struct Scrubbing {
    out: String,
    known: Arc<Known>,
}

impl Scrubbing {
    fn add(&mut self, field: &Field, value: &str) {
        let name = field.name();
        // What the `log` crate's records carry of where they come from.
        if name.starts_with("log.") {
            return;
        }
        if !self.out.is_empty() {
            self.out.push(' ');
        }
        if name == "message" {
            self.out.push_str(&scrub::message(value, &self.known));
        } else {
            let name = name.strip_prefix("r#").unwrap_or(name);
            let value = scrub::field(name, value, &self.known);
            self.out.push_str(&format!("{name}={value}"));
        }
    }
}

impl Visit for Scrubbing {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.add(field, &format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.add(field, value);
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.add(field, &value.to_string());
    }
}

// ----- The day files -----

/// Writes each formatted event to the file of its day. A log that cannot be written is
/// left unwritten: it never stops anything else.
#[derive(Clone)]
pub(crate) struct DayFiles(Arc<Inner>);

struct Inner {
    dir: PathBuf,
    open: Mutex<Option<Open>>,
}

struct Open {
    day: i64,
    file: File,
    written: u64,
    full: bool,
}

impl DayFiles {
    pub(crate) fn new(data: &Path) -> DayFiles {
        DayFiles(Arc::new(Inner {
            dir: data.join(DIR),
            open: Mutex::new(None),
        }))
    }

    fn write_on(&self, day: i64, line: &[u8]) -> io::Result<()> {
        let mut open = self.0.open.lock().unwrap_or_else(|e| e.into_inner());
        if open.as_ref().is_none_or(|o| o.day != day) {
            *open = None;
            make_dir(&self.0.dir)?;
            tidy(&self.0.dir, day);
            let file = create(&self.0.dir.join(file_name(day)))?;
            let written = file.metadata().map(|m| m.len()).unwrap_or(0);
            *open = Some(Open {
                day,
                file,
                written,
                full: written >= DAY_CAP,
            });
        }
        let Some(o) = open.as_mut() else {
            return Ok(());
        };
        if o.full {
            return Ok(());
        }
        if o.written + line.len() as u64 > DAY_CAP {
            o.full = true;
            return o
                .file
                .write_all(b"(the log of this day stops here: it reached its size limit)\n");
        }
        o.file.write_all(line)?;
        o.written += line.len() as u64;
        Ok(())
    }
}

/// Makes the folder, its user's only (the window may have made it first, as its system
/// makes folders).
fn make_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    owner_only(dir, 0o700);
    Ok(())
}

/// Opens `path` to add to it, readable by its owner only.
fn create(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    // A file made before (by an older copy, say) is made its owner's only too.
    owner_only(path, 0o600);
    Ok(file)
}

/// Gives `path` the unix permissions `mode`; elsewhere a file in the user's data folder is
/// the user's already.
#[cfg(unix)]
fn owner_only(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn owner_only(_: &Path, _: u32) {}
impl io::Write for DayFiles {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let _ = self.write_on(today(), buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for DayFiles {
    type Writer = DayFiles;

    fn make_writer(&'a self) -> DayFiles {
        self.clone()
    }
}

/// Days since 1970-01-01, UTC.
fn today() -> i64 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (secs / 86_400) as i64
}

/// Removes the day files older than the days kept, the window's own (`app-YYYY-MM-DD.log`)
/// with them, and makes the window's files its user's only.
fn tidy(dir: &Path, today: i64) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let day = day_of(name).or_else(|| {
            name.strip_prefix("app-")
                .and_then(|rest| day_of(&format!("fogata-{rest}")))
        });
        if let Some(day) = day
            && day < today - KEEP_DAYS
        {
            let _ = fs::remove_file(entry.path());
        } else if name.starts_with("app-") && name.ends_with(".log") {
            // The window's errors, written beside these by the application.
            owner_only(&entry.path(), 0o600);
        }
    }
}

/// `fogata-YYYY-MM-DD.log` for a day.
fn file_name(day: i64) -> String {
    let (y, m, d) = civil(day);
    format!("fogata-{y:04}-{m:02}-{d:02}.log")
}

/// The day a file of [`file_name`] is for.
fn day_of(name: &str) -> Option<i64> {
    let date = name.strip_prefix("fogata-")?.strip_suffix(".log")?;
    let mut parts = date.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days(y, m, d))
}

/// The date of a day since 1970-01-01 (proleptic Gregorian).
fn civil(day: i64) -> (i64, u32, u32) {
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The day since 1970-01-01 of a date.
fn days(y: i64, m: u32, d: u32) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEER: &str = "12D3KooWGRUVh7MLpAFtJ6tXc1oFmDy2X1WbSmN4QZbWq9VtXk3A";
    const OTHER: &str = "12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN";
    const DEVICE: &str = "b1946ac92492d2347c6235b4d2611184e2b6a1bda1c0e2c8a1f9d5c3b4a2e1f0";
    const SECRET: &str = "kQ7vR2xN9pL4mW8sT1zY6bC3dF5gH0jK2nP7qR9sU4w=";

    /// What a call step holds, as the network's events print it.
    #[allow(dead_code)]
    #[derive(Debug)]
    enum Signal {
        Ring { log: Vec<Vec<u8>> },
        Offer { sdp: String },
    }

    /// The lines the crates write (see the `tracing` calls in fogata-net and fogata-core),
    /// and the libraries' warnings, with every kind of personal value in them.
    fn write_events() {
        let fp = "7Q4MK2XR9PMD3VHC";
        let address = format!("/ip4/203.0.113.7/udp/40123/quic-v1/p2p/{PEER}");
        tracing::info!(target: "fogata_net::node", peer = %PEER, path = %"direct",
            address = %address, "member {fp} connected");
        tracing::info!(target: "fogata_net::node", peer = %OTHER,
            path = %"through member 4kx6nXTN", hidden = false, "friend {fp} online");
        tracing::info!(target: "fogata_net::node",
            address = %"/ip6/2001:db8::42/udp/4001/quic-v1", "address confirmed");
        tracing::info!(target: "fogata_net::node",
            observed = %"/ip4/198.51.100.23/udp/51820/quic-v1", "this device's public address changed");
        tracing::info!(target: "fogata_net::node", "joined the group {}", "The usual crew");
        tracing::info!(target: "fogata_net::node", "{} asks to join a group", "Bea Ruiz");
        tracing::warn!(target: "fogata_core", "group {} not loaded: {}", "The usual crew",
            "cannot read the log: damaged");
        tracing::warn!(target: "fogata_core", "cannot save group {}: disk full", "Corner Café");
        tracing::warn!(target: "fogata_net::group",
            "the group key of \"{}\" is broken: {fp} gave this device a key that does not open",
            "Gaming");
        tracing::info!(target: "fogata_net::node",
            signal = ?Signal::Offer { sdp: "v=0 o=- 1 2 IN IP4 203.0.113.7 a=ice-pwd:abc".into() },
            "call step from {fp}");
        tracing::info!(target: "fogata_net::node",
            signal = ?Signal::Ring { log: vec![vec![1, 2, 3, 4, 5, 6, 7, 8, 9]] },
            "call step from {fp}");
        tracing::warn!(target: "fogata_core::direct", "cannot remove {}: busy",
            format!("{}/direct/{DEVICE}/files/beach holiday.jpg", data().display()));
        tracing::warn!(target: "fogata_core::store",
            "data folder stays at {}, not moved to {}: denied",
            r"C:\Users\Sofía Gil\AppData\Roaming\Fogata",
            r"C:\Users\Sofía Gil\AppData\Local\Fogata");
        tracing::warn!(target: "fogata_core::transfer",
            "{:?} does not move: its name cannot be kept everywhere",
            format!("direct/{DEVICE}/files/final report.pdf"));
        tracing::info!(target: "fogata_core::updates",
            "update: build 4 (0.1.0) from {fp}, through 4kx6nXTN");
        tracing::info!(target: "fogata_net::node",
            "hole punch with 4kx6nXTN through a member failed (Failed to dial \
             /ip4/192.168.1.20/udp/4001/quic-v1/p2p/{OTHER}); messages travel through other members");
        tracing::error!(target: "fogata_net::node",
            "the network node goes on after an internal error: {}",
            "panicked at 'message \"hello Leo, are you joining at 8?\" too long'");
        tracing::info!(target: "fogata_net::node",
            "friend request failed: the code FGT-7Q4MK-2XR9P is wrong");
        tracing::warn!(target: "fogata_core",
            "cannot use fogata:friend:AQIDBAUGBwgJCgsMDQ4PEA from marta.gil@example.com \
             (key {SECRET}, device {DEVICE})");
        tracing::warn!(target: "fogata_ffi", "sound: cannot open Leo's tone at ~/Music/Leo.ogg");
        tracing::warn!(target: "fogata_media", "datagrams limited to 1200 bytes on this path");
        tracing::warn!(target: "libp2p_swarm",
            "Connection to {PEER} at [2001:db8::42]:4001 failed: Timeout");
        tracing::warn!(target: "libp2p_kad", peer = %PEER,
            error = %"Transport([(/ip4/192.168.1.20/tcp/4001, Other(Custom { kind: Other, \
                      error: \"refused by 192.168.1.20\" }))])",
            "Failed to dial a peer");
        tracing::warn!(target: "libp2p_mdns", host = "Martas-Laptop.local",
            text = "see you at 8, Leo", "Discovered marta-pc.lan at fe80::1c2b:3cff:fe4d:5e6f%en0");
        let span = tracing::warn_span!("connection",
            remote_addr = %format!("/ip4/203.0.113.9/tcp/4001/p2p/{OTHER}"), peer = %OTHER);
        let _entered = span.enter();
        tracing::warn!(target: "libp2p_tcp", "port 51820 closed on marta-pc after 3 tries");
    }

    fn data() -> PathBuf {
        std::env::temp_dir()
            .join(format!("fogata-logs-scrub-{}", std::process::id()))
            .join("Marta Gil")
            .join("fogata")
    }

    #[test]
    fn nothing_personal_reaches_the_file() {
        use tracing_subscriber::{Layer, fmt, prelude::*};
        let data = data();
        let _ = fs::remove_dir_all(data.parent().unwrap().parent().unwrap());
        know_computer(&data);
        know_names(
            [
                "Leo",
                "Marta Gil",
                "The usual crew",
                "Gaming",
                "Corner Café",
                "marta-pc",
            ]
            .map(String::from),
        );
        let subscriber = tracing_subscriber::registry().with(
            fmt::layer()
                .with_ansi(false)
                .fmt_fields(ScrubFields)
                .with_writer(DayFiles::new(&data))
                .with_filter(tracing_subscriber::EnvFilter::new(LEVELS)),
        );
        tracing::subscriber::with_default(subscriber, write_events);
        let path = data.join(DIR).join(file_name(today()));
        // Without each line's timestamp: its digits (".4001..." microseconds) can spell a port.
        let out = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|line| line.split_once(' ').map_or(line, |(_, rest)| rest))
            .collect::<Vec<_>>()
            .join("\n");
        for gone in [
            // Message text and other text in quotes.
            "see you at 8",
            "hello",
            "joining",
            "ice-pwd",
            "IN IP4",
            // Names of people, groups, the computer and its user.
            "Leo",
            "Marta",
            "Gil",
            "usual crew",
            "Gaming",
            "Café",
            "Corner",
            "Bea",
            "Ruiz",
            "Sofía",
            "marta-pc",
            "Laptop",
            // Addresses and ports.
            "203.0.113",
            "198.51.100",
            "192.168.1.20",
            "2001:db8",
            "fe80",
            "40123",
            "4001",
            "51820",
            // Full ids and keys.
            PEER,
            OTHER,
            "12D3KooW",
            DEVICE,
            SECRET,
            fp_full(),
            "K2XR9PMD",
            "AQIDBAUG",
            "[1, 2, 3",
            // Files and where they are.
            "holiday",
            "beach",
            "report",
            "Music",
            ".ogg",
            "AppData",
            "Users",
            "/tmp",
            // Links, codes and e-mail addresses.
            "fogata:",
            "FGT-7Q4MK",
            "example.com",
        ] {
            assert!(!out.contains(gone), "{gone:?} is still in:\n{out}");
        }
        for kept in [
            "INFO fogata_net::node: member 7Q4M·K2XR connected peer=…VtXk3A path=direct \
             address=/ip4/‹ip›/udp/‹port›/quic-v1/p2p/…VtXk3A",
            "friend 7Q4M·K2XR online peer=…x6nXTN path=through member 4kx6nXTN hidden=false",
            "address confirmed address=/ip6/‹ip›/udp/‹port›/quic-v1",
            "joined the group ‹name›",
            "‹name› asks to join a group",
            "group ‹name› not loaded: cannot read the log: damaged",
            "cannot save group ‹name›: disk full",
            "is broken: 7Q4M·K2XR gave this device a key",
            "call step from 7Q4M·K2XR signal=Offer {‹…›}",
            "signal=Ring {‹…›}",
            "update: build 4 (0.1.0) from 7Q4M·K2XR, through 4kx6nXTN",
            "datagrams limited to 1200 bytes on this path",
            "friend request failed: the code ‹code› is wrong",
            "device b1946a…",
            "WARN connection{remote_addr=/ip4/‹ip›/tcp/‹port›/p2p/…x6nXTN peer=…x6nXTN}: \
             libp2p_tcp: port ‹port› closed on ‹name› after 3 tries",
            "host=‹host› text=‹value›",
        ] {
            assert!(out.contains(kept), "{kept:?} is missing from:\n{out}");
        }
        assert_eq!(out.lines().count(), 25, "{out}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&data.join(DIR)), 0o700);
            assert_eq!(mode(&path), 0o600);
        }
        fs::remove_dir_all(data.parent().unwrap().parent().unwrap()).unwrap();
    }

    /// A writer that keeps what is written to it.
    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// With `RUST_LOG` set to a level that shows everything, standard error is scrubbed as
    /// the days' log is; only the raw switch leaves the values as they are.
    #[test]
    fn standard_error_is_scrubbed_at_any_level() {
        use tracing_subscriber::prelude::*;
        // Nothing checked here needs the names known: learning them again would take them
        // from the test above while it runs.
        for (raw, shown) in [(false, false), (true, true)] {
            let sink = Sink::default();
            let writer = sink.clone();
            let subscriber = tracing_subscriber::registry().with(scrubbed_layer(
                move || writer.clone(),
                EnvFilter::new("trace"),
                raw,
            ));
            tracing::subscriber::with_default(subscriber, write_events);
            let out = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
            assert!(out.contains("datagrams limited"), "{out}");
            for secret in [PEER, "192.168.1.20", "see you at 8", "ice-pwd"] {
                assert_eq!(
                    out.contains(secret),
                    shown,
                    "{secret:?} (raw {raw}) in:\n{out}"
                );
            }
        }
    }

    fn fp_full() -> &'static str {
        "7Q4MK2XR9PMD3VHC"
    }

    #[test]
    fn dates() {
        assert_eq!(file_name(0), "fogata-1970-01-01.log");
        assert_eq!(file_name(20_391), "fogata-2025-10-30.log");
        assert_eq!(file_name(19_782), "fogata-2024-02-29.log");
        for day in [0, 59, 60, 365, 11_016, 19_782, 20_391, 40_000] {
            assert_eq!(day_of(&file_name(day)), Some(day));
        }
        assert_eq!(day_of("fogata-2025-13-01.log"), None);
        assert_eq!(day_of("fogata-2025-10-30.txt"), None);
        assert_eq!(day_of("notes.log"), None);
    }

    #[test]
    fn keeps_two_days_before_today() {
        let dir = std::env::temp_dir().join(format!("fogata-logs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let files = DayFiles::new(&dir);
        for day in 100..=104 {
            files.write_on(day, b"a line\n").unwrap();
        }
        fs::write(dir.join(DIR).join("other.txt"), b"kept").unwrap();
        let mut left: Vec<String> = fs::read_dir(dir.join(DIR))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                file_name(102),
                file_name(103),
                file_name(104),
                "other.txt".into()
            ]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The window's own day files (app-YYYY-MM-DD.log, written by the application) go
    /// after the same days as the library's.
    #[test]
    fn the_window_files_are_kept_as_long_as_the_day_files() {
        let dir = std::env::temp_dir().join(format!("fogata-logs-app-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(DIR)).unwrap();
        let app = |day| file_name(day).replacen("fogata-", "app-", 1);
        for day in [90, 100, 104] {
            fs::write(dir.join(DIR).join(app(day)), b"an error\n").unwrap();
        }
        let files = DayFiles::new(&dir);
        files.write_on(104, b"a line\n").unwrap();
        let mut left: Vec<String> = fs::read_dir(dir.join(DIR))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left, [app(104), file_name(104)]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_day_stops_at_its_cap() {
        let dir = std::env::temp_dir().join(format!("fogata-logs-cap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let files = DayFiles::new(&dir);
        let line = vec![b'x'; 1024 * 1024];
        for _ in 0..10 {
            files.write_on(7, &line).unwrap();
        }
        let path = dir.join(DIR).join(file_name(7));
        let size = fs::metadata(&path).unwrap().len();
        assert!(size <= DAY_CAP + 100, "{size}");
        assert!(fs::read(&path).unwrap().ends_with(b"size limit)\n"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
