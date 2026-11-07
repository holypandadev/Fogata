//! Finding friends on the DHT: the secrets this device announces and looks up under at each
//! slot, and the peers announced under them taken as friends, links or codes to dial.

use std::{collections::HashSet, time::Instant};

use libp2p::{Multiaddr, PeerId, kad::RecordKey};

use super::{
    LOOKUP_CONNECTED, LOOKUP_IDLE, LOOKUP_NOBODY_MISSING, Node, PUNCH_SLOT,
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
        }
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
                PUNCH_SLOT
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
}
