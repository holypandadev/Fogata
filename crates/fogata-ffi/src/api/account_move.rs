//! Moving the account to another computer, or bringing one here (fogata-core's
//! `transfer`). A move runs on the core's runtime with no core running on the folder: the
//! screens stop it first ([`super::app::stop_core`]) and start it again after.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::Result;
use flutter_rust_bridge::frb;
use fogata_core::{
    NetOptions,
    transfer::{
        self, CONNECTION_LOST, Failure, MovedState, PeerReason, Summary, TransferControl,
        TransferEvent, TransferOptions, TransferState,
    },
};

use super::{
    app::{RUNTIME, blocking, bridge_enabled, use_keychain},
    types::{UiProfile, profile},
};
use crate::frb_generated::StreamSink;
use fogata_friends::CodeProblem;

/// The move under way, to answer or stop it.
static CONTROL: Mutex<Option<TransferControl>> = Mutex::new(None);

/// Where a data folder stands with moves.
#[derive(Clone, Debug)]
pub struct UiMoveState {
    /// `none`, `moving` (sent, the other computer has not confirmed), `moved`, or `staged`
    /// (an account came here whole and waits for its old computer to finish).
    pub kind: String,
    /// The account's name.
    pub name: String,
    /// The other computer's name.
    pub computer: String,
    pub at_ms: i64,
}

/// What moves, as the old computer counts it.
#[derive(Clone, Debug)]
pub struct UiMoveSummary {
    pub profile: UiProfile,
    pub friends: u32,
    pub groups: u32,
    pub chats: u32,
    /// Pictures, videos and files kept in chats.
    pub files: u32,
    /// Bytes of everything that moves.
    pub bytes: i64,
}

/// What a move says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiMoveEventKind {
    /// New computer: the code to show, and until when it works.
    Code,
    Looking,
    /// New computer: a wrong code was tried against the one shown.
    WrongCodeTried,
    /// The code matched on both computers; the old one waits for the person's yes.
    Matched,
    Progress,
    Finishing,
    Done,
    Failed,
}

/// Why a move stopped, beyond its kind, when it is one the screens word; the other
/// computer's (`Peer…`) as far as its words are known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiMoveReason {
    /// This computer has too little free space (`need` and `free` say how much).
    NoRoom,
    /// The other computer offered far more than an account holds (`offered`).
    TooLarge,
    /// This computer is finishing another move.
    Busy,
    /// This computer has no move to finish from that computer.
    NothingToFinish,
    /// This computer's network stopped.
    NetworkStopped,
    /// The connection to the other computer was lost.
    ConnectionLost,
    PeerNoRoom,
    PeerTooLarge,
    PeerBusy,
    PeerCouldNotKeep,
    PeerCancelled,
    PeerCouldNotFinish,
    PeerStopped,
    PeerNothingToFinish,
    PeerNotProven,
}

/// Why typed text is not a code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiCodeProblem {
    /// Not ten letters and digits.
    NotACode,
    /// Ten letters and digits whose check character does not fit.
    Mistyped,
}

#[derive(Clone, Debug)]
pub struct UiMoveEvent {
    pub kind: UiMoveEventKind,
    pub code: Option<String>,
    pub expires_ms: i64,
    /// The other computer's name.
    pub computer: Option<String>,
    /// The two are on one local network.
    pub lan: bool,
    /// The profile the new computer has now, which the move replaces.
    pub replaces: Option<String>,
    pub summary: Option<UiMoveSummary>,
    pub done: i64,
    pub total: i64,
    /// `wrong_code`, `expired`, `too_many_tries`, `no_contact`, `interrupted`,
    /// `unconfirmed`, `cancelled`, `declined`, `later`, `old_version` (the other computer
    /// moves accounts the way of an older version: update both) or `other`.
    pub failure: Option<String>,
    /// Why, when the screens word it.
    pub reason: Option<UiMoveReason>,
    /// Otherwise the words that came with it: the system's (a disk, a keychain, the
    /// network), or the other computer's for a reason this version does not know. Not
    /// translated.
    pub detail: Option<String>,
    /// Bytes for [`UiMoveReason::NoRoom`] (needed and free) and
    /// [`UiMoveReason::TooLarge`] (offered).
    pub need: i64,
    pub free: i64,
    pub offered: i64,
}

fn summary(s: &Summary) -> UiMoveSummary {
    UiMoveSummary {
        profile: profile(&s.profile),
        friends: s.friends,
        groups: s.groups,
        chats: s.chats,
        files: s.files,
        bytes: s.bytes as i64,
    }
}

fn ui_event(event: TransferEvent) -> UiMoveEvent {
    let mut out = UiMoveEvent {
        kind: UiMoveEventKind::Looking,
        code: None,
        expires_ms: 0,
        computer: None,
        lan: false,
        replaces: None,
        summary: None,
        done: 0,
        total: 0,
        failure: None,
        reason: None,
        detail: None,
        need: 0,
        free: 0,
        offered: 0,
    };
    match event {
        TransferEvent::Code { code, expires_ms } => {
            out.kind = UiMoveEventKind::Code;
            out.code = Some(code);
            out.expires_ms = expires_ms as i64;
        }
        TransferEvent::Looking => {}
        TransferEvent::WrongCodeTried => out.kind = UiMoveEventKind::WrongCodeTried,
        TransferEvent::Matched {
            computer,
            summary: s,
            lan,
            replaces,
        } => {
            out.kind = UiMoveEventKind::Matched;
            out.computer = Some(computer);
            out.lan = lan;
            out.replaces = replaces;
            out.summary = Some(summary(&s));
        }
        TransferEvent::Progress { done, total } => {
            out.kind = UiMoveEventKind::Progress;
            out.done = done as i64;
            out.total = total as i64;
        }
        TransferEvent::Finishing => out.kind = UiMoveEventKind::Finishing,
        TransferEvent::Done {
            computer,
            summary: s,
        } => {
            out.kind = UiMoveEventKind::Done;
            out.computer = Some(computer);
            out.summary = s.as_ref().map(summary);
        }
        TransferEvent::Failed(failure) => {
            out.kind = UiMoveEventKind::Failed;
            out.failure = Some(
                match &failure {
                    Failure::WrongCode => "wrong_code",
                    Failure::Expired => "expired",
                    Failure::TooManyTries => "too_many_tries",
                    Failure::NoContact => "no_contact",
                    Failure::Interrupted(_) => "interrupted",
                    Failure::Unconfirmed => "unconfirmed",
                    Failure::Cancelled => "cancelled",
                    Failure::Declined(_) => "declined",
                    Failure::Later(_) => "later",
                    // The other computer runs a version that moves accounts another way: the
                    // screens ask to update both.
                    Failure::OldVersion => "old_version",
                    Failure::NoRoom { .. }
                    | Failure::TooLarge { .. }
                    | Failure::Busy
                    | Failure::NothingToFinish
                    | Failure::NetworkStopped
                    | Failure::Other(_) => "other",
                }
                .into(),
            );
            out.reason = match &failure {
                Failure::NoRoom { need, free } => {
                    out.need = *need as i64;
                    out.free = *free as i64;
                    Some(UiMoveReason::NoRoom)
                }
                Failure::TooLarge { offered } => {
                    out.offered = *offered as i64;
                    Some(UiMoveReason::TooLarge)
                }
                Failure::Busy => Some(UiMoveReason::Busy),
                Failure::NothingToFinish => Some(UiMoveReason::NothingToFinish),
                Failure::NetworkStopped => Some(UiMoveReason::NetworkStopped),
                Failure::Interrupted(why) if why == CONNECTION_LOST => {
                    Some(UiMoveReason::ConnectionLost)
                }
                Failure::Declined(_) => failure.peer_reason().map(|r| match r {
                    PeerReason::NoRoom => UiMoveReason::PeerNoRoom,
                    PeerReason::TooLarge => UiMoveReason::PeerTooLarge,
                    PeerReason::Busy => UiMoveReason::PeerBusy,
                    PeerReason::CouldNotKeep => UiMoveReason::PeerCouldNotKeep,
                    PeerReason::Cancelled => UiMoveReason::PeerCancelled,
                    PeerReason::CouldNotFinish => UiMoveReason::PeerCouldNotFinish,
                    PeerReason::Stopped => UiMoveReason::PeerStopped,
                    PeerReason::NothingToFinish => UiMoveReason::PeerNothingToFinish,
                    PeerReason::NotProven => UiMoveReason::PeerNotProven,
                }),
                _ => None,
            };
            if out.reason.is_none() {
                out.detail = match failure {
                    Failure::Interrupted(why)
                    | Failure::Declined(why)
                    | Failure::Later(why)
                    | Failure::Other(why) => Some(why),
                    _ => None,
                };
            }
        }
    }
    out
}

/// Where the folder stands with moves; finishes first a move that was being put in place
/// when the computer stopped.
pub async fn move_state(data_dir: String) -> Result<UiMoveState> {
    use_keychain();
    let dir = PathBuf::from(data_dir);
    let state = blocking(move || transfer::state(&dir)).await?;
    Ok(match state {
        TransferState::None => UiMoveState {
            kind: "none".into(),
            name: String::new(),
            computer: String::new(),
            at_ms: 0,
        },
        TransferState::Moved(m) => UiMoveState {
            kind: match m.state {
                MovedState::Moving => "moving",
                MovedState::Moved => "moved",
            }
            .into(),
            name: m.name,
            computer: m.to,
            at_ms: m.at_ms as i64,
        },
        TransferState::Staged(i) => UiMoveState {
            kind: "staged".into(),
            name: i.name,
            computer: i.from,
            at_ms: 0,
        },
    })
}

/// Why `code` is not a code, as said while it is typed; `None` for one that reads.
#[frb(sync)]
pub fn move_code_problem(code: String) -> Option<UiCodeProblem> {
    transfer::check_code(&code).err().map(|p| match p {
        CodeProblem::NotACode => UiCodeProblem::NotACode,
        CodeProblem::Mistyped => UiCodeProblem::Mistyped,
    })
}

fn options(data_dir: &Path, computer: String) -> TransferOptions {
    let mut options = TransferOptions::new(
        NetOptions {
            bridge: bridge_enabled(data_dir),
            ..NetOptions::default()
        },
        computer,
    );
    // Debug builds take `FOGATA_TEST_LOSE_COMMIT=1`: the computer with the account stops
    // using it and breaks the connection before telling the new one, as if that last
    // message were lost, for tests of finishing such a move.
    options.free_space = Some(crate::disk::free_space);
    options.lose_commit =
        cfg!(debug_assertions) && std::env::var("FOGATA_TEST_LOSE_COMMIT").as_deref() == Ok("1");
    options
}

/// Runs a move, its events going to `sink`.
fn follow(moving: transfer::Transfer, sink: StreamSink<UiMoveEvent>) {
    *CONTROL.lock().unwrap_or_else(|e| e.into_inner()) = Some(moving.controls());
    let mut events = moving.events;
    RUNTIME.spawn(async move {
        while let Some(event) = events.recv().await {
            let end = matches!(event, TransferEvent::Done { .. } | TransferEvent::Failed(_));
            if sink.add(ui_event(event)).is_err() || end {
                break;
            }
        }
    });
}

/// New computer: shows a code and brings the account of the computer where it is typed,
/// replacing the profile of this folder, if any (the screens warn first). No core may run.
pub fn move_receive(
    data_dir: String,
    computer: String,
    sink: StreamSink<UiMoveEvent>,
) -> Result<()> {
    use_keychain();
    let _runtime = RUNTIME.enter();
    let dir = PathBuf::from(data_dir);
    let moving = transfer::receive(dir.clone(), options(&dir, computer))?;
    follow(moving, sink);
    Ok(())
}

/// Old computer: moves this account to the computer showing `code`; with the folder
/// "moving", finishes that move. No core may run.
pub fn move_send(
    data_dir: String,
    code: String,
    computer: String,
    sink: StreamSink<UiMoveEvent>,
) -> Result<()> {
    use_keychain();
    let _runtime = RUNTIME.enter();
    let dir = PathBuf::from(data_dir);
    let moving = transfer::send(dir.clone(), &code, options(&dir, computer))?;
    follow(moving, sink);
    Ok(())
}

/// The old computer's person said yes or no to the move shown.
#[frb(sync)]
pub fn move_confirm(yes: bool) {
    if let Some(control) = &*CONTROL.lock().unwrap_or_else(|e| e.into_inner()) {
        control.confirm(yes);
    }
}

/// Stops the move under way, if any; whether it stops (not once its end began).
#[frb(sync)]
pub fn move_cancel() -> bool {
    let control = CONTROL.lock().unwrap_or_else(|e| e.into_inner()).clone();
    control.is_none_or(|control| control.cancel())
}

/// Old computer, after the move: deletes what is left of the account, and its key.
pub async fn move_forget(data_dir: String) -> Result<()> {
    use_keychain();
    blocking(move || transfer::forget(Path::new(&data_dir))).await?
}

/// Old computer, "moving": uses the account here again (the screens warn first).
pub async fn move_keep(data_dir: String) -> Result<()> {
    use_keychain();
    blocking(move || transfer::keep_this_computer(Path::new(&data_dir))).await?
}

/// New computer: throws away an account that came whole but was not finished; `false`
/// when it was already being put in place, which then finished.
pub async fn move_discard(data_dir: String) -> Result<bool> {
    use_keychain();
    blocking(move || transfer::discard_incoming(Path::new(&data_dir))).await?
}
