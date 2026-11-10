//! Reaching friends: what each found peer is asked or told first (a probe, a hello, an
//! acceptance, a link or code request, a removal), the friend heartbeat, and the sessions
//! of each friend's device taken as online, a few at most.

use std::time::Instant;

use fogata_crypto::{DevicePublicKey, random_bytes};
use fogata_friends::Animated;
use fogata_proto::FriendRequest;
use libp2p::PeerId;

use super::{
    Node, SEARCH_HARD,
    friends::{FriendPending, Target},
    now_ms,
};
use crate::{NodeEvent, Path};

/// Hellos sent at most to a peer that may be one of the friends waiting for this device's
/// secret.
const MAX_GREETINGS: usize = 4;

/// Sessions of one friend online at once: a restart's new one beside the old one still
/// connected, the same identity open on two computers. Past it, the oldest is let go.
const MAX_FRIEND_SESSIONS: usize = 4;

/// What was sent to a peer on its current connection, so it is sent once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Contact {
    /// A hello or an acceptance.
    Friend,
    Link,
    Code,
    Probe,
    /// A new friend secret for a friend already online.
    Update,
    /// This device's banner.
    Banner,
    /// This device's bio.
    Bio,
    /// This device's animation of its picture or banner.
    Animation(Animated),
    /// That the friend was removed.
    Unfriend,
    /// Hellos to the friends waiting for this device's secret (`greet_waiting_friends`).
    Greet,
}

impl Node {
    /// Keeps connections to online friends open and retries peers not yet contacted.
    pub(super) fn friend_heartbeat(&mut self) {
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        f.forget_stale_offers(Instant::now());
        let f = &*f;
        let online: Vec<PeerId> = f.online.keys().copied().collect();
        let waiting: Vec<PeerId> = f
            .targets
            .keys()
            .chain(f.lan.iter())
            .filter(|p| !f.online.contains_key(p))
            .copied()
            .collect();
        let retry: Vec<(PeerId, DevicePublicKey)> = f
            .profile_retry
            .iter()
            .filter_map(|p| f.online.get(p).map(|d| (*p, *d)))
            .collect();
        for peer in online {
            self.friend_request(peer, FriendRequest::Heartbeat, FriendPending::Heartbeat);
        }
        for (peer, device) in retry {
            self.send_profile(peer, device);
        }
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        let statuses: Vec<(PeerId, DevicePublicKey)> = f
            .status_retry
            .drain()
            .filter_map(|p| f.online.get(&p).map(|d| (p, *d)))
            .collect();
        for (peer, device) in statuses {
            self.send_status_as(peer, device, true);
        }
        for peer in waiting {
            self.contact(peer);
        }
        self.update_online_friends();
        self.share_group();
    }

    /// Sends our current friend secret to online friends that do not have it yet, after a
    /// friend was removed.
    pub(super) fn update_online_friends(&mut self) {
        let Some(f) = &self.friends else {
            return;
        };
        let due: Vec<(PeerId, DevicePublicKey)> = f
            .online
            .iter()
            .filter(|(peer, device)| {
                f.book.friends.get(device).is_some_and(|fr| !fr.delivered)
                    && !f.contacted.contains(&(**peer, Contact::Update))
            })
            .map(|(peer, device)| (*peer, *device))
            .collect();
        for (peer, device) in due {
            self.mark_contacted(peer, Contact::Update);
            self.send_accept(peer, device);
        }
    }

    pub(super) fn friend_lan_peer(&mut self, peer: PeerId) {
        if let Some(f) = self.friends.as_mut() {
            f.lan.insert(peer);
        }
        self.contact(peer);
    }

    pub(super) fn friend_connected(&mut self, peer: PeerId) {
        self.contact(peer);
    }

    pub(super) fn friend_disconnected(&mut self, peer: PeerId) {
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        f.requesting.remove(&peer);
        f.contacted.retain(|(p, _)| *p != peer);
        f.group_shared.retain(|(p, _)| *p != peer);
        f.offering.retain(|(p, _), _| *p != peer);
        f.online_since.remove(&peer);
        f.acked.retain(|(p, _)| *p != peer);
        f.profile_sends.remove(&peer);
        f.profile_retry.remove(&peer);
        f.status_retry.remove(&peer);
        if let Some(device) = f.online.remove(&peer)
            && !f.is_online(&device)
        {
            f.offers_in.remove(&device);
            // Maybe gone to another network: look for it at every slot for a while.
            f.searching_until = Some(Instant::now() + SEARCH_HARD);
            f.gone_at.insert(device, Instant::now());
            f.last_lookup = None;
            self.emit(NodeEvent::FriendOffline { device });
        }
    }

    /// Sends a connected peer what it is due: a hello or an acceptance to a friend, a friend
    /// request to the owner of a link or code used here, a probe to a peer on the local
    /// network.
    pub(super) fn contact(&mut self, peer: PeerId) {
        if !self.swarm.is_connected(&peer) {
            return;
        }
        let Some(f) = &self.friends else {
            return;
        };
        if f.online.contains_key(&peer) {
            return;
        }
        let todo = |c| !f.contacted.contains(&(peer, c));
        match f.targets.get(&peer).copied() {
            Some(Target::Friend(device)) => {
                let Some(friend) = f.book.friends.get(&device) else {
                    return;
                };
                if !todo(Contact::Friend) {
                    return;
                }
                if friend.delivered {
                    self.send_hello(peer, device);
                } else {
                    self.send_accept(peer, device);
                }
            }
            Some(Target::Departing(device)) if todo(Contact::Unfriend) => {
                if f.book.departing.contains_key(&device) {
                    self.send_unfriend(peer, device);
                }
            }
            // The device a request went to, which sends its acceptance itself.
            Some(Target::Accepting) => {}
            Some(Target::LinkOwner) if todo(Contact::Link) => self.send_link_request(peer),
            Some(Target::CodeOwner) if todo(Contact::Code) => self.send_code_start(peer),
            Some(_) => {}
            None if f.lan.contains(&peer) => {
                let code = f.sought_code.is_some() && todo(Contact::Code);
                let probe = f.anyone_offline() && todo(Contact::Probe);
                if code {
                    self.send_code_start(peer);
                }
                if probe {
                    self.send_probe(peer);
                }
            }
            None => {}
        }
    }

    pub(super) fn mark_contacted(&mut self, peer: PeerId, contact: Contact) {
        if let Some(f) = self.friends.as_mut() {
            f.contacted.insert((peer, contact));
        }
    }

    fn send_probe(&mut self, peer: PeerId) {
        let mut nonce = [0u8; 16];
        if random_bytes(&mut nonce).is_err() {
            return;
        }
        self.mark_contacted(peer, Contact::Probe);
        self.friend_request(
            peer,
            FriendRequest::Probe {
                nonce: nonce.to_vec(),
            },
            FriendPending::Probe(nonce),
        );
    }

    fn send_unfriend(&mut self, peer: PeerId, device: DevicePublicKey) {
        let local = self.local_peer().to_bytes();
        let sealed =
            match fogata_friends::unfriend(&self.identity, &device, &local, &peer.to_bytes()) {
                Ok(sealed) => sealed,
                Err(e) => {
                    tracing::debug!("cannot tell a former friend: {e}");
                    return;
                }
            };
        self.mark_contacted(peer, Contact::Unfriend);
        let digest = fogata_friends::removal_digest(&sealed);
        self.friend_request(
            peer,
            FriendRequest::Unfriend { sealed },
            FriendPending::Unfriend(device, digest),
        );
    }

    /// A hello keyed by the secret this device shares with the friend, so that whoever
    /// else answered for it (another friend of it, which knows its own secret and its probe
    /// tags) learns nothing from it; it says, sealed to the friend, which of the friend's
    /// secrets this device has.
    pub(super) fn send_hello(&mut self, peer: PeerId, device: DevicePublicKey) {
        let local = self.local_peer().to_bytes();
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        let (Some(pair), Some(friend)) =
            (f.pair(&self.identity, &device), f.book.friends.get(&device))
        else {
            return;
        };
        let envelope = match fogata_friends::hello(
            &self.identity,
            &device,
            &pair,
            &friend.secret,
            &local,
            &peer.to_bytes(),
            friend.profile.version_ms,
            self.hidden,
        ) {
            Ok(envelope) => envelope,
            Err(e) => {
                tracing::debug!("cannot greet a friend: {e}");
                return;
            }
        };
        self.mark_contacted(peer, Contact::Friend);
        self.friend_request(
            peer,
            FriendRequest::Hello {
                mac: envelope.mac.to_vec(),
                sealed: envelope.sealed,
            },
            FriendPending::Hello(device),
        );
    }

    /// Greets `peer` as each offline friend that does not have this device's current secret
    /// (a few at most): a hello is keyed by the secret the two share and sealed to that
    /// friend's key, so only that friend can open it, and nobody else learns from it whom
    /// this device is friends with (a friend's own secret is known to its other friends). For
    /// two friends who both changed their secrets while apart, whose probes on the local
    /// network no longer tell them apart from strangers.
    pub(super) fn greet_waiting_friends(&mut self, peer: PeerId) {
        let Some(f) = &self.friends else {
            return;
        };
        if f.online.contains_key(&peer) || f.contacted.contains(&(peer, Contact::Greet)) {
            return;
        }
        let waiting: Vec<DevicePublicKey> = f
            .book
            .friends
            .iter()
            .filter(|(device, friend)| !friend.delivered && !f.is_online(device))
            .map(|(device, _)| *device)
            .take(MAX_GREETINGS)
            .collect();
        if waiting.is_empty() {
            return;
        }
        let met = f.contacted.contains(&(peer, Contact::Friend));
        self.mark_contacted(peer, Contact::Greet);
        for device in waiting {
            self.send_hello(peer, device);
        }
        // Hellos to friends it may not be: once it turns out to be a friend, it still gets
        // its own.
        if !met && let Some(f) = self.friends.as_mut() {
            f.contacted.remove(&(peer, Contact::Friend));
        }
    }

    fn send_accept(&mut self, peer: PeerId, device: DevicePublicKey) {
        let local = self.local_peer().to_bytes();
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        if !f.book.friends.contains_key(&device) {
            return;
        }
        let Some(pair) = f.pair(&self.identity, &device) else {
            return;
        };
        let envelope = match fogata_friends::accept(
            &self.identity,
            &f.book.profile,
            &f.book.secret,
            &device,
            &pair,
            &local,
            &peer.to_bytes(),
        ) {
            Ok(envelope) => envelope,
            Err(e) => {
                tracing::debug!("cannot send an acceptance: {e}");
                return;
            }
        };
        self.mark_contacted(peer, Contact::Friend);
        self.friend_request(
            peer,
            FriendRequest::Accept {
                mac: envelope.mac.to_vec(),
                sealed: envelope.sealed,
            },
            FriendPending::Accept(device),
        );
    }

    fn send_link_request(&mut self, peer: PeerId) {
        let local = self.local_peer().to_bytes();
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        let Some(link) = f.sought_link.clone() else {
            return;
        };
        let request_secret = match f.book.request_sent(link.device, &link.name, now_ms()) {
            Ok(secret) => secret,
            Err(e) => {
                tracing::debug!("cannot make a friend request: {e}");
                return;
            }
        };
        let envelope = match fogata_friends::link_request(
            &self.identity,
            &f.book.profile,
            &request_secret,
            &link,
            &local,
            &peer.to_bytes(),
        ) {
            Ok(envelope) => envelope,
            Err(e) => {
                tracing::debug!("cannot make a friend request: {e}");
                return;
            }
        };
        f.recent = Some(Instant::now());
        self.changed();
        self.mark_contacted(peer, Contact::Link);
        self.friend_request(
            peer,
            FriendRequest::LinkRequest {
                mac: envelope.mac.to_vec(),
                sealed: envelope.sealed,
            },
            FriendPending::LinkRequest,
        );
    }

    fn send_code_start(&mut self, peer: PeerId) {
        let local = self.local_peer().to_bytes();
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        let Some(sought) = &f.sought_code else {
            return;
        };
        let (requester, start) =
            fogata_friends::code_start(&sought.secrets, &local, &peer.to_bytes());
        f.requesting.insert(peer, requester);
        self.mark_contacted(peer, Contact::Code);
        self.friend_request(
            peer,
            FriendRequest::CodeStart {
                gate: start.gate.to_vec(),
                spake: start.spake,
            },
            FriendPending::CodeStart,
        );
    }

    pub(super) fn mark_friend_online(
        &mut self,
        peer: PeerId,
        device: DevicePublicKey,
        hidden: bool,
    ) {
        let best = self.best_connection(&peer);
        if let Some(best) = &best {
            self.note_reached(best, device);
        }
        let path = best.map(|c| c.path).unwrap_or(Path::Direct);
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        f.targets.insert(peer, Target::Friend(device));
        let was_online = f.is_online(&device);
        // Another session of the friend still connected: a restart's old one, or the
        // same identity open on another computer (see sessions.rs).
        let mut older: Vec<PeerId> = f
            .online
            .iter()
            .filter(|(p, d)| **d == device && **p != peer)
            .map(|(p, _)| *p)
            .collect();
        let new_session = f.online.insert(peer, device).is_none();
        let mut let_go = Vec::new();
        if new_session {
            f.online_since.insert(peer, Instant::now());
            // Each session holds state of its own here: past a few, the oldest go.
            if older.len() >= MAX_FRIEND_SESSIONS {
                older.sort_by_key(|p| f.online_since.get(p).copied());
                let_go = older.drain(..=older.len() - MAX_FRIEND_SESSIONS).collect();
            }
        }
        for gone in &let_go {
            f.online.remove(gone);
            f.online_since.remove(gone);
        }
        // It acknowledged our secret on this connection before it proved who it is.
        let delivered = f.acked.remove(&(peer, device))
            && f.book.friends.get(&device).is_some_and(|fr| !fr.delivered);
        if delivered {
            f.book.set_delivered(&device);
            self.changed();
        }
        for gone in let_go {
            tracing::debug!(peer = %gone, "letting go of a friend's oldest session");
            let _ = self.swarm.disconnect_peer_id(gone);
        }
        self.show_protocols(peer);
        self.note_level(peer);
        if new_session && !older.is_empty() {
            self.check_sessions(device, older, peer, false);
        }
        if new_session && !was_online {
            self.emit(NodeEvent::FriendOnline {
                device,
                peer,
                path,
                hidden,
            });
        }
        // The hello said only whether this device appears offline; a new session (the friend
        // restarted while its old connection lingers) has heard nothing else yet either.
        if new_session && (self.in_a_room().is_some() || self.shown_away()) {
            self.send_status(peer, device);
        }
        self.send_banner(peer, device);
        self.send_bio(peer, device);
        self.send_animations(peer, device);
        self.share_group();
        self.offer_direct(peer, device);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::DeviceIdentity;
    use libp2p::PeerId;

    use crate::{
        NodeConfig,
        node::{friends::FriendPending, start, tests::inspect},
    };

    #[tokio::test(flavor = "multi_thread")]
    async fn a_friends_new_session_hears_that_this_device_is_away() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let book = fogata_friends::FriendBook::new(&ana, "Ana", 1).unwrap();
        let (node, _events) = start(NodeConfig {
            identity: ana,
            groups: Vec::new(),
            friends: Some(book),
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
        let bea = DeviceIdentity::generate().unwrap().public_key();
        let told = inspect(&node, move |n| {
            n.away = true;
            // Bea's session, then her new one after a restart while the old connection is
            // still open.
            let (old, new) = (PeerId::random(), PeerId::random());
            n.mark_friend_online(old, bea, false);
            n.mark_friend_online(new, bea, false);
            let pending = &n.friends.as_ref().unwrap().pending;
            let told = |peer| {
                pending
                    .values()
                    .any(|(p, kind)| *p == peer && matches!(kind, FriendPending::Status(_)))
            };
            (told(old), told(new))
        })
        .await;
        assert_eq!(told, (true, true));
        node.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_friend_keeps_only_a_few_sessions_online() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let book = fogata_friends::FriendBook::new(&ana, "Ana", 1).unwrap();
        let (node, _events) = start(NodeConfig {
            identity: ana,
            groups: Vec::new(),
            friends: Some(book),
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
        let bea = DeviceIdentity::generate().unwrap().public_key();
        let (online, newest_kept) = inspect(&node, move |n| {
            // Bea opens session after session, each saying hello.
            let sessions: Vec<PeerId> = (0..40).map(|_| PeerId::random()).collect();
            for peer in &sessions {
                n.mark_friend_online(*peer, bea, false);
            }
            let f = n.friends.as_ref().unwrap();
            let online: Vec<PeerId> = f
                .online
                .iter()
                .filter(|(_, d)| **d == bea)
                .map(|(p, _)| *p)
                .collect();
            let newest = sessions.last().is_some_and(|p| online.contains(p));
            (online.len(), newest)
        })
        .await;
        assert_eq!(online, 4);
        assert!(newest_kept);
        node.shutdown().await.unwrap();
    }
}
