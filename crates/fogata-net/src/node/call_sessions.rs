//! The steps of a direct call to a friend whose identity is open on more than one computer at
//! once (see sessions.rs). A ring goes to every session of the friend, and the first one to
//! answer or decline takes the call: the others are told it was taken elsewhere, so they stop
//! ringing and show nothing missed. The friend is busy only when every session rung said so.
//! The caller's later steps (stop ringing, hang up) go to every session as well, while the
//! one called answers the session that rang it. A step counts as undelivered only when no
//! session took it.
//!
//! A device too old to read "taken elsewhere" refuses it and is told the ring stopped
//! instead, which it shows as a missed call.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use fogata_crypto::DevicePublicKey;
use fogata_friends::{CallMessage, CallSignal};
use fogata_proto::FriendRequest;
use libp2p::PeerId;

use super::{Node, friends::FriendPending};
use crate::NodeEvent;

/// How long a call's sessions are remembered: past the ring (40 s at most on the side
/// called), which is when a second session may still answer.
const ROUTE_LIFETIME: Duration = Duration::from_secs(120);

#[derive(Default)]
pub(super) struct CallRoutes {
    /// Calls rung lately, from here or to here, by call.
    routes: HashMap<[u8; 16], Route>,
    /// Steps on their way, by number: one request per session.
    sends: HashMap<u64, Send>,
    next: u64,
}

struct Route {
    device: DevicePublicKey,
    since: Instant,
    /// This device rang.
    outgoing: bool,
    /// The sessions rung from here that may still answer; or the session that rang here.
    peers: HashSet<PeerId>,
    /// The session that answered or declined first, of those rung from here.
    taken_by: Option<PeerId>,
    /// The sessions rung from here that said they are in another call.
    busy: HashSet<PeerId>,
}

impl Route {
    /// Every session rung from here said it is busy.
    fn all_busy(&self) -> bool {
        self.taken_by.is_none() && !self.peers.is_empty() && self.peers.is_subset(&self.busy)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Ring,
    /// "Taken elsewhere", to the sessions that did not take the call.
    Elsewhere,
    /// "Stopped ringing", to an older session that refused "taken elsewhere".
    Stopped,
    /// What nobody waits for: that this device takes group calls, or is still in a call.
    Quiet,
    Other,
}

struct Send {
    to: DevicePublicKey,
    call: [u8; 16],
    step: Step,
    /// Sessions that have not answered yet.
    left: usize,
    delivered: bool,
    /// Every session that did not take it refused it, as an older device does with a step
    /// it cannot read.
    refused: bool,
}

fn step(signal: &CallSignal) -> Step {
    match signal {
        CallSignal::Ring { .. } | CallSignal::Invite { .. } => Step::Ring,
        CallSignal::Elsewhere => Step::Elsewhere,
        CallSignal::GroupCalls | CallSignal::Going { .. } => Step::Quiet,
        _ => Step::Other,
    }
}

impl Node {
    /// Sends a step of a direct call to a friend connected now, sealed to each of its
    /// sessions it goes to.
    pub(super) fn send_call(
        &mut self,
        to: DevicePublicKey,
        message: CallMessage,
    ) -> Result<(), String> {
        if self.friends.is_none() {
            return Err("this node runs without friends".into());
        }
        self.prune_call_routes();
        let sessions = self.friend_peers(&[to]);
        let peers: Vec<PeerId> = match self.call_routes.routes.get(&message.call) {
            // The one called answers the session that rang, while it is connected.
            Some(route)
                if !route.outgoing
                    && route.device == to
                    && route.peers.iter().any(|p| sessions.contains(p)) =>
            {
                route.peers.iter().copied().collect()
            }
            _ => sessions,
        };
        if peers.is_empty() {
            return Err("that friend is not online".into());
        }
        let kind = step(&message.signal);
        if kind == Step::Ring {
            self.call_routes.routes.insert(
                message.call,
                Route {
                    device: to,
                    since: Instant::now(),
                    outgoing: true,
                    peers: peers.iter().copied().collect(),
                    taken_by: None,
                    busy: HashSet::new(),
                },
            );
        }
        self.send_call_to(to, &message, kind, &peers)
    }

    fn send_call_to(
        &mut self,
        to: DevicePublicKey,
        message: &CallMessage,
        kind: Step,
        peers: &[PeerId],
    ) -> Result<(), String> {
        let local = self.local_peer().to_bytes();
        let sealed: Vec<(PeerId, Vec<u8>)> = peers
            .iter()
            .map(|peer| {
                fogata_friends::call_message(&to, message, &local, &peer.to_bytes())
                    .map(|sealed| (*peer, sealed))
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<_, _>>()?;
        let routes = &mut self.call_routes;
        let id = routes.next;
        routes.next += 1;
        routes.sends.insert(
            id,
            Send {
                to,
                call: message.call,
                step: kind,
                left: sealed.len(),
                delivered: false,
                refused: true,
            },
        );
        for (peer, sealed) in sealed {
            self.friend_request(
                peer,
                FriendRequest::Call { sealed },
                FriendPending::Call(id),
            );
        }
        Ok(())
    }

    /// Tells sessions of `device` that another one took `call`. Best effort: nobody waits
    /// for it.
    fn tell_elsewhere(
        &mut self,
        device: DevicePublicKey,
        call: [u8; 16],
        peers: &[PeerId],
        kind: Step,
    ) {
        let signal = if kind == Step::Stopped {
            CallSignal::Cancel
        } else {
            CallSignal::Elsewhere
        };
        let message = CallMessage { call, signal };
        if let Err(e) = self.send_call_to(device, &message, kind, peers) {
            tracing::debug!("cannot tell a session the call was taken elsewhere: {e}");
        }
    }

    /// A step of a direct call from `peer`, a session of the friend `device`: passed on,
    /// unless another session of the friend took the call already.
    pub(super) fn on_call_step(
        &mut self,
        peer: PeerId,
        device: DevicePublicKey,
        message: CallMessage,
    ) {
        // Each step goes to the application; a friend sending them without end would fill
        // its queue (see limits.rs).
        if !self.limits.take_call_step(device, Instant::now()) {
            tracing::debug!(%peer, "too many call steps from a friend");
            return;
        }
        self.prune_call_routes();
        let call = message.call;
        let routes = &mut self.call_routes.routes;
        if let CallSignal::Ring { .. } | CallSignal::Invite { .. } = message.signal {
            if routes.get(&call).is_none_or(|r| r.device == device) {
                routes.insert(
                    call,
                    Route {
                        device,
                        since: Instant::now(),
                        outgoing: false,
                        peers: HashSet::from([peer]),
                        taken_by: None,
                        busy: HashSet::new(),
                    },
                );
            }
            self.emit(NodeEvent::Call {
                from: device,
                message,
            });
            return;
        }
        let ours = routes
            .get_mut(&call)
            .filter(|r| r.outgoing && r.device == device);
        // Whether the step goes on to the core, and which sessions hear the call was taken.
        let (pass, elsewhere): (bool, Vec<PeerId>) = match (&message.signal, ours) {
            (CallSignal::Accept | CallSignal::Decline, Some(route)) => match route.taken_by {
                None => {
                    route.taken_by = Some(peer);
                    let others = route.peers.iter().filter(|p| **p != peer).copied();
                    (true, others.collect())
                }
                Some(first) if first == peer => (true, Vec::new()),
                // Answered late, after another session: that one has the call.
                Some(_) => (false, vec![peer]),
            },
            (CallSignal::Busy, Some(route)) => {
                route.busy.insert(peer);
                // Unless another session took the call, or still rings.
                (route.all_busy(), Vec::new())
            }
            // From a session that did not take the call: not about this one.
            (CallSignal::Cancel | CallSignal::End | CallSignal::Elsewhere, Some(route)) => {
                (route.taken_by.is_none_or(|first| first == peer), Vec::new())
            }
            _ => (true, Vec::new()),
        };
        if !elsewhere.is_empty() {
            self.tell_elsewhere(device, call, &elsewhere, Step::Elsewhere);
        }
        if pass {
            self.emit(NodeEvent::Call {
                from: device,
                message,
            });
        }
    }

    /// A session took the step `id`.
    pub(super) fn call_step_delivered(&mut self, id: u64) {
        let sends = &mut self.call_routes.sends;
        if let Some(send) = sends.get_mut(&id) {
            send.delivered = true;
            send.left = send.left.saturating_sub(1);
            if send.left == 0 {
                sends.remove(&id);
            }
        }
    }

    /// The step `id` did not reach `peer`, or `peer` refused it.
    pub(super) fn call_step_failed(&mut self, id: u64, peer: PeerId, refused: bool) {
        let Some(send) = self.call_routes.sends.get_mut(&id) else {
            return;
        };
        send.left = send.left.saturating_sub(1);
        send.refused &= refused;
        let (to, call, kind, done, delivered, all_refused) = (
            send.to,
            send.call,
            send.step,
            send.left == 0,
            send.delivered,
            send.refused,
        );
        if done {
            self.call_routes.sends.remove(&id);
        }
        match kind {
            // That session will not answer; maybe all those left said they are busy.
            Step::Ring => {
                if let Some(route) = self.call_routes.routes.get_mut(&call) {
                    route.peers.remove(&peer);
                    if route.all_busy() {
                        route.busy.clear();
                        self.emit(NodeEvent::Call {
                            from: to,
                            message: CallMessage {
                                call,
                                signal: CallSignal::Busy,
                            },
                        });
                    }
                }
            }
            Step::Elsewhere if refused => self.tell_elsewhere(to, call, &[peer], Step::Stopped),
            _ => {}
        }
        if done && !delivered && matches!(kind, Step::Ring | Step::Other) {
            self.emit(NodeEvent::CallUndelivered {
                to,
                call,
                refused: all_refused,
            });
        }
    }

    fn prune_call_routes(&mut self) {
        self.call_routes
            .routes
            .retain(|_, r| r.since.elapsed() < ROUTE_LIFETIME);
    }
}
