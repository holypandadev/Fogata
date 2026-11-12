//! What the application asks of the node through its handle: groups and calls to run or
//! drop, dials, reports, signals, and how this device appears (offline, away); what is about
//! rooms or friends goes to those.

use std::time::Instant;

use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_proto::MemberRequest;
use libp2p::{PeerId, multiaddr::Protocol};

use super::{GroupSlot, Node, Pending, group_changes, now_ms};
use crate::{Command, addr::Reach, group::GroupSetup};

/// Whether friends and members see a device as away (see [`Node::shown_away`]).
fn shown_away(away: bool, hidden: bool, in_room: bool) -> bool {
    away && !hidden && !in_room
}

impl Node {
    /// Starts running a group, or adds what a copy of its log brings to one already running.
    pub(super) fn add_group(&mut self, setup: GroupSetup) {
        let slot = GroupSlot::new(self.identity.clone(), setup);
        let group = slot.ctx.group_id();
        if let Some(existing) = self.groups.get_mut(&group) {
            let new = existing.ctx.merge(slot.ctx.log());
            self.after_group_change(group, new, None);
            return;
        }
        self.groups.insert(group, slot);
        // Announce and look for it at the next slot, and try the peers already connected.
        self.provided.1 = None;
        let peers: Vec<PeerId> = self
            .interesting
            .iter()
            .filter(|p| self.swarm.is_connected(p))
            .copied()
            .collect();
        for peer in peers {
            self.approach(peer, Some(group));
        }
        // Friends in the group who are online get it now.
        self.group_changed_for_friends(group);
    }

    /// Runs a direct call's group (see [`NodeHandle::add_call`](crate::NodeHandle::add_call)).
    fn add_call(&mut self, log: fogata_state::GroupLog) {
        let group = log.group_id();
        let me = self.identity.public_key();
        let others: Vec<DevicePublicKey> = log
            .state()
            .members
            .keys()
            .filter(|d| **d != me)
            .copied()
            .collect();
        if !self.groups.contains_key(&group) {
            let mut slot = GroupSlot::new(self.identity.clone(), GroupSetup::Member(log));
            slot.call = true;
            self.groups.insert(group, slot);
            self.provided.1 = None;
        }
        for peer in self.friend_peers(&others) {
            self.approach(peer, Some(group));
        }
    }

    /// Stops running a group without leaving it: its members are let go and it is no longer
    /// announced.
    pub(super) fn drop_group(&mut self, group: GroupId) {
        let Some(slot) = self.groups.get(&group) else {
            return;
        };
        let peers: Vec<PeerId> = slot.members.iter().copied().collect();
        let call = slot.call;
        let served = crate::locked(&self.relay_peers).clone();
        for peer in &peers {
            self.drop_member(group, *peer);
        }
        self.groups.remove(&group);
        self.publish_members();
        self.provided.1 = None;
        // A group this device left or was removed from, an invite that expired: its members
        // the relay no longer serves lose what it gave them, as a member removed from a group
        // does (see `let_go`). When a call ends, a few minutes later: its people are friends
        // of someone in it, and taking the relay back closes their connections, which a call
        // they go back into needs; they are no longer served from now.
        let still = crate::locked(&self.relay_peers).clone();
        let gone = peers
            .into_iter()
            .filter(|p| served.contains(p) && !still.contains(p));
        if call {
            let at = Instant::now() + group_changes::CALL_GRACE;
            self.after_call.extend(gone.map(|p| (p, at)));
        } else {
            for peer in gone.collect::<Vec<_>>() {
                self.let_go(peer);
            }
        }
    }

    pub(super) fn on_command(&mut self, command: Command) {
        match command {
            command @ (Command::JoinRoom { .. }
            | Command::LeaveRoom(..)
            | Command::RoomChat { .. }
            | Command::Room(..)
            | Command::Rooms(..)
            | Command::ShareFile { .. }
            | Command::UnshareFile { .. }
            | Command::RemoveFromRoom { .. }) => self.on_room_command(command),
            Command::AddGroup(setup) => self.add_group(setup),
            Command::AnswerJoin {
                group,
                device,
                accept,
            } => self.answer_join(group, device, accept),
            Command::Dial {
                mut addrs,
                group,
                named,
            } => {
                if named {
                    let own = self.own_networks();
                    addrs.retain(|a| own.may_dial(a, Reach::SamePlace));
                }
                if let Some(group) = group {
                    let mut peers: Vec<PeerId> = Vec::new();
                    for address in &addrs {
                        if let Some(Protocol::P2p(peer)) = address.iter().last()
                            && !peers.contains(&peer)
                        {
                            peers.push(peer);
                        }
                    }
                    for peer in peers {
                        self.hinted.entry(peer).or_default().insert(group);
                        // Already connected another way: no new connection will ask.
                        if self.swarm.is_connected(&peer) {
                            self.approach(peer, Some(group));
                        }
                    }
                }
                self.dial_addresses(addrs);
            }
            Command::MergeGroup(log) => {
                let group = log.group_id();
                match self.groups.get_mut(&group) {
                    Some(slot) => {
                        let new = slot.ctx.merge(&log);
                        let banner = log
                            .banner_record()
                            .is_some_and(|b| slot.ctx.take_banner(b.clone()));
                        let picture = log
                            .picture_record()
                            .is_some_and(|p| slot.ctx.take_picture(p.clone()));
                        self.after_group_change(group, new, None);
                        if banner {
                            self.banner_changed(group, None);
                        }
                        if picture {
                            self.picture_changed(group, None);
                        }
                    }
                    None => self.add_group(GroupSetup::Member(log)),
                }
            }
            Command::Groups(reply) => {
                let summaries = self
                    .groups
                    .values()
                    .map(|s| crate::GroupSummary::of(s.ctx.log()))
                    .collect();
                let _ = reply.send(summaries);
            }
            Command::Group(group, reply) => {
                let _ = reply.send(self.groups.get(&group).map(|s| s.ctx.log().clone()));
            }
            Command::Members(group, reply) => {
                let list = self
                    .groups
                    .get(&group)
                    .map(|slot| {
                        slot.members
                            .iter()
                            .filter_map(|p| self.members.get(p))
                            .map(|m| (m.device, m.path.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                let _ = reply.send(list);
            }
            Command::Report(reply) => {
                let _ = reply.send(self.network_report());
            }
            Command::MemberPeer(device, reply) => {
                let peer = self
                    .members
                    .iter()
                    .find(|(_, m)| m.device == device)
                    .map(|(p, _)| *p);
                let _ = reply.send(peer);
            }
            Command::Addresses(reply) => self.answer_addresses(reply),
            Command::MemberPeers(reply) => {
                let _ = reply.send(self.members.keys().copied().collect());
            }
            Command::Signal { group, to, payload } => {
                let peer = self
                    .members
                    .iter()
                    .find(|(_, m)| m.device == to)
                    .map(|(p, _)| *p);
                match peer {
                    Some(peer) if self.is_member_of(&group, &peer) => {
                        let group_id = group.as_bytes().to_vec();
                        self.request(
                            peer,
                            MemberRequest::Signal { group_id, payload },
                            Pending::Signal,
                        );
                    }
                    _ => tracing::debug!("call signal dropped: that member is not connected"),
                }
            }
            Command::SetHidden(hidden) => self.set_hidden(hidden),
            Command::SetAway(away, reply) => {
                self.set_away(away);
                let _ = reply.send(self.shown_away());
            }
            Command::RetryNow => self.retry_now(),
            Command::Seek(secret, lan) => self.seek(secret, lan),
            Command::MoveCandidates(reply) => {
                let _ = reply.send(self.move_candidates());
            }
            Command::PeerPath(peer, reply) => {
                let _ = reply.send(self.best_connection(&peer).map(|c| c.path));
            }
            Command::Trusted(peer, reply) => {
                let device = self
                    .members
                    .get(&peer)
                    .map(|m| m.device)
                    .or_else(|| self.friend_device(&peer));
                let _ = reply.send(device);
            }
            Command::AddCall(log) => self.add_call(log),
            Command::AddDirect(log) => self.add_direct(log),
            Command::SendDirect {
                group,
                to,
                payloads,
                reply,
            } => {
                let _ = reply.send(self.send_direct(group, to, payloads));
            }
            Command::DropGroup(group) => self.drop_group(group),
            Command::SetGroupBanner {
                group,
                picture,
                reply,
            } => {
                let result = match self.groups.get_mut(&group) {
                    Some(slot) => slot.ctx.set_banner(picture, now_ms()),
                    None => Err("no such group here".into()),
                };
                if result.is_ok() {
                    self.banner_changed(group, None);
                }
                let _ = reply.send(result);
            }
            Command::SetGroupPicture {
                group,
                picture,
                reply,
            } => {
                let result = match self.groups.get_mut(&group) {
                    Some(slot) => slot.ctx.set_picture(picture, now_ms()),
                    None => Err("no such group here".into()),
                };
                let result = result.map(|new| {
                    self.after_group_change(group, new, None);
                    self.picture_changed(group, None);
                });
                let _ = reply.send(result);
            }
            Command::GiveEntries {
                to,
                entries,
                declined,
            } => self.give_entries(&to, entries, declined),
            Command::Typing { group, typing } => self.send_typing(group, typing),
            other => self.on_friend_command(other),
        }
    }

    /// Forgets which peers failed and looks everyone up again at once: what failed before
    /// (on another network, or behind a firewall that now lets the port in) may work now.
    pub(super) fn retry_now(&mut self) {
        self.failures.clear();
        self.lookup_dials.clear();
        self.provided.1 = None;
        for slot in self.groups.values_mut() {
            slot.last_lookup = None;
            slot.quiet_lookups = 0;
        }
        self.friends_look_again();
        self.dial_remembered();
    }

    fn set_hidden(&mut self, hidden: bool) {
        if self.hidden == hidden {
            return;
        }
        if hidden {
            self.stop_typing_everywhere();
        }
        self.hidden = hidden;
        self.send_status_to_friends();
        self.send_presence_to_members();
    }

    /// Whether friends and members see this device as away: not while it appears offline,
    /// which says nothing more, nor while it is in a room or a call, where someone may be
    /// listening without touching the computer.
    pub(crate) fn shown_away(&self) -> bool {
        shown_away(self.away, self.hidden, self.in_a_room().is_some())
    }

    fn set_away(&mut self, away: bool) {
        if self.away == away {
            return;
        }
        let before = self.shown_away();
        self.away = away;
        if self.shown_away() != before {
            self.send_status_to_friends();
            self.send_presence_to_members();
        }
    }

    /// Joining or leaving a room changes whether friends and members see this device as
    /// away; friends get the status in any case.
    pub(crate) fn tell_room_changed(&mut self, in_room_before: Option<GroupId>) {
        let in_room = self.in_a_room();
        if in_room == in_room_before {
            return;
        }
        self.send_status_to_friends();
        if self.away
            && shown_away(self.away, self.hidden, in_room_before.is_some()) != self.shown_away()
        {
            self.send_presence_to_members();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use fogata_crypto::DeviceIdentity;
    use fogata_state::GroupLog;
    use libp2p::Multiaddr;

    use super::*;
    use crate::{
        Path,
        node::{
            Member,
            tests::{add_member, inspect, lone_member},
        },
    };

    /// An invite's maker chose its addresses: those of this computer, of this device itself
    /// and local ones of a VPN this device is not on are left out, and their peers are not
    /// asked about the group; the others are dialled. Addresses given by hand (a test's, a
    /// peer on this computer) are dialled as they are.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_invite_is_dialled_only_at_addresses_this_device_may_dial() {
        let (node, group) = lone_member();
        let (invited, by_hand) = inspect(&node, move |n| {
            n.listen_addrs = vec!["/ip4/192.168.1.20/udp/31000/quic-v1".parse().unwrap()];
            let named = |n: &mut Node, named: bool| {
                let at: Vec<(PeerId, Multiaddr)> = [
                    "/ip4/198.51.100.7/udp/31000/quic-v1",
                    "/ip4/192.168.2.123/udp/31000/quic-v1",
                    "/ip4/127.0.0.1/udp/22/quic-v1",
                    "/ip4/100.70.8.9/udp/80/quic-v1",
                    "/ip4/192.168.1.20/udp/31000/quic-v1",
                ]
                .iter()
                .map(|a| {
                    let peer = PeerId::random();
                    let a: Multiaddr = a.parse().unwrap();
                    (peer, a.with(Protocol::P2p(peer)))
                })
                .collect();
                n.on_command(Command::Dial {
                    addrs: at.iter().map(|(_, a)| a.clone()).collect(),
                    group: Some(group),
                    named,
                });
                at.iter()
                    .map(|(peer, _)| {
                        n.hinted.contains_key(peer) && n.explicit_peers.contains_key(peer)
                    })
                    .collect::<Vec<bool>>()
            };
            (named(n, true), named(n, false))
        })
        .await;
        assert_eq!(invited, [true, true, false, false, false]);
        assert_eq!(by_hand, [true; 5]);
        node.shutdown().await.unwrap();
    }

    #[test]
    fn away_shows_only_outside_rooms_and_while_visible() {
        assert!(shown_away(true, false, false));
        assert!(!shown_away(false, false, false));
        // Someone in a room or a call may just be listening.
        assert!(!shown_away(true, false, true));
        // Appearing offline wins, and says nothing more.
        assert!(!shown_away(true, true, false));
        assert!(!shown_away(true, true, true));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_left_only_in_a_chat_loses_the_relay_when_its_group_goes() {
        let (node, group) = lone_member();
        let (served_before, served_after, user_after) = inspect(&node, move |n| {
            let bea = PeerId::random();
            n.members.insert(
                bea,
                Member {
                    device: DeviceIdentity::generate().unwrap().public_key(),
                    path: Path::Direct,
                    address: "/ip4/198.51.100.7/udp/4001/quic-v1".parse().unwrap(),
                    told: Path::Direct,
                    worse_since: None,
                },
            );
            n.groups.get_mut(&group).unwrap().members.insert(bea);
            // A direct chat with Bea too: the relay never serves that.
            let chat = GroupLog::create(&n.identity, "chat", 2).unwrap();
            let chat_id = chat.group_id();
            let mut slot = GroupSlot::new(n.identity.clone(), GroupSetup::Member(chat));
            slot.direct = true;
            slot.members.insert(bea);
            n.groups.insert(chat_id, slot);
            n.publish_members();
            let served_before = crate::locked(&n.relay_peers).contains(&bea);
            n.relay_users.insert(bea);
            // The group goes (this device left it, say): Bea is still here through the chat.
            n.drop_group(group);
            let served_after = crate::locked(&n.relay_peers).contains(&bea);
            (served_before, served_after, n.relay_users.contains(&bea))
        })
        .await;
        assert!(served_before);
        assert!(!served_after);
        assert!(!user_after, "its slot and ways through were kept");
        node.shutdown().await.unwrap();
    }

    /// The people of a call that ended here keep their slot and ways through this device's
    /// relay a few minutes, to go back into the call, and then lose them, unless the relay
    /// serves them again by then.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_people_of_a_call_that_ended_lose_the_relay_a_few_minutes_later() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let (bea, leo) = (PeerId::random(), PeerId::random());
            // Both in a call, and in a direct chat each, which the relay never serves.
            let call = GroupLog::create(&n.identity, "call", 2).unwrap();
            let call_id = call.group_id();
            let mut slot = GroupSlot::new(n.identity.clone(), GroupSetup::Member(call));
            slot.call = true;
            for peer in [bea, leo] {
                add_member(n, group, peer);
                n.groups.get_mut(&group).unwrap().members.remove(&peer);
                slot.members.insert(peer);
                let chat = GroupLog::create(&n.identity, "chat", 2).unwrap();
                let chat_id = chat.group_id();
                let mut chat = GroupSlot::new(n.identity.clone(), GroupSetup::Member(chat));
                chat.direct = true;
                chat.members.insert(peer);
                n.groups.insert(chat_id, chat);
            }
            n.groups.insert(call_id, slot);
            n.publish_members();
            n.relay_users.extend([bea, leo]);
            n.drop_group(call_id);
            n.redial_let_go();
            let kept = (n.relay_users.contains(&bea), n.relay_users.contains(&leo));
            // Leo is in another group the relay serves by the time Bea's minutes are over.
            add_member(n, group, leo);
            n.publish_members();
            for at in n.after_call.values_mut() {
                *at = Instant::now() - Duration::from_secs(1);
            }
            n.redial_let_go();
            (
                kept,
                (n.relay_users.contains(&bea), n.relay_users.contains(&leo)),
                n.after_call.is_empty(),
            )
        })
        .await;
        assert_eq!(found, ((true, true), (false, true), true));
        node.shutdown().await.unwrap();
    }
}
