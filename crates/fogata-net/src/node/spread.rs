//! Group messages among members: each is taken once, handed to the application and passed
//! on to the members that may not have it yet; log entries that come along count against
//! their author while they are refused.
//!
//! A group message goes from its author to every member the author is connected to, naming
//! them (`Spread`); a member passes it on only to members it is connected to that it was not
//! named for, and of those, leaves out the ones that a member named before it (by an order
//! drawn for each message) said it is connected to (its roster), since that one passes it
//! on. Older devices take only `Deliver`, which they flood to all the members they are
//! connected to; a message from one of them is passed on to the members its sender is not
//! known to be connected to. Something lost on the way is asked for by id: participants
//! name their room's latest events in their presences (see room.rs).
//!
//! Nothing proves whom a message reached: a member could pass a message on first, naming
//! every member, so that those it got to left the others to it. A later copy from another
//! member that names fewer has it passed on to those, once each (see [`PassedOn`]).

use std::{
    collections::{BTreeMap, HashSet},
    time::{Duration, Instant},
};

use fogata_crypto::{DevicePublicKey, GroupId, SealedMessage};
use fogata_proto::{GroupPayload, MemberRequest, SHORT_PEER_BYTES, SPREAD_MARKER, WireMessage};
use libp2p::{PeerId, StreamProtocol};

use super::{Node, Pending, limits, members::group_of};
use crate::short_peer;

/// See [`SPREAD_MARKER`].
pub(super) const SPREAD_PROTOCOL: StreamProtocol = StreamProtocol::new(SPREAD_MARKER);
/// Most devices a spread message names; past that, members pass it on to more members
/// than needed, never to fewer.
const MAX_REACHED: usize = 64;

/// How long a member's roster counts for leaving a member to it: rosters are asked for at
/// every heartbeat.
pub(super) const ROSTER_FRESH: Duration = Duration::from_secs(45);
/// Spread messages whose way on is kept (see [`PassedOn`]), the latest.
const PASSED_ON_KEPT: usize = 1024;

/// A spread message this device passed on: its group, what it is (a hash of the message as
/// sent, so that only the same message goes on again) and the members that have it from or
/// through this device. A later copy of it that names fewer members than the first goes on
/// to those this device would pass it on to now and did not, so that a member naming every
/// member in the copy it sent first does not keep it from the others.
pub(super) struct PassedOn {
    group: GroupId,
    hash: [u8; 32],
    sent: HashSet<PeerId>,
}

/// What a group message is, as sent: a copy with the same id and anything else changed is
/// another message.
fn wire_hash(wire: &WireMessage) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    for part in [
        &wire.group_id,
        &wire.sender,
        &wire.message_id,
        &wire.nonce,
        &wire.ciphertext,
        &wire.signature,
    ] {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    hasher.update(&wire.epoch.to_le_bytes());
    hasher.update(&wire.timestamp_ms.to_le_bytes());
    *hasher.finalize().as_bytes()
}

/// How a group message that arrived goes on.
pub(super) enum Forward {
    /// Not at all: it came among a room's events, asked for.
    No,
    /// From an older device, which floods it to all the members it is connected to.
    Flooded,
    /// Spread, having reached these devices already (short ids, see [`short_id`]).
    Spread(Vec<u8>),
}

/// The short form of a peer id in a spread message: its last bytes, which end its public
/// key or a hash of it.
pub(super) fn short_id(peer: &PeerId) -> [u8; SHORT_PEER_BYTES] {
    let bytes = peer.to_bytes();
    let mut id = [0; SHORT_PEER_BYTES];
    let start = bytes.len().saturating_sub(SHORT_PEER_BYTES);
    id[..bytes.len() - start].copy_from_slice(&bytes[start..]);
    id
}

/// The short ids named in a spread message.
fn short_ids(reached: &[u8]) -> HashSet<[u8; SHORT_PEER_BYTES]> {
    reached
        .as_chunks::<SHORT_PEER_BYTES>()
        .0
        .iter()
        .take(MAX_REACHED)
        .copied()
        .collect()
}

/// A member's place in the order, drawn for each message, in which members named in it
/// pass it on; every member draws the same.
fn rank(short: &[u8; SHORT_PEER_BYTES], message_id: &[u8; 16]) -> u64 {
    let mut first = [0; 8];
    first.copy_from_slice(&message_id[..8]);
    // splitmix64's finaliser: a small change of either input changes the whole order.
    let mut x = u64::from_le_bytes(*short) ^ u64::from_le_bytes(first);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// The members a new spread message goes on to from this device (`me`), which got it from
/// `from`: those of `members` (the ones it is connected to) that `named` does not name,
/// except those that a `rosters` entry lists for a named member that passes it on before
/// this device: one earlier in the message's order, or an older device (the `bool` says
/// whether it takes spread messages), which floods it to everyone it is connected to.
fn spread_to<'a>(
    members: &[PeerId],
    from: PeerId,
    me: PeerId,
    id: &[u8; 16],
    named: &HashSet<[u8; SHORT_PEER_BYTES]>,
    rosters: impl Iterator<Item = (PeerId, &'a HashSet<PeerId>, bool)>,
) -> Vec<PeerId> {
    let mine = rank(&short_id(&me), id);
    let left_to: Vec<&HashSet<PeerId>> = rosters
        .filter(|(member, _, spreads)| {
            *member != me
                && named.contains(&short_id(member))
                && (!spreads || rank(&short_id(member), id) < mine)
        })
        .map(|(_, listed, _)| listed)
        .collect();
    members
        .iter()
        .copied()
        .filter(|p| *p != from && *p != me && !named.contains(&short_id(p)))
        .filter(|p| !left_to.iter().any(|listed| listed.contains(p)))
        .collect()
}

impl Node {
    /// Opens a group message from `from` and applies it; a new one goes on as `forward`
    /// says, so that it reaches the members without a connection to its author.
    pub(super) fn receive(&mut self, from: PeerId, wire: WireMessage, forward: Forward) {
        let Some(group) = group_of(&wire.group_id) else {
            return;
        };
        if wire.ciphertext.len() > limits::MAX_GROUP_MESSAGE_BYTES {
            return;
        }
        let Ok(sealed) = SealedMessage::try_from(&wire) else {
            return;
        };
        let id = sealed.header.message_id;
        if !matches!(forward, Forward::No) && self.seen.contains(&id) {
            if let Forward::Spread(reached) = &forward {
                self.spread_again(from, id, &wire, reached);
            }
            return;
        }
        // A direct chat's group carries no room events.
        let Some(slot) = self.groups.get(&group).filter(|s| !s.direct) else {
            return;
        };
        let Some(keys) = slot.ctx.keys(sealed.header.epoch).cloned() else {
            tracing::debug!("message for an epoch this device cannot read");
            return;
        };
        let payload = match sealed.open(&keys).and_then(|p| GroupPayload::decode(&p)) {
            Ok(payload) => payload,
            Err(e) => {
                tracing::debug!("dropping a message from {from}: {e}");
                return;
            }
        };
        // Only members may speak in the group, even with a key they still hold.
        if !slot.ctx.is_member_device(&sealed.header.sender) {
            return;
        }
        // Each member writes only so much, wherever it comes from: past that its messages are
        // neither shown nor passed on, so one member cannot have the others carry a flood.
        // Events asked for again (a room's past) are not counted.
        if !matches!(forward, Forward::No)
            && !self.limits.take_author_message(
                sealed.header.sender,
                wire.ciphertext.len(),
                Instant::now(),
            )
        {
            tracing::debug!("a member wrote more group messages than this device takes");
            return;
        }
        // Remembered only now that it opened and verified: a copy that fails the checks must
        // not stop the real message from being applied later. A copy among a room's events
        // is applied again even if it was seen: when it first came, its sender may not have
        // been known to be in the room yet.
        let fresh = self.remember(id);
        let onward = match (&forward, fresh) {
            (Forward::No, _) | (_, false) => None,
            (Forward::Flooded, true) => Some(self.after_flood(group, from)),
            (Forward::Spread(reached), true) => Some(self.pass_on(group, from, &id, reached)),
        };
        // The latest events a participant names, checked on a presence that just came (one
        // among a room's events may be long past).
        let recent = match &payload {
            GroupPayload::RoomHere { recent, .. } if onward.is_some() => recent.clone(),
            _ => Vec::new(),
        };
        self.apply_room(group, &sealed, payload, wire.clone(), Some(from));
        if !recent.is_empty() {
            self.ask_missing(group, sealed.header.sender, from, &recent);
        }
        if matches!(forward, Forward::Spread(_)) && onward.is_some() {
            let sent = onward.iter().flat_map(|(peers, _)| peers.iter().copied());
            self.keep_passed_on(
                id,
                group,
                &wire,
                std::iter::once(from).chain(sent).collect(),
            );
        }
        if let Some((peers, reached)) = onward
            && !peers.is_empty()
        {
            tracing::trace!(count = peers.len(), "passing a group message on");
            self.send_group_message(&peers, &wire, &reached);
        }
    }

    /// Keeps how spread message `id` went on from here (see [`PassedOn`]).
    fn keep_passed_on(
        &mut self,
        id: [u8; 16],
        group: GroupId,
        wire: &WireMessage,
        sent: HashSet<PeerId>,
    ) {
        let hash = wire_hash(wire);
        if self
            .passed_on
            .insert(id, PassedOn { group, hash, sent })
            .is_none()
        {
            self.passed_on_order.push_back(id);
        }
        while self.passed_on_order.len() > PASSED_ON_KEPT {
            if let Some(old) = self.passed_on_order.pop_front() {
                self.passed_on.remove(&old);
            }
        }
    }

    /// Another copy of spread message `id`, already passed on, from `from`, that reached
    /// `reached`: it goes on to the members it would go to from this copy that it did not
    /// go to from here yet, once each. Copies from honest members name at least those the
    /// first named, so nothing more goes; one whose first copy named members it did not
    /// reach no longer keeps it from them.
    fn spread_again(&mut self, from: PeerId, id: [u8; 16], wire: &WireMessage, reached: &[u8]) {
        let Some(kept) = self.passed_on.get_mut(&id) else {
            return;
        };
        if kept.hash != wire_hash(wire) {
            return;
        }
        kept.sent.insert(from);
        let group = kept.group;
        let (peers, reached) = self.pass_on(group, from, &id, reached);
        let Some(kept) = self.passed_on.get_mut(&id) else {
            return;
        };
        let peers: Vec<PeerId> = peers.into_iter().filter(|p| kept.sent.insert(*p)).collect();
        if !peers.is_empty() {
            tracing::debug!(
                count = peers.len(),
                "a later copy of a group message named fewer members; passing it on to them"
            );
            self.send_group_message(&peers, wire, &reached);
        }
    }

    /// Whether a group message handed over by `peer` is opened now: not too long, and within
    /// what one connection may hand this device (see limits.rs).
    pub(super) fn take_group_message(&mut self, peer: PeerId, wire: &WireMessage) -> bool {
        let bytes = wire.ciphertext.len();
        bytes <= limits::MAX_GROUP_MESSAGE_BYTES
            && self.limits.take_peer_message(peer, bytes, Instant::now())
    }

    /// Keeps of `entries` from `peer` as many as it may make this device check now (see
    /// limits.rs), the first ones: entries go in the common order, so those kept never wait
    /// for those left out, which come with a later sync.
    pub(super) fn within_entry_budget(
        &mut self,
        peer: PeerId,
        mut entries: Vec<Vec<u8>>,
    ) -> Vec<Vec<u8>> {
        let sender = self.entry_sender(peer);
        let taken = self.limits.take_entries(sender, &entries, Instant::now());
        if taken < entries.len() {
            tracing::debug!(
                "{} sent {} log entries; {taken} checked now",
                short_peer(&peer),
                entries.len()
            );
            entries.truncate(taken);
        }
        entries
    }

    /// Who `peer`'s entries are charged to: its device once it is known.
    fn entry_sender(&self, peer: PeerId) -> limits::Sender {
        self.members
            .get(&peer)
            .map_or(limits::Sender::Peer(peer), |m| {
                limits::Sender::Device(m.device)
            })
    }

    /// Charges the authors of entries judged and refused (see [`fogata_state::Taken`]),
    /// whoever sent them: an honest member passes on entries that wait for their parents
    /// without judging them, and an author that sent its entries ahead of their parent
    /// pays for them once the parent comes, from anyone.
    pub(super) fn refused_entries(&mut self, refused: &BTreeMap<DevicePublicKey, usize>) {
        let now = Instant::now();
        for (author, count) in refused {
            (self.limits).refused_entries(limits::Sender::Device(*author), *count, now);
        }
    }

    /// Sends a group message of this device to every connected member of `group`.
    pub(super) fn deliver(&mut self, group: GroupId, wire: WireMessage) {
        let peers: Vec<PeerId> = self
            .groups
            .get(&group)
            .map(|s| s.members.iter().copied().collect())
            .unwrap_or_default();
        let reached = self.reached_with(&[], &peers);
        self.send_group_message(&peers, &wire, &reached);
    }

    /// Sends a group message to `peers`: spread, naming `reached`, to those that take it,
    /// and as before to older devices, which flood it on.
    fn send_group_message(&mut self, peers: &[PeerId], wire: &WireMessage, reached: &[u8]) {
        for &peer in peers {
            let request = if self.takes_spread(&peer) {
                MemberRequest::Spread {
                    message: wire.clone(),
                    reached: reached.to_vec(),
                }
            } else {
                MemberRequest::Deliver(wire.clone())
            };
            self.request(peer, request, Pending::Deliver);
        }
    }

    /// Whether `peer` takes [`MemberRequest::Spread`] (it said so when it connected).
    pub(super) fn takes_spread(&self, peer: &PeerId) -> bool {
        crate::locked(&self.advertised)
            .get(peer)
            .is_some_and(|set| set.contains(&SPREAD_PROTOCOL))
    }

    /// The devices a message reached, named in `earlier`, with this device and `peers`, the
    /// members it is sending it to now. A member whose connection went silent is left out:
    /// it may not get it from here, and others then pass it on to it.
    fn reached_with(&self, earlier: &[u8], peers: &[PeerId]) -> Vec<u8> {
        let mut named = Vec::new();
        let mut reached = Vec::new();
        let mine = std::iter::once(self.local_peer());
        let now = peers
            .iter()
            .copied()
            .filter(|p| !self.silent.contains_key(p));
        // Of those named before, the first that could be kept: a list of one id again and
        // again would otherwise be read whole.
        let ids = earlier
            .as_chunks::<SHORT_PEER_BYTES>()
            .0
            .iter()
            .take(MAX_REACHED)
            .copied()
            .chain(mine.chain(now).map(|p| short_id(&p)));
        for id in ids {
            if named.len() >= MAX_REACHED {
                break;
            }
            if !named.contains(&id) {
                named.push(id);
                reached.extend_from_slice(&id);
            }
        }
        reached
    }

    /// Where a new spread message from `from`, that reached `reached` already, goes on
    /// (see [`spread_to`]), and the devices to name when sending it: those named before, and
    /// this device's members, the ones left to others among them.
    fn pass_on(
        &self,
        group: GroupId,
        from: PeerId,
        id: &[u8; 16],
        reached: &[u8],
    ) -> (Vec<PeerId>, Vec<u8>) {
        let Some(slot) = self.groups.get(&group) else {
            return (Vec::new(), Vec::new());
        };
        let members: Vec<PeerId> = slot.members.iter().copied().collect();
        let rosters = slot
            .rosters
            .iter()
            .filter(|(member, (_, at))| {
                at.elapsed() < ROSTER_FRESH && slot.members.contains(member)
            })
            .map(|(member, (listed, _))| (*member, listed, self.takes_spread(member)));
        let peers = spread_to(
            &members,
            from,
            self.local_peer(),
            id,
            &short_ids(reached),
            rosters,
        );
        if peers.is_empty() {
            return (peers, Vec::new());
        }
        let reached = self.reached_with(reached, &members);
        (peers, reached)
    }

    /// Where a new message flooded by an older device, `from`, goes on: to the members this
    /// device is connected to that `from` is not known to be connected to, since it sends to
    /// all of those. And the devices to name when sending it.
    fn after_flood(&self, group: GroupId, from: PeerId) -> (Vec<PeerId>, Vec<u8>) {
        let Some(slot) = self.groups.get(&group) else {
            return (Vec::new(), Vec::new());
        };
        let listed = slot
            .rosters
            .get(&from)
            .filter(|(_, at)| at.elapsed() < ROSTER_FRESH)
            .map(|(listed, _)| listed);
        let members: Vec<PeerId> = slot.members.iter().copied().collect();
        let peers: Vec<PeerId> = members
            .iter()
            .copied()
            .filter(|p| *p != from && !listed.is_some_and(|l| l.contains(p)))
            .collect();
        let mut reached = self.reached_with(&short_id(&from), &members);
        if peers.is_empty() {
            reached.clear();
        }
        (peers, reached)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::DeviceIdentity;
    use fogata_proto::{MemberResponse, SharedBytes};
    use fogata_state::{Body, Entry, GroupLog};
    use libp2p::Multiaddr;

    use super::*;
    use crate::{
        NodeConfig, Path,
        group::GroupSetup,
        node::{Member, start, tests::inspect},
    };

    fn peers(n: usize) -> Vec<PeerId> {
        (0..n).map(|_| PeerId::random()).collect()
    }

    fn named(list: &[PeerId]) -> HashSet<[u8; SHORT_PEER_BYTES]> {
        list.iter().map(short_id).collect()
    }

    /// Everyone connected to everyone: whoever gets it from its author passes it on to
    /// nobody.
    #[test]
    fn a_message_to_everyone_goes_no_further() {
        let all = peers(6);
        let author = all[0];
        for &me in &all[1..] {
            let members: Vec<PeerId> = all.iter().copied().filter(|p| *p != me).collect();
            let to = spread_to(
                &members,
                author,
                me,
                &[1; 16],
                &named(&all),
                std::iter::empty(),
            );
            assert!(to.is_empty());
        }
    }

    /// The author is not connected to `far`, which three members it sent to are connected
    /// to: exactly one of them passes it on, whatever the message, and without the others'
    /// rosters each one does.
    #[test]
    fn one_member_passes_a_message_on_to_one_its_author_misses() {
        let all = peers(5);
        let (author, far, near) = (all[0], all[4], &all[1..4]);
        let reached = named(&all[..4]);
        let rosters: Vec<(PeerId, HashSet<PeerId>)> = near
            .iter()
            .map(|p| (*p, all.iter().copied().filter(|q| q != p).collect()))
            .collect();
        for id in 0..50u8 {
            let id = [id; 16];
            let mut senders = 0;
            for &me in near {
                let members: Vec<PeerId> = all.iter().copied().filter(|p| *p != me).collect();
                let others = rosters
                    .iter()
                    .filter(|(p, _)| *p != me)
                    .map(|(p, listed)| (*p, listed, true));
                let to = spread_to(&members, author, me, &id, &reached, others);
                assert!(to.is_empty() || to == [far]);
                senders += to.len();
                let alone = spread_to(&members, author, me, &id, &reached, std::iter::empty());
                assert_eq!(alone, [far]);
            }
            assert_eq!(senders, 1, "passed on by one member");
        }
    }

    /// An older device floods every message it gets to everyone it is connected to: the
    /// members it is connected to are left to it.
    #[test]
    fn members_an_older_device_floods_to_are_left_to_it() {
        let all = peers(4);
        let (author, me, older, far) = (all[0], all[1], all[2], all[3]);
        let listed: HashSet<PeerId> = [author, me, far].into();
        let members = [author, older, far];
        let reached = named(&[author, me, older]);
        let to = spread_to(
            &members,
            author,
            me,
            &[0; 16],
            &reached,
            [(older, &listed, false)].into_iter(),
        );
        assert!(to.is_empty());
        // A member not named, whose roster does not say, is not trusted to pass it on.
        let to = spread_to(
            &members,
            author,
            me,
            &[0; 16],
            &named(&[author, me]),
            [(older, &listed, false)].into_iter(),
        );
        assert_eq!(to.len(), 2);
    }

    #[test]
    fn short_ids_are_the_end_of_the_peer_id() {
        let peer = PeerId::random();
        let bytes = peer.to_bytes();
        assert_eq!(short_id(&peer), bytes[bytes.len() - SHORT_PEER_BYTES..]);
        let mut many = Vec::new();
        for p in peers(MAX_REACHED + 10) {
            many.extend_from_slice(&short_id(&p));
        }
        many.push(1);
        assert_eq!(short_ids(&many).len(), MAX_REACHED);
    }

    /// Leo sends renames he could not make ahead of their parent, his leave, then the leave
    /// alone: he pays for each once it comes. Marta does the same, but Bea passes on her
    /// leave, then her renames again: Marta pays for them, each time, and Bea, who passed
    /// them on, only for their place.
    /// A member that passes a message on first, naming every member, keeps it from none: a
    /// later copy from another member, naming only those it reached, has this device pass
    /// it on to the rest, once; a copy that is not the same message has nothing go on.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_first_copy_naming_everyone_keeps_the_message_from_nobody() {
        use crate::node::tests::add_member;
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "Bea", 2).unwrap();
        let group = log.group_id();
        let (node, _events) = start(NodeConfig {
            identity: ana,
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
        })
        .unwrap();
        let found = inspect(&node, move |n| {
            let (liar, honest, cai, dan) = (
                PeerId::random(),
                PeerId::random(),
                PeerId::random(),
                PeerId::random(),
            );
            for peer in [liar, honest, cai, dan] {
                add_member(n, group, peer);
                crate::locked(&n.advertised).insert(peer, HashSet::from([SPREAD_PROTOCOL]));
            }
            let keys = n.groups[&group].ctx.current().cloned().unwrap();
            let payload = GroupPayload::RoomLeave {
                voice: fogata_state::FIRST_ROOM,
                room: [3; 16],
            };
            let sealed =
                SealedMessage::seal(&keys, &bea, crate::node::now_ms(), &payload.encode()).unwrap();
            let wire = WireMessage::from(&sealed);
            let ids = |list: &[PeerId]| list.iter().flat_map(short_id).collect::<Vec<u8>>();
            let deliveries = |n: &Node, peer: PeerId| {
                n.pending
                    .values()
                    .filter(|(p, k)| *p == peer && matches!(k, Pending::Deliver))
                    .count()
            };
            n.receive(
                liar,
                wire.clone(),
                Forward::Spread(ids(&[liar, honest, cai, dan, n.local_peer()])),
            );
            let first = (deliveries(n, cai), deliveries(n, dan));
            // Another message under the same id: nothing goes on.
            let mut other = wire.clone();
            other.ciphertext[0] ^= 1;
            n.receive(
                honest,
                other,
                Forward::Spread(ids(&[honest, n.local_peer()])),
            );
            let forged = (deliveries(n, cai), deliveries(n, dan));
            let reached = ids(&[honest, cai, n.local_peer()]);
            n.receive(honest, wire.clone(), Forward::Spread(reached.clone()));
            let later = (deliveries(n, cai), deliveries(n, dan));
            n.receive(honest, wire, Forward::Spread(reached));
            let again = (deliveries(n, cai), deliveries(n, dan), deliveries(n, liar));
            (first, forged, later, again)
        })
        .await;
        assert_eq!(found, ((0, 0), (0, 0), (0, 1), (0, 1, 0)));
        node.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn entries_sent_ahead_of_their_parent_cost_their_author_once_refused() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let [leo, marta, bea] = [(); 3].map(|_| DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, leo.public_key(), "Leo", 2).unwrap();
        log.add(&ana, marta.public_key(), "Marta", 3).unwrap();
        log.add(&ana, bea.public_key(), "Bea", 4).unwrap();
        let group = log.group_id();
        // A member leaves, and renames the group on that, which a member that left cannot.
        let round = |who: &DeviceIdentity, at: u64| {
            let leave = Entry::sign(who, group, log.heads(), at, Body::Leave);
            let ahead: Vec<Entry> = (0..8)
                .map(|i| {
                    let body = Body::Rename {
                        name: format!("x{i}"),
                    };
                    Entry::sign(who, group, vec![leave.hash()], at + 1 + i, body)
                })
                .collect();
            (leave, ahead)
        };
        let (first, second) = (round(&leo, 10), round(&marta, 20));
        let [leo, marta, bea] = [leo, marta, bea].map(|d| d.public_key());
        let (node, _events) = start(NodeConfig {
            identity: ana,
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
        })
        .unwrap();
        let spent = inspect(&node, move |n| {
            let member = |n: &mut Node, device| {
                let peer = PeerId::random();
                let address: Multiaddr = "/ip4/192.168.1.30/udp/4001/quic-v1".parse().unwrap();
                n.members.insert(
                    peer,
                    Member {
                        device,
                        path: Path::Lan,
                        address,
                        told: Path::Lan,
                        worse_since: None,
                    },
                );
                n.groups.get_mut(&group).unwrap().members.insert(peer);
                peer
            };
            let push = |n: &mut Node, peer, entries: &[Entry]| {
                let entries = entries.iter().map(|e| SharedBytes::from(e.encode()));
                let request = MemberRequest::Push {
                    group_id: group.as_bytes().to_vec(),
                    entries: entries.collect(),
                };
                assert!(matches!(n.on_request(peer, request), MemberResponse::Ack));
            };
            let left = |n: &Node, device| {
                let tokens = n.limits.entry_tokens(limits::Sender::Device(device));
                // Out of a burst of 4096.
                4096.0 - tokens.expect("a budget")
            };
            let [at_leo, at_marta, at_bea] = [leo, marta, bea].map(|d| member(n, d));
            let (leave, ahead) = first;
            push(n, at_leo, &ahead);
            push(n, at_leo, &[leave]);
            let (leave, ahead) = second;
            push(n, at_marta, &ahead);
            push(n, at_bea, &[leave]);
            push(n, at_bea, &ahead);
            (left(n, leo), left(n, marta), left(n, bea), ahead.len())
        })
        .await;
        let (leo_spent, marta_spent, bea_spent, n) = spent;
        let near = |spent: f64, cost: usize| (spent - cost as f64).abs() < 4.0;
        // Their place, and 64 for each refused.
        assert!(near(leo_spent, n + 1 + 64 * n), "Leo spent {leo_spent}");
        assert!(
            near(marta_spent, n + 64 * 2 * n),
            "Marta spent {marta_spent}"
        );
        assert!(near(bea_spent, 1 + n), "Bea spent {bea_spent}");
        node.shutdown().await.unwrap();
    }
}
