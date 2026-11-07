//! Proofs of membership: a peer that may be a member is challenged with the groups it may
//! share, a challenge names a group only by tags, and a proof checked lets the peer in, as
//! a member of each group it proved, with what it said before kept or dropped.

use std::{sync::Arc, time::Instant};

use fogata_crypto::{
    DevicePublicKey, GroupId, GroupKeys, MembershipProof, SealedProof, group_tag,
    group_tag_matches, new_challenge,
};
use fogata_proto::{MemberRequest, SharedBytes, WireProof};
use libp2p::PeerId;

use super::{GroupSlot, Member, Node, Pending, members::bytes};
use crate::NodeEvent;

/// Most group tags in a challenge, one per key epoch.
pub(super) const MAX_TAGS: usize = 4;

impl Node {
    pub(super) fn is_member_of(&self, group: &GroupId, peer: &PeerId) -> bool {
        self.groups
            .get(group)
            .is_some_and(|s| s.members.contains(peer))
    }

    /// Asks a connected peer to prove membership of `only`, or of the groups it may belong
    /// to: those it was found for on the DHT, or every group when nothing says which. A
    /// joining group shows its invite instead.
    pub(super) fn approach(&mut self, peer: PeerId, only: Option<GroupId>) {
        let groups: Vec<GroupId> = match (only, self.hinted.get(&peer)) {
            (Some(group), _) => vec![group],
            (None, Some(hinted)) => hinted.iter().copied().collect(),
            // A direct chat's group is only asked about on its friend's connection.
            (None, None) => self
                .groups
                .iter()
                .filter(|(_, s)| !s.direct)
                .map(|(id, _)| *id)
                .collect(),
        };
        for group in groups {
            let Some(slot) = self.groups.get(&group) else {
                continue;
            };
            if slot.ctx.joining().is_some() {
                // The invite names the group and the manager who wrote it: it goes only to
                // peers found for this group (on the DHT or in the invite), not to whoever
                // shares the network.
                if self.hinted.get(&peer).is_some_and(|h| h.contains(&group)) {
                    self.send_join(peer, group);
                }
            } else {
                self.challenge(peer, group);
            }
        }
    }

    pub(super) fn challenge(&mut self, peer: PeerId, group: GroupId) {
        self.challenge_with(peer, group, None);
    }

    /// [`Self::challenge`], also naming the group under key epoch `also`: the one a member
    /// that missed many rotations asked with, which may be older than the newest few.
    pub(super) fn challenge_with(&mut self, peer: PeerId, group: GroupId, also: Option<u32>) {
        let own = self.is_own_session(&peer);
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        // One naming an older epoch goes even while another is pending: the pending one
        // may name only epochs the peer does not know.
        if slot.members.contains(&peer)
            || own
            || (slot.challenges.contains_key(&peer) && also.is_none())
            || !slot.ctx.can_challenge()
        {
            return;
        }
        self.send_challenge(peer, group, also);
    }

    /// Asks `peer` to prove membership of `group`, named only by tags its members recognise:
    /// one for each of the newest key epochs, and one for `also`.
    fn send_challenge(&mut self, peer: PeerId, group: GroupId, also: Option<u32>) {
        let local = self.local_peer().to_bytes();
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let Ok(challenge) = new_challenge() else {
            return;
        };
        let mut epochs: Vec<u32> = slot.ctx.known_epochs().into_iter().take(MAX_TAGS).collect();
        if let Some(also) = also.filter(|n| !epochs.contains(n)) {
            epochs.push(also);
        }
        let tags = epochs
            .into_iter()
            .filter_map(|n| slot.ctx.keys(n))
            .map(|keys| group_tag(keys, &local, &peer.to_bytes(), &challenge))
            .map(|tag| serde_bytes::ByteBuf::from(tag.to_vec()))
            .collect();
        slot.challenges.insert(peer, challenge);
        let request = MemberRequest::Challenge {
            challenge: challenge.to_vec(),
            tags,
        };
        self.request(peer, request, Pending::Challenge(group, challenge));
    }

    /// The group and key epoch a challenge's tags name, if one of ours: the newest the asker
    /// knows that this device knows too. Each group's newest epochs come first. The older
    /// ones only a member that missed several key changes still names, while anyone's
    /// challenge that names nothing makes this device try them all: those are tried for a
    /// peer that is not a member yet only now and then (see limits.rs).
    pub(super) fn named_group(
        &mut self,
        peer: &PeerId,
        challenge: &[u8; 32],
        tags: &[serde_bytes::ByteBuf],
    ) -> Option<(GroupId, Arc<GroupKeys>)> {
        let (asker, local) = (peer.to_bytes(), self.local_peer().to_bytes());
        let named = |groups: &std::collections::BTreeMap<GroupId, GroupSlot>, all: bool| {
            tags.iter().take(MAX_TAGS + 1).find_map(|tag| {
                groups.iter().find_map(|(group, slot)| {
                    let matches = |keys: &&Arc<GroupKeys>| {
                        group_tag_matches(keys, &asker, &local, challenge, tag)
                    };
                    let found = if all {
                        slot.ctx.epoch_keys().find(matches)
                    } else {
                        slot.ctx.newest_epoch_keys(MAX_TAGS).find(matches)
                    };
                    found.map(|keys| (*group, keys.clone()))
                })
            })
        };
        if let Some(found) = named(&self.groups, false) {
            return Some(found);
        }
        if !self.members.contains_key(peer) {
            let vouch = self.search_vouch(peer);
            if !self
                .limits
                .take_deep_search(*peer, self.peer_ip(peer), vouch, Instant::now())
            {
                return None;
            }
        }
        named(&self.groups, true)
    }

    /// A device that started again comes back under a new transport identity, while the
    /// old one may linger until its connections time out. Messages for the device (call
    /// signals among them) must go to the new one, so the old session is forgotten and its
    /// connections closed: at once, or once it was asked whether it is still there, which
    /// only another copy of the identity is (see sessions.rs).
    fn forget_older_sessions(&mut self, device: DevicePublicKey, current: PeerId) {
        let older: Vec<PeerId> = self
            .members
            .iter()
            .filter(|(peer, m)| **peer != current && m.device == device)
            .map(|(peer, _)| *peer)
            .collect();
        for &peer in &older {
            tracing::debug!(%peer, "older session of a member forgotten");
            let groups: Vec<GroupId> = self
                .groups
                .iter()
                .filter(|(_, slot)| slot.members.contains(&peer))
                .map(|(group, _)| *group)
                .collect();
            for group in groups {
                self.drop_member(group, peer);
            }
        }
        self.check_sessions(device, older, current, true);
    }

    pub(super) fn on_proof(
        &mut self,
        peer: PeerId,
        group: GroupId,
        challenge: [u8; 32],
        wire: &WireProof,
    ) {
        self.check_proof(peer, group, challenge, wire);
        self.drop_early_unless_asked(peer);
    }

    fn check_proof(&mut self, peer: PeerId, group: GroupId, challenge: [u8; 32], wire: &WireProof) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        slot.forget_challenge(&peer, &challenge);
        let local = self.swarm.local_peer_id().to_bytes();
        let verified = SealedProof::try_from(wire).and_then(|sealed| {
            let keys = slot
                .ctx
                .keys(sealed.epoch)
                .ok_or(fogata_crypto::CryptoError::WrongEpoch(sealed.epoch))?;
            MembershipProof::open(&sealed, keys, &peer.to_bytes(), &local, &challenge)
        });
        let device = match verified {
            Ok(device) => device,
            Err(e) => {
                // Not a member of this group (a decoy), or a damaged proof.
                tracing::debug!("{peer} gave no valid membership proof: {e}");
                return;
            }
        };
        if device == self.identity.public_key() {
            // Another live session of this same device: the identity is open on another
            // computer too. Nothing to talk about, but the person should know.
            self.own_session_proved(peer);
            return;
        }
        // Knowing a group key is not enough: the device must be in the signed member list.
        if !slot.ctx.is_member_device(&device) {
            tracing::debug!("{peer} proved a key but is not a member");
            // A removed device is sent the signed log up to its removal, once, so it learns
            // why it is refused and nothing that happened after.
            let entries = slot.ctx.until_removed(&device);
            if !entries.is_empty() && slot.told_removed.insert(device) {
                let entries = entries.into_iter().map(SharedBytes::from).collect();
                self.request(
                    peer,
                    MemberRequest::Push {
                        group_id: group.as_bytes().to_vec(),
                        entries,
                    },
                    Pending::Push,
                );
            }
            return;
        }
        if slot.members.contains(&peer) {
            // Asked again only so that it asks us again (see `held_back`).
            return;
        }
        let Some(best) = self.best_connection(&peer) else {
            return;
        };
        self.forget_older_sessions(device, peer);
        // Addresses, profile and presence go once, to the first regular group's member;
        // a direct chat's group only syncs its log.
        let direct = self.groups.get(&group).is_some_and(|s| s.direct);
        let new_peer = !direct && !self.in_regular_group(&peer);
        let slot = self.groups.get_mut(&group).expect("checked above");
        slot.members.insert(peer);
        let held_back = slot.held_back.remove(&peer);
        let have = slot.ctx.hashes();
        let member = self.members.entry(peer).or_insert(Member {
            device,
            path: best.path.clone(),
            address: best.address.clone(),
            told: best.path.clone(),
            worse_since: None,
        });
        // Told now, whatever was told for its other groups.
        member.told = member.path.clone();
        member.worse_since = None;
        let (path, address) = (member.path.clone(), member.address.clone());
        self.note_reached(&best, device);
        self.publish_members();
        self.show_protocols(peer);
        self.note_level(peer);
        self.tell_key_problem(group);
        self.candidates.entry(peer).or_default();
        self.emit(NodeEvent::MemberConnected {
            group,
            device,
            peer,
            path: self.path_in(&group, &path),
            address,
        });
        self.name_carrier(group, peer);
        // What it said while this device was still checking it.
        if let Some(early) = self.early.remove(&peer) {
            if let Some((hidden, away)) = early.presence {
                self.on_request(peer, MemberRequest::Presence { hidden, away });
            }
            if let Some((profile, animation)) = early.profile {
                self.on_request(peer, MemberRequest::Profile { profile, animation });
            }
        }
        if direct {
            self.request(
                peer,
                MemberRequest::Sync {
                    group_id: group.as_bytes().to_vec(),
                    have: bytes(have),
                },
                Pending::Sync(group),
            );
            if held_back {
                self.ask_again(peer, group, wire.epoch);
            }
            return;
        }

        self.reserve_slot(peer);
        self.want_datagrams(peer);
        if new_peer {
            let addresses = self
                .lan_addresses()
                .iter()
                .map(|a| serde_bytes::ByteBuf::from(a.to_vec()))
                .collect();
            self.request(
                peer,
                MemberRequest::Addresses { addresses },
                Pending::Addresses,
            );
        }
        // With the banner's and picture's digests, so that a banner and a picture chosen
        // while the two were apart cross now.
        let banner = self
            .groups
            .get(&group)
            .map(|slot| serde_bytes::ByteBuf::from(slot.ctx.banner_digest()));
        let picture = self
            .groups
            .get(&group)
            .map(|slot| serde_bytes::ByteBuf::from(slot.ctx.picture_digest()));
        self.request(
            peer,
            MemberRequest::Roster {
                group_id: group.as_bytes().to_vec(),
                heads: None,
                banner,
                picture,
            },
            Pending::Roster(group),
        );
        if new_peer && let Some(profile) = self.own_profile() {
            let request = MemberRequest::Profile {
                profile: profile.encode(),
                animation: self.own_photo_animation().map(serde_bytes::ByteBuf::from),
            };
            self.request(peer, request, Pending::Profile);
        }
        if new_peer {
            self.send_presence(peer);
        }
        // Exchange log entries so both ends know the same members and keys.
        self.request(
            peer,
            MemberRequest::Sync {
                group_id: group.as_bytes().to_vec(),
                have: bytes(have),
            },
            Pending::Sync(group),
        );
        // Learn about a room that started before this device came online, and tell the
        // member at once if this device is in one.
        self.ask_room(group, peer);
        self.say_here(group);
        if held_back {
            self.ask_again(peer, group, wire.epoch);
        }
    }

    /// Challenges a member that has proved itself, so that it challenges us in turn: its
    /// own challenge got nothing while it had not (see `held_back`). Named also under the
    /// epoch it proved with, which may be the only one it knows.
    fn ask_again(&mut self, peer: PeerId, group: GroupId, epoch: u32) {
        self.send_challenge(peer, group, Some(epoch));
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    use fogata_crypto::DeviceIdentity;
    use fogata_state::GroupLog;
    use libp2p::PeerId;

    use crate::{
        NodeConfig,
        group::GroupSetup,
        node::{Node, start, tests::inspect},
    };

    /// What a challenge from a peer that is no member costs this device, with 50 groups of
    /// one key epoch and of 300: one that names none of the newest keys (as any stranger's
    /// can), and one that makes this device try every key it has (a few each second at
    /// most, from all strangers together; see limits.rs). Run by hand, in a release build:
    /// `cargo test --release -p fogata-net --lib strangers_challenges_cost -- --ignored
    /// --nocapture`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn strangers_challenges_cost() {
        use fogata_proto::MemberRequest;
        for epochs in [1u64, 300] {
            let ana = Arc::new(DeviceIdentity::generate().unwrap());
            let groups: Vec<GroupSetup> = (0..50)
                .map(|i| {
                    let mut log = GroupLog::create(&ana, &format!("g{i}"), 1).unwrap();
                    for k in 1..epochs {
                        log.rotate(&ana, 1 + k).unwrap();
                    }
                    GroupSetup::Member(log)
                })
                .collect();
            let (node, _events) = start(NodeConfig {
                identity: ana,
                groups,
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
            let (newest, every) = inspect(&node, |n| {
                let challenge = |n: &mut Node, peer: PeerId| {
                    let tags = (0..5)
                        .map(|_| serde_bytes::ByteBuf::from(vec![7u8; 32]))
                        .collect();
                    let request = MemberRequest::Challenge {
                        challenge: vec![1; 32],
                        tags,
                    };
                    n.serving = Some(peer);
                    let at = Instant::now();
                    n.on_request(peer, request);
                    n.serving = None;
                    at.elapsed()
                };
                let median = |mut times: Vec<Duration>| {
                    times.sort();
                    times[times.len() / 2]
                };
                // Each new peer makes this device try every key, while the budget of all
                // strangers together lasts.
                let every: Vec<Duration> =
                    (0..16).map(|_| challenge(n, PeerId::random())).collect();
                // Past it, only the newest.
                let peer = PeerId::random();
                let newest: Vec<Duration> = (0..400).map(|_| challenge(n, peer)).collect();
                (median(newest), median(every))
            })
            .await;
            println!(
                "50 groups of {epochs} epochs: {newest:?} per challenge from a stranger, \
                 {every:?} per one that tries every key"
            );
            node.shutdown().await.unwrap();
        }
    }
}
