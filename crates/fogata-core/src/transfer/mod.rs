//! Moving an account to another computer, directly between the two.
//!
//! The new computer shows a short code ([`receive`]); the person types it on the computer
//! that has the account ([`send`]). Both meet under a rendezvous point made from the code's
//! first characters (on the local network over mDNS, elsewhere on the DHT with punches at
//! the shared slots, as friends and members meet), check the code with SPAKE2 and encrypt
//! everything after it with the run's keys (fogata-friends' `transfer`).
//!
//! The computer with the account stops its node before anything is read, so what moves is
//! the folder as it was, the MLS state and the group logs at the same epochs. Every file goes
//! in the form a folder without a key keeps it and is sealed again on the new computer with a
//! key of its own keychain, in `incoming/` of its data folder. Nothing is used there until the
//! end, which goes in three steps so that there is never a second "you":
//!
//! 1. The new computer checks what it got (the identity opens, the profile and every group
//!    log read) and says `Staged`.
//! 2. The old one writes `moved.json` ("moving": from now on it never runs as that identity)
//!    and says `Commit`.
//! 3. The new one puts `incoming/` in place of its folder's files, a step that goes on at the
//!    next start if the computer stops in the middle, and says `Active`; the old one then
//!    deletes its identity, its friend secrets and its MLS state, and shows that the account
//!    moved. Chats and files stay there, sealed, until the person deletes them ([`forget`]).
//!
//! A move cut short before step 2 changes nothing on either side: the new computer throws
//! away `incoming/`, or, when only its `Staged` was lost, keeps it until the person throws it
//! away or the old computer offers the account anew. Cut between 2 and 3, the old computer stays "moving" and the new one
//! keeps `incoming/` as staged: showing a code again there and typing it on the old one
//! finishes the move ([`Msg::Resume`]). Only the person can make the old computer use the
//! account again then ([`keep_this_computer`]), after a warning.

pub(crate) mod files;
pub(crate) mod wire;

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use fogata_crypto::{DeviceIdentity, at_rest::DataKey};
use fogata_friends::{
    CodeProblem, MAX_FAILED_TRANSFER_RUNS, Profile, ReceiverRun, RunError,
    TRANSFER_CODE_LIFETIME_MS, TransferCode, run_reply, run_start,
};
use fogata_net::{NodeConfig, NodeHandle, PeerId};
use futures::StreamExt as _;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub(crate) use self::files::{INCOMING_DIR, MOVED_FILE, REPLACED_DIR, STAGE_FILE};
use self::{
    files::{Item, Kind, MOVED_IN_FILE, Sink, Source},
    wire::{CHUNK, Channel, Frame, Msg, OLD_PROTOCOL, PROTOCOL, within},
};
use crate::{
    NetOptions, Store, now_ms,
    store::write_private,
    vault::{self, Keychain},
};

/// How long the computer where the code was typed looks for the one showing it.
pub const NO_CONTACT_MS: u64 = 3 * 60 * 1000;
/// How long the computer with the account waits for the person to say yes.
const CONFIRM_WAIT: Duration = Duration::from_secs(5 * 60);
/// How long either side waits for the next message while files go, and for the new computer
/// to check and put away what it got.
const STEP_WAIT: Duration = Duration::from_secs(60);
const CHECK_WAIT: Duration = Duration::from_secs(10 * 60);
/// How long the old computer waits, once it stopped using the account, for the new one to
/// say it put the account in place (it says so before it removes what it replaced).
const ACTIVE_WAIT: Duration = Duration::from_secs(5 * 60);
/// How long the new computer waits, once it proved it took the account, for the old one to
/// say it stopped for good.
const MOVED_WAIT: Duration = Duration::from_secs(10);
/// How long the new computer keeps the secret of a move it took when that word did not
/// come: an old computer that did not hear it took the account stays "moving" until a code
/// shown here finishes the move with it, which its person does on the spot. After that it
/// goes at the next start ([`forget_stale_move_key`]); the old computer can then only be
/// told by hand (forgetting the account there, or keeping it).
const MOVE_KEY_KEPT_MS: u64 = 7 * 24 * 60 * 60 * 1000;
/// The run of a code, both ways.
const RUN_WAIT: Duration = Duration::from_secs(20);
/// How long the new computer waits for the first message of a run: it is sent as the
/// connection opens, so a computer that says nothing does not hold the code for long.
const START_WAIT: Duration = Duration::from_secs(5);
/// How long the new computer waits for the proof of a run once it answered its start: the
/// computer typing the code sends it as soon as the answer comes, so a few round trips
/// over the slowest relay are plenty.
const PROOF_WAIT: Duration = Duration::from_secs(10);
/// Most runs of the code the new computer answers at once, each of another computer: a
/// stranger that learnt the nameplate and stalls holds one of them, not the code. The
/// computer where the code was typed runs it with at most as many at once.
const MAX_RUNS: usize = 8;
/// How long a computer found only over the DHT waits for one on the local network.
const LAN_FIRST: Duration = Duration::from_secs(5);
/// A computer that took the protocol but did not answer a run is tried again after this.
const RETRY_PEER: Duration = Duration::from_secs(5);
/// Most computers the computer where the code was typed proves it to. Each one that answers
/// a run under the code's nameplate with a SPAKE2 message learns from that proof whether
/// the one guess of the code it made was right (one in 2^25 for the five characters the
/// nameplate does not show), and whoever learnt the nameplate can answer, so every proof
/// is a guess given away: a stranger that answers first no longer spends the move, but the
/// guesses stay few, and as many as the computer showing the code allows the other way
/// ([`MAX_FAILED_TRANSFER_RUNS`]).
const MAX_PROVED_TO: u32 = 3;
/// How long, at least, the computer where the code was typed looks for the computer showing
/// it once one answered under the code and did not prove it back (or showed it the old way):
/// that one may be a stranger, and the one showing the code found a little later (over the
/// DHT, [`LAN_FIRST`] after it shows). Counted from when it began to look, so a code
/// mistyped is told a few seconds after it was tried, not minutes.
const SETTLE: Duration = Duration::from_secs(15);
/// How often the computer where the code was typed looks at the computers it found.
const LOOK_EVERY: Duration = Duration::from_millis(300);
/// What may come past the bytes the old computer said would come (files that grew).
const SLACK_BYTES: u64 = 64 * 1024 * 1024;
/// Most files a move takes: far more than a data folder holds.
pub(crate) const MAX_FILES: u32 = 500_000;
/// Most bytes a move takes: far more than a data folder holds.
pub(crate) const MAX_BYTES: u64 = 1 << 40;
/// The slowest pace a move is given time for, besides a fixed part: a move that goes slower
/// (a peer that sends a byte now and then) is stopped.
const SLOWEST_BYTES_PER_SEC: u64 = 32 * 1024;
const FIXED_TIME: Duration = Duration::from_secs(10 * 60);
/// Settings in `app.json` that belong to the computer, not to the person.
const COMPUTER_SETTINGS: [&str; 3] = ["audioInput", "audioOutput", "videoInput"];

/// How a move is run.
#[derive(Clone, Debug)]
pub struct TransferOptions {
    pub net: NetOptions,
    /// What this computer calls itself, shown on the other one.
    pub computer: String,
    /// The keychain the new computer seals with; `None` for the one this process uses
    /// ([`vault::use_keychain`]).
    pub keychain: Option<Keychain>,
    /// How long the code shown stays valid (new computer).
    pub code_lifetime_ms: u64,
    /// How long the code typed is looked for (old computer).
    pub no_contact_ms: u64,
    /// The free space on the disk of a folder, as the system tells it (the application
    /// asks the system itself; this crate makes no system calls): the new computer refuses
    /// an account it has no room for. `None`, or a disk it cannot tell, and a full disk
    /// stops the move instead.
    pub free_space: Option<fn(&Path) -> Option<u64>>,
    /// Tests only: the code the new computer shows.
    #[doc(hidden)]
    pub code: Option<String>,
    /// Tests only: the old computer stops right after it stopped using the account, as if
    /// the connection broke before it could tell the new one.
    #[doc(hidden)]
    pub lose_commit: bool,
    /// Tests only: the new computer stops right after it kept what came, as if its word
    /// that all came were lost.
    #[doc(hidden)]
    pub lose_staged: bool,
    /// Tests only: the new computer stops right after it took the account, as if its word
    /// that it did were lost.
    #[doc(hidden)]
    pub lose_active: bool,
    /// Tests only: the old computer stops right after it stopped for good, as if its word
    /// that it did were lost.
    #[doc(hidden)]
    pub lose_moved: bool,
}

impl TransferOptions {
    pub fn new(net: NetOptions, computer: impl Into<String>) -> Self {
        Self {
            net,
            computer: computer.into(),
            keychain: None,
            code_lifetime_ms: TRANSFER_CODE_LIFETIME_MS,
            no_contact_ms: NO_CONTACT_MS,
            free_space: None,
            code: None,
            lose_commit: false,
            lose_staged: false,
            lose_active: false,
            lose_moved: false,
        }
    }
}

/// What moves, as the old computer counts it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// The account's profile: its name and picture, signed by its identity.
    pub profile: Profile,
    pub friends: u32,
    pub groups: u32,
    pub chats: u32,
    /// Attachments kept in chats.
    pub files: u32,
    /// Bytes of every file that moves.
    pub bytes: u64,
}

/// Why a move stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The computer that answered did not accept the code typed (nothing was sent).
    WrongCode,
    /// More than one computer answered under the code typed and none proved it back
    /// (nothing was sent): a code mistyped makes only the computer showing it say no, so at
    /// least one of them is another device, which may be trying to keep the move from
    /// happening.
    OthersAnswered,
    /// The code shown ran out before it was used.
    Expired,
    /// Several runs with wrong codes: the code shown stops working.
    TooManyTries,
    /// The two computers did not reach each other.
    NoContact,
    /// The connection was lost, or a computer stopped answering, before the end; nothing
    /// changed on either side.
    Interrupted(String),
    /// Everything went but the very end was not confirmed: the old computer stays "moving",
    /// the new one keeps what it got, until a code shown on the new one finishes the move.
    Unconfirmed,
    /// The person said no on this computer.
    Cancelled,
    /// The other computer stopped the move (its person said no, say).
    Declined(String),
    /// The new computer has everything and the old one stopped using the account, but
    /// putting it in place here stopped (a full disk, say): it goes on when Fogata starts
    /// again here.
    Later(String),
    /// The new computer has too little free space for what the old one offers.
    NoRoom { need: u64, free: u64 },
    /// The old computer offered far more than an account holds.
    TooLarge { offered: u64 },
    /// The new computer is finishing another move.
    Busy,
    /// The new computer has no move to finish from the computer asking.
    NothingToFinish,
    /// This computer's network stopped.
    NetworkStopped,
    /// The other computer runs the code the way of versions up to 0.1.0-beta.7
    /// (`/fogata/transfer/1`), which this one does not: both must run the same version.
    /// Nothing was tried or sent on either side.
    OldVersion,
    /// Something on this computer (its disk, its keychain).
    Other(String),
}

/// What [`Failure::Interrupted`] holds when the connection went with no other reason.
pub const CONNECTION_LOST: &str = "the connection was lost";

/// Why the other computer stopped a move, among the reasons this version words (sent as
/// text in `Msg::Fail`, which older and newer versions may word otherwise), or decided here
/// from what it answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerReason {
    NoRoom,
    TooLarge,
    Busy,
    CouldNotKeep,
    Cancelled,
    CouldNotFinish,
    Stopped,
    NothingToFinish,
    NotProven,
}

impl PeerReason {
    const ALL: [PeerReason; 9] = [
        PeerReason::NoRoom,
        PeerReason::TooLarge,
        PeerReason::Busy,
        PeerReason::CouldNotKeep,
        PeerReason::Cancelled,
        PeerReason::CouldNotFinish,
        PeerReason::Stopped,
        PeerReason::NothingToFinish,
        PeerReason::NotProven,
    ];

    /// The words for it, as sent and as the command line shows them.
    pub fn text(self) -> &'static str {
        match self {
            PeerReason::NoRoom => "the new computer has not enough free space",
            PeerReason::TooLarge => "the account is larger than a move takes",
            PeerReason::Busy => "the new computer is finishing another move",
            PeerReason::CouldNotKeep => "the new computer could not keep the files",
            PeerReason::Cancelled => "the move was cancelled on the other computer",
            PeerReason::CouldNotFinish => "the new computer could not finish the move yet",
            PeerReason::Stopped => "the move was stopped on the other computer",
            PeerReason::NothingToFinish => "that computer has no move to finish from this one",
            PeerReason::NotProven => "that computer did not prove it has this move",
        }
    }

    /// The reason the other computer's words stand for; `None` for words this version
    /// does not send.
    pub fn of(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.text() == text)
    }
}

impl Failure {
    /// Why the other computer stopped the move, when it is one of the known reasons.
    pub fn peer_reason(&self) -> Option<PeerReason> {
        match self {
            Failure::Declined(why) => PeerReason::of(why),
            _ => None,
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::WrongCode => {
                f.write_str("the other computer did not accept the code, so nothing was sent")
            }
            Failure::OthersAnswered => f.write_str(
                "several computers answered this code and none of them knew it, so nothing was \
                 sent; another device may be trying to stop the move",
            ),
            Failure::Expired => f.write_str("the code expired before it was used"),
            Failure::TooManyTries => {
                f.write_str("several wrong codes were tried; the code stopped working")
            }
            Failure::NoContact => f.write_str("the two computers could not reach each other"),
            Failure::Interrupted(why) => write!(
                f,
                "the move stopped before the end ({why}); nothing changed"
            ),
            Failure::Unconfirmed => f.write_str("the very end of the move was not confirmed"),
            Failure::Cancelled => f.write_str("cancelled"),
            Failure::Declined(why) => write!(f, "the other computer stopped the move ({why})"),
            Failure::Later(why) => write!(
                f,
                "the account came, and is put in place the next time Fogata starts here ({why})"
            ),
            Failure::NoRoom { need, free } => write!(
                f,
                "this computer needs {} free for the account, and has {}",
                human_bytes(*need),
                human_bytes(*free)
            ),
            Failure::TooLarge { offered } => write!(
                f,
                "the other computer offered {}, far more than an account holds",
                human_bytes(*offered)
            ),
            Failure::Busy => {
                f.write_str("this computer is finishing another move: finish or discard it first")
            }
            Failure::NothingToFinish => {
                f.write_str("this computer has no move to finish from that computer")
            }
            Failure::NetworkStopped => f.write_str("the network stopped"),
            Failure::OldVersion => f.write_str(
                "the other computer has an older version of Fogata: update both computers to \
                 the same version",
            ),
            Failure::Other(why) => f.write_str(why),
        }
    }
}

/// What a move tells the screens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferEvent {
    /// New computer: the code to show, and until when it works.
    Code {
        code: String,
        expires_ms: u64,
    },
    /// Looking for the other computer.
    Looking,
    /// New computer: someone tried a wrong code against the one shown.
    WrongCodeTried,
    /// The code matched on both computers. The old one waits for the person's yes
    /// ([`Transfer::confirm`]); the new one shows what comes. `computer` is the other
    /// computer's name, `lan` whether the two are on one local network, `replaces` the
    /// profile the new computer has now.
    Matched {
        computer: String,
        summary: Summary,
        lan: bool,
        replaces: Option<String>,
    },
    /// Bytes gone or come so far, of all.
    Progress {
        done: u64,
        total: u64,
    },
    /// Everything went; the two are finishing.
    Finishing,
    /// The account is on the new computer and the old one stopped using it.
    Done {
        computer: String,
        summary: Option<Summary>,
    },
    Failed(Failure),
}

/// A move under way: its events, and the person's answer on the old computer.
#[derive(Debug)]
pub struct Transfer {
    pub events: mpsc::UnboundedReceiver<TransferEvent>,
    control: mpsc::UnboundedSender<bool>,
    task: tokio::task::JoinHandle<()>,
    stage: Arc<Stage>,
}

/// How far a move got, for what stopping it may do: before the code checked out, the move's
/// task simply goes; after, it stops at a point where nothing is half done; once the end
/// began (the account staged on the new computer, or the old one no longer using it), it
/// is not stopped any more.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Seeking,
    Running,
    Ending,
}

#[derive(Debug, Default)]
struct Stage {
    phase: std::sync::Mutex<Phase>,
    stop: std::sync::atomic::AtomicBool,
}

impl Stage {
    /// Moves on to `phase`, unless the move was stopped meanwhile.
    fn enter(&self, phase: Phase) -> Result<()> {
        let mut now = self.phase.lock().unwrap_or_else(|e| e.into_inner());
        if self.stopped() {
            bail!(Failure::Cancelled);
        }
        *now = phase;
        Ok(())
    }

    fn stopped(&self) -> bool {
        self.stop.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Stops the move as far as it may be; whether it will stop.
    fn stop(
        &self,
        control: &mpsc::UnboundedSender<bool>,
        abort: &tokio::task::AbortHandle,
    ) -> bool {
        let phase = self.phase.lock().unwrap_or_else(|e| e.into_inner());
        if *phase == Phase::Ending {
            return false;
        }
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = control.send(false);
        if *phase == Phase::Seeking {
            abort.abort();
        }
        true
    }
}

/// What a move's task shares with its handle, and holds while it runs.
#[derive(Debug)]
struct Flow {
    stage: Arc<Stage>,
    /// The data folder's running lock. The work that puts a move in place holds it too, so
    /// the folder is never let go of while that goes on.
    lock: Arc<fs::File>,
}

tokio::task_local! {
    static FLOW: Arc<Flow>;
}

/// The flow of the move this task runs, if any (tests call parts of a move without one).
fn flow() -> Option<Arc<Flow>> {
    FLOW.try_with(Arc::clone).ok()
}

/// Moves on to `phase` (see [`Stage::enter`]).
fn enter(phase: Phase) -> Result<()> {
    match flow() {
        Some(flow) => flow.stage.enter(phase),
        None => Ok(()),
    }
}

/// Runs `work` until it ends or the move is stopped.
async fn stoppable<T>(work: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    let Some(flow) = flow() else {
        return work.await;
    };
    tokio::select! {
        done = work => done,
        () = async {
            while !flow.stage.stopped() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        } => bail!(Failure::Cancelled),
    }
}

impl Transfer {
    /// The old computer's person said yes (`true`) or no to the move shown.
    pub fn confirm(&self, yes: bool) {
        let _ = self.control.send(yes);
    }

    /// Stops the move, unless its end began (see [`Phase`]); whether it stops. Stopped,
    /// nothing changes on either computer.
    pub fn cancel(&self) -> bool {
        self.stage.stop(&self.control, &self.task.abort_handle())
    }

    /// What answers or stops the move, apart from its events.
    pub fn controls(&self) -> TransferControl {
        TransferControl {
            control: self.control.clone(),
            abort: self.task.abort_handle(),
            stage: self.stage.clone(),
        }
    }

    /// Waits for the move to end, and gives its last event.
    pub async fn finish(mut self) -> TransferEvent {
        let mut last = TransferEvent::Failed(Failure::Cancelled);
        while let Some(event) = self.events.recv().await {
            let end = matches!(event, TransferEvent::Done { .. } | TransferEvent::Failed(_));
            last = event;
            if end {
                break;
            }
        }
        last
    }
}

/// Answers or stops a move ([`Transfer::controls`]).
#[derive(Clone, Debug)]
pub struct TransferControl {
    control: mpsc::UnboundedSender<bool>,
    abort: tokio::task::AbortHandle,
    stage: Arc<Stage>,
}

impl TransferControl {
    /// See [`Transfer::confirm`].
    pub fn confirm(&self, yes: bool) {
        let _ = self.control.send(yes);
    }

    /// See [`Transfer::cancel`].
    pub fn cancel(&self) -> bool {
        self.stage.stop(&self.control, &self.abort)
    }
}

/// How far a move got on this computer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovedState {
    /// The new computer may not have confirmed yet; this one never runs the account again
    /// unless the person says so.
    Moving,
    Moved,
}

/// `moved.json`: this computer's account went to another one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moved {
    pub state: MovedState,
    pub transfer: String,
    /// The other computer's name.
    pub to: String,
    /// The account's name.
    pub name: String,
    pub at_ms: u64,
    /// What the computer the account went to proves it is with.
    #[serde(default)]
    pub(crate) key: MoveKey,
}

/// The secret of a move ([`Channel::move_key`]), which both computers keep until it ends: the
/// new one proves with it, in a later run that finishes the move, that it is the computer
/// the account went to.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MoveKey(pub(crate) String);

impl std::fmt::Debug for MoveKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MoveKey(..)")
    }
}

impl<S> From<&Channel<S>> for MoveKey {
    fn from(channel: &Channel<S>) -> Self {
        Self(channel.move_key().to_string())
    }
}

/// How far a move into this computer got (`incoming.json`, beside `incoming/`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageState {
    /// Files are coming.
    Receiving,
    /// Everything came and was checked; the old computer has not said it stopped.
    Staged,
    /// The old computer stopped: the files are being put in place.
    Commit,
    /// Being put in place, and the folder's own files went aside to `replaced/`.
    Placing,
}

impl StageState {
    /// Whether the move is being put in place: from here on it is finished, never thrown
    /// away.
    fn placing(&self) -> bool {
        matches!(self, StageState::Commit | StageState::Placing)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Incoming {
    pub state: StageState,
    pub transfer: String,
    /// The old computer's name and the account's.
    pub from: String,
    pub name: String,
    /// The account's device key in hex.
    pub device: String,
    /// What this computer proves with that the account came here.
    #[serde(default)]
    pub(crate) key: MoveKey,
    /// Set when what comes is a copy being restored ([`crate::recovery`]) rather than a move:
    /// whether the account it replaces is kept in `replaced/` afterwards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) restore: Option<bool>,
}

/// Where a data folder stands with moves, for the screens to choose what to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferState {
    None,
    /// The account moved away (or is moving): the folder never runs it again.
    Moved(Moved),
    /// A move into this folder came whole but was not confirmed by the old computer.
    Staged(Incoming),
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// How far a move into `dir` got, if one did.
fn stage_of(dir: &Path) -> Option<Incoming> {
    read_json(&dir.join(STAGE_FILE))
}

pub(crate) fn write_stage(dir: &Path, record: &Incoming) -> Result<()> {
    write_private(&dir.join(STAGE_FILE), &serde_json::to_vec_pretty(record)?)
}

/// The account of the folder `dir` moved away, if it did.
pub fn moved(dir: &Path) -> Option<Moved> {
    let path = dir.join(MOVED_FILE);
    if !path.exists() {
        return None;
    }
    // A marker that does not read still means moved: never run the account again.
    Some(read_json(&path).unwrap_or(Moved {
        state: MovedState::Moving,
        transfer: String::new(),
        to: String::new(),
        name: String::new(),
        at_ms: 0,
        key: MoveKey::default(),
    }))
}

/// Errors while an account that came to `dir` waits to be finished there: a profile made
/// meanwhile would be replaced by it.
pub(crate) fn refuse_if_staged(dir: &Path) -> Result<()> {
    if dir.join(STAGE_FILE).exists() {
        bail!(
            "an account that came from another computer waits to be finished here: finish \
             the move or throw it away first"
        );
    }
    Ok(())
}

/// Errors when the account of `dir` moved away.
pub(crate) fn refuse_if_moved(dir: &Path) -> Result<()> {
    if let Some(moved) = moved(dir) {
        let to = if moved.to.is_empty() {
            "another computer".to_string()
        } else {
            format!("“{}”", moved.to)
        };
        bail!("this account moved to {to}; it is no longer used on this computer");
    }
    Ok(())
}

/// Where the folder `dir` stands with moves. Finishes first a move that was being put in
/// place when the computer stopped, when no copy of Fogata runs on the folder.
pub fn state(dir: &Path) -> TransferState {
    recover(dir);
    if let Some(moved) = moved(dir) {
        return TransferState::Moved(moved);
    }
    match stage_of(dir) {
        Some(incoming) if incoming.state == StageState::Staged => TransferState::Staged(incoming),
        _ => TransferState::None,
    }
}

/// Finishes or throws away what a move into `dir` left, if no copy of Fogata runs on it.
pub fn recover(dir: &Path) {
    if !dir.join(INCOMING_DIR).exists()
        && !dir.join(REPLACED_DIR).exists()
        && !dir.join(STAGE_FILE).exists()
    {
        return;
    }
    let Ok(store) = Store::open(dir.to_path_buf()) else {
        return;
    };
    let Ok(_running) = store.hold_running() else {
        return;
    };
    if let Err(e) = recover_held(dir, &vault::chosen()) {
        tracing::warn!("cannot finish the move into {}: {e:#}", dir.display());
    }
}

/// [`recover`] for a caller that holds the folder's running lock: a move put in place half
/// way goes on, files of one that did not come whole go, and the secret of a move taken
/// long ago goes.
pub(crate) fn recover_held(dir: &Path, keychain: &Keychain) -> Result<()> {
    let incoming = dir.join(INCOMING_DIR);
    match stage_of(dir) {
        Some(stage) if stage.state.placing() => put_in_place(dir, &stage, keychain)?,
        Some(stage) if stage.state == StageState::Staged && incoming.exists() => {}
        _ if incoming.exists() || dir.join(STAGE_FILE).exists() => {
            discard(dir, keychain)?;
        }
        _ => {}
    }
    forget_stale_move_key(dir, now_ms());
    remove_replaced(dir, keychain)
}

/// Forgets the secret of the move that brought the account to `dir` once it is
/// [`MOVE_KEY_KEPT_MS`] old at `now_ms`, when the old computer's word that it stopped for
/// good never came to forget it at once ([`heard_moved`]).
fn forget_stale_move_key(dir: &Path, now_ms: u64) {
    let Some(moved_in) = read_json::<serde_json::Value>(&dir.join(MOVED_IN_FILE)) else {
        return;
    };
    let text = |name: &str| moved_in.get(name).and_then(|v| v.as_str()).unwrap_or("");
    let at_ms = moved_in.get("at_ms").and_then(|v| v.as_u64()).unwrap_or(0);
    if text("key").is_empty() || now_ms.saturating_sub(at_ms) < MOVE_KEY_KEPT_MS {
        return;
    }
    if let Err(e) = forget_move_key(dir, text("transfer")) {
        tracing::warn!("cannot forget the secret of a move taken long ago: {e:#}");
    }
}

/// Throws away a move into `dir` that did not finish, and the key it was sealed with.
pub(crate) fn discard(dir: &Path, keychain: &Keychain) -> Result<bool> {
    let incoming = dir.join(INCOMING_DIR);
    let record = dir.join(STAGE_FILE);
    // Once being put in place, a move is finished, never thrown away half way.
    match stage_of(dir) {
        Some(stage) if stage.state.placing() => {
            put_in_place(dir, &stage, keychain)?;
            return Ok(false);
        }
        // A record that does not read while the folder's own files are aside: neither
        // finished nor thrown away, so nothing of either goes.
        None if record.exists() && dir.join(REPLACED_DIR).exists() => {
            bail!("{} does not read", record.display());
        }
        _ => {}
    }
    if let (Some(name), Some(keys)) = (vault::key_name(&incoming), keychain.store())
        && vault::key_name(dir).as_deref() != Some(name.as_str())
        && let Err(e) = keys.delete(&name)
    {
        tracing::warn!("cannot remove a key from the keychain: {e}");
    }
    if incoming.exists() {
        fs::remove_dir_all(&incoming)
            .with_context(|| format!("cannot remove {}", incoming.display()))?;
    }
    remove_if_there(&record)?;
    Ok(true)
}

/// The new computer: throws away a move that came whole but was not confirmed by the old
/// computer, which then stays "moving" until its person keeps using it there.
///
/// Whether it was thrown away: `false` when it was already being put in place, which then
/// finished (the folder holds the account that came).
pub fn discard_incoming(dir: &Path) -> Result<bool> {
    let store = Store::open(dir.to_path_buf())?;
    let _running = store.hold_running()?;
    discard(dir, &vault::chosen())
}

/// Puts the files of `incoming/` in place of the folder's: the folder's own go to
/// `replaced/` first, then the new ones come out; `replaced/` goes afterwards. Each step can
/// be taken again, so a computer that stops in the middle goes on at the next start.
pub(crate) fn put_in_place(dir: &Path, stage: &Incoming, keychain: &Keychain) -> Result<()> {
    let incoming = dir.join(INCOMING_DIR);
    let replaced = dir.join(REPLACED_DIR);
    // What this computer's running application holds stays: its locks (the core's, and the
    // window's in `window/`, which a restart leaves a note in too), and the move's record.
    let keep = |name: &str| {
        matches!(
            name,
            INCOMING_DIR | REPLACED_DIR | STAGE_FILE | "running.lock" | "window"
        )
    };
    // The folder's own files go aside once: after that, files of the folder are new ones
    // already put in place.
    if stage.state == StageState::Commit {
        fs::create_dir_all(&replaced)?;
        for entry in fs::read_dir(dir)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if keep(&name) {
                continue;
            }
            // Already aside, the folder's own went there in an earlier try: this one was made
            // since (the empty folder for group logs a store makes as it opens).
            let to = replaced.join(&name);
            if fs::symlink_metadata(&to).is_ok() {
                remove_any(&entry.path())?;
                continue;
            }
            #[cfg(test)]
            crate::store::faults::check()?;
            fs::rename(entry.path(), &to)
                .with_context(|| format!("cannot move {} aside", entry.path().display()))?;
        }
        let mut placing = stage.clone();
        placing.state = StageState::Placing;
        write_stage(dir, &placing)?;
    }
    // The key of what was replaced, whatever it was when it went aside (a profile made
    // after the move came, say), goes now, while the files that name it are sure to be here.
    // An account a restore replaced is kept, and its key with it.
    if stage.restore != Some(true) {
        forget_replaced_key(dir, keychain);
    }
    if incoming.exists() {
        for entry in fs::read_dir(&incoming)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if keep(&name) {
                continue;
            }
            // Whatever is there was made after the folder's own files went aside (the empty
            // folder for group logs a store makes as it opens), never one of them.
            let to = dir.join(&name);
            if fs::symlink_metadata(&to).is_ok() {
                remove_any(&to)?;
            }
            #[cfg(test)]
            crate::store::faults::check()?;
            fs::rename(entry.path(), &to).with_context(|| format!("cannot put {name} in place"))?;
        }
    }
    match stage.restore {
        Some(kept) => crate::recovery::note_restored(dir, kept)?,
        None => write_private(
            &dir.join(MOVED_IN_FILE),
            &serde_json::to_vec_pretty(&serde_json::json!({
                "transfer": stage.transfer,
                "from": stage.from,
                "at_ms": now_ms(),
                "key": stage.key.0,
            }))?,
        )?,
    }
    if incoming.exists() {
        fs::remove_dir_all(&incoming)?;
    }
    // The account is in place: what it replaced goes after (see [`remove_replaced`]).
    remove_if_there(&dir.join(STAGE_FILE))
}

/// Removes the keychain key `replaced/` names, unless the folder or what came uses it.
pub(crate) fn forget_replaced_key(dir: &Path, keychain: &Keychain) {
    let Some(name) = vault::key_name(&dir.join(REPLACED_DIR)) else {
        return;
    };
    let used = [dir.to_path_buf(), dir.join(INCOMING_DIR)]
        .iter()
        .any(|d| vault::key_name(d).as_deref() == Some(name.as_str()));
    if let Some(keys) = keychain.store()
        && !used
        && let Err(e) = keys.delete(&name)
    {
        tracing::warn!("cannot remove the replaced folder's key from the keychain: {e}");
    }
}

/// Removes `replaced/` once a move into `dir` is in place, with its key (again, in case the
/// keychain could not be reached before).
pub(crate) fn remove_replaced(dir: &Path, keychain: &Keychain) -> Result<()> {
    let replaced = dir.join(REPLACED_DIR);
    if replaced.exists()
        && !dir.join(INCOMING_DIR).exists()
        && !dir.join(STAGE_FILE).exists()
        && !crate::recovery::keeps_replaced(dir)
    {
        forget_replaced_key(dir, keychain);
        fs::remove_dir_all(&replaced)
            .with_context(|| format!("cannot remove {}", replaced.display()))?;
    }
    Ok(())
}

pub(crate) fn remove_if_there(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(e).with_context(|| format!("cannot remove {}", path.display()))
        }
        _ => Ok(()),
    }
}

fn remove_any(path: &Path) -> Result<()> {
    if fs::symlink_metadata(path)?.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// The old computer, after the move: deletes everything left of the account in `dir`
/// (chats, files, logs) and the folder's key in the keychain. Fogata then starts there
/// like new.
pub fn forget(dir: &Path) -> Result<()> {
    let Some(moved) = moved(dir) else {
        bail!("this account did not move to another computer");
    };
    if moved.state != MovedState::Moved {
        bail!("the move is not finished: the other computer has not confirmed it yet");
    }
    // Another person's account, which a restore replaced and the folder keeps until its
    // person says otherwise, does not go with what is left of this one.
    crate::recovery::refuse_if_kept(dir)?;
    let store = Store::open(dir.to_path_buf())?;
    let _running = store.hold_running()?;
    let key = vault::key_name(dir);
    for entry in fs::read_dir(dir)?.flatten() {
        if entry.file_name() == "running.lock" || entry.file_name() == MOVED_FILE {
            continue;
        }
        remove_any(&entry.path())?;
    }
    if let (Some(name), Some(keys)) = (key, vault::chosen().store())
        && let Err(e) = keys.delete(&name)
    {
        tracing::warn!("cannot remove the folder's key from the keychain: {e}");
    }
    // Last, so a deletion cut short is tried again and the account never runs meanwhile.
    fs::remove_file(dir.join(MOVED_FILE))?;
    Ok(())
}

/// The old computer, "moving": uses the account here again, when the new computer never
/// got it. If it did, there would be two of the same person; the screens warn first.
pub fn keep_this_computer(dir: &Path) -> Result<()> {
    let Some(moved) = moved(dir) else {
        return Ok(());
    };
    if moved.state == MovedState::Moved || !dir.join("identity.key").exists() {
        bail!("the account is on the other computer now");
    }
    let store = Store::open(dir.to_path_buf())?;
    let _running = store.hold_running()?;
    fs::remove_file(dir.join(MOVED_FILE))?;
    Ok(())
}

/// Whether `text` reads as a code, before anything is looked for.
pub fn check_code(text: &str) -> Result<(), CodeProblem> {
    TransferCode::parse(text).map(|_| ())
}

/// A node of its own for the move, with a new transport identity and nothing else: no
/// friend or group sees this computer while the move is under way.
fn start_node(options: &TransferOptions, port: Option<u16>) -> Result<NodeHandle> {
    let net = &options.net;
    let (node, mut events) = fogata_net::start(NodeConfig {
        identity: Arc::new(DeviceIdentity::generate()?),
        groups: Vec::new(),
        friends: None,
        listen_port: if net.listen_port != 0 {
            net.listen_port
        } else {
            port.unwrap_or(0)
        },
        bootstrap: net.bootstrap.clone(),
        use_dht: net.use_dht,
        use_mdns: net.use_mdns,
        bridge: false,
        peers: net.peers.clone(),
        remembered: Vec::new(),
        hidden: false,
    })?;
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            if let fogata_net::NodeEvent::Info(text) = event {
                tracing::debug!("move: {text}");
            }
        }
    });
    Ok(node)
}

async fn is_lan(node: &NodeHandle, peer: PeerId) -> bool {
    matches!(node.peer_path(peer).await, Ok(Some(fogata_net::Path::Lan)))
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(fogata_net::task_failed)?
}

// ----- The new computer -----

/// Starts showing a code on the new computer, and moves the account typed with it into the
/// folder `dir` (replacing what is there: the screens warn first). No core may run on it.
pub fn receive(dir: PathBuf, options: TransferOptions) -> Result<Transfer> {
    let store = Store::open(dir.clone())?;
    let running = Arc::new(store.hold_running()?);
    let keychain = options.keychain.clone().unwrap_or_else(vault::chosen);
    recover_held(&dir, &keychain)?;
    refuse_if_moved(&dir)?;
    crate::recovery::refuse_if_kept(&dir)?;
    let (events, rx) = mpsc::unbounded_channel();
    let (control, mut control_rx) = mpsc::unbounded_channel();
    let stage = Arc::new(Stage::default());
    let flow = Arc::new(Flow {
        stage: stage.clone(),
        lock: running,
    });
    let task = tokio::spawn(async move {
        let end = FLOW
            .scope(
                flow.clone(),
                receiving(&dir, &options, &keychain, &events, &mut control_rx),
            )
            .await;
        let end = match end {
            Ok(end) => end,
            Err(e) => TransferEvent::Failed(failure_or_other(e)),
        };
        // The folder is free once the end is told: the core may start on it at once. The
        // scope's own hold went with it.
        drop(flow);
        let _ = events.send(end);
    });
    Ok(Transfer {
        events: rx,
        control,
        task,
        stage,
    })
}

async fn receiving(
    dir: &Path,
    options: &TransferOptions,
    keychain: &Keychain,
    events: &mpsc::UnboundedSender<TransferEvent>,
    control: &mut mpsc::UnboundedReceiver<bool>,
) -> Result<TransferEvent> {
    let store = Store::open_with(dir.to_path_buf(), keychain.clone())?;
    // The port the account keeps on this computer, picked now when the folder has none, so
    // that a firewall rule made for it serves the move as well.
    let mut meta = store.meta();
    let port = match meta.port {
        Some(port) => port,
        None => {
            let port = crate::own_port()?;
            meta.port = Some(port);
            store.save_meta(&meta)?;
            port
        }
    };
    let replaces = if store.has_profile()? {
        store.friends().ok().flatten().map(|b| b.profile.name)
    } else {
        None
    };
    let code = match &options.code {
        Some(code) => TransferCode::parse(code).map_err(anyhow::Error::msg)?,
        None => TransferCode::generate()?,
    };
    let expires_ms = now_ms() + options.code_lifetime_ms;
    let _ = events.send(TransferEvent::Code {
        code: code.to_string(),
        expires_ms,
    });
    let node = start_node(options, Some(port))?;
    let mut incoming = node.streams().accept(PROTOCOL)?;
    // Only to tell a computer of the first way showing this code apart.
    let mut old_incoming = node.streams().accept(OLD_PROTOCOL)?;
    node.seek(Some(code.rendezvous()), Some(*code.lan_secret()))
        .await?;
    let _ = events.send(TransferEvent::Looking);
    let mut failed = 0;
    // Starts of the first way that named this code (see `answer_old_run`).
    let mut old_starts = 0;
    let expiry = tokio::time::sleep(Duration::from_millis(options.code_lifetime_ms));
    tokio::pin!(expiry);
    // Each run goes on its own up to the proof it brings, so that one that stalls keeps
    // neither the others, nor the person's Cancel, nor the code's end waiting. Proofs are
    // checked here, one at a time, so a stranger gets no more guesses at once than one
    // after the other.
    let mut runs = Runs::default();
    let mut old_runs = Runs::default();
    let end = loop {
        tokio::select! {
            _ = &mut expiry => break TransferEvent::Failed(Failure::Expired),
            answer = control.recv() => {
                if answer != Some(true) {
                    break TransferEvent::Failed(Failure::Cancelled);
                }
            }
            stream = old_incoming.next() => {
                let Some((peer, stream)) = stream else {
                    break TransferEvent::Failed(Failure::NetworkStopped);
                };
                let code = code.clone();
                old_runs.start(peer, async move { answer_old_run(&code, stream).await });
            }
            Some(named) = old_runs.next() => {
                if named {
                    old_starts += 1;
                    if old_starts >= MAX_FAILED_TRANSFER_RUNS {
                        break TransferEvent::Failed(Failure::OldVersion);
                    }
                }
            }
            stream = incoming.next() => {
                let Some((peer, stream)) = stream else {
                    break TransferEvent::Failed(Failure::NetworkStopped);
                };
                let lan = is_lan(&node, peer).await;
                let (local, remote) = (node.local_peer().to_bytes(), peer.to_bytes());
                let code = code.clone();
                runs.start(peer, async move {
                    (lan, answer_start(&code, &local, &remote, stream).await)
                });
            }
            Some((lan, started)) = runs.next() => {
                let answered = match started {
                    Ok(Some((run, proof, stream))) => answer_proof(run, &proof, stream).await,
                    Ok(None) => continue,
                    Err(e) => Err(e),
                };
                match answered {
                    Ok(channel) => {
                        runs.stop_all();
                        old_runs.stop_all();
                        if let Err(e) = enter(Phase::Running) {
                            break TransferEvent::Failed(failure_of(e));
                        }
                        break match received(dir, options, keychain, events, control, channel, lan, replaces.clone()).await {
                            Ok(done) => done,
                            Err(e) => TransferEvent::Failed(failure_of(e)),
                        };
                    }
                    Err(RunError::WrongCode) => {
                        failed += 1;
                        let _ = events.send(TransferEvent::WrongCodeTried);
                        if failed >= MAX_FAILED_TRANSFER_RUNS {
                            break TransferEvent::Failed(Failure::TooManyTries);
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    };
    runs.stop_all();
    old_runs.stop_all();
    let _ = node.seek(None, None).await;
    let _ = node.shutdown().await;
    Ok(end)
}

/// A name the other computer gave, as the screens may show it: tidied as names are, no
/// control characters, at most 64 characters; empty when nothing of it shows, for the
/// screens to word.
fn shown(name: &str) -> String {
    let name: String = fogata_crypto::names::tidy(name)
        .chars()
        .filter(|c| !c.is_control() && !fogata_crypto::names::hiding_mark(*c))
        .take(64)
        .collect();
    if name.trim().is_empty() {
        String::new()
    } else {
        name
    }
}

/// Putting what came in place failed after the old computer stopped using the account: it
/// goes on at the next start (see [`recover`]).
fn later(e: anyhow::Error) -> Failure {
    tracing::warn!("the account that came is put in place at the next start: {e:#}");
    Failure::Later(format!("{e:#}"))
}

/// A failure of the move's setup or end as the screens tell it: one it named, or
/// something on this computer.
fn failure_or_other(e: anyhow::Error) -> Failure {
    match e.downcast_ref::<Failure>() {
        Some(f) => f.clone(),
        None => Failure::Other(format!("{e:#}")),
    }
}

/// A failure as the screens tell it.
fn failure_of(e: anyhow::Error) -> Failure {
    if let Some(wire::Failed(reason)) = e.downcast_ref::<wire::Failed>() {
        return Failure::Declined(reason.clone());
    }
    if let Some(f) = e.downcast_ref::<Failure>() {
        return f.clone();
    }
    Failure::Interrupted(format!("{e:#}"))
}

impl std::error::Error for Failure {}

/// Answers a run on `stream`: the channel when the code matched, `None` for a run of
/// another code, an error otherwise. Only a wrong proof of the code is
/// [`RunError::WrongCode`], which counts: the answer to a start tells nothing of the code,
/// so a run that stops before its proof (a stranger that learnt the nameplate and stalls)
/// was no guess, and costs the code nothing. The new computer runs the two halves apart
/// ([`answer_start`] in a task of its own, [`answer_proof`] in its loop).
#[cfg(test)]
async fn answer_run<S: Link>(
    code: &TransferCode,
    local: &[u8],
    remote: &[u8],
    stream: S,
) -> Result<Option<Channel<S>>, RunError> {
    match answer_start(code, local, remote, stream).await? {
        Some((run, proof, stream)) => answer_proof(run, &proof, stream).await.map(Some),
        None => Ok(None),
    }
}

/// [`answer_run`] up to the proof the other computer sends: the run, that proof and the
/// stream, to check it on ([`answer_proof`]); `None` for a run of another code.
async fn answer_start<S: Link>(
    code: &TransferCode,
    local: &[u8],
    remote: &[u8],
    mut stream: S,
) -> Result<Option<(ReceiverRun, Vec<u8>, S)>, RunError> {
    let start = within(START_WAIT, wire::read_start(&mut stream))
        .await
        .map_err(|_| RunError::Malformed)?;
    let (run, reply) = match run_reply(code, local, remote, &start) {
        Ok(answer) => answer,
        Err(RunError::NotMine) => {
            let _ = wire::send_reply(&mut stream, None).await;
            return Ok(None);
        }
        Err(e) => return Err(e),
    };
    // Gone, or no proof in time: nothing was learnt of the code.
    wire::send_reply(&mut stream, Some(&reply))
        .await
        .map_err(|_| RunError::Malformed)?;
    let proof = within(PROOF_WAIT, wire::read_confirm(&mut stream))
        .await
        .map_err(|_| RunError::Malformed)?;
    Ok(Some((run, proof, stream)))
}

/// [`answer_run`] from the proof the other computer sent: the channel if it proves the
/// code. Its answer is one short message, given [`START_WAIT`] to go.
async fn answer_proof<S: Link>(
    run: ReceiverRun,
    proof: &[u8],
    mut stream: S,
) -> Result<Channel<S>, RunError> {
    let (keys, answer) = match run.finish(proof) {
        Ok(done) => done,
        Err(e) => {
            let _ = within(START_WAIT, wire::send_answer(&mut stream, None)).await;
            return Err(e);
        }
    };
    within(START_WAIT, wire::send_answer(&mut stream, Some(&answer)))
        .await
        .map_err(|_| RunError::Malformed)?;
    Ok(Channel::new(stream, &keys, false))
}

/// Runs of the code being answered, each in a task of its own: one per computer, the
/// newest stream of a computer in place of the run it had, and at most [`MAX_RUNS`], the
/// oldest stopped for a new one. A stranger that stalls holds one run until its wait ends;
/// stopping the oldest rather than turning the newest away means it must keep bringing new
/// computers faster than a real run ends (two round trips) to keep that one out.
struct Runs<T> {
    tasks: tokio::task::JoinSet<T>,
    /// The computer of each run, oldest first.
    of: Vec<(PeerId, tokio::task::AbortHandle)>,
}

impl<T> Default for Runs<T> {
    fn default() -> Self {
        Self {
            tasks: tokio::task::JoinSet::new(),
            of: Vec::new(),
        }
    }
}

impl<T: Send + 'static> Runs<T> {
    /// Starts `run`, of `peer`.
    fn start(&mut self, peer: PeerId, run: impl std::future::Future<Output = T> + Send + 'static) {
        self.of.retain(|(p, task)| {
            let same = *p == peer;
            if same {
                task.abort();
            }
            !same
        });
        if self.of.len() >= MAX_RUNS {
            let (_, oldest) = self.of.remove(0);
            oldest.abort();
        }
        let task = self.tasks.spawn(run);
        self.of.push((peer, task));
    }

    /// The next run to end, of those not stopped; `None` when none is left.
    async fn next(&mut self) -> Option<T> {
        loop {
            let ended = self.tasks.join_next_with_id().await?;
            let id = match &ended {
                Ok((id, _)) => *id,
                Err(e) => e.id(),
            };
            self.of.retain(|(_, task)| task.id() != id);
            if let Ok((_, out)) = ended {
                return Some(out);
            }
        }
    }

    /// Stops every run.
    fn stop_all(&mut self) {
        self.tasks.abort_all();
        self.of.clear();
    }
}

/// A start of the first way of running the code (`/fogata/transfer/1`) on `stream`: whether
/// it names this code. Nothing is answered then: a computer of a version up to 0.1.0-beta.7
/// typing the code tries again every few seconds until it gives up on reaching this one,
/// having spent nothing, and the move stops once [`MAX_FAILED_TRANSFER_RUNS`] such starts
/// came, so that the person updates both ([`Failure::OldVersion`]). One alone does not stop
/// it: whoever learnt the nameplate can send one, and the computer of this version that
/// typed the code may be on its way. A stranger can still end the code with that many, as
/// it can with as many wrong proofs; it learns nothing. A start for another code is told
/// so, as that way does.
async fn answer_old_run(code: &TransferCode, mut stream: impl Link) -> bool {
    let Ok(start) = within(START_WAIT, wire::read_start(&mut stream)).await else {
        return false;
    };
    if code.named_by(&start) {
        tracing::info!("a computer with an older Fogata typed this code");
        return true;
    }
    let _ = wire::send_reply(&mut stream, None).await;
    false
}

/// Whether `peer`, a computer that takes only the first way of running the code, shows
/// `code` ([`fogata_friends::old_protocol_probe`]): `Some(true)` when it does, `Some(false)`
/// when it shows another, `None` when that could not be told (no stream, no answer in time).
async fn shows_code_the_old_way(
    node: &NodeHandle,
    peer: PeerId,
    code: &TransferCode,
) -> Option<bool> {
    let stream = node.streams().open_stream(peer, OLD_PROTOCOL).await.ok()?;
    probe_old_way(code, stream).await
}

/// [`shows_code_the_old_way`] on `stream`.
async fn probe_old_way(code: &TransferCode, mut stream: impl Link) -> Option<bool> {
    wire::send_start(&mut stream, &fogata_friends::old_protocol_probe(code))
        .await
        .ok()?;
    match tokio::time::timeout(START_WAIT, wire::read_old_reply(&mut stream)).await {
        Ok(Ok(other)) => Some(!other),
        // It dropped the stream on a start it could not run: it shows this code.
        Ok(Err(_)) => Some(true),
        Err(_) => None,
    }
}

/// How a run of the code typed here ended ([`run_code`]).
enum Ran<S> {
    /// The two computers proved the code to each other.
    Both(Channel<S>),
    /// The other computer shows another code, or went before the code was tried on it, or
    /// answered when this computer had no proof left to give: not a try of the code.
    Untried { other_code: bool },
    /// This computer proved the code and the other did not: one of the code's
    /// [`MAX_PROVED_TO`] tries is spent, on a computer that is not the one showing it, or on
    /// a code mistyped.
    Wrong,
    /// This computer proved the code and heard nothing back: a try spent too.
    Lost,
}

/// The proofs of the code typed here that are left to give: one to each computer that
/// answers a run with a SPAKE2 message, at most [`MAX_PROVED_TO`] in all, shared by the runs
/// that go side by side.
#[derive(Clone, Debug, Default)]
struct Proofs(Arc<std::sync::atomic::AtomicU32>);

impl Proofs {
    /// Takes one, if any is left.
    fn take(&self) -> bool {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < MAX_PROVED_TO
    }

    /// Whether every one was given.
    fn spent(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::SeqCst) >= MAX_PROVED_TO
    }
}

/// Runs the code typed here on `stream`, to the computer at its other end: this computer
/// proves the code first, with one of `proofs`, and one of the code's tries is spent only
/// then.
async fn run_code<S: Link>(
    code: &TransferCode,
    local: &[u8],
    remote: &[u8],
    mut stream: S,
    proofs: &Proofs,
) -> Ran<S> {
    let (run, start) = run_start(code, local, remote);
    let reply = async {
        wire::send_start(&mut stream, &start).await?;
        wire::read_reply(&mut stream).await
    };
    let reply = match within(RUN_WAIT, reply).await {
        Ok(Some(reply)) => reply,
        Ok(None) => return Ran::Untried { other_code: true },
        Err(_) => return Ran::Untried { other_code: false },
    };
    // An answer that is no SPAKE2 message is no try of the code.
    let Ok((run, proof)) = run.prove(&reply) else {
        return Ran::Untried { other_code: false };
    };
    // The proof tells whoever answered whether its guess of the code was right.
    if !proofs.take() {
        return Ran::Untried { other_code: false };
    }
    let answer = async {
        wire::send_confirm(&mut stream, &proof).await?;
        wire::read_answer(&mut stream).await
    };
    match within(RUN_WAIT, answer).await {
        Ok(Some(answer)) => match run.finish(&answer) {
            Ok(keys) => Ran::Both(Channel::new(stream, &keys, true)),
            Err(_) => Ran::Wrong,
        },
        Ok(None) => Ran::Wrong,
        Err(_) => Ran::Lost,
    }
}

/// What runs over a move's connection.
trait Link: futures::AsyncRead + futures::AsyncWrite + Unpin {}
impl<S: futures::AsyncRead + futures::AsyncWrite + Unpin> Link for S {}

#[allow(clippy::too_many_arguments)]
async fn received(
    dir: &Path,
    options: &TransferOptions,
    keychain: &Keychain,
    events: &mpsc::UnboundedSender<TransferEvent>,
    control: &mut mpsc::UnboundedReceiver<bool>,
    mut channel: Channel<impl Link>,
    lan: bool,
    replaces: Option<String>,
) -> Result<TransferEvent> {
    channel
        .send(&Msg::Hello {
            computer: options.computer.clone(),
            replaces: replaces.clone(),
        })
        .await?;
    let staged = stage_of(dir);
    let offer = within(STEP_WAIT, channel.recv_msg()).await?;
    let (transfer, computer, summary) = match offer {
        Msg::Resume { transfer } => {
            return resumed(dir, keychain, events, &mut channel, staged, transfer).await;
        }
        Msg::Offer {
            transfer,
            computer,
            device,
            profile,
            friends,
            groups,
            chats,
            files,
            bytes,
        } => {
            let profile = base64::engine::general_purpose::STANDARD
                .decode(profile)
                .ok()
                .and_then(|b| Profile::decode(&b).ok())
                .context("the other computer sent a profile that does not check out")?;
            if hex::encode(profile.device.as_bytes()) != device || transfer.len() != 32 {
                bail!("the other computer's offer does not check out");
            }
            (
                transfer,
                shown(&computer),
                Summary {
                    profile,
                    friends,
                    groups,
                    chats,
                    files,
                    bytes,
                },
            )
        }
        _ => bail!("the other computer said something out of place"),
    };
    if summary.bytes > MAX_BYTES {
        channel
            .send(&Msg::Fail {
                reason: PeerReason::TooLarge.text().into(),
            })
            .await?;
        channel.linger().await;
        bail!(Failure::TooLarge {
            offered: summary.bytes
        });
    }
    // Room for all of it on this computer's disk, said before anything comes.
    if let Some(free) = options.free_space.and_then(|free_space| free_space(dir))
        && free < room_needed(summary.bytes)
    {
        channel
            .send(&Msg::Fail {
                reason: PeerReason::NoRoom.text().into(),
            })
            .await?;
        channel.linger().await;
        bail!(Failure::NoRoom {
            need: room_needed(summary.bytes),
            free
        });
    }
    // An account that came whole but whose old computer never stopped using it (it offers
    // it anew: one that did would finish the move with `Resume`) gives way to this offer of
    // the same account, once both say yes; that of another account waits to be finished or
    // thrown away.
    let device = hex::encode(summary.profile.device.as_bytes());
    if staged
        .as_ref()
        .is_some_and(|s| s.device != device || s.state.placing())
    {
        channel
            .send(&Msg::Fail {
                reason: PeerReason::Busy.text().into(),
            })
            .await?;
        channel.linger().await;
        bail!(Failure::Busy);
    }
    let _ = events.send(TransferEvent::Matched {
        computer: computer.clone(),
        summary: summary.clone(),
        lan,
        replaces,
    });
    // This computer's person says yes too, to what comes and from where: it takes the place
    // of what this folder holds. Then the old computer's yes, which may have come meanwhile.
    let yes = tokio::time::timeout(CONFIRM_WAIT, control.recv())
        .await
        .ok()
        .flatten()
        .unwrap_or(false);
    if !yes {
        tell_cancelled(&mut channel).await;
        bail!(Failure::Cancelled);
    }
    channel.send(&Msg::Go).await?;
    let go = stoppable(within(CONFIRM_WAIT + STEP_WAIT, channel.recv_msg())).await;
    if go.as_ref().is_err_and(cancelled) {
        tell_cancelled(&mut channel).await;
    }
    match go? {
        Msg::Go => {}
        _ => bail!("the other computer said something out of place"),
    }
    // The folder being filled, sealed with a key of this computer's own keychain.
    discard(dir, keychain)?;
    let stage = dir.join(INCOMING_DIR);
    fs::create_dir(&stage).with_context(|| format!("cannot create {}", stage.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&stage, fs::Permissions::from_mode(0o700));
    }
    let mut record = Incoming {
        state: StageState::Receiving,
        transfer: transfer.clone(),
        from: computer.clone(),
        name: summary.profile.name.clone(),
        device: hex::encode(summary.profile.device.as_bytes()),
        key: MoveKey::from(&channel),
        restore: None,
    };
    write_stage(dir, &record)?;
    // From the end of this on, what came stays until the old computer finishes the move or
    // the person throws it away: the move is not stopped any more.
    let result = async {
        take_files(dir, &stage, keychain, events, &mut channel, &summary).await?;
        enter(Phase::Ending)
    }
    .await;
    if let Err(e) = result {
        // Stopped here or failed: the other computer hears why, and what came goes with the
        // key it was sealed with, before the folder is let go of.
        if cancelled(&e) {
            tell_cancelled(&mut channel).await;
        } else {
            let _ = channel
                .send(&Msg::Fail {
                    reason: PeerReason::CouldNotKeep.text().into(),
                })
                .await;
            channel.linger().await;
        }
        let dir = dir.to_path_buf();
        let keychain = keychain.clone();
        if let Err(e) = blocking(move || discard(&dir, &keychain)).await {
            tracing::warn!("what came goes at the next start: {e:#}");
        }
        return Err(e);
    }
    record.state = StageState::Staged;
    write_stage(dir, &record)?;
    let _ = events.send(TransferEvent::Finishing);
    if options.lose_staged {
        channel.close().await;
        return Ok(TransferEvent::Failed(Failure::Unconfirmed));
    }
    channel.send(&Msg::Staged).await?;
    // Past this point the old computer may stop using the account at any moment: what came
    // stays until it says so or the person throws it away.
    match within(STEP_WAIT, channel.recv_msg()).await {
        Ok(Msg::Commit) => {}
        Ok(_) | Err(_) => return Ok(TransferEvent::Failed(Failure::Unconfirmed)),
    }
    let key = record.key.clone();
    let activated = match activate(dir, keychain, record).await {
        Ok(activated) => activated,
        Err(e) => return Ok(TransferEvent::Failed(uncommitted(&mut channel, e).await)),
    };
    // Put in place, or bound to be at the next start: the old computer stops for good.
    let proof = channel.proof(&transfer, &key.0);
    if options.lose_active {
        channel.close().await;
    } else if channel.send(&Msg::Active { proof }).await.is_ok() {
        heard_moved(dir, &mut channel, &transfer).await;
    }
    match activated {
        Activated::Now => {
            tokio::join!(channel.linger(), tidy(dir, keychain));
            Ok(TransferEvent::Done {
                computer,
                summary: Some(summary),
            })
        }
        Activated::Later(e) => {
            channel.linger().await;
            Ok(TransferEvent::Failed(later(e)))
        }
    }
}

/// Whether `e` is the move being stopped on this computer.
fn cancelled(e: &anyhow::Error) -> bool {
    matches!(e.downcast_ref::<Failure>(), Some(Failure::Cancelled))
}

/// Tells the other computer the move was stopped here.
async fn tell_cancelled(channel: &mut Channel<impl Link>) {
    let _ = channel
        .send(&Msg::Fail {
            reason: PeerReason::Cancelled.text().into(),
        })
        .await;
    channel.linger().await;
}

/// The old computer finishes a move it already sent.
async fn resumed(
    dir: &Path,
    keychain: &Keychain,
    events: &mpsc::UnboundedSender<TransferEvent>,
    channel: &mut Channel<impl Link>,
    staged: Option<Incoming>,
    transfer: String,
) -> Result<TransferEvent> {
    let moved_in = read_json::<serde_json::Value>(&dir.join(MOVED_IN_FILE))
        .filter(|v| v.get("transfer").and_then(|t| t.as_str()) == Some(transfer.as_str()));
    let mut deferred = None;
    let (from, key) = match staged {
        Some(record) if record.transfer == transfer => {
            enter(Phase::Ending)?;
            let _ = events.send(TransferEvent::Finishing);
            let found = (record.from.clone(), record.key.clone());
            match activate(dir, keychain, record).await {
                Ok(Activated::Now) => {}
                Ok(Activated::Later(e)) => deferred = Some(e),
                Err(e) => return Ok(TransferEvent::Failed(uncommitted(channel, e).await)),
            }
            found
        }
        _ if moved_in.is_some() => {
            let text = |name: &str| {
                moved_in
                    .as_ref()
                    .and_then(|v| v.get(name)?.as_str().map(str::to_string))
                    .unwrap_or_default()
            };
            (text("from"), MoveKey(text("key")))
        }
        _ => {
            channel.send(&Msg::Unknown).await?;
            channel.linger().await;
            return Ok(TransferEvent::Failed(Failure::NothingToFinish));
        }
    };
    let proof = channel.proof(&transfer, &key.0);
    let told = channel.send(&Msg::Active { proof }).await;
    if told.is_ok() {
        heard_moved(dir, channel, &transfer).await;
    }
    if let Some(e) = deferred {
        channel.linger().await;
        return Ok(TransferEvent::Failed(later(e)));
    }
    told?;
    tokio::join!(channel.linger(), tidy(dir, keychain));
    Ok(TransferEvent::Done {
        computer: from,
        summary: None,
    })
}

/// Waits for the old computer to say it stopped for good after the proof of move `transfer`
/// went, and then forgets the move's secret here too: it only ever proves this computer took
/// the account to an old one still waiting for it. Without that word (the link went, say)
/// it is kept, for the old computer to finish the move with.
async fn heard_moved(dir: &Path, channel: &mut Channel<impl Link>, transfer: &str) {
    if !matches!(within(MOVED_WAIT, channel.recv_msg()).await, Ok(Msg::Moved)) {
        return;
    }
    let (dir, transfer) = (dir.to_path_buf(), transfer.to_string());
    let lock = flow().map(|flow| flow.lock.clone());
    let forgot = blocking(move || {
        let _lock = lock;
        forget_move_key(&dir, &transfer)
    })
    .await;
    if let Err(e) = forgot {
        tracing::warn!("cannot forget the secret of a finished move: {e:#}");
    }
}

/// Empties the secret of move `transfer` where the new computer keeps it: `moved-in.json`,
/// or the record of a move still to be put in place at the next start, which writes it there.
fn forget_move_key(dir: &Path, transfer: &str) -> Result<()> {
    let path = dir.join(MOVED_IN_FILE);
    if let Some(mut moved_in) = read_json::<serde_json::Value>(&path)
        && moved_in.get("transfer").and_then(|t| t.as_str()) == Some(transfer)
        && moved_in.get("key").is_some_and(|k| k != "")
    {
        moved_in["key"] = "".into();
        write_private(&path, &serde_json::to_vec_pretty(&moved_in)?)?;
    }
    if let Some(mut stage) = stage_of(dir)
        && stage.transfer == transfer
        && stage.key != MoveKey::default()
    {
        stage.key = MoveKey::default();
        write_stage(dir, &stage)?;
    }
    Ok(())
}

/// The move could not be committed here: it stays staged, and the old computer, told so,
/// stays "moving" until a code shown here again finishes it.
async fn uncommitted(channel: &mut Channel<impl Link>, e: anyhow::Error) -> Failure {
    tracing::warn!("cannot start putting the account that came in place: {e:#}");
    let _ = channel
        .send(&Msg::Fail {
            reason: PeerReason::CouldNotFinish.text().into(),
        })
        .await;
    channel.linger().await;
    Failure::Unconfirmed
}

/// How putting what came in place went, once the move was committed here.
enum Activated {
    Now,
    /// It goes on at the next start (see [`recover`]).
    Later(anyhow::Error),
}

/// Puts what came in place. It runs to its end once begun, holding the folder's lock itself:
/// a move stopped meanwhile (its task gone) never lets another part of Fogata at the folder
/// half way. An error when the move could not even be committed here (it stays staged);
/// once committed, it is finished now or at the next start.
async fn activate(dir: &Path, keychain: &Keychain, mut record: Incoming) -> Result<Activated> {
    record.state = StageState::Commit;
    let dir = dir.to_path_buf();
    let keychain = keychain.clone();
    let lock = flow().map(|flow| flow.lock.clone());
    blocking(move || {
        let _lock = lock;
        write_stage(&dir, &record)?;
        Ok(match put_in_place(&dir, &record, &keychain) {
            Ok(()) => Activated::Now,
            Err(e) => Activated::Later(e),
        })
    })
    .await
}

/// Removes what a move put in place replaced, holding the folder's lock as [`activate`] does.
async fn tidy(dir: &Path, keychain: &Keychain) {
    let dir = dir.to_path_buf();
    let keychain = keychain.clone();
    let lock = flow().map(|flow| flow.lock.clone());
    let done = blocking(move || {
        let _lock = lock;
        remove_replaced(&dir, &keychain)
    })
    .await;
    if let Err(e) = done {
        tracing::warn!("what the move replaced goes at the next start: {e:#}");
    }
}

/// What arrives for the file being written.
enum Piece {
    File(String, Kind),
    Data(zeroize::Zeroizing<Vec<u8>>),
    End(String),
}

/// Takes every file the old computer sends into `stage`, sealed with a new key of this
/// computer's keychain (or plain without one, when this computer's person chose so), then
/// checks that the account opens there.
async fn take_files<S: futures::AsyncRead + futures::AsyncWrite + Unpin>(
    dir: &Path,
    stage: &Path,
    keychain: &Keychain,
    events: &mpsc::UnboundedSender<TransferEvent>,
    channel: &mut Channel<S>,
    summary: &Summary,
) -> Result<()> {
    let key = match keychain {
        // An account comes in the clear only to a computer whose person chose so.
        Keychain::Seal(keys) => match vault::try_create(stage, keys.as_ref())? {
            Ok(key) => Some(key),
            Err(why) if !vault::may_stay_plain(dir) => bail!(vault::NoKeychain(why)),
            Err(_) => None,
        },
        _ => None,
    };
    let own_port = Store::open(dir.to_path_buf())?.meta().port;
    let (tx, mut rx) = mpsc::channel::<Piece>(16);
    let writer_stage = stage.to_path_buf();
    let writer_key = key.clone();
    let writer = tokio::task::spawn_blocking(move || -> Result<u32> {
        let mut sink: Option<(Sink, blake3::Hasher)> = None;
        let mut count = 0;
        let mut seen = HashSet::new();
        while let Some(piece) = rx.blocking_recv() {
            match piece {
                Piece::File(rel, kind) => {
                    if sink.is_some() || !seen.insert(rel.to_lowercase()) {
                        bail!("{rel} came twice, or before the last file ended");
                    }
                    sink = Some((
                        files::create(&writer_stage, &rel, kind, writer_key.as_ref())?,
                        blake3::Hasher::new(),
                    ));
                }
                Piece::Data(data) => {
                    let (s, hash) = sink.as_mut().context("a piece of no file")?;
                    hash.update(&data);
                    s.write(&data)?;
                }
                Piece::End(digest) => {
                    let (s, hash) = sink.take().context("the end of no file")?;
                    if hash.finalize().to_hex().as_str() != digest {
                        bail!("a file changed on the way");
                    }
                    s.finish(writer_key.as_ref(), |rel, bytes| {
                        adjust(rel, bytes, own_port)
                    })?;
                    count += 1;
                }
            }
        }
        if sink.is_some() {
            bail!("the last file did not end");
        }
        Ok(count)
    });
    let mut done = 0u64;
    let total = summary.bytes.max(1);
    let mut told = Instant::now();
    let mut files_in = 0u32;
    let deadline = Instant::now()
        + FIXED_TIME
        + Duration::from_secs(summary.bytes.min(MAX_BYTES) / SLOWEST_BYTES_PER_SEC);
    // Stopping the move stops this wait; the files being written end first (below).
    let sent = stoppable(async {
        loop {
            if Instant::now() > deadline {
                bail!("the other computer sends too slowly");
            }
            match within(STEP_WAIT, channel.recv()).await? {
                Frame::Msg(Msg::File { path }) => {
                    files_in += 1;
                    if files_in > MAX_FILES {
                        bail!("far more files came than a data folder holds");
                    }
                    let kind = files::check_path(&path)?;
                    tx.send(Piece::File(path, kind))
                        .await
                        .ok()
                        .context("cannot keep the files")?;
                }
                Frame::Data(data) => {
                    // Every piece carries something: empty ones would only keep the wait
                    // going.
                    if data.is_empty() {
                        bail!("an empty piece of a file");
                    }
                    done += data.len() as u64;
                    if done > summary.bytes.saturating_add(SLACK_BYTES) {
                        bail!("far more came than the other computer said would");
                    }
                    tx.send(Piece::Data(data))
                        .await
                        .ok()
                        .context("cannot keep the files")?;
                    if told.elapsed() >= Duration::from_millis(100) {
                        told = Instant::now();
                        let _ = events.send(TransferEvent::Progress {
                            done: done.min(total),
                            total,
                        });
                    }
                }
                Frame::Msg(Msg::FileEnd { blake3 }) => {
                    tx.send(Piece::End(blake3))
                        .await
                        .ok()
                        .context("cannot keep the files")?;
                }
                Frame::Msg(Msg::Sent { files }) => return Ok::<u32, anyhow::Error>(files),
                Frame::Msg(Msg::Fail { reason }) => bail!(wire::Failed(reason)),
                Frame::Msg(_) => bail!("the other computer said something out of place"),
            }
        }
    })
    .await;
    drop(tx);
    let written = writer.await.context("a task stopped")?;
    let sent = sent?;
    let written = written?;
    if written != sent {
        bail!("{sent} files were sent and {written} kept");
    }
    let _ = events.send(TransferEvent::Progress { done: total, total });
    let stage = stage.to_path_buf();
    let device = hex::encode(summary.profile.device.as_bytes());
    let keychain = keychain.clone();
    within(
        CHECK_WAIT,
        blocking(move || check_stage(&stage, &keychain, &device).map(drop)),
    )
    .await
}

/// The disk space a move of `bytes` takes here: the files sealed again (a little larger)
/// and some room left over.
pub(crate) fn room_needed(bytes: u64) -> u64 {
    bytes.saturating_add(bytes / 50).saturating_add(SLACK_BYTES)
}

fn human_bytes(n: u64) -> String {
    match n {
        n if n >= 1 << 30 => format!("{:.1} GB", n as f64 / (1u64 << 30) as f64),
        n if n >= 1 << 20 => format!("{:.0} MB", n as f64 / (1u64 << 20) as f64),
        n => format!("{} KB", n.div_ceil(1024)),
    }
}

/// Settings that belong to the new computer: its own chosen devices are not replaced by the
/// old computer's, and it keeps its own port (a firewall rule may name it).
pub(crate) fn adjust(rel: &str, bytes: &[u8], own_port: Option<u16>) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let object = value.as_object_mut()?;
    match rel {
        "app.json" => {
            for key in COMPUTER_SETTINGS {
                object.remove(key);
            }
        }
        "meta.json" => match own_port {
            Some(port) => {
                object.insert("port".into(), port.into());
            }
            None => {
                object.remove("port");
            }
        },
        _ => return None,
    }
    serde_json::to_vec_pretty(&value).ok()
}

/// The account opens in the folder being filled: its identity is the one offered, its
/// profile reads, is its own and signed by it, and every group's log reads. Gives the
/// profile.
pub(crate) fn check_stage(stage: &Path, keychain: &Keychain, device: &str) -> Result<Profile> {
    if !stage.join("identity.key").exists() || !stage.join("friends.bin").exists() {
        bail!("the identity or the profile did not come");
    }
    // Opened only: nothing here may seal again or take a key.
    let opening = match keychain {
        Keychain::Seal(keys) | Keychain::Open(keys) => Keychain::Open(keys.clone()),
        Keychain::Off => Keychain::Off,
    };
    let store = Store::open_with(stage.to_path_buf(), opening)?;
    let (identity, _) = store.identity()?;
    if hex::encode(identity.public_key().as_bytes()) != device {
        bail!("the identity that came is not the account's");
    }
    let book = store.friends()?.context("the profile did not come")?;
    if book.profile.device != identity.public_key() || book.profile.verify().is_err() {
        bail!("the profile that came is not the account's");
    }
    for group in store.groups()? {
        if !group.pending {
            store
                .load_log(&group)
                .with_context(|| format!("the group {} did not come whole", group.name))?;
        }
    }
    drop(store);
    let _ = fs::remove_file(stage.join("running.lock"));
    Ok(book.profile)
}

// ----- The computer with the account -----

/// Moves the account of the folder `dir` to the computer showing `code`. No core may run on
/// the folder: its node stops first, so that nothing changes while it is read.
pub fn send(dir: PathBuf, code: &str, options: TransferOptions) -> Result<Transfer> {
    let code = TransferCode::parse(code).map_err(anyhow::Error::msg)?;
    let keychain = options.keychain.clone().unwrap_or_else(vault::chosen);
    let store = Store::open_with(dir.clone(), keychain.clone())?;
    let running = Arc::new(store.hold_running()?);
    recover_held(&dir, &keychain)?;
    let resume = match moved(&dir) {
        Some(m) if m.state == MovedState::Moving && dir.join("identity.key").exists() => Some(m),
        Some(_) => bail!("this account already moved to another computer"),
        None => None,
    };
    if resume.is_none() && !store.has_profile()? {
        bail!("there is no account on this computer to move");
    }
    let (events, rx) = mpsc::unbounded_channel();
    let (control, mut control_rx) = mpsc::unbounded_channel();
    let stage = Arc::new(Stage::default());
    let flow = Arc::new(Flow {
        stage: stage.clone(),
        lock: running,
    });
    let task = tokio::spawn(async move {
        let end = FLOW
            .scope(
                flow.clone(),
                sending(
                    &dir,
                    &code,
                    &options,
                    &keychain,
                    resume,
                    &events,
                    &mut control_rx,
                ),
            )
            .await;
        let end = match end {
            Ok(end) => end,
            Err(e) => TransferEvent::Failed(failure_or_other(e)),
        };
        // The folder is free once the end is told: another move or the core may start on it
        // at once. The scope's own hold went with it.
        drop(flow);
        let _ = events.send(end);
    });
    Ok(Transfer {
        events: rx,
        control,
        task,
        stage,
    })
}

async fn sending(
    dir: &Path,
    code: &TransferCode,
    options: &TransferOptions,
    keychain: &Keychain,
    resume: Option<Moved>,
    events: &mpsc::UnboundedSender<TransferEvent>,
    control: &mut mpsc::UnboundedReceiver<bool>,
) -> Result<TransferEvent> {
    let store = Store::open_with(dir.to_path_buf(), keychain.clone())?;
    let key = store.data_key()?;
    let (items, summary) = {
        let dir = dir.to_path_buf();
        let keychain = keychain.clone();
        let key = key.clone();
        blocking(move || {
            let store = Store::open_with(dir.clone(), keychain)?;
            let items = files::leave_out_unreadable(&dir, files::list(&dir)?, key.as_ref());
            let summary = summarize(&store, &items)?;
            Ok((items, summary))
        })
        .await?
    };
    let node = start_node(options, store.meta().port)?;
    node.seek(Some(code.rendezvous()), Some(*code.lan_secret()))
        .await?;
    let _ = events.send(TransferEvent::Looking);
    let began = Instant::now();
    let deadline = began + Duration::from_millis(options.no_contact_ms);
    let local = node.local_peer();
    let proofs = Proofs::default();
    let mut search = Search::default();
    // The runs under way, each in a task of its own, so that a computer that stalls keeps
    // neither the others nor the person's Cancel waiting.
    let mut runs: tokio::task::JoinSet<(PeerId, Tried<fogata_net::Stream>)> =
        tokio::task::JoinSet::new();
    let mut look = tokio::time::interval(LOOK_EVERY);
    look.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let end = loop {
        let (peer, channel) = tokio::select! {
            () = tokio::time::sleep_until(deadline.into()) => {
                break TransferEvent::Failed(search.verdict().unwrap_or(Failure::NoContact));
            }
            answer = control.recv() => {
                if answer != Some(true) {
                    break TransferEvent::Failed(Failure::Cancelled);
                }
                continue;
            }
            Some(ended) = runs.join_next() => {
                let Ok((peer, tried)) = ended else {
                    continue;
                };
                match search.heard(peer, tried) {
                    Some(channel) => (peer, channel),
                    None => {
                        if let Some(failure) = search.over(proofs.spent(), began.elapsed() >= SETTLE) {
                            break TransferEvent::Failed(failure);
                        }
                        continue;
                    }
                }
            }
            _ = look.tick() => {
                let candidates = node.move_candidates().await.unwrap_or_default();
                let taking = node.peers_taking(&PROTOCOL);
                // Those that take only the first way: told apart, never run with.
                let old_only: Vec<PeerId> = node
                    .peers_taking(&OLD_PROTOCOL)
                    .into_iter()
                    .filter(|p| !taking.contains(p))
                    .collect();
                let listing = taking.iter().chain(&old_only).copied().collect();
                let ready = to_try(listing, candidates, |p| *p != local && search.may_try(p));
                let mut found = Vec::with_capacity(ready.len());
                for peer in ready {
                    found.push((peer, is_lan(&node, peer).await));
                }
                let chosen = choose(&found, search.near_running(), &mut search.first_far);
                let room = MAX_RUNS.saturating_sub(runs.len());
                search.waiting = chosen.len() > room || chosen.len() < found.len();
                for peer in chosen.into_iter().take(room) {
                    let lan = found.iter().any(|(p, lan)| *p == peer && *lan);
                    search.started(peer, lan);
                    let (node, code, proofs) = (node.clone(), code.clone(), proofs.clone());
                    let old_only = old_only.contains(&peer);
                    runs.spawn(async move {
                        let tried = try_peer(&node, &code, peer, old_only, &proofs).await;
                        (peer, tried)
                    });
                }
                if let Some(failure) = search.over(proofs.spent(), began.elapsed() >= SETTLE) {
                    break TransferEvent::Failed(failure);
                }
                continue;
            }
        };
        // The code checked out with this one: the others stop, whatever they were at.
        runs.abort_all();
        if let Err(e) = enter(Phase::Running) {
            break TransferEvent::Failed(failure_of(e));
        }
        let lan = is_lan(&node, peer).await;
        let result = match &resume {
            Some(moved) => {
                finish_resumed(dir, channel, moved, &summary, events, control, lan).await
            }
            None => {
                sent(
                    dir, options, &key, &items, &summary, events, control, channel, lan,
                )
                .await
            }
        };
        break match result {
            Ok(done) => done,
            Err(e) => TransferEvent::Failed(failure_of(e)),
        };
    };
    runs.abort_all();
    let _ = node.seek(None, None).await;
    let _ = node.shutdown().await;
    Ok(end)
}

/// How a try of the code typed here on one computer ended ([`try_peer`]).
enum Tried<S> {
    Ran(Ran<S>),
    /// The computer takes only the first way of running the code: whether it shows this
    /// code ([`shows_code_the_old_way`]).
    OldWay(Option<bool>),
}

/// Tries the code typed here on `peer`: runs it, or, when the computer does not take this
/// way of running it (`old_only`, or it refuses the stream), tells whether it shows the code
/// the first way.
async fn try_peer(
    node: &NodeHandle,
    code: &TransferCode,
    peer: PeerId,
    old_only: bool,
    proofs: &Proofs,
) -> Tried<fogata_net::Stream> {
    let stream = match old_only {
        true => None,
        false => node.streams().open_stream(peer, PROTOCOL).await.ok(),
    };
    match stream {
        Some(stream) => {
            let (local, remote) = (node.local_peer().to_bytes(), peer.to_bytes());
            Tried::Ran(run_code(code, &local, &remote, stream, proofs).await)
        }
        None => Tried::OldWay(shows_code_the_old_way(node, peer, code).await),
    }
}

/// How a computer that answered under the code typed here ended its try without proving it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Answered {
    Wrong,
    Lost,
    /// It shows the code the first way, or said so: it can be a stranger dropping the probe.
    OldWay,
}

/// What the computer where the code was typed knows of the computers it tries it on.
///
/// Nothing tells the computer showing the code apart from one that learnt its nameplate
/// (the rendezvous point, the announcement sealed under the nameplate, a connection that
/// lists the move's protocol: any of them is had for the nameplate), and only the run's
/// proofs tell whether a computer knows the rest. So every computer found is tried, side by
/// side; one that answered and did not prove the code back is not tried again, and is no
/// reason to stop while another may still be the one, until [`MAX_PROVED_TO`] proofs went.
#[derive(Debug, Default)]
struct Search {
    /// Computers that show another code: not tried again.
    other_code: HashSet<PeerId>,
    /// When each computer was last tried: one that went, or did not answer in time, is
    /// tried again a little later.
    tried: std::collections::HashMap<PeerId, Instant>,
    /// Computers being tried, and whether each is on this local network.
    running: std::collections::HashMap<PeerId, bool>,
    /// Computers that answered under the code and did not prove it back: not tried again.
    answered: std::collections::HashMap<PeerId, Answered>,
    /// When a computer found only over the DHT first showed (see [`choose`]).
    first_far: Option<Instant>,
    /// Whether computers found at the last look are still to be tried.
    waiting: bool,
}

impl Search {
    /// Whether `peer` may be tried now.
    fn may_try(&self, peer: &PeerId) -> bool {
        !self.other_code.contains(peer)
            && !self.answered.contains_key(peer)
            && !self.running.contains_key(peer)
            && self
                .tried
                .get(peer)
                .is_none_or(|t| t.elapsed() >= RETRY_PEER)
    }

    /// A try of `peer` begins.
    fn started(&mut self, peer: PeerId, lan: bool) {
        self.tried.insert(peer, Instant::now());
        self.running.insert(peer, lan);
    }

    /// Whether a computer on this local network is being tried.
    fn near_running(&self) -> bool {
        self.running.values().any(|lan| *lan)
    }

    /// The try of `peer` ended: the channel when the code checked out with it.
    fn heard<S>(&mut self, peer: PeerId, tried: Tried<S>) -> Option<Channel<S>> {
        self.running.remove(&peer);
        let answered = match tried {
            Tried::Ran(Ran::Both(channel)) => return Some(channel),
            Tried::Ran(Ran::Untried { other_code: true }) | Tried::OldWay(Some(false)) => {
                self.other_code.insert(peer);
                return None;
            }
            Tried::Ran(Ran::Untried { other_code: false }) | Tried::OldWay(None) => return None,
            Tried::Ran(Ran::Wrong) => Answered::Wrong,
            Tried::Ran(Ran::Lost) => Answered::Lost,
            Tried::OldWay(Some(true)) => Answered::OldWay,
        };
        self.answered.insert(peer, answered);
        None
    }

    /// How the move stops, if it does now: when a computer answered and did not prove the
    /// code, nothing is being tried, and either no proof is left (`spent`) or nothing more
    /// is to be tried once the search `settled` ([`SETTLE`]).
    fn over(&self, spent: bool, settled: bool) -> Option<Failure> {
        if !self.running.is_empty() || !(spent || (settled && !self.waiting)) {
            return None;
        }
        self.verdict()
    }

    /// Why no computer took the move, of those that answered: the person sees whether one
    /// said no (a code mistyped, or a stranger) or several did (a stranger at least).
    fn verdict(&self) -> Option<Failure> {
        if self.answered.len() > 1 {
            return Some(Failure::OthersAnswered);
        }
        Some(match self.answered.values().next()? {
            Answered::Wrong => Failure::WrongCode,
            Answered::Lost => Failure::Interrupted(CONNECTION_LOST.into()),
            Answered::OldWay => Failure::OldVersion,
        })
    }
}

/// The computers the code may be tried on now, of those `keep` takes: those that list the
/// move's protocol to this computer (found it, or on its local network) first; only when
/// none does, those this computer found under the code (`candidates`), since the other
/// computer lists it only to computers it found itself and may not have found this one yet:
/// finding it is enough. Trying one does not spend one of the code's tries unless it
/// answers the move's protocol and runs the exchange: one that does not take the protocol
/// refuses the stream before anything is said, and one that shows another code is not
/// counted. The candidates come last so that a stranger that announced itself under the
/// code is not tried before the computer that showed it takes the move; one being tried,
/// or that answered already, is no longer among those listing, so the candidates come then.
fn to_try(
    listing: Vec<PeerId>,
    candidates: Vec<PeerId>,
    keep: impl Fn(&PeerId) -> bool,
) -> Vec<PeerId> {
    let first: Vec<PeerId> = listing.iter().copied().filter(&keep).collect();
    if !first.is_empty() {
        return first;
    }
    candidates
        .into_iter()
        .filter(|p| keep(p) && !listing.contains(p))
        .collect()
}

/// The computers to try the code on now, among those found (and whether each is on this
/// local network): those on the local network first; those found over the DHT only when
/// none is here, found or being tried (`near_running`), and [`LAN_FIRST`] after such a one
/// first showed, so that one on the local network found a little later still comes first.
/// `first_far` keeps when that was.
fn choose(
    found: &[(PeerId, bool)],
    near_running: bool,
    first_far: &mut Option<Instant>,
) -> Vec<PeerId> {
    let near: Vec<PeerId> = found
        .iter()
        .filter(|(_, lan)| *lan)
        .map(|(peer, _)| *peer)
        .collect();
    if !near.is_empty() || near_running || found.is_empty() {
        return near;
    }
    let since = *first_far.get_or_insert_with(Instant::now);
    match since.elapsed() >= LAN_FIRST {
        true => found.iter().map(|(peer, _)| *peer).collect(),
        false => Vec::new(),
    }
}

/// What moves, counted for the person's yes.
fn summarize(store: &Store, items: &[Item]) -> Result<Summary> {
    let book = store.require_friends()?;
    let groups = store.groups()?.iter().filter(|g| !g.pending).count() as u32;
    let chats: HashSet<&str> = items
        .iter()
        .filter_map(|i| {
            i.rel
                .strip_prefix("direct/")?
                .split_once('/')
                .map(|(c, _)| c)
        })
        .filter(|c| c.len() == 64)
        .collect();
    let files = items
        .iter()
        .filter(|i| i.rel.starts_with("direct/") && i.rel.contains("/files/"))
        .count() as u32;
    Ok(Summary {
        profile: book.profile.clone(),
        friends: book.friends.len() as u32,
        groups,
        chats: chats.len() as u32,
        files,
        bytes: items.iter().map(|i| i.len).sum(),
    })
}

#[allow(clippy::too_many_arguments)]
async fn sent(
    dir: &Path,
    options: &TransferOptions,
    key: &Option<DataKey>,
    items: &[Item],
    summary: &Summary,
    events: &mpsc::UnboundedSender<TransferEvent>,
    control: &mut mpsc::UnboundedReceiver<bool>,
    mut channel: Channel<fogata_net::Stream>,
    lan: bool,
) -> Result<TransferEvent> {
    let (computer, replaces) = match within(STEP_WAIT, channel.recv_msg()).await? {
        Msg::Hello { computer, replaces } => (shown(&computer), replaces.map(|r| shown(&r))),
        _ => bail!("the other computer said something out of place"),
    };
    let mut transfer = [0u8; 16];
    fogata_crypto::random_bytes(&mut transfer)?;
    let transfer = hex::encode(transfer);
    channel
        .send(&Msg::Offer {
            transfer: transfer.clone(),
            computer: options.computer.clone(),
            device: hex::encode(summary.profile.device.as_bytes()),
            profile: base64::engine::general_purpose::STANDARD.encode(summary.profile.encode()),
            friends: summary.friends,
            groups: summary.groups,
            chats: summary.chats,
            files: summary.files,
            bytes: summary.bytes,
        })
        .await?;
    let _ = events.send(TransferEvent::Matched {
        computer: computer.clone(),
        summary: summary.clone(),
        lan,
        replaces,
    });
    let yes = tokio::time::timeout(CONFIRM_WAIT, control.recv())
        .await
        .ok()
        .flatten()
        .unwrap_or(false);
    if !yes {
        let _ = channel
            .send(&Msg::Fail {
                reason: PeerReason::Cancelled.text().into(),
            })
            .await;
        channel.linger().await;
        return Ok(TransferEvent::Failed(Failure::Cancelled));
    }
    channel.send(&Msg::Go).await?;
    // Nothing goes before the new computer's person said yes as well.
    let go = stoppable(within(CONFIRM_WAIT + STEP_WAIT, channel.recv_msg())).await;
    if go.as_ref().is_err_and(cancelled) {
        tell_cancelled(&mut channel).await;
    }
    match go? {
        Msg::Go => {}
        _ => bail!("the other computer said something out of place"),
    }
    if let Err(e) = stoppable(send_files(
        dir,
        key,
        items,
        summary.bytes,
        events,
        &mut channel,
    ))
    .await
    {
        let _ = channel
            .send(&Msg::Fail {
                reason: PeerReason::Stopped.text().into(),
            })
            .await;
        channel.linger().await;
        return Err(e);
    }
    let _ = events.send(TransferEvent::Finishing);
    match stoppable(within(CHECK_WAIT, channel.recv_msg())).await? {
        Msg::Staged => {}
        _ => bail!("the other computer said something out of place"),
    }
    // From here on this computer never runs the account again by itself, and the move is
    // not stopped any more.
    enter(Phase::Ending)?;
    let mut moved = Moved {
        state: MovedState::Moving,
        transfer,
        to: computer.clone(),
        name: summary.profile.name.clone(),
        at_ms: now_ms(),
        key: MoveKey::from(&channel),
    };
    write_private(&dir.join(MOVED_FILE), &serde_json::to_vec_pretty(&moved)?)?;
    if options.lose_commit {
        channel.close().await;
        return Ok(TransferEvent::Failed(Failure::Unconfirmed));
    }
    let active = async {
        channel.send(&Msg::Commit).await?;
        within(ACTIVE_WAIT, channel.recv_msg()).await
    }
    .await;
    // Only the computer the account went to can say it took it: anything else leaves this
    // one "moving", with the account kept.
    match active {
        Ok(Msg::Active { proof }) if channel.proves(&moved.transfer, &moved.key.0, &proof) => {}
        _ => return Ok(TransferEvent::Failed(Failure::Unconfirmed)),
    }
    moved.state = MovedState::Moved;
    mark_moved(dir, &moved)?;
    if !options.lose_moved {
        let _ = channel.send(&Msg::Moved).await;
    }
    deactivate(dir)?;
    // The new computer forgets the move's secret only once that word came: the connection
    // stays until it closes its end, or the word could go with it on a busy computer.
    channel.linger().await;
    Ok(TransferEvent::Done {
        computer,
        summary: Some(summary.clone()),
    })
}

/// A move whose end was not confirmed: the new computer is asked to finish it.
#[allow(clippy::too_many_arguments)]
async fn finish_resumed(
    dir: &Path,
    mut channel: Channel<fogata_net::Stream>,
    moved: &Moved,
    summary: &Summary,
    events: &mpsc::UnboundedSender<TransferEvent>,
    control: &mut mpsc::UnboundedReceiver<bool>,
    lan: bool,
) -> Result<TransferEvent> {
    let computer = match within(STEP_WAIT, channel.recv_msg()).await? {
        Msg::Hello { computer, .. } => shown(&computer),
        _ => bail!("the other computer said something out of place"),
    };
    // As on a first move, the person sees which computer this is and says yes: the account
    // goes for good once it answers.
    let _ = events.send(TransferEvent::Matched {
        computer: computer.clone(),
        summary: summary.clone(),
        lan,
        replaces: None,
    });
    let yes = tokio::time::timeout(CONFIRM_WAIT, control.recv())
        .await
        .ok()
        .flatten()
        .unwrap_or(false);
    if !yes {
        let _ = channel
            .send(&Msg::Fail {
                reason: PeerReason::Cancelled.text().into(),
            })
            .await;
        channel.linger().await;
        return Ok(TransferEvent::Failed(Failure::Cancelled));
    }
    channel
        .send(&Msg::Resume {
            transfer: moved.transfer.clone(),
        })
        .await?;
    match within(CHECK_WAIT + ACTIVE_WAIT, channel.recv_msg()).await? {
        Msg::Active { proof } if channel.proves(&moved.transfer, &moved.key.0, &proof) => {
            let mut moved = moved.clone();
            moved.state = MovedState::Moved;
            mark_moved(dir, &moved)?;
            let _ = channel.send(&Msg::Moved).await;
            deactivate(dir)?;
            // As above: the word that the move is over reaches the new computer first.
            channel.linger().await;
            Ok(TransferEvent::Done {
                computer: moved.to,
                summary: None,
            })
        }
        Msg::Unknown => Ok(TransferEvent::Failed(Failure::Declined(
            PeerReason::NothingToFinish.text().into(),
        ))),
        Msg::Active { .. } => Ok(TransferEvent::Failed(Failure::Declined(
            PeerReason::NotProven.text().into(),
        ))),
        _ => bail!("the other computer said something out of place"),
    }
}

/// Marks the account of `dir` as gone for good to the computer that proved it took it, and
/// forgets the move's secret, which nothing asks for any more.
fn mark_moved(dir: &Path, moved: &Moved) -> Result<()> {
    let moved = Moved {
        key: MoveKey::default(),
        ..moved.clone()
    };
    write_private(&dir.join(MOVED_FILE), &serde_json::to_vec_pretty(&moved)?)
}

/// The new computer uses the account now: this one keeps nothing that makes it that person
/// (the identity, the friend secrets, the MLS state), only what the person may delete later.
/// Each of those files is written over before it goes ([`wipe`]).
fn deactivate(dir: &Path) -> Result<()> {
    if vault::key_name(dir).is_none() {
        tracing::warn!(
            "the data in {} was not encrypted: what this computer kept of the account may \
             still be read from the disk after it is deleted",
            dir.display()
        );
    }
    for entry in fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // The identity, the friend secrets, the recovery key that proves copies, and the
        // tickets of invites still waiting for a manager (they name this device's identity).
        if name == "identity.key"
            || name.starts_with("friends.bin")
            || name.starts_with(crate::recovery::KEY_FILE)
            || name == "joins.json"
        {
            wipe(&entry.path())?;
        }
    }
    // The plain files refused by sealing are secrets of the account too, or were taken
    // for some, in the clear.
    for sub in ["mls", crate::store::BACKUP_DIR, crate::store::REFUSED_DIR] {
        let path = dir.join(sub);
        if fs::symlink_metadata(&path).is_ok() {
            wipe(&path)?;
        }
    }
    Ok(())
}

/// Removes the file or folder at `path`, writing every file in it over with zeros first, so
/// that a plain identity (in a folder without a key, or the copies made before sealing) is
/// not left whole in the disk's free space while the account lives on another computer. Only
/// as far as the disk lets: one that writes elsewhere (a copy-on-write file system, a flash
/// drive's own mapping) may still keep the old blocks. A link is removed, never followed.
pub(crate) fn wipe(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        for entry in fs::read_dir(path)?.flatten() {
            wipe(&entry.path())?;
        }
        return fs::remove_dir(path).with_context(|| format!("cannot remove {}", path.display()));
    }
    if meta.is_file() {
        let written = fs::OpenOptions::new()
            .write(true)
            .open(path)
            .and_then(|mut file| {
                use std::io::Write as _;
                let zeros = [0u8; 4096];
                let mut left = meta.len();
                while left > 0 {
                    let n = left.min(zeros.len() as u64) as usize;
                    file.write_all(&zeros[..n])?;
                    left -= n as u64;
                }
                file.sync_all()
            });
        if let Err(e) = written {
            tracing::debug!("cannot write over {}: {e}", path.display());
        }
    }
    fs::remove_file(path).with_context(|| format!("cannot remove {}", path.display()))
}

/// What the reading thread hands over.
enum Out {
    File(String),
    Data(zeroize::Zeroizing<Vec<u8>>),
    End(String),
}

async fn send_files(
    dir: &Path,
    key: &Option<DataKey>,
    items: &[Item],
    total: u64,
    events: &mpsc::UnboundedSender<TransferEvent>,
    channel: &mut Channel<fogata_net::Stream>,
) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<Result<Out>>(16);
    let (dir, key, list) = (dir.to_path_buf(), key.clone(), items.to_vec());
    let reader = tokio::task::spawn_blocking(move || {
        for item in &list {
            let result = read_one(&dir, item, key.as_ref(), &tx);
            if let Err(e) = result {
                let _ = tx.blocking_send(Err(e));
                return;
            }
        }
    });
    let mut done = 0u64;
    let mut count = 0u32;
    let mut told = Instant::now();
    let total = total.max(1);
    while let Some(out) = rx.recv().await {
        match out? {
            Out::File(path) => channel.send(&Msg::File { path }).await?,
            Out::Data(data) => {
                done += data.len() as u64;
                channel.send_data(&data).await?;
                if told.elapsed() >= Duration::from_millis(100) {
                    told = Instant::now();
                    let _ = events.send(TransferEvent::Progress {
                        done: done.min(total),
                        total,
                    });
                }
            }
            Out::End(blake3) => {
                count += 1;
                channel.send(&Msg::FileEnd { blake3 }).await?;
            }
        }
    }
    reader.await.context("a task stopped")?;
    if count as usize != items.len() {
        bail!("not every file was read");
    }
    channel.send(&Msg::Sent { files: count }).await?;
    let _ = events.send(TransferEvent::Progress { done: total, total });
    Ok(())
}

fn read_one(
    dir: &Path,
    item: &Item,
    key: Option<&DataKey>,
    tx: &mpsc::Sender<Result<Out>>,
) -> Result<()> {
    let send = |out| tx.blocking_send(Ok(out)).ok().context("the move stopped");
    send(Out::File(item.rel.clone()))?;
    let mut hash = blake3::Hasher::new();
    match files::open(dir, item, key)? {
        Source::Whole(plain) => {
            for chunk in plain.chunks(CHUNK) {
                hash.update(chunk);
                send(Out::Data(zeroize::Zeroizing::new(chunk.to_vec())))?;
            }
        }
        Source::Reader(mut reader) => loop {
            let mut buf = zeroize::Zeroizing::new(vec![0u8; CHUNK]);
            let mut got = 0;
            while got < CHUNK {
                match reader.read(&mut buf[got..])? {
                    0 => break,
                    n => got += n,
                }
            }
            if got == 0 {
                break;
            }
            buf.truncate(got);
            hash.update(&buf);
            send(Out::Data(buf))?;
            if got < CHUNK {
                break;
            }
        },
    }
    send(Out::End(hash.finalize().to_hex().to_string()))
}

use std::io::Read as _;

#[cfg(test)]
mod tests {
    use super::*;

    /// One end of a connection in memory, as a move's stream is to the run.
    struct End(tokio::io::DuplexStream);

    impl futures::AsyncRead for End {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut [u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            let mut read = tokio::io::ReadBuf::new(buf);
            tokio::io::AsyncRead::poll_read(std::pin::Pin::new(&mut self.0), cx, &mut read)
                .map_ok(|()| read.filled().len())
        }
    }

    impl futures::AsyncWrite for End {
        fn poll_write(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            tokio::io::AsyncWrite::poll_write(std::pin::Pin::new(&mut self.0), cx, buf)
        }

        fn poll_flush(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            tokio::io::AsyncWrite::poll_flush(std::pin::Pin::new(&mut self.0), cx)
        }

        fn poll_close(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            tokio::io::AsyncWrite::poll_shutdown(std::pin::Pin::new(&mut self.0), cx)
        }
    }

    /// The two ends of a connection in memory.
    fn ends() -> (End, End) {
        let (a, b) = tokio::io::duplex(1 << 16);
        (End(a), End(b))
    }

    /// A code with the nameplate of `code` (where the two meet) and another rest.
    fn same_place(code: &TransferCode) -> TransferCode {
        let text: Vec<char> = code.to_string().chars().filter(|c| *c != '-').collect();
        let digits = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        for other in digits.chars().filter(|c| *c != text[6]) {
            for check in digits.chars() {
                let mut guess = text.clone();
                guess[6] = other;
                guess[9] = check;
                if let Ok(guess) = TransferCode::parse(&guess.iter().collect::<String>()) {
                    return guess;
                }
            }
        }
        unreachable!("some check character fits")
    }

    /// The computer where the code is typed proves it first. The right code ends with the
    /// two channels; a guess of the rest is one try, which the computer showing the code
    /// counts and the one typing it hears as a no; a stranger that learnt the nameplate and
    /// goes once it has the answer to its start learnt nothing, and costs the code nothing.
    #[tokio::test]
    async fn the_computer_typing_the_code_proves_it_first_and_only_a_wrong_proof_is_a_try() {
        let code = TransferCode::generate().unwrap();
        let proofs = Proofs::default();
        let (old, new) = ends();
        let (ran, answered) = tokio::join!(
            run_code(&code, b"old", b"new", old, &proofs),
            answer_run(&code, b"new", b"old", new)
        );
        let Ran::Both(mut sender) = ran else {
            panic!("the right code runs")
        };
        let mut receiver = answered.unwrap().expect("the right code runs");
        sender.send(&Msg::Go).await.unwrap();
        assert_eq!(receiver.recv_msg().await.unwrap(), Msg::Go);

        let guess = same_place(&code);
        let (old, new) = ends();
        let (ran, answered) = tokio::join!(
            run_code(&guess, b"old", b"new", old, &proofs),
            answer_run(&code, b"new", b"old", new)
        );
        assert!(matches!(ran, Ran::Wrong));
        assert_eq!(answered.err(), Some(RunError::WrongCode));

        // The stranger: a start with the nameplate, then gone with the answer.
        let (mut stranger, new) = ends();
        let shown = code.clone();
        let answering =
            tokio::spawn(async move { answer_run(&shown, b"new", b"x", new).await.err() });
        let (_, start) = run_start(&guess, b"x", b"new");
        wire::send_start(&mut stranger, &start).await.unwrap();
        let reply = wire::read_reply(&mut stranger).await.unwrap().unwrap();
        // Its SPAKE2 message, and nothing made from the run's key to test a guess against.
        assert_eq!(reply.spake.len(), 33);
        drop(stranger);
        assert_eq!(answering.await.unwrap(), Some(RunError::Malformed));

        // Another code altogether: told so, no try either.
        let other = loop {
            let other = TransferCode::generate().unwrap();
            if other.rendezvous() != code.rendezvous() {
                break other;
            }
        };
        let (old, new) = ends();
        let (ran, answered) = tokio::join!(
            run_code(&other, b"old", b"new", old, &proofs),
            answer_run(&code, b"new", b"old", new)
        );
        assert!(matches!(ran, Ran::Untried { other_code: true }));
        assert!(matches!(answered, Ok(None)));
    }

    /// A stranger that learnt the nameplate and stalls keeps no other run waiting: the run
    /// of the computer typing the code, started after it, ends while the stranger still
    /// waits, and the stranger's run ends without a try of the code. A computer has one run
    /// at a time, its newest stream in place of the one before, and past [`MAX_RUNS`] the
    /// oldest stops.
    #[tokio::test]
    async fn a_stranger_that_stalls_keeps_no_other_run_waiting() {
        type Started = Result<Option<(ReceiverRun, Vec<u8>, End)>, RunError>;
        let code = TransferCode::generate().unwrap();
        let answering = |stream: End| {
            let code = code.clone();
            async move { answer_start(&code, b"new", b"old", stream).await }
        };
        let mut runs: Runs<Started> = Runs::default();

        // The stranger: a start with the nameplate, then nothing once it has the answer.
        let (mut stranger, new) = ends();
        runs.start(PeerId::random(), answering(new));
        let (_, start) = run_start(&same_place(&code), b"old", b"new");
        wire::send_start(&mut stranger, &start).await.unwrap();
        wire::read_reply(&mut stranger).await.unwrap().unwrap();
        // One that says nothing at all.
        let (silent, new) = ends();
        runs.start(PeerId::random(), answering(new));

        let (old, new) = ends();
        let sender = {
            let code = code.clone();
            tokio::spawn(async move {
                matches!(
                    run_code(&code, b"old", b"new", old, &Proofs::default()).await,
                    Ran::Both(_)
                )
            })
        };
        runs.start(PeerId::random(), answering(new));
        let Some(Ok(Some((run, proof, stream)))) = runs.next().await else {
            panic!("the run of the computer typing the code ends first")
        };
        assert!(answer_proof(run, &proof, stream).await.is_ok());
        assert!(sender.await.unwrap());
        // Then those that stalled, once they go (or their waits end), neither a try.
        drop((stranger, silent));
        for _ in 0..2 {
            assert!(matches!(runs.next().await, Some(Err(RunError::Malformed))));
        }
        assert!(runs.next().await.is_none());

        // A second stream of a computer in place of its first; the oldest past the most.
        let mut quiet = Vec::new();
        let twice = PeerId::random();
        for peer in [twice, twice] {
            let (other, new) = ends();
            quiet.push(other);
            runs.start(peer, answering(new));
        }
        assert_eq!(runs.of.len(), 1);
        for _ in 0..MAX_RUNS {
            let (other, new) = ends();
            quiet.push(other);
            runs.start(PeerId::random(), answering(new));
        }
        assert_eq!(runs.of.len(), MAX_RUNS);
        assert!(runs.of.iter().all(|(peer, _)| *peer != twice));
        drop(quiet);
        let mut ended = 0;
        while runs.next().await.is_some() {
            ended += 1;
        }
        assert_eq!(ended, MAX_RUNS);
    }

    /// A stranger that learnt the nameplate and answers the computer typing the code first,
    /// with a SPAKE2 message of its own guess, gets that computer's proof (one guess, which
    /// missed) and is not tried again; but the computer showing the code, found after it, is
    /// still tried and the move goes on with it. One no alone stops the search only once it
    /// settled with nothing else to try (a code mistyped).
    #[tokio::test]
    async fn a_stranger_answering_first_does_not_keep_the_code_from_the_computer_showing_it() {
        let code = TransferCode::generate().unwrap();
        let proofs = Proofs::default();
        let mut search = Search::default();
        let (stranger, real) = (PeerId::random(), PeerId::random());

        search.started(stranger, true);
        assert!(!search.may_try(&stranger));
        let (old, theirs) = ends();
        let guess = same_place(&code);
        let (ran, answered) = tokio::join!(
            run_code(&code, b"old", b"x", old, &proofs),
            answer_run(&guess, b"x", b"old", theirs)
        );
        // Its guess missed, and that is all it learnt.
        assert_eq!(answered.err(), Some(RunError::WrongCode));
        assert!(search.heard(stranger, Tried::Ran(ran)).is_none());
        assert_eq!(search.answered.get(&stranger), Some(&Answered::Wrong));
        assert!(!search.may_try(&stranger), "a computer gets one guess");
        // Not the end of the move: the search has not settled, and proofs are left.
        assert!(!proofs.spent());
        assert_eq!(search.over(proofs.spent(), false), None);

        // The computer showing the code shows up.
        assert!(search.may_try(&real));
        search.started(real, true);
        let (old, new) = ends();
        let (ran, answered) = tokio::join!(
            run_code(&code, b"old", b"new", old, &proofs),
            answer_run(&code, b"new", b"old", new)
        );
        let mut receiver = answered.unwrap().expect("the right code runs");
        let mut sender = search
            .heard(real, Tried::Ran(ran))
            .expect("the move goes on with the computer showing the code");
        sender.send(&Msg::Go).await.unwrap();
        assert_eq!(receiver.recv_msg().await.unwrap(), Msg::Go);

        // Had nothing else shown up once the search settled: the code did not match.
        let mut alone = Search::default();
        alone.started(stranger, true);
        assert!(
            alone
                .heard::<End>(stranger, Tried::Ran(Ran::Wrong))
                .is_none()
        );
        assert_eq!(alone.over(false, false), None);
        alone.waiting = true;
        assert_eq!(
            alone.over(false, true),
            None,
            "one found is still to be tried"
        );
        alone.waiting = false;
        assert_eq!(alone.over(false, true), Some(Failure::WrongCode));
        // Nor while another is being tried.
        alone.started(real, false);
        assert_eq!(alone.over(true, true), None);
    }

    /// However many computers answer under the code's nameplate, the computer typing it
    /// proves it to [`MAX_PROVED_TO`] of them, side by side or one after the other: each
    /// one a guess of the code, and no more. The others' runs stop before the proof, so
    /// they learn nothing, and the computer showing the code, coming last, is not tried
    /// either (its count of wrong proofs unspent). Several answered: the person is told so.
    #[tokio::test]
    async fn a_stranger_gets_at_most_three_guesses_of_a_code_typed() {
        let code = TransferCode::generate().unwrap();
        let guess = same_place(&code);
        let proofs = Proofs::default();
        let mut search = Search::default();
        let strangers: Vec<PeerId> = (0..5).map(|_| PeerId::random()).collect();
        let mut runs = Vec::new();
        for peer in &strangers {
            search.started(*peer, true);
            let (old, theirs) = ends();
            let (code, guess, proofs) = (code.clone(), guess.clone(), proofs.clone());
            runs.push(tokio::spawn(async move {
                let (ran, answered) = tokio::join!(
                    run_code(&code, b"old", b"x", old, &proofs),
                    answer_run(&guess, b"x", b"old", theirs)
                );
                (ran, answered.err())
            }));
        }
        let mut guesses = 0;
        for (peer, run) in strangers.iter().zip(runs) {
            let (ran, theirs) = run.await.unwrap();
            match theirs {
                // It got this computer's proof: a guess (which missed).
                Some(RunError::WrongCode) => guesses += 1,
                // Its run stopped before any proof: nothing learnt.
                Some(RunError::Malformed) => assert!(matches!(ran, Ran::Untried { .. })),
                other => panic!("{other:?}"),
            }
            assert!(search.heard(*peer, Tried::Ran(ran)).is_none());
        }
        assert_eq!(guesses, MAX_PROVED_TO);
        assert!(proofs.spent());
        assert_eq!(
            search.over(proofs.spent(), false),
            Some(Failure::OthersAnswered)
        );

        // The computer showing the code, last: no proof left, and no try counted there.
        let (old, new) = ends();
        let (ran, answered) = tokio::join!(
            run_code(&code, b"old", b"new", old, &proofs),
            answer_run(&code, b"new", b"old", new)
        );
        assert!(matches!(ran, Ran::Untried { other_code: false }));
        assert_eq!(answered.err(), Some(RunError::Malformed));
    }

    /// A start of the first way naming the code shown here does not stop the move: anyone
    /// who learnt the nameplate can send one, and the computer typing the code may be of
    /// this version and on its way. A computer of that way sends one every few seconds, and
    /// after [`MAX_FAILED_TRANSFER_RUNS`] of them the person is told to update both.
    #[tokio::test(flavor = "multi_thread")]
    async fn one_start_of_the_first_way_does_not_stop_the_code_shown() {
        let free_port = || {
            std::net::UdpSocket::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port()
        };
        let net = |port: u16, peers: Vec<fogata_net::Multiaddr>| NetOptions {
            listen_port: port,
            use_dht: false,
            use_mdns: false,
            peers,
            ..Default::default()
        };
        let code = TransferCode::generate().unwrap();
        let dir = folder("first-way-start");
        let port = free_port();
        let mut options = TransferOptions::new(net(port, Vec::new()), "new");
        options.keychain = Some(Keychain::Off);
        options.code = Some(code.to_string());
        let mut shown = receive(dir.clone(), options).unwrap();
        let at: fogata_net::Multiaddr = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
            .parse()
            .unwrap();
        let old = start_node(&TransferOptions::new(net(0, vec![at]), "old"), None).unwrap();
        let new = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if let Some(peer) = old.peers_taking(&OLD_PROTOCOL).first() {
                    return *peer;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("the new computer lists the first way");
        let (_, start) = run_start(&code, b"old", b"new");
        for _ in 0..MAX_FAILED_TRANSFER_RUNS {
            // Each start is taken (the code still shown) and left unanswered.
            let mut stream = old
                .streams()
                .open_stream(new, OLD_PROTOCOL)
                .await
                .expect("the code is still shown");
            wire::send_start(&mut stream, &start).await.unwrap();
            assert!(wire::read_reply(&mut stream).await.is_err());
        }
        let end = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                match shown.events.recv().await {
                    Some(TransferEvent::Failed(failure)) => return failure,
                    Some(_) => {}
                    None => panic!("no end"),
                }
            }
        })
        .await
        .expect("the move ended");
        assert_eq!(end, Failure::OldVersion);
        let _ = old.shutdown().await;
        fs::remove_dir_all(&dir).unwrap();
    }

    /// What a new computer of version 0.1.0-beta.7 or before (`/fogata/transfer/1`) does with
    /// a start (its `answer_run`, with what this version still has of its run): another
    /// code is told so; a start it cannot run (no SPAKE2 message) is dropped without an
    /// answer and is not counted; any other is a run of that way, which it counts when it
    /// does not finish. `true` when it counted one.
    async fn as_beta7_new_computer(code: &TransferCode, mut stream: End) -> bool {
        let Ok(start) = wire::read_start(&mut stream).await else {
            return false;
        };
        match run_reply(code, b"new", b"old", &start) {
            Err(RunError::NotMine) => {
                let _ = wire::send_reply(&mut stream, None).await;
                false
            }
            Err(_) => false,
            Ok(_) => true,
        }
    }

    /// The two ways of running a code meet only to tell each other apart, and stop with
    /// [`Failure::OldVersion`], having spent and sent nothing. This version typing the code
    /// on a computer of the first way that shows it: its probe is dropped there without a
    /// try being counted, and a computer of that way showing another code says so. A
    /// computer of the first way typing the code on this one: its start (the nameplate and
    /// a SPAKE2 message) is left unanswered, so that computer goes on trying (the code shown
    /// here stops after a few, see `answer_old_run`); one for another code is told so.
    #[tokio::test]
    async fn a_computer_of_the_first_way_and_this_one_tell_each_other_apart() {
        let code = TransferCode::generate().unwrap();
        let (old, new) = ends();
        let (shows, counted) =
            tokio::join!(probe_old_way(&code, old), as_beta7_new_computer(&code, new));
        assert_eq!(shows, Some(true));
        assert!(!counted, "the first way's computer counted a try");
        let other = TransferCode::generate().unwrap();
        let (old, new) = ends();
        let (shows, counted) = tokio::join!(
            probe_old_way(&code, old),
            as_beta7_new_computer(&other, new)
        );
        assert_eq!(shows, Some(other.rendezvous() == code.rendezvous()));
        assert!(!counted);

        // A computer of the first way types the code shown here: its start, as it sends one.
        let (mut old, new) = ends();
        let (_, start) = run_start(&code, b"old", b"new");
        wire::send_start(&mut old, &start).await.unwrap();
        assert!(answer_old_run(&code, new).await);
        // Nothing came back: it hears that the stream ended, and tries again or gives up.
        assert!(wire::read_reply(&mut old).await.is_err());
        let (mut old, new) = ends();
        let (_, start) = run_start(&other, b"old", b"new");
        wire::send_start(&mut old, &start).await.unwrap();
        let (told, reply) =
            tokio::join!(answer_old_run(&code, new), wire::read_old_reply(&mut old));
        if other.rendezvous() != code.rendezvous() {
            assert!(!told);
            assert!(reply.unwrap(), "told it shows another code");
        }
        assert_eq!(
            Failure::OldVersion.to_string(),
            "the other computer has an older version of Fogata: update both computers to \
             the same version"
        );
    }

    #[test]
    fn the_other_computers_known_reasons_read_back_and_others_stay_words() {
        for reason in PeerReason::ALL {
            assert_eq!(PeerReason::of(reason.text()), Some(reason));
            assert_eq!(
                Failure::Declined(reason.text().into()).peer_reason(),
                Some(reason)
            );
        }
        assert_eq!(PeerReason::of("the disk caught fire"), None);
        assert_eq!(
            Failure::Other(PeerReason::Busy.text().into()).peer_reason(),
            None
        );
        // The command line words the new failures as before.
        assert_eq!(
            Failure::NoRoom {
                need: 2_000_000,
                free: 1024
            }
            .to_string(),
            format!(
                "this computer needs {} free for the account, and has {}",
                human_bytes(2_000_000),
                human_bytes(1024)
            )
        );
    }

    fn folder(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fogata-transfer-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn stage(dir: &Path, state: StageState) -> Incoming {
        let incoming = dir.join(INCOMING_DIR);
        fs::create_dir_all(&incoming).unwrap();
        fs::write(incoming.join("identity.key"), b"new identity").unwrap();
        fs::create_dir_all(incoming.join("logs")).unwrap();
        fs::write(incoming.join("logs").join("g.log"), b"new log").unwrap();
        let record = Incoming {
            state,
            transfer: "ab".repeat(16),
            from: "old".into(),
            name: "Ana".into(),
            device: String::new(),
            key: MoveKey::default(),
            restore: None,
        };
        write_stage(dir, &record).unwrap();
        record
    }

    #[test]
    fn a_move_put_in_place_half_way_goes_on_at_the_next_start() {
        let dir = folder("half");
        fs::write(dir.join("identity.key"), b"old identity").unwrap();
        fs::write(dir.join("friends.bin"), b"old friends").unwrap();
        let mut record = stage(&dir, StageState::Commit);
        // The computer stopped after the folder's own files went aside, and one of the new
        // ones came out.
        let replaced = dir.join(REPLACED_DIR);
        fs::create_dir_all(&replaced).unwrap();
        fs::rename(dir.join("identity.key"), replaced.join("identity.key")).unwrap();
        fs::rename(dir.join("friends.bin"), replaced.join("friends.bin")).unwrap();
        record.state = StageState::Placing;
        write_stage(&dir, &record).unwrap();
        // And a store opened meanwhile made an empty folder for logs.
        fs::create_dir_all(dir.join("logs")).unwrap();
        fs::rename(
            dir.join(INCOMING_DIR).join("identity.key"),
            dir.join("identity.key"),
        )
        .unwrap();
        recover_held(&dir, &Keychain::Off).unwrap();
        assert_eq!(fs::read(dir.join("identity.key")).unwrap(), b"new identity");
        assert_eq!(fs::read(dir.join("logs/g.log")).unwrap(), b"new log");
        assert!(!dir.join("friends.bin").exists());
        assert!(!dir.join(INCOMING_DIR).exists() && !replaced.exists());
        assert!(dir.join(MOVED_IN_FILE).exists() && !dir.join(STAGE_FILE).exists());
        // Again: nothing more to do.
        recover_held(&dir, &Keychain::Off).unwrap();
        assert_eq!(fs::read(dir.join("identity.key")).unwrap(), b"new identity");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A folder with a profile of its own (`own-N` files) and a move staged at `Commit`
    /// (`new-N` files and the account's identity), as when the old computer's Commit came.
    fn committed(name: &str, own: usize, new: usize) -> (PathBuf, Incoming) {
        let dir = folder(name);
        fs::write(dir.join("identity.key"), b"own identity").unwrap();
        for i in 0..own {
            fs::write(dir.join(format!("own-{i}")), b"own").unwrap();
        }
        let incoming = dir.join(INCOMING_DIR);
        fs::create_dir_all(&incoming).unwrap();
        fs::write(incoming.join("identity.key"), b"moved identity").unwrap();
        for i in 0..new {
            fs::write(incoming.join(format!("new-{i}")), b"new").unwrap();
        }
        let record = Incoming {
            state: StageState::Commit,
            transfer: "ab".repeat(16),
            from: "old".into(),
            name: "Ana".into(),
            device: String::new(),
            key: MoveKey::default(),
            restore: None,
        };
        write_stage(&dir, &record).unwrap();
        (dir, record)
    }

    /// The move's task with the folder's lock, as `receive` runs it, putting `record` in
    /// place; stopped (aborted) `after` a moment.
    async fn activate_then_abort(dir: &Path, record: Incoming, after: Duration) {
        let lock = Arc::new(
            Store::open(dir.to_path_buf())
                .unwrap()
                .hold_running()
                .unwrap(),
        );
        let flow = Arc::new(Flow {
            stage: Arc::default(),
            lock,
        });
        let dir = dir.to_path_buf();
        let task = tokio::spawn(FLOW.scope(flow, async move {
            activate(&dir, &Keychain::Off, record).await.map(|_| ())
        }));
        tokio::time::sleep(after).await;
        task.abort();
        let _ = task.await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_move_being_put_in_place_keeps_the_folder_until_it_is_done() {
        let (dir, record) = committed("held", 30_000, 30_000);
        activate_then_abort(&dir, record, Duration::from_millis(30)).await;
        // While what came is still being put in place, nothing else gets the folder. The
        // lock is taken first and the folder looked at after: taking it may wait a moment,
        // during which the move can finish.
        let lock = Store::open(dir.clone()).unwrap().hold_running();
        let busy = dir.join(INCOMING_DIR).exists();
        assert!(!(busy && lock.is_ok()), "the folder was let go of half way");
        drop(lock);
        for _ in 0..1200 {
            if !dir.join(INCOMING_DIR).exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(!dir.join(INCOMING_DIR).exists());
        assert_eq!(
            fs::read(dir.join("identity.key")).unwrap(),
            b"moved identity"
        );
        // It lets go once done.
        let mut free = false;
        for _ in 0..1200 {
            if Store::open(dir.clone()).unwrap().hold_running().is_ok() {
                free = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(free);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The person throws away a staged move just as the old computer finishes it: the folder
    /// ends with its own profile whole or with the moved account whole, never half of each.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn throwing_away_a_move_being_finished_never_leaves_half_of_it() {
        for round in 0..5u64 {
            let (dir, record) = committed(&format!("race-{round}"), 20_000, 20_000);
            activate_then_abort(&dir, record, Duration::from_millis(5 + 10 * round)).await;
            let discarded = {
                let dir = dir.clone();
                tokio::task::spawn_blocking(move || discard_incoming(&dir))
                    .await
                    .unwrap()
            };
            // As the next start does: with the folder's lock, once the move has let go of it
            // (it may still be removing `replaced/`).
            let store = Store::open(dir.clone()).unwrap();
            let mut running = None;
            for _ in 0..1200 {
                if let Ok(lock) = store.hold_running() {
                    running = Some(lock);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            assert!(running.is_some(), "round {round}: the move never let go");
            recover_held(&dir, &Keychain::Off).unwrap();
            drop(running);
            let own = (0..20_000)
                .filter(|i| dir.join(format!("own-{i}")).exists())
                .count();
            let new = (0..20_000)
                .filter(|i| dir.join(format!("new-{i}")).exists())
                .count();
            let identity = fs::read(dir.join("identity.key")).unwrap_or_default();
            let untouched = own == 20_000 && new == 0 && identity == b"own identity";
            let moved = own == 0 && new == 20_000 && identity == b"moved identity";
            assert!(
                untouched || moved,
                "round {round}: discard {discarded:?}, own {own}, new {new}"
            );
            // It is never thrown away: being put in place, it is finished.
            assert!(moved, "round {round}");
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[tokio::test]
    async fn a_move_stops_only_before_its_end_began() {
        let task = tokio::spawn(std::future::pending::<()>());
        let (control, mut answers) = mpsc::unbounded_channel();
        let stage = Stage::default();
        // Still looking: the task goes at once.
        assert!(stage.stop(&control, &task.abort_handle()));
        assert_eq!(answers.recv().await, Some(false));
        assert!(task.await.unwrap_err().is_cancelled());
        // Under way: told to stop where nothing is half done, the task goes on until then.
        let task = tokio::spawn(std::future::pending::<()>());
        let stage = Stage::default();
        stage.enter(Phase::Running).unwrap();
        assert!(stage.stop(&control, &task.abort_handle()));
        assert!(stage.stopped() && !task.is_finished());
        assert!(
            stage.enter(Phase::Ending).is_err(),
            "a stopped move never ends"
        );
        task.abort();
        // Its end begun: it is not stopped any more.
        let task = tokio::spawn(std::future::pending::<()>());
        let stage = Stage::default();
        stage.enter(Phase::Ending).unwrap();
        assert!(!stage.stop(&control, &task.abort_handle()));
        assert!(!stage.stopped() && !task.is_finished());
        task.abort();
    }

    /// What came cannot be put in place yet (here something is in the way of `replaced/`):
    /// the move is committed all the same, says it goes on at the next start, never that
    /// nothing changed, and it does.
    #[tokio::test]
    async fn a_move_that_cannot_be_put_in_place_yet_goes_on_at_the_next_start() {
        let (dir, record) = committed("later", 2, 2);
        fs::write(dir.join(REPLACED_DIR), b"in the way").unwrap();
        let Activated::Later(failed) = activate(&dir, &Keychain::Off, record).await.unwrap() else {
            panic!("it cannot be put in place yet");
        };
        let said = later(failed).to_string();
        assert!(said.contains("next time Fogata starts"), "{said}");
        assert_eq!(stage_of(&dir).unwrap().state, StageState::Commit);
        fs::remove_file(dir.join(REPLACED_DIR)).unwrap();
        recover_held(&dir, &Keychain::Off).unwrap();
        assert_eq!(
            fs::read(dir.join("identity.key")).unwrap(),
            b"moved identity"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A move that cannot even be committed here (the folder cannot be written) stays
    /// staged: the old computer is not told it took the account.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_move_that_cannot_be_committed_stays_staged() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, mut record) = committed("uncommitted", 2, 2);
        record.state = StageState::Staged;
        write_stage(&dir, &record).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).unwrap();
        assert!(activate(&dir, &Keychain::Off, record).await.is_err());
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(stage_of(&dir).unwrap().state, StageState::Staged);
        assert_eq!(fs::read(dir.join("identity.key")).unwrap(), b"own identity");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The account is in place, and can be told so, before what it replaced (maybe much)
    /// is removed; that goes after, or at the next start.
    #[test]
    fn the_account_is_in_place_before_what_it_replaced_goes() {
        let (dir, record) = committed("in-place-first", 3, 3);
        put_in_place(&dir, &record, &Keychain::Off).unwrap();
        assert_eq!(
            fs::read(dir.join("identity.key")).unwrap(),
            b"moved identity"
        );
        assert!(dir.join(MOVED_IN_FILE).exists() && !dir.join(STAGE_FILE).exists());
        assert!(dir.join(REPLACED_DIR).join("own-0").exists());
        recover_held(&dir, &Keychain::Off).unwrap();
        assert!(!dir.join(REPLACED_DIR).exists());
        assert!(dir.join("new-0").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_move_being_put_in_place_is_finished_rather_than_thrown_away() {
        let (dir, _) = committed("finish-not-discard", 3, 3);
        assert!(!discard_incoming(&dir).unwrap());
        assert_eq!(
            fs::read(dir.join("identity.key")).unwrap(),
            b"moved identity"
        );
        assert!(dir.join("new-0").exists() && !dir.join("own-0").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The key of the folder that is replaced goes from the keychain, the one it had when
    /// the files went aside, never the new one's.
    #[test]
    fn the_replaced_folder_takes_its_key_from_the_keychain_with_it() {
        let keys = Arc::new(vault::MemoryKeyStore::default());
        let keychain = Keychain::Seal(keys.clone());
        let (dir, _) = committed("replaced-key", 1, 1);
        // A key made for the folder after the move came (a profile made meanwhile).
        let own = vault::create(&dir, keys.as_ref()).unwrap().unwrap();
        let new = vault::create(&dir.join(INCOMING_DIR), keys.as_ref())
            .unwrap()
            .unwrap();
        assert_ne!(own.as_bytes(), new.as_bytes());
        let (own_name, new_name) = (
            vault::key_name(&dir).unwrap(),
            vault::key_name(&dir.join(INCOMING_DIR)).unwrap(),
        );
        recover_held(&dir, &keychain).unwrap();
        assert_eq!(keys.names(), vec![new_name.clone()]);
        assert_ne!(own_name, new_name);
        assert_eq!(vault::key_name(&dir), Some(new_name));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The replaced folder's key goes as soon as its files went aside, and a `replaced/`
    /// left behind (the computer stopped before it went) takes its key with it too.
    #[test]
    fn the_replaced_folders_key_is_never_left_without_its_files() {
        let keys = Arc::new(vault::MemoryKeyStore::default());
        let keychain = Keychain::Seal(keys.clone());
        let (dir, mut record) = committed("replaced-key-early", 1, 1);
        vault::create(&dir, keys.as_ref()).unwrap().unwrap();
        vault::create(&dir.join(INCOMING_DIR), keys.as_ref())
            .unwrap()
            .unwrap();
        let new_name = vault::key_name(&dir.join(INCOMING_DIR)).unwrap();
        // The folder's own files went aside, and the computer stopped before the new ones
        // came out.
        let replaced = dir.join(REPLACED_DIR);
        fs::create_dir_all(&replaced).unwrap();
        for name in ["identity.key", "own-0", vault::SEALED_FILE] {
            fs::rename(dir.join(name), replaced.join(name)).unwrap();
        }
        record.state = StageState::Placing;
        write_stage(&dir, &record).unwrap();
        recover_held(&dir, &keychain).unwrap();
        assert_eq!(keys.names(), vec![new_name.clone()]);
        // A `replaced/` left behind with a key the keychain still has.
        let (dir2, _) = committed("replaced-key-left", 1, 0);
        fs::remove_dir_all(dir2.join(INCOMING_DIR)).unwrap();
        fs::remove_file(dir2.join(STAGE_FILE)).unwrap();
        let left = dir2.join(REPLACED_DIR);
        fs::create_dir_all(&left).unwrap();
        vault::create(&left, keys.as_ref()).unwrap().unwrap();
        assert_eq!(keys.names().len(), 2);
        recover_held(&dir2, &keychain).unwrap();
        assert!(!left.exists());
        assert_eq!(keys.names(), vec![new_name]);
        fs::remove_dir_all(&dir).unwrap();
        fs::remove_dir_all(&dir2).unwrap();
    }

    #[test]
    fn no_profile_is_made_while_an_account_that_came_waits() {
        let dir = folder("no-profile");
        stage(&dir, StageState::Staged);
        let made = crate::Core::create_profile(&dir, "Leo", None);
        assert!(format!("{:#}", made.unwrap_err()).contains("waits to be finished"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_old_computer_keeps_nothing_that_is_the_person() {
        let dir = folder("deactivate");
        for file in [
            "identity.key",
            "friends.bin",
            "friends.bin.bak",
            "joins.json",
            crate::recovery::KEY_FILE,
        ] {
            fs::write(dir.join(file), b"x").unwrap();
        }
        for sub in [
            "mls",
            crate::store::BACKUP_DIR,
            crate::store::REFUSED_DIR,
            "logs",
        ] {
            fs::create_dir_all(dir.join(sub)).unwrap();
            fs::write(dir.join(sub).join("x"), b"x").unwrap();
        }
        let moved = Moved {
            state: MovedState::Moved,
            transfer: "ab".repeat(16),
            to: "new".into(),
            name: "Ana".into(),
            at_ms: 1,
            key: MoveKey::default(),
        };
        mark_moved(&dir, &moved).unwrap();
        deactivate(&dir).unwrap();
        let mut left: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["logs", "moved.json"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The secrets a move leaves behind are written over before they go (a second name of
    /// the same file shows it), and a link among them is removed without touching what it
    /// points to.
    #[cfg(unix)]
    #[test]
    fn what_a_move_leaves_behind_is_written_over() {
        let dir = folder("wiped");
        fs::create_dir_all(dir.join("mls")).unwrap();
        let secret = b"0123456789abcdef0123456789abcdef";
        fs::write(dir.join("identity.key"), secret).unwrap();
        fs::write(dir.join("mls").join("g.bin"), secret).unwrap();
        let outside = folder("wiped-outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("kept"), secret).unwrap();
        std::os::unix::fs::symlink(outside.join("kept"), dir.join("friends.bin")).unwrap();
        let (seen, seen_mls) = (outside.join("seen"), outside.join("seen-mls"));
        fs::hard_link(dir.join("identity.key"), &seen).unwrap();
        fs::hard_link(dir.join("mls").join("g.bin"), &seen_mls).unwrap();
        deactivate(&dir).unwrap();
        assert!(!dir.join("identity.key").exists());
        assert!(fs::symlink_metadata(dir.join("friends.bin")).is_err());
        assert!(!dir.join("mls").exists());
        assert_eq!(fs::read(&seen).unwrap(), [0; 32]);
        assert_eq!(fs::read(&seen_mls).unwrap(), [0; 32]);
        assert_eq!(fs::read(outside.join("kept")).unwrap(), secret);
        fs::remove_dir_all(&dir).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }

    #[test]
    fn a_computer_on_the_local_network_gets_the_code_first() {
        let (near, far) = (PeerId::random(), PeerId::random());
        let mut first_far = None;
        // Only one found over the DHT: not yet.
        assert!(choose(&[(far, false)], false, &mut first_far).is_empty());
        assert!(first_far.is_some());
        // One on the local network shows meanwhile: it goes first.
        assert_eq!(
            choose(&[(far, false), (near, true)], false, &mut first_far),
            vec![near]
        );
        // None here after a while: the one found over the DHT, unless one here is being
        // tried still.
        first_far = Some(Instant::now() - LAN_FIRST);
        assert!(choose(&[(far, false)], true, &mut first_far).is_empty());
        assert_eq!(choose(&[(far, false)], false, &mut first_far), vec![far]);
        assert!(choose(&[], false, &mut first_far).is_empty());
        // Every one here at once.
        let other = PeerId::random();
        assert_eq!(
            choose(
                &[(near, true), (far, false), (other, true)],
                false,
                &mut first_far
            ),
            vec![near, other]
        );
    }

    /// Those that list the move's protocol go first; the computers found under the code that
    /// do not list it only when none does; a peer that is neither is never tried, nor one
    /// `keep` leaves out (this computer, another code, one tried a moment ago).
    #[test]
    fn computers_that_list_the_move_come_before_those_only_found_under_the_code() {
        let [listing, found, both, neither, left_out] = [(); 5].map(|_| PeerId::random());
        let keep = |p: &PeerId| *p != left_out;
        assert_eq!(
            to_try(vec![listing, both], vec![found, both], keep),
            vec![listing, both]
        );
        assert_eq!(
            to_try(vec![left_out], vec![found, both, left_out], keep),
            vec![found, both]
        );
        assert!(!to_try(vec![], vec![found], keep).contains(&neither));
        assert!(to_try(vec![], vec![left_out], keep).is_empty());
    }

    /// Another move on this local network lists the protocol to this computer before the
    /// computer found under the code does: it goes first, shows another code (not counted,
    /// see `sending`), and is left out from then on, so the one found under the code comes
    /// next, though it does not list the protocol yet.
    #[test]
    fn another_move_nearby_does_not_keep_the_one_found_under_the_code_waiting() {
        let [other_move, found] = [(); 2].map(|_| PeerId::random());
        let mut other_code: HashSet<PeerId> = HashSet::new();
        let first = to_try(vec![other_move], vec![found], |p| !other_code.contains(p));
        assert_eq!(first, vec![other_move]);
        // Its run answered for another code.
        other_code.insert(other_move);
        let next = to_try(vec![other_move], vec![found], |p| !other_code.contains(p));
        assert_eq!(next, vec![found]);
    }

    #[test]
    fn the_window_of_the_application_running_here_stays_in_place() {
        let dir = folder("window");
        fs::write(dir.join("identity.key"), b"old identity").unwrap();
        fs::create_dir_all(dir.join("window")).unwrap();
        fs::write(dir.join("window/running"), b"held").unwrap();
        stage(&dir, StageState::Commit);
        recover_held(&dir, &Keychain::Off).unwrap();
        assert_eq!(fs::read(dir.join("identity.key")).unwrap(), b"new identity");
        // Its lock, and where a restart leaves its note, are where the application has them.
        assert_eq!(fs::read(dir.join("window/running")).unwrap(), b"held");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_move_that_did_not_come_whole_is_thrown_away_and_a_staged_one_kept() {
        let dir = folder("partial");
        fs::write(dir.join("identity.key"), b"own identity").unwrap();
        stage(&dir, StageState::Receiving);
        recover_held(&dir, &Keychain::Off).unwrap();
        assert!(!dir.join(INCOMING_DIR).exists());
        assert_eq!(fs::read(dir.join("identity.key")).unwrap(), b"own identity");
        // No record at all (cut before it was written): thrown away too.
        fs::create_dir_all(dir.join(INCOMING_DIR)).unwrap();
        recover_held(&dir, &Keychain::Off).unwrap();
        assert!(!dir.join(INCOMING_DIR).exists());
        // Staged: kept for the old computer to finish, and shown as such.
        let record = stage(&dir, StageState::Staged);
        recover_held(&dir, &Keychain::Off).unwrap();
        assert!(dir.join(INCOMING_DIR).exists());
        assert_eq!(state(&dir), TransferState::Staged(record));
        assert_eq!(fs::read(dir.join("identity.key")).unwrap(), b"own identity");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Files the old computer sends land in `incoming/` only, and none of them can say how
    /// far the move got: a record there, whatever it says, is a file like any other, and a
    /// move cut short is thrown away with it, the folder's own files untouched.
    #[test]
    fn nothing_the_old_computer_sends_can_say_the_move_was_committed() {
        let dir = folder("forged-record");
        fs::write(dir.join("identity.key"), b"own identity").unwrap();
        fs::write(dir.join("friends.bin"), b"own friends").unwrap();
        let mut record = stage(&dir, StageState::Receiving);
        record.state = StageState::Commit;
        let forged = serde_json::to_vec(&record).unwrap();
        // What an older layout kept in `incoming/`, written by the sender.
        fs::write(dir.join(INCOMING_DIR).join(".transfer.json"), &forged).unwrap();
        fs::write(dir.join(INCOMING_DIR).join(".aside"), b"").unwrap();
        fs::write(dir.join(INCOMING_DIR).join(STAGE_FILE), &forged).unwrap();
        recover_held(&dir, &Keychain::Off).unwrap();
        assert!(!dir.join(INCOMING_DIR).exists() && !dir.join(STAGE_FILE).exists());
        assert_eq!(fs::read(dir.join("identity.key")).unwrap(), b"own identity");
        assert_eq!(fs::read(dir.join("friends.bin")).unwrap(), b"own friends");
        // Without any record of the move's own, the same.
        fs::create_dir_all(dir.join(INCOMING_DIR)).unwrap();
        fs::write(dir.join(INCOMING_DIR).join(".transfer.json"), &forged).unwrap();
        recover_held(&dir, &Keychain::Off).unwrap();
        assert!(!dir.join(INCOMING_DIR).exists());
        assert_eq!(fs::read(dir.join("identity.key")).unwrap(), b"own identity");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The new computer refuses, before writing anything, a file named as one of the move's
    /// own records, in any case.
    #[tokio::test]
    async fn the_old_computer_cannot_send_the_moves_own_records() {
        use wire::{Frame, Msg};
        for path in [
            ".transfer.json",
            ".aside",
            STAGE_FILE,
            "INCOMING.JSON",
            "INCOMI~1.JSO",
            "logs/.hidden",
        ] {
            let said = take(vec![Frame::Msg(Msg::File { path: path.into() })], 10).await;
            assert!(said.contains("refused a file"), "{path}: {said}");
        }
    }

    #[test]
    fn a_moved_folder_never_makes_or_reads_an_identity() {
        let dir = folder("moved");
        fs::write(dir.join(MOVED_FILE), b"not even json").unwrap();
        let store = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        assert!(format!("{:#}", store.identity().unwrap_err()).contains("moved"));
        assert!(!dir.join("identity.key").exists());
        assert!(matches!(state(&dir), TransferState::Moved(m) if m.state == MovedState::Moving));
        // Not finished: nothing is deleted.
        assert!(forget(&dir).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// What the old computer sends, as the new one reads it: `frames` of a run's channel.
    fn sent_frames(keys: &fogata_friends::RunKeys, frames: Vec<wire::Frame>) -> Vec<u8> {
        futures::executor::block_on(async {
            let mut out = Channel::new(futures::io::Cursor::new(Vec::new()), keys, true);
            for frame in frames {
                match frame {
                    wire::Frame::Msg(msg) => out.send(&msg).await.unwrap(),
                    wire::Frame::Data(data) => out.send_data(&data).await.unwrap(),
                }
            }
            out.into_inner().into_inner()
        })
    }

    fn summary_of(bytes: u64) -> Summary {
        let identity = DeviceIdentity::generate().unwrap();
        let book = fogata_friends::FriendBook::new(&identity, "Ana", 1).unwrap();
        Summary {
            profile: book.profile,
            friends: 0,
            groups: 0,
            chats: 0,
            files: 0,
            bytes,
        }
    }

    /// Takes `frames` into a new folder; what it says when it stops.
    async fn take(frames: Vec<wire::Frame>, declared: u64) -> String {
        let code = fogata_friends::TransferCode::generate().unwrap();
        let (keys, receiver_keys) = wire::tests::run_keys(&code);
        let bytes = sent_frames(&keys, frames);
        let mut channel = Channel::new(futures::io::Cursor::new(bytes), &receiver_keys, false);
        let dir = folder(&format!("take-{}", rand_name()));
        let stage = dir.join(INCOMING_DIR);
        fs::create_dir_all(&stage).unwrap();
        let (events, _) = mpsc::unbounded_channel();
        let said = take_files(
            &dir,
            &stage,
            &Keychain::Off,
            &events,
            &mut channel,
            &summary_of(declared),
        )
        .await
        .map(|()| "taken".to_string())
        .unwrap_or_else(|e| format!("{e:#}"));
        fs::remove_dir_all(&dir).unwrap();
        said
    }

    fn rand_name() -> String {
        let mut b = [0u8; 4];
        fogata_crypto::random_bytes(&mut b).unwrap();
        hex::encode(b)
    }

    #[tokio::test]
    async fn a_peer_that_sends_empty_pieces_or_more_than_it_said_is_stopped() {
        use wire::{Frame, Msg};
        let file = || {
            Frame::Msg(Msg::File {
                path: "logs/x.log".into(),
            })
        };
        let empty = take(
            vec![file(), Frame::Data(zeroize::Zeroizing::new(Vec::new()))],
            10,
        )
        .await;
        assert!(empty.contains("empty piece"), "{empty}");
        let big = zeroize::Zeroizing::new(vec![1u8; wire::CHUNK]);
        let mut frames = vec![file()];
        for _ in 0..(SLACK_BYTES as usize / wire::CHUNK + 2) {
            frames.push(Frame::Data(big.clone()));
        }
        let more = take(frames, 10).await;
        assert!(more.contains("far more came"), "{more}");
    }

    /// A connection that gives `input`, then nothing more without closing, and keeps what
    /// is written to it.
    struct Stalled {
        input: futures::io::Cursor<Vec<u8>>,
        output: Arc<std::sync::Mutex<Vec<u8>>>,
    }

    impl futures::AsyncRead for Stalled {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut [u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            if self.input.position() as usize >= self.input.get_ref().len() {
                return std::task::Poll::Pending;
            }
            std::pin::Pin::new(&mut self.input).poll_read(cx, buf)
        }
    }

    impl futures::AsyncWrite for Stalled {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            self.output.lock().unwrap().extend_from_slice(buf);
            std::task::Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    /// Cancelled on the new computer while files come (the old one sends a piece now and
    /// then, or nothing for a while): the move stops at once, the other computer hears so,
    /// and what came goes with the key it was sealed with.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelling_on_the_new_computer_stops_the_files_coming() {
        use wire::{Frame, Msg};
        let code = fogata_friends::TransferCode::generate().unwrap();
        let (keys, receiver_keys) = wire::tests::run_keys(&code);
        let summary = summary_of(1 << 20);
        let frames = vec![
            Frame::Msg(Msg::Offer {
                transfer: "ab".repeat(16),
                computer: "old".into(),
                device: hex::encode(summary.profile.device.as_bytes()),
                profile: base64::engine::general_purpose::STANDARD.encode(summary.profile.encode()),
                friends: 0,
                groups: 0,
                chats: 0,
                files: 1,
                bytes: summary.bytes,
            }),
            Frame::Msg(Msg::Go),
            Frame::Msg(Msg::File {
                path: "logs/x.log".into(),
            }),
            Frame::Data(zeroize::Zeroizing::new(vec![1u8; 1000])),
        ];
        let output = Arc::new(std::sync::Mutex::new(Vec::new()));
        let link = Stalled {
            input: futures::io::Cursor::new(sent_frames(&keys, frames)),
            output: output.clone(),
        };
        let channel = Channel::new(link, &receiver_keys, false);
        let dir = folder("cancel-receiving");
        let store = Store::open(dir.clone()).unwrap();
        let lock = Arc::new(store.hold_running().unwrap());
        let stage = Arc::new(Stage::default());
        stage.enter(Phase::Running).unwrap();
        let flow = Arc::new(Flow {
            stage: stage.clone(),
            lock,
        });
        let memory = Arc::new(vault::MemoryKeyStore::default());
        let keychain = Keychain::Seal(memory.clone());
        let (events, mut seen) = mpsc::unbounded_channel();
        // This computer's person said yes.
        let (yes, mut answers) = mpsc::unbounded_channel();
        yes.send(true).unwrap();
        let task = {
            let dir = dir.clone();
            tokio::spawn(FLOW.scope(flow, async move {
                let options = TransferOptions::new(NetOptions::default(), "new");
                let answers = &mut answers;
                received(
                    &dir, &options, &keychain, &events, answers, channel, false, None,
                )
                .await
            }))
        };
        // Files are coming.
        loop {
            if let Some(TransferEvent::Matched { .. }) = seen.recv().await {
                break;
            }
        }
        for _ in 0..100 {
            if dir.join(INCOMING_DIR).join("logs").exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(!memory.names().is_empty(), "the new key was made");
        let (control, _answers) = mpsc::unbounded_channel();
        let idle = tokio::spawn(std::future::pending::<()>());
        assert!(stage.stop(&control, &idle.abort_handle()));
        idle.abort();
        let ended = tokio::time::timeout(Duration::from_secs(15), task)
            .await
            .expect("the move stopped")
            .unwrap();
        assert!(cancelled(&ended.unwrap_err()));
        assert!(!dir.join(INCOMING_DIR).exists() && !dir.join(STAGE_FILE).exists());
        assert!(memory.names().is_empty(), "the new key went too");
        // What the old computer reads: the new one's hello and yes, then why it stopped.
        let written = output.lock().unwrap().clone();
        let mut back = Channel::new(futures::io::Cursor::new(written), &keys, true);
        assert!(matches!(back.recv_msg().await.unwrap(), Msg::Hello { .. }));
        assert!(matches!(back.recv_msg().await.unwrap(), Msg::Go));
        let said = format!("{:#}", back.recv_msg().await.unwrap_err());
        assert!(said.contains("cancelled"), "{said}");
        drop(store);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// An offer of more bytes than any account holds is refused before anything comes, also
    /// one so large that the room it needs would wrap round to a small number.
    #[tokio::test]
    async fn an_offer_too_large_to_hold_is_refused() {
        use wire::{Frame, Msg};
        let code = fogata_friends::TransferCode::generate().unwrap();
        let (keys, receiver_keys) = wire::tests::run_keys(&code);
        for bytes in [u64::MAX, u64::MAX - SLACK_BYTES, MAX_BYTES + 1] {
            let summary = summary_of(bytes);
            let offer = Frame::Msg(Msg::Offer {
                transfer: "ab".repeat(16),
                computer: "old".into(),
                device: hex::encode(summary.profile.device.as_bytes()),
                profile: base64::engine::general_purpose::STANDARD.encode(summary.profile.encode()),
                friends: 0,
                groups: 0,
                chats: 0,
                files: 1,
                bytes,
            });
            let link = Stalled {
                input: futures::io::Cursor::new(sent_frames(&keys, vec![offer])),
                output: Arc::default(),
            };
            let channel = Channel::new(link, &receiver_keys, false);
            let dir = folder("too-large");
            let mut options = TransferOptions::new(NetOptions::default(), "new");
            options.free_space = Some(|_| Some(1 << 40));
            let (events, _) = mpsc::unbounded_channel();
            let (_, mut answers) = mpsc::unbounded_channel();
            let said = received(
                &dir,
                &options,
                &Keychain::Off,
                &events,
                &mut answers,
                channel,
                false,
                None,
            )
            .await
            .unwrap_err();
            assert!(
                format!("{said:#}").contains("far more"),
                "{bytes}: {said:#}"
            );
            assert!(!dir.join(INCOMING_DIR).exists());
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn room_is_left_over() {
        assert!(room_needed(1 << 30) > (1 << 30) + SLACK_BYTES);
        // A size near the largest never wraps round to a small one.
        assert_eq!(room_needed(u64::MAX), u64::MAX);
        assert_eq!(room_needed(u64::MAX - 1000), u64::MAX);
        assert_eq!(human_bytes(3 << 30), "3.0 GB");
    }

    #[test]
    fn the_new_computer_keeps_its_own_devices_and_port() {
        let app = br#"{"theme":"light","audioInput":"mic-1","audioOutput":"spk","videoInput":"cam","sounds":false}"#;
        let out: serde_json::Value =
            serde_json::from_slice(&adjust("app.json", app, None).unwrap()).unwrap();
        assert_eq!(out, serde_json::json!({"theme": "light", "sounds": false}));
        let meta = br#"{"port":23456,"last_seen":{"a":1}}"#;
        let out: serde_json::Value =
            serde_json::from_slice(&adjust("meta.json", meta, Some(31000)).unwrap()).unwrap();
        assert_eq!(out["port"], 31000);
        assert_eq!(out["last_seen"]["a"], 1);
        let out: serde_json::Value =
            serde_json::from_slice(&adjust("meta.json", meta, None).unwrap()).unwrap();
        assert!(out.get("port").is_none());
        assert!(adjust("logs/x.log", b"{}", None).is_none());
    }

    #[test]
    fn names_from_the_other_computer_show_tidy() {
        assert_eq!(shown("  Leo's   desktop "), "Leo's desktop");
        assert_eq!(shown("photo\u{202E}gnp.exe\n"), "photognp.exe");
        assert_eq!(shown(&"x".repeat(500)).chars().count(), 64);
        // Nothing that shows: the screens word it.
        assert_eq!(shown("\u{200B}"), "");
    }
}
