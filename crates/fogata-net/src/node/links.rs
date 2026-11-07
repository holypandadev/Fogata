//! How each member is reached: the best of its connections, told to its groups when it
//! changes; a QUIC connection beside one that cannot carry media; and direct connections
//! that went silent left for a way through another member, and back.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use fogata_crypto::GroupId;
use libp2p::{
    Multiaddr, PeerId,
    swarm::{
        ConnectionId,
        dial_opts::{DialOpts, PeerCondition},
    },
};

use super::{ConnectionInfo, Member, Node};
use crate::{NodeEvent, Path, addr};

/// How often the direct connections to members are checked for silence, and how long one
/// may hear nothing before the member is reached through another member instead. QUIC
/// keep-alives every 5 s make a live connection hear something at least that often; a
/// few more seconds leave room for a slow path.
pub(super) const LINK_CHECK: Duration = Duration::from_secs(2);
const LINK_SILENT: Duration = Duration::from_secs(10);
/// How long a connection through a member stays once the member is reached directly
/// again, so that the other end has its direct connection too before this one goes.
const RELAYED_KEPT: Duration = Duration::from_secs(3);
/// How long a dial through a member is given before another member's way is tried.
pub(super) const THROUGH_DIAL_WAIT: Duration = Duration::from_secs(10);

/// How long a path worse than the one told must last before it is told.
const STEADY: Duration = Duration::from_secs(2);

/// `path` as told to the members of a group, `in_group` the peers that proved membership of
/// it: the member whose relay slot carries the path is named only when it is one of them, so
/// nobody learns of a member of a group it is not in.
fn path_in_group(
    path: &Path,
    in_group: &HashSet<PeerId>,
    members: &HashMap<PeerId, Member>,
) -> Path {
    match path {
        Path::ThroughMember { relay, .. } => Path::ThroughMember {
            relay: *relay,
            member: in_group
                .contains(relay)
                .then(|| members.get(relay).map(|m| m.device))
                .flatten(),
        },
        other => other.clone(),
    }
}

/// Lower is better. Direct and hole-punched paths are equally good once open.
fn path_rank(path: &Path) -> u8 {
    match path {
        Path::Lan => 0,
        Path::Direct | Path::HolePunched => 1,
        Path::ThroughMember { .. } => 2,
    }
}

impl Node {
    /// The best open connection to `peer`.
    /// `peer`'s best open connection: the best path, QUIC first on the same path (media
    /// travels on QUIC), the oldest first after that, so the choice holds still.
    pub(super) fn best_connection(&self, peer: &PeerId) -> Option<ConnectionInfo> {
        self.connections
            .iter()
            .filter(|(id, c)| c.peer == *peer && !self.closing.contains(id))
            .min_by_key(|(id, c)| (path_rank(&c.path), !addr::is_quic(&c.address), **id))
            .map(|(_, c)| c.clone())
    }

    /// A member reached only over TCP (or through another member) cannot receive media
    /// datagrams: try once to open a QUIC connection to the addresses it listens on.
    pub(super) fn want_datagrams(&mut self, peer: PeerId) {
        if !self.in_regular_group(&peer)
            || crate::datagram::link(&peer).is_some()
            || self.quic_tried.contains(&peer)
        {
            return;
        }
        let quic = self.member_quic_addresses(&peer);
        if quic.is_empty() {
            return;
        }
        self.quic_tried.insert(peer);
        tracing::debug!(%peer, "also connecting over QUIC for media");
        self.dial_quic(peer, quic);
    }

    /// The QUIC addresses a member listens on, its local ones first: the public ones fail
    /// between two devices behind the same router, which rarely sends traffic back in.
    pub(super) fn member_quic_addresses(&self, peer: &PeerId) -> Vec<Multiaddr> {
        self.member_lan
            .get(peer)
            .into_iter()
            .flatten()
            .chain(self.peer_listening.get(peer).into_iter().flatten())
            .filter(|a| addr::is_quic(a) && !addr::is_relayed(a))
            .cloned()
            .collect()
    }

    /// Dials `peer` over QUIC at `quic`, even while connected: the media link is a QUIC
    /// connection of its own.
    pub(super) fn dial_quic(&mut self, peer: PeerId, quic: Vec<Multiaddr>) {
        let opts = DialOpts::peer_id(peer)
            .addresses(quic)
            .condition(PeerCondition::Always)
            .build();
        let _ = self.swarm.dial(opts);
    }

    /// Picks the best open connection to a member and reports it, for each of its groups, if
    /// it changed.
    pub(super) fn update_member_path(&mut self, peer: PeerId) {
        let Some(best) = self.best_connection(&peer) else {
            return;
        };
        let Some(member) = self.members.get_mut(&peer) else {
            return;
        };
        // Better or worse: the member's path is that of its best open connection, which
        // relaying and rosters rely on.
        if best.path == member.path && best.address == member.address {
            return;
        }
        member.path = best.path.clone();
        member.address = best.address.clone();
        let device = member.device;
        self.note_reached(&best, device);
        self.tell_path(peer);
        if !matches!(best.path, Path::ThroughMember { .. }) && self.has_relayed(&peer) {
            // Not at once: the other end may not have the new connection yet, and would
            // see the member gone in between.
            self.redundant.insert(peer, Instant::now());
        }
    }

    /// Tells the groups of member `peer` how it is reached, when that changed: a better path
    /// at once, another one once it has lasted [`STEADY`].
    fn tell_path(&mut self, peer: PeerId) {
        let Some(member) = self.members.get_mut(&peer) else {
            return;
        };
        if member.path == member.told {
            member.worse_since = None;
            return;
        }
        let better = path_rank(&member.path) < path_rank(&member.told);
        let since = *member.worse_since.get_or_insert_with(Instant::now);
        if !better && since.elapsed() < STEADY {
            return;
        }
        member.told = member.path.clone();
        member.worse_since = None;
        let (device, path, address) = (member.device, member.path.clone(), member.address.clone());
        let groups: Vec<GroupId> = self
            .groups
            .iter()
            .filter(|(_, s)| s.members.contains(&peer))
            .map(|(id, _)| *id)
            .collect();
        for group in groups {
            self.emit(NodeEvent::MemberConnected {
                group,
                device,
                peer,
                path: self.path_in(&group, &path),
                address: address.clone(),
            });
        }
    }

    /// `path` as told to `group` (see [`path_in_group`]).
    pub(super) fn path_in(&self, group: &GroupId, path: &Path) -> Path {
        let none = HashSet::new();
        let in_group = self.groups.get(group).map_or(&none, |s| &s.members);
        path_in_group(path, in_group, &self.members)
    }

    /// `relay` just proved membership of `group`: the group's members that come through its
    /// relay slot are told again, now naming it.
    pub(super) fn name_carrier(&mut self, group: GroupId, relay: PeerId) {
        let Some(slot) = self.groups.get(&group).filter(|s| !s.direct) else {
            return;
        };
        let carried: Vec<_> = slot
            .members
            .iter()
            .filter_map(|peer| {
                let m = self.members.get(peer)?;
                matches!(m.path, Path::ThroughMember { relay: r, .. } if r == relay)
                    .then(|| (*peer, m.device, m.path.clone(), m.address.clone()))
            })
            .collect();
        for (peer, device, path, address) in carried {
            self.emit(NodeEvent::MemberConnected {
                group,
                device,
                peer,
                path: self.path_in(&group, &path),
                address,
            });
        }
    }

    /// Whether `peer` is a member reached only through another member.
    pub(super) fn only_relayed(&self, peer: &PeerId) -> bool {
        self.members
            .get(peer)
            .is_some_and(|m| matches!(m.path, Path::ThroughMember { .. }))
    }

    /// Whether a connection to `peer` goes through a member.
    pub(super) fn has_relayed(&self, peer: &PeerId) -> bool {
        self.connections
            .values()
            .any(|c| c.peer == *peer && matches!(c.path, Path::ThroughMember { .. }))
    }

    /// Finds members whose direct connections went silent (client isolation on a Wi-Fi, a
    /// NAT that forgot its mapping), which would otherwise stay open until their idle
    /// timeout while messages and media sent on them are lost. The other members are asked
    /// for a way to them, and once one is open the silent connections are closed.
    pub(super) fn check_member_links(&mut self) {
        self.close_redundant_relayed();
        self.redial_lost();
        self.drop_cut_off();
        let waiting: Vec<PeerId> = self
            .members
            .iter()
            .filter(|(_, m)| m.worse_since.is_some())
            .map(|(p, _)| *p)
            .collect();
        for peer in waiting {
            self.tell_path(peer);
        }
        let direct: Vec<PeerId> = self
            .members
            .iter()
            .filter(|(_, m)| !matches!(m.path, Path::ThroughMember { .. }))
            .map(|(p, _)| *p)
            .collect();
        self.silent.retain(|p, _| direct.contains(p));
        for peer in direct {
            // Only QUIC connections tell; a member reached over TCP alone is left to the
            // pings.
            let silent = crate::datagram::silent_for(&peer).is_some_and(|d| d >= LINK_SILENT);
            if !silent {
                self.silent.remove(&peer);
                continue;
            }
            if self.has_relayed(&peer) {
                self.leave_silent_links(peer);
                continue;
            }
            if self
                .silent
                .get(&peer)
                .is_some_and(|t| t.elapsed() < THROUGH_DIAL_WAIT)
            {
                continue;
            }
            tracing::debug!(%peer, "a member's direct connections went silent");
            self.silent.insert(peer, Instant::now());
            let groups: Vec<GroupId> = self
                .groups
                .iter()
                .filter(|(_, s)| s.members.contains(&peer))
                .map(|(id, _)| *id)
                .collect();
            self.look_through_others(peer, &groups);
        }
    }

    /// Closes the direct connections to a member that went silent, now that a way through
    /// another member is open: its path becomes that way. Should the direct way still work
    /// after all, the hole punch through the member brings it back.
    pub(super) fn leave_silent_links(&mut self, peer: PeerId) {
        self.silent.remove(&peer);
        let direct: Vec<ConnectionId> = self
            .connections
            .iter()
            .filter(|(_, c)| c.peer == peer && !matches!(c.path, Path::ThroughMember { .. }))
            .map(|(id, _)| *id)
            .collect();
        tracing::debug!(%peer, "silent direct connections closed for the way through a member");
        // Marked first: as each one goes, the member's path must become the way through the
        // member, not another silent connection that would close that way as redundant.
        self.closing.extend(direct.iter().copied());
        for id in direct {
            self.swarm.close_connection(id);
        }
    }

    /// Once a member is reachable without a relay, the relayed connections are redundant;
    /// they close [`RELAYED_KEPT`] later, if the member is still reached directly then.
    fn close_redundant_relayed(&mut self) {
        let due: Vec<PeerId> = self
            .redundant
            .iter()
            .filter(|(_, since)| since.elapsed() >= RELAYED_KEPT)
            .map(|(peer, _)| *peer)
            .collect();
        for peer in due {
            self.redundant.remove(&peer);
            let direct = self
                .members
                .get(&peer)
                .is_some_and(|m| !matches!(m.path, Path::ThroughMember { .. }));
            if direct {
                self.close_relayed_connections(peer);
            }
        }
    }

    pub(super) fn close_relayed_connections(&mut self, peer: PeerId) {
        let relayed: Vec<ConnectionId> = self
            .connections
            .iter()
            .filter(|(_, c)| c.peer == peer && matches!(c.path, Path::ThroughMember { .. }))
            .map(|(id, _)| *id)
            .collect();
        for id in relayed {
            self.swarm.close_connection(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use fogata_crypto::DeviceIdentity;

    use super::*;

    #[test]
    fn a_relaying_member_is_named_only_to_its_own_groups() {
        let ana = DeviceIdentity::generate().unwrap().public_key();
        let relay = PeerId::random();
        let members = HashMap::from([(
            relay,
            Member {
                device: ana,
                path: Path::Direct,
                address: "/ip4/203.0.113.50/udp/4001/quic-v1".parse().unwrap(),
                told: Path::Direct,
                worse_since: None,
            },
        )]);
        let carried = Path::ThroughMember {
            relay,
            member: None,
        };
        // Bea comes through Ana: a group Ana is in hears it is Ana.
        let with_ana = HashSet::from([relay, PeerId::random()]);
        assert_eq!(
            path_in_group(&carried, &with_ana, &members),
            Path::ThroughMember {
                relay,
                member: Some(ana),
            }
        );
        // A group without Ana only hears it is a member.
        let without_ana = HashSet::from([PeerId::random()]);
        assert_eq!(path_in_group(&carried, &without_ana, &members), carried);
        // Not verified yet as anyone: unnamed as well.
        assert_eq!(path_in_group(&carried, &with_ana, &HashMap::new()), carried);
        // Other paths stay as they are.
        assert_eq!(
            path_in_group(&Path::HolePunched, &with_ana, &members),
            Path::HolePunched
        );
    }
}
