//! One identity open on two computers at once, as when a data folder is copied to another
//! computer: the others see one device key with two sessions and reach one or the other.
//! Both copies are told, so the person closes one.
//!
//! The transport identity is new every time a device starts, so two sessions of one device
//! key that are both alive at once can only be two copies; the folder lock keeps a second
//! session off the same folder. A session that ended (a restart) is not alive, and so never
//! counts:
//! - a copy that proves its membership to the other names this device's own key, which only
//!   a live session can do (the proof is fresh and bound to both transport identities);
//! - a member or friend that sees a new session of a device asks the older one, while it is
//!   still connected, whether it is there ([`MemberRequest::OtherSession`]); only a live
//!   older session answers, and then the newer one is told too.
//!
//! Only sessions that proved themselves to be the device are told, so nobody learns
//! anything about a device it could not already see.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use fogata_crypto::DevicePublicKey;
use fogata_proto::{MemberRequest, MemberResponse};
use libp2p::PeerId;

use super::{Node, Pending};
use crate::{NodeEvent, short_peer};

/// The person is told at most once in this long, and the sessions of a device seen here
/// at most once too.
const QUIET: Duration = Duration::from_secs(10 * 60);
/// Longer than a member request may take to be answered.
const ASKING: Duration = Duration::from_secs(60);

/// What a [`MemberRequest::OtherSession`] sent from here was for.
#[derive(Clone, Copy, Debug)]
pub(super) enum SessionCheck {
    /// Asks an older session whether it is still there, now that `newer` proved to be
    /// the same device; `close` drops its connection afterwards (a member's).
    Older { newer: PeerId, close: bool },
    /// Tells the newer session, after the older one answered.
    Newer { again: bool },
}

#[derive(Default)]
pub(super) struct Sessions {
    /// When the person here was last told of another session.
    noticed: Option<Instant>,
    /// When the sessions of a member or friend were last told of each other.
    told: HashMap<DevicePublicKey, Instant>,
    /// Devices whose older sessions are being asked now, since when. A question that never
    /// went (its connection closed first) stops counting after [`ASKING`].
    asking: HashMap<DevicePublicKey, Instant>,
    /// Newer sessions that did not take the word, maybe because they had not verified this
    /// device yet: told once more at the next heartbeat.
    again: HashSet<PeerId>,
    /// Connected sessions of this device's own identity: not challenged again, or the two
    /// would answer each other's challenges with challenges for as long as they are
    /// connected.
    own: HashSet<PeerId>,
}

impl Node {
    /// `peer` proved itself to be this very device: another copy of it.
    pub(super) fn own_session_proved(&mut self, peer: PeerId) {
        if self.sessions.own.insert(peer) {
            self.other_session_seen(peer);
        }
    }

    pub(super) fn is_own_session(&self, peer: &PeerId) -> bool {
        self.sessions.own.contains(peer)
    }

    /// `peer` has no connection left.
    pub(super) fn session_gone(&mut self, peer: &PeerId) {
        self.sessions.own.remove(peer);
        self.sessions.again.remove(peer);
    }

    /// Another live session of this device: it proved itself here, or a member or friend
    /// saw it. The person is told, at most once in a while.
    pub(super) fn other_session_seen(&mut self, peer: PeerId) {
        tracing::warn!(
            "this identity is open on another computer too (seen through {})",
            short_peer(&peer)
        );
        if self.sessions.noticed.is_some_and(|t| t.elapsed() < QUIET) {
            return;
        }
        self.sessions.noticed = Some(Instant::now());
        self.emit(NodeEvent::OtherSession);
    }

    /// A member or friend's session `newer` just proved to be `device`, which the connected
    /// `older` ones proved too. Each older one is asked whether it is still there; a
    /// restart's old session cannot answer. Members' older sessions are disconnected after
    /// (`close`), as before.
    pub(super) fn check_sessions(
        &mut self,
        device: DevicePublicKey,
        older: Vec<PeerId>,
        newer: PeerId,
        close: bool,
    ) {
        let quiet = self
            .sessions
            .told
            .get(&device)
            .is_some_and(|t| t.elapsed() < QUIET);
        let asking = self
            .sessions
            .asking
            .get(&device)
            .is_some_and(|t| t.elapsed() < ASKING);
        for peer in older {
            if quiet || asking || !self.swarm.is_connected(&peer) {
                if close {
                    let _ = self.swarm.disconnect_peer_id(peer);
                }
                continue;
            }
            self.sessions.asking.insert(device, Instant::now());
            self.request(
                peer,
                MemberRequest::OtherSession,
                Pending::OtherSession(device, SessionCheck::Older { newer, close }),
            );
        }
    }

    /// A member or friend says another session of this device is live.
    pub(super) fn on_other_session(&mut self, peer: PeerId) -> MemberResponse {
        if !self.members.contains_key(&peer) && !self.is_friend_peer(&peer) {
            return MemberResponse::Refused;
        }
        self.other_session_seen(peer);
        MemberResponse::Ack
    }

    /// The answer to a [`MemberRequest::OtherSession`], or its failure (`answered` false).
    pub(super) fn on_other_session_answer(
        &mut self,
        peer: PeerId,
        device: DevicePublicKey,
        check: SessionCheck,
        answered: bool,
    ) {
        match check {
            SessionCheck::Older { newer, close } => {
                self.sessions.asking.remove(&device);
                if close {
                    let _ = self.swarm.disconnect_peer_id(peer);
                }
                if !answered {
                    return;
                }
                tracing::info!(
                    "two sessions of one device are live: {} and {}",
                    short_peer(&peer),
                    short_peer(&newer)
                );
                self.sessions.told.retain(|_, t| t.elapsed() < QUIET);
                self.sessions.told.insert(device, Instant::now());
                self.tell_newer(newer, device, false);
            }
            SessionCheck::Newer { again } => {
                if !answered && !again {
                    self.sessions.again.insert(peer);
                }
            }
        }
    }

    fn tell_newer(&mut self, newer: PeerId, device: DevicePublicKey, again: bool) {
        if self.swarm.is_connected(&newer) {
            self.request(
                newer,
                MemberRequest::OtherSession,
                Pending::OtherSession(device, SessionCheck::Newer { again }),
            );
        }
    }

    /// Tells newer sessions once more that did not take the word the first time.
    pub(super) fn sessions_heartbeat(&mut self) {
        let again: Vec<PeerId> = self.sessions.again.drain().collect();
        for peer in again {
            let device = self
                .members
                .get(&peer)
                .map(|m| m.device)
                .or_else(|| self.friend_device(&peer));
            if let Some(device) = device {
                self.tell_newer(peer, device, true);
            }
        }
    }
}
