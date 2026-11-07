//! What friends are told of this device: its status, profile, banner, bio and animations,
//! sent to each friend online as they change, and the questions about mutual friends.

use fogata_crypto::DevicePublicKey;
use fogata_friends::{Animated, Animation, MutualAsk};
use fogata_proto::FriendRequest;
use libp2p::PeerId;

use super::{Node, friend_contact::Contact, friends::FriendPending};

/// Where the answer to a question about friends in common goes.
pub(super) type MutualReply = tokio::sync::oneshot::Sender<Result<Vec<DevicePublicKey>, String>>;

impl Node {
    pub(super) fn own_profile(&self) -> Option<&fogata_friends::Profile> {
        self.friends.as_ref().map(|f| &f.book.profile)
    }

    /// Tells every online friend whether this device is in a room, or that it appears offline.
    pub(super) fn send_status_to_friends(&mut self) {
        let Some(f) = &self.friends else {
            return;
        };
        let online: Vec<(PeerId, DevicePublicKey)> =
            f.online.iter().map(|(p, d)| (*p, *d)).collect();
        for (peer, device) in online {
            self.send_status(peer, device);
        }
    }

    /// Sends this device's profile to every online friend, sealed to each.
    pub(super) fn send_profile_to_friends(&mut self) {
        let Some(f) = &self.friends else {
            return;
        };
        let online: Vec<(PeerId, DevicePublicKey)> =
            f.online.iter().map(|(p, d)| (*p, *d)).collect();
        for (peer, device) in online {
            self.send_profile(peer, device);
        }
    }

    /// Sends this device's profile to the friend on `peer`, or once the one on its way there
    /// has arrived.
    pub(super) fn send_profile(&mut self, peer: PeerId, device: DevicePublicKey) {
        let local = self.local_peer().to_bytes();
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        f.profile_retry.remove(&peer);
        if let Some(again) = f.profile_sends.get_mut(&peer) {
            *again = true;
            return;
        }
        match fogata_friends::profile_update(&device, &f.book.profile, &local, &peer.to_bytes()) {
            Ok(sealed) => {
                f.profile_sends.insert(peer, false);
                self.friend_request(
                    peer,
                    FriendRequest::Profile { sealed },
                    FriendPending::Profile,
                );
            }
            Err(e) => tracing::debug!("cannot send a profile: {e}"),
        }
    }

    /// The profile sent to `peer` arrived, or not: the newest goes now if one waits, or at the
    /// next heartbeat if this one was lost.
    pub(super) fn profile_went(&mut self, peer: PeerId, arrived: bool) {
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        let again = f.profile_sends.remove(&peer).unwrap_or(false);
        let Some(device) = f.online.get(&peer).copied() else {
            return;
        };
        if !arrived {
            f.profile_retry.insert(peer);
        } else if again {
            self.send_profile(peer, device);
        }
    }

    /// The room's group goes only to friends that are members of it, so presence never
    /// shows a group to someone outside it.
    pub(super) fn send_status(&mut self, peer: PeerId, device: DevicePublicKey) {
        self.send_status_as(peer, device, false);
    }

    pub(super) fn send_status_as(&mut self, peer: PeerId, device: DevicePublicKey, again: bool) {
        let room = self.in_a_room();
        let group = room
            .filter(|g| {
                self.groups
                    .get(g)
                    .is_some_and(|s| s.ctx.is_member_device(&device))
            })
            .map(|g| *g.as_bytes());
        // Appearing offline says nothing about rooms.
        let status = fogata_friends::Status {
            in_room: room.is_some() && !self.hidden,
            group: group.filter(|_| !self.hidden),
            hidden: self.hidden,
            away: self.shown_away(),
        };
        let local = self.local_peer().to_bytes();
        match fogata_friends::status(&device, &status, &local, &peer.to_bytes()) {
            Ok(sealed) => self.friend_request(
                peer,
                FriendRequest::Status { sealed },
                FriendPending::Status(again),
            ),
            Err(e) => tracing::debug!("cannot send a status: {e}"),
        }
    }

    /// Sends this device's banner to a friend on this connection, once.
    pub(super) fn send_banner(&mut self, peer: PeerId, device: DevicePublicKey) {
        let Some(f) = &self.friends else {
            return;
        };
        let Some(banner) = f.book.banner.clone() else {
            return;
        };
        if f.contacted.contains(&(peer, Contact::Banner)) {
            return;
        }
        let local = self.local_peer().to_bytes();
        match fogata_friends::banner_update(&device, &banner, &local, &peer.to_bytes()) {
            Ok(sealed) => {
                self.mark_contacted(peer, Contact::Banner);
                self.friend_request(
                    peer,
                    FriendRequest::Banner { sealed },
                    FriendPending::Banner,
                );
            }
            Err(e) => tracing::debug!("cannot send the banner: {e}"),
        }
    }

    /// Sends a changed banner to every online friend.
    pub(super) fn send_banner_to_friends(&mut self) {
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        f.contacted.retain(|(_, c)| *c != Contact::Banner);
        let online: Vec<(PeerId, DevicePublicKey)> =
            f.online.iter().map(|(p, d)| (*p, *d)).collect();
        for (peer, device) in online {
            self.send_banner(peer, device);
        }
    }

    /// Sends this device's bio to a friend on this connection, once.
    pub(super) fn send_bio(&mut self, peer: PeerId, device: DevicePublicKey) {
        let Some(f) = &self.friends else {
            return;
        };
        let Some(bio) = f.book.bio.clone() else {
            return;
        };
        if f.contacted.contains(&(peer, Contact::Bio)) {
            return;
        }
        let local = self.local_peer().to_bytes();
        match fogata_friends::bio_update(&device, &bio, &local, &peer.to_bytes()) {
            Ok(sealed) => {
                self.mark_contacted(peer, Contact::Bio);
                self.friend_request(peer, FriendRequest::Bio { sealed }, FriendPending::Bio);
            }
            Err(e) => tracing::debug!("cannot send the bio: {e}"),
        }
    }

    /// Sends this device's animations to a friend on this connection, once each.
    pub(super) fn send_animations(&mut self, peer: PeerId, device: DevicePublicKey) {
        let Some(f) = &self.friends else {
            return;
        };
        let local = self.local_peer().to_bytes();
        let mut sent = Vec::new();
        for (of, animation) in &f.animations {
            if f.contacted.contains(&(peer, Contact::Animation(*of))) {
                continue;
            }
            match fogata_friends::animation_update(&device, animation, &local, &peer.to_bytes()) {
                Ok(sealed) => sent.push((*of, sealed)),
                Err(e) => tracing::debug!("cannot send an animation: {e}"),
            }
        }
        for (of, sealed) in sent {
            self.mark_contacted(peer, Contact::Animation(of));
            self.friend_request(
                peer,
                FriendRequest::Animation { sealed },
                FriendPending::Animation,
            );
        }
    }

    /// Sends a changed animation to every online friend.
    pub(super) fn send_animation_to_friends(&mut self, of: Animated) {
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        f.contacted.retain(|(_, c)| *c != Contact::Animation(of));
        let online: Vec<(PeerId, DevicePublicKey)> =
            f.online.iter().map(|(p, d)| (*p, *d)).collect();
        for (peer, device) in online {
            self.send_animations(peer, device);
        }
    }

    /// This device's animation of its picture, encoded, for the members it sends its
    /// profile to.
    pub(super) fn own_photo_animation(&self) -> Option<Vec<u8>> {
        self.friends
            .as_ref()?
            .animations
            .get(&Animated::Photo)
            .map(Animation::encode)
    }

    /// Sends a changed bio to every online friend.
    pub(super) fn send_bio_to_friends(&mut self) {
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        f.contacted.retain(|(_, c)| *c != Contact::Bio);
        let online: Vec<(PeerId, DevicePublicKey)> =
            f.online.iter().map(|(p, d)| (*p, *d)).collect();
        for (peer, device) in online {
            self.send_bio(peer, device);
        }
    }

    /// Asks a friend connected now which friends the two have in common.
    pub(super) fn ask_mutual(&mut self, device: DevicePublicKey, reply: MutualReply) {
        let local = self.local_peer().to_bytes();
        let Some(f) = self.friends.as_mut() else {
            let _ = reply.send(Err("this node runs without friends".into()));
            return;
        };
        let Some(peer) = f
            .online
            .iter()
            .find(|(_, d)| **d == device)
            .map(|(peer, _)| *peer)
        else {
            let _ = reply.send(Err("that friend is not online".into()));
            return;
        };
        if let Some((_, waiting)) = f.asking.get_mut(&device) {
            waiting.push(reply);
            return;
        }
        let friends: Vec<DevicePublicKey> = f
            .book
            .friends
            .keys()
            .filter(|d| **d != device)
            .copied()
            .collect();
        let started = MutualAsk::start(&friends).and_then(|(ask, points)| {
            fogata_friends::mutual_ask(&device, &points, &local, &peer.to_bytes())
                .map(|sealed| (ask, sealed))
        });
        match started {
            Ok((ask, sealed)) => {
                f.asking.insert(device, (ask, vec![reply]));
                self.friend_request(
                    peer,
                    FriendRequest::Mutual { sealed },
                    FriendPending::Mutual(device),
                );
            }
            Err(e) => {
                let _ = reply.send(Err(e.to_string()));
            }
        }
    }

    /// Gives the waiters of a question about friends in common its outcome.
    pub(super) fn mutual_done(
        &mut self,
        device: DevicePublicKey,
        outcome: Result<Vec<DevicePublicKey>, String>,
    ) {
        let Some((_, waiting)) = self.friends.as_mut().and_then(|f| f.asking.remove(&device))
        else {
            return;
        };
        for reply in waiting {
            let _ = reply.send(outcome.clone());
        }
    }
}
