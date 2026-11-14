//! Finding friends on the DHT: the secrets this device announces and looks up under at each
//! slot, and the peers announced under them taken as friends, links or codes to dial.

use std::{collections::HashSet, time::Instant};

use libp2p::{Multiaddr, PeerId, kad::RecordKey};

use super::{
    LOOKUP_CONNECTED, LOOKUP_IDLE, LOOKUP_NOBODY_MISSING, Node, SEARCH_AT_START, SEARCH_PACE,
    friends::{FriendContext, Target},
    now_ms,
};
use crate::{NodeEvent, lan, rendezvous};

/// A former friend that has not been reached in this long is no longer looked for.
const DEPARTING_LIFETIME_MS: u64 = 90 * 24 * 60 * 60 * 1000;

/// Whether `peer`, announced under a link used here, may be the device that made it. The link
/// names that device's transport identity of the session it was made in, and the link only
/// lasts that long; anyone else who saw the link may announce there too, to take the request
/// (and learn where this device is) or to answer it with a refusal in the maker's name.
fn may_own_link(target: Target, link_peer: Option<PeerId>, peer: PeerId) -> bool {
    target != Target::LinkOwner || link_peer.is_none_or(|p| p == peer)
}

impl Node {
    /// Rendezvous secrets to announce: this device's own, and those of the link or code it
    /// shows.
    /// Looks friends up again at the next slot.
    pub(super) fn friends_look_again(&mut self) {
        if let Some(f) = self.friends.as_mut() {
            f.last_lookup = None;
            f.look_for_the_away_again();
        }
    }

    /// This device announced for the first time: the groups it found nobody of, and its
    /// friends, are looked up at the next slot and at every slot for [`SEARCH_AT_START`],
    /// rather than once and then at the next lookup, half a minute or more away. Those who
    /// started at about the same time announced a moment ago too, after the lookups made at
    /// the start found nobody, and their record may not be found yet at the first look.
    pub(super) fn look_again_from_the_start(&mut self) {
        let until = Instant::now() + SEARCH_AT_START;
        let later = |old: Option<Instant>| Some(old.map_or(until, |old| old.max(until)));
        for slot in self.groups.values_mut() {
            if slot.members.is_empty() {
                slot.last_lookup = None;
                slot.searching_until = later(slot.searching_until);
            }
        }
        if let Some(f) = self.friends.as_mut() {
            f.searching_until = later(f.searching_until);
        }
        self.friends_look_again();
        self.seek_again();
    }

    /// This device's friend rendezvous secret, under which it seals its local addresses for
    /// its friends, and its friends' own, which open theirs.
    pub(super) fn friend_lan_secrets(&self) -> (Option<[u8; 32]>, Vec<[u8; 32]>) {
        let Some(f) = &self.friends else {
            return (None, Vec::new());
        };
        let theirs = f
            .book
            .friends
            .values()
            .map(|friend| friend.secret.rendezvous())
            .collect();
        (Some(f.book.secret.rendezvous()), theirs)
    }

    /// Whether a friend link or code is shown or used here (see `FriendContext::inviting`).
    pub(super) fn friends_inviting(&self) -> bool {
        self.friends.as_ref().is_some_and(FriendContext::inviting)
    }

    pub(super) fn friend_announce_secrets(&mut self) -> Vec<[u8; 32]> {
        let Some(f) = self.friends.as_mut() else {
            return Vec::new();
        };
        let mut out = vec![f.book.secret.rendezvous()];
        out.extend(f.extra_announcements(&self.identity));
        out
    }

    pub(super) fn friend_slot(&mut self, now_secs: u64) {
        let now = now_ms();
        let forgot;
        let mut notes = Vec::new();
        let mut failures = Vec::new();
        let lookups = {
            let Some(f) = self.friends.as_mut() else {
                return;
            };
            if f.link.as_ref().is_some_and(|l| l.is_expired(now)) {
                f.link = None;
                notes.push("the friend link expired");
            }
            if f.code.as_ref().is_some_and(|c| now > c.expires_ms) {
                f.code = None;
                f.inviting.clear();
                notes.push("the friend code expired");
            }
            if f.sought_link.as_ref().is_some_and(|l| l.is_expired(now)) {
                f.sought_link = None;
                failures.push("the link expired before its device could be reached");
            }
            if f.sought_code.as_ref().is_some_and(|c| now > c.expires_ms) {
                f.sought_code = None;
                failures.push(
                    "no device answered the code; check it and that the other device still shows it",
                );
            }
            let departing = f.book.departing.len();
            f.book
                .departing
                .retain(|_, d| now.saturating_sub(d.since_ms) < DEPARTING_LIFETIME_MS);
            if f.book.departing.len() != departing {
                notes.push("stopped looking for a former friend that was never reached");
            }
            if f.book.forget_old_requests(now) {
                notes.push("stopped waiting for friend requests that had no answer in a month");
            }
            let forgot_removals = f.book.forget_old_removals(now);
            forgot = f.book.forget_previous(now) | forgot_removals;
            let interval = if f.searching_until.is_some_and(|t| Instant::now() < t) {
                SEARCH_PACE
            } else if f.in_a_hurry() {
                LOOKUP_IDLE
            } else if f.nobody_missing() {
                LOOKUP_NOBODY_MISSING
            } else {
                LOOKUP_CONNECTED
            };
            let due = f.last_lookup.is_none_or(|t| t.elapsed() >= interval);
            if due && self.swarm.behaviour().kad.is_enabled() {
                f.last_lookup = Some(Instant::now());
                f.keys.clear();
                f.forget_pairs();
                for (secret, target, proof) in f.lookups(&self.identity) {
                    for key in rendezvous::lookup_keys(&[secret], now_secs) {
                        f.keys.insert(key, (target, proof));
                    }
                }
                f.keys.keys().cloned().collect()
            } else {
                Vec::new()
            }
        };
        if notes.iter().any(|n| n.starts_with("stopped")) {
            self.changed();
        }
        if forgot {
            self.changed();
        }
        for note in notes {
            self.info(note);
        }
        for reason in failures {
            self.emit(NodeEvent::FriendRequestFailed(reason.to_string()));
        }
        if !lookups.is_empty() {
            self.lookup_dials.clear();
            for key in lookups {
                self.look_up(key);
            }
        }
    }

    pub(super) fn friend_target(&self, key: &RecordKey) -> Option<(Target, Option<[u8; 32]>)> {
        self.friends.as_ref()?.keys.get(key).copied()
    }

    /// The addresses `peer` gave in the announcement a DHT lookup running now found, its
    /// sealed names among them: those the lookup would dial.
    fn announced_addresses(&mut self, peer: PeerId) -> Vec<Multiaddr> {
        use libp2p::{
            core::Endpoint,
            swarm::{ConnectionId, NetworkBehaviour},
        };
        let Some(kad) = self.swarm.behaviour_mut().kad.as_mut() else {
            return Vec::new();
        };
        kad.handle_pending_outbound_connection(
            ConnectionId::new_unchecked(0),
            Some(peer),
            &[],
            Endpoint::Dialer,
        )
        .unwrap_or_default()
    }

    /// Peers found under a friend's rendezvous secret (`proof` is that secret when it is the
    /// friend's own) are dialled as `target`. Under a friend's own secret, only a peer whose
    /// announcement proves it is that friend: any other friend of it can announce there,
    /// and would learn this device's address and transport identity from the dial.
    pub(super) fn on_friend_providers(
        &mut self,
        key: &RecordKey,
        target: Target,
        proof: Option<[u8; 32]>,
        providers: HashSet<PeerId>,
    ) {
        let local = self.local_peer();
        let mut room = super::candidates::MAX_NEW_PER_ANSWER;
        let link_peer = self
            .friends
            .as_ref()
            .and_then(|f| f.sought_link.as_ref())
            .and_then(|link| PeerId::from_bytes(&link.peer).ok());
        let window = fogata_crypto::window_at(now_ms() / 1000);
        for peer in providers {
            if peer == local || !may_own_link(target, link_peer, peer) {
                continue;
            }
            if let (Some(secret), Target::Friend(device) | Target::Departing(device)) =
                (proof, target)
            {
                let names = self.announced_addresses(peer);
                if !lan::proven(&names, &device, &secret, &peer, window) {
                    tracing::debug!(
                        %peer,
                        addresses = names.len(),
                        names = names.iter().filter(|a| lan::is_name(a)).count(),
                        "{} is announced by a peer that does not prove it is that friend",
                        device.fingerprint()
                    );
                    continue;
                }
            }
            // As many made-up peers as a DHT node likes can be named here too.
            if !self.take_named(
                peer,
                super::candidates::Budget::Friend(target),
                key,
                &mut room,
            ) {
                continue;
            }
            if let Some(f) = self.friends.as_mut() {
                // A peer already recognised as something more specific keeps that.
                let known = f.targets.get(&peer).copied();
                if !(matches!(target, Target::Requester | Target::Accepting) && known.is_some()) {
                    f.targets.insert(peer, target);
                }
            }
            self.candidates.entry(peer).or_default();
            if self.swarm.is_connected(&peer) {
                self.contact(peer);
            } else if !self.resting(&peer) && !self.lookup_dials.contains(&peer) {
                self.dial_found(peer);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use libp2p::PeerId;

    use super::*;
    use crate::node::friends::Target;

    #[test]
    fn only_the_device_that_made_a_link_is_asked_with_it() {
        let (maker, other) = (PeerId::random(), PeerId::random());
        assert!(may_own_link(Target::LinkOwner, Some(maker), maker));
        assert!(!may_own_link(Target::LinkOwner, Some(maker), other));
        // A link without the maker's identity is found by the DHT alone.
        assert!(may_own_link(Target::LinkOwner, None, other));
        assert!(may_own_link(Target::Accepting, Some(maker), other));
    }

    /// After its first announcement a device looks up a group it found nobody of at the
    /// next slot and at the two after it, and not a group it has someone of; a longer search
    /// under way is not cut short.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_first_announcement_is_followed_by_a_look_at_every_slot_for_a_while() {
        use std::time::Duration;

        use crate::node::{
            PUNCH_SLOT, SEARCH_HARD,
            tests::{inspect, lone_member},
        };
        let (node, group) = lone_member();
        let (before, due, pace, window, longer, with_someone) = inspect(&node, move |n| {
            // The lookup made at the start found nobody.
            let slot = n.groups.get_mut(&group).unwrap();
            slot.last_lookup = Some(Instant::now());
            let before = slot.lookup_interval(0);
            n.look_again_from_the_start();
            let slot = n.groups.get_mut(&group).unwrap();
            let due = slot.last_lookup.is_none();
            let pace = slot.lookup_interval(0);
            let window = slot.searching_until.unwrap() - Instant::now();
            // A search for someone who just went lasts longer: it stays.
            slot.searching_until = Some(Instant::now() + SEARCH_HARD);
            n.look_again_from_the_start();
            let slot = n.groups.get_mut(&group).unwrap();
            let longer = slot.searching_until.unwrap() - Instant::now();
            // With a member there, the group is looked up as it was.
            slot.searching_until = None;
            slot.last_lookup = Some(Instant::now());
            slot.members.insert(PeerId::random());
            n.look_again_from_the_start();
            let slot = &n.groups[&group];
            let with_someone = slot.last_lookup.is_some() && slot.searching_until.is_none();
            (before, due, pace, window, longer, with_someone)
        })
        .await;
        assert!(before > PUNCH_SLOT, "{before:?}");
        assert!(due, "no lookup at the next slot");
        // Under a slot's length, so the lookup of each slot is due at the next one.
        assert!(pace < PUNCH_SLOT, "{pace:?}");
        // The slot that follows and the two after it, and no more than one beyond.
        assert!(
            window > 3 * PUNCH_SLOT - Duration::from_secs(1),
            "{window:?}"
        );
        assert!(
            window <= 3 * PUNCH_SLOT + Duration::from_secs(1),
            "{window:?}"
        );
        assert!(longer > SEARCH_HARD - Duration::from_secs(1), "{longer:?}");
        assert!(with_someone);
        node.shutdown().await.unwrap();
    }
}
