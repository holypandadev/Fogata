//! This device's room events on their way to the members it sent them to: each is kept
//! until every request carrying it is gone, answered by the member or failed, so that one
//! that failed on its way, or was dropped from a full queue, goes again once, and quitting
//! waits a moment for them (see [`OWN_GOING_WAIT`]). Nothing of it is on the wire: a member
//! takes the copy sent again as any copy of a message it has, by its id.
//!
//! A presence (`RoomHere`) is not kept: another goes at the next heartbeat. Every other room
//! event of this device's is: a line, a file shared or no longer, entering, leaving, a
//! taking out. A line or a leave lost here is never said again: once this device left, its
//! presences no longer name its latest events, and once its node stopped, nothing has them.

use std::{collections::HashMap, time::Duration};

use fogata_crypto::GroupId;
use fogata_proto::{MemberRequest, WireMessage};
use libp2p::{PeerId, swarm::ConnectionId};

use super::{Node, Pending};

/// Most time quitting waits for this device's room events still on their way to go (see
/// `Node::let_own_go` in run.rs):a line said just before quitting, and the leave, are otherwise
/// dropped with the connections when the node stops. A request and its answer take about
/// two round trips, under 1.5 s even through a member's relay over a slow link; a member
/// that does not answer within this has a connection that died unnoticed, and its events
/// are lost as before. A quit with nothing of this device's on its way does not wait.
pub(super) const OWN_GOING_WAIT: Duration = Duration::from_secs(2);

/// One of this device's room events on its way: the message, the devices it named as
/// reached when it went (sent again the same), and how many requests carry it now.
struct Going {
    wire: WireMessage,
    reached: Vec<u8>,
    requests: usize,
}

/// This device's room events on their way, by message id. Each lasts as long as a request
/// that carries it: at most one per member it went to, and one more for each first that
/// failed, so they are bounded as the requests waiting for each member are (see outbox.rs).
#[derive(Default)]
pub(super) struct OwnGoing(HashMap<[u8; 16], Going>);

impl OwnGoing {
    /// Whether none of this device's room events is waiting or on its way.
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Node {
    /// Sends room event `wire` of this device's, message `id` of `group`, to `peers`, naming
    /// `reached` (see spread.rs), and keeps it until each request is gone.
    pub(super) fn send_own(
        &mut self,
        group: GroupId,
        id: [u8; 16],
        peers: &[PeerId],
        wire: WireMessage,
        reached: Vec<u8>,
    ) {
        if peers.is_empty() {
            return;
        }
        self.own_going.0.insert(
            id,
            Going {
                wire,
                reached,
                requests: 0,
            },
        );
        for &peer in peers {
            self.own_request(peer, group, id, false, None);
        }
    }

    /// Sends kept event `id` to `peer`, the first time or `again`, on another connection
    /// than `besides` if it has one.
    fn own_request(
        &mut self,
        peer: PeerId,
        group: GroupId,
        id: [u8; 16],
        again: bool,
        besides: Option<ConnectionId>,
    ) {
        let spread = self.takes_spread(&peer);
        let Some(going) = self.own_going.0.get_mut(&id) else {
            return;
        };
        going.requests += 1;
        let request = if spread {
            MemberRequest::Spread {
                message: going.wire.clone(),
                reached: going.reached.clone(),
            }
        } else {
            MemberRequest::Deliver(going.wire.clone())
        };
        self.request_besides(peer, request, Pending::Own(group, id, again), besides);
    }

    /// A request carrying event `id` is gone: answered, whatever the answer, or failed and
    /// dealt with. The event is let go with the last.
    pub(super) fn own_answered(&mut self, id: [u8; 16]) {
        if let Some(going) = self.own_going.0.get_mut(&id) {
            going.requests = going.requests.saturating_sub(1);
            if going.requests == 0 {
                self.own_going.0.remove(&id);
            }
        }
    }

    /// The request carrying event `id` of `group` to `peer` failed on its way or was
    /// dropped unsent: the first goes again once, on the connection the member is best
    /// reached by now, but `besides`, the one it failed on, while it has another: the
    /// failure may come before that one is known to close. Only while `peer` is still a
    /// connected member of the group. A refusal is an answer, not this: the member took the
    /// request and said no (behind, it asks for the room again later; not counting this
    /// device in, it would refuse again).
    pub(super) fn own_lost(
        &mut self,
        peer: PeerId,
        group: GroupId,
        id: [u8; 16],
        again: bool,
        besides: Option<ConnectionId>,
    ) {
        // Its last connection gone, the requests waiting for it are dropped before it stops
        // counting as a member (see `on_connection_closed`): it is not dialled again here.
        let member = self
            .groups
            .get(&group)
            .is_some_and(|slot| slot.members.contains(&peer));
        let connected = self.connections.values().any(|c| c.peer == peer);
        if !again && member && connected {
            tracing::debug!("a room event of this device's failed on its way; sending it again");
            self.own_request(peer, group, id, true, besides);
        }
        self.own_answered(id);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::{DeviceIdentity, SealedMessage};
    use fogata_proto::{GroupPayload, MemberResponse};
    use fogata_state::{FIRST_ROOM, GroupLog, RoomChange};

    use super::*;
    use crate::{
        GroupSetup, NodeConfig, NodeEvent, NodeEvents, NodeHandle,
        node::{
            now_ms,
            spread::Forward,
            tests::{add_member, inspect, lone_member, through_relay},
        },
        start,
    };

    /// A line of the node's own, sealed with its group's key.
    fn own_line(n: &Node, group: GroupId) -> ([u8; 16], WireMessage) {
        let keys = n.groups[&group].ctx.current().cloned().unwrap();
        let payload = GroupPayload::RoomChat {
            voice: FIRST_ROOM,
            room: [3; 16],
            nick: "ana".into(),
            text: "last word".into(),
        };
        let sealed = SealedMessage::seal(&keys, &n.identity, 1, &payload.encode()).unwrap();
        (sealed.header.message_id, WireMessage::from(&sealed))
    }

    /// The node's own room events on their way to `peer`, sent or waiting: whether each is
    /// the one sent again.
    fn own_requests(n: &Node, peer: PeerId) -> Vec<bool> {
        let sent = n
            .pending
            .values()
            .filter(|(p, _)| *p == peer)
            .map(|(_, kind)| *kind);
        let waiting = n
            .outbox
            .get(&peer)
            .into_iter()
            .flat_map(|o| o.waiting.iter().map(|(_, kind)| *kind));
        sent.chain(waiting)
            .filter_map(|kind| match kind {
                Pending::Own(_, _, again) => Some(again),
                _ => None,
            })
            .collect()
    }

    /// The node's own room event on its way to `peer`, taken out of what waits for an
    /// answer as an answer or a failure takes it.
    fn take_own(n: &mut Node, peer: PeerId) -> Pending {
        let id = n
            .pending
            .iter()
            .find(|(_, (p, kind))| *p == peer && matches!(kind, Pending::Own(..)))
            .map(|(id, _)| *id)
            .unwrap();
        let (to, kind) = n.pending.remove(&id).unwrap();
        n.request_done(to);
        kind
    }

    /// Fails the node's own room event on its way to `peer` as an `OutboundFailure` does.
    fn fail_own(n: &mut Node, peer: PeerId) {
        let kind = take_own(n, peer);
        n.request_dropped(peer, kind);
    }

    /// A room event of this device's that fails on its way to a member still connected goes
    /// again once, and no more; one answered, even with a refusal, or lost to a member no
    /// longer connected, does not; each is let go once nothing carries it.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_own_room_event_lost_on_its_way_goes_again_once() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let (bea, cai, dan) = (PeerId::random(), PeerId::random(), PeerId::random());
            for (i, peer) in [bea, cai].into_iter().enumerate() {
                add_member(n, group, peer);
                through_relay(n, peer, PeerId::random(), i + 1);
            }
            // Dan still counts as a member, with no connection left.
            add_member(n, group, dan);
            let (id, wire) = own_line(n, group);
            n.send_own(group, id, &[bea, cai, dan], wire, Vec::new());
            let sent = [bea, cai, dan].map(|p| own_requests(n, p));
            fail_own(n, bea);
            let again = own_requests(n, bea);
            fail_own(n, bea);
            let after_twice = own_requests(n, bea);
            let kind = take_own(n, cai);
            n.on_response(cai, kind, MemberResponse::Refused);
            fail_own(n, dan);
            let left = [cai, dan].map(|p| own_requests(n, p));
            (sent, again, after_twice, left, n.own_going.is_empty())
        })
        .await;
        let (sent, again, after_twice, left, let_go) = found;
        assert_eq!(sent, [vec![false], vec![false], vec![false]]);
        assert_eq!(again, [true]);
        assert!(after_twice.is_empty());
        assert_eq!(left, [Vec::<bool>::new(), Vec::new()]);
        assert!(let_go);
        node.shutdown().await.unwrap();
    }

    /// A node of `log`'s group, its events kept.
    fn member(identity: Arc<DeviceIdentity>, log: GroupLog) -> (NodeHandle, NodeEvents) {
        start(NodeConfig {
            identity,
            groups: vec![GroupSetup::Member(log)],
            friends: None,
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
            spill_dir: None,
        })
        .unwrap()
    }

    /// A member's line that comes twice, the first copy and the one its author sent again
    /// after the first seemed lost, is kept and shown once: the second is known by its id.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_line_that_comes_again_is_shown_once() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        let group = log.group_id();
        let keys = log.current_keys(&ana).unwrap();
        let (node, mut events) = member(ana, log);
        let seal = |payload: GroupPayload, at: u64| {
            WireMessage::from(&SealedMessage::seal(&keys, &bea, at, &payload.encode()).unwrap())
        };
        let (room, now) = ([3; 16], now_ms());
        let join = seal(
            GroupPayload::RoomJoin {
                voice: FIRST_ROOM,
                room,
                started_ms: now,
                nick: "bea".into(),
            },
            now,
        );
        let line = seal(
            GroupPayload::RoomChat {
                voice: FIRST_ROOM,
                room,
                nick: "bea".into(),
                text: "last word".into(),
            },
            now + 1,
        );
        let lines = inspect(&node, move |n| {
            let from = PeerId::random();
            n.receive(from, join, Forward::Spread(Vec::new()));
            // The first copy, and the one sent again: spread, or delivered to an older device.
            n.receive(from, line.clone(), Forward::Spread(Vec::new()));
            n.receive(from, line.clone(), Forward::Spread(Vec::new()));
            n.receive(from, line, Forward::Flooded);
            n.groups[&group]
                .rooms
                .values()
                .filter_map(|t| t.room())
                .map(|r| r.chat.len())
                .sum::<usize>()
        })
        .await;
        let mut shown = 0;
        while let Ok(event) = events.try_recv() {
            if let NodeEvent::Room {
                change: RoomChange::Chat(_),
                ..
            } = event
            {
                shown += 1;
            }
        }
        assert_eq!((lines, shown), (1, 1));
        node.shutdown().await.unwrap();
    }
}
