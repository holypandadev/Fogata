//! Meeting another computer to move an account to it (see fogata-core's `transfer`): both
//! computers announce themselves under a rendezvous secret that comes from the code's first
//! characters, look each other up there, and dial what they find at the punch slots, as
//! members do. On the local network they find each other over mDNS anyway. What the two say
//! goes over a stream protocol of their own, checked with the code; the node only brings them
//! together.

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use libp2p::{PeerId, kad::RecordKey};

use super::Node;
use crate::rendezvous;

/// How often the rendezvous point is looked up while a move is under way: at every punch
/// slot (10 s apart; a little less here so that a slot that comes early still counts), since
/// the other computer may start any time after this one and is found one slot sooner.
const LOOKUP_EVERY: Duration = Duration::from_secs(9);

/// A rendezvous point this node announces itself under and looks up.
pub(super) struct Seeking {
    secret: [u8; 32],
    /// What this node's local addresses are sealed under in its announcements, and what
    /// opens the other computer's (see lan.rs).
    lan: Option<[u8; 32]>,
    /// The DHT keys of the last lookup.
    keys: HashSet<RecordKey>,
    last_lookup: Option<Instant>,
}

impl Node {
    /// Starts meeting another computer under `secret`, or stops (`None`).
    pub(super) fn seek(&mut self, secret: Option<[u8; 32]>, lan: Option<[u8; 32]>) {
        // What an earlier code found is no candidate of this one (or of none).
        self.candidate_found
            .retain(|_, (budget, _)| *budget != super::candidates::Budget::Move);
        self.seeking = secret.map(|secret| Seeking {
            secret,
            lan,
            keys: HashSet::new(),
            last_lookup: None,
        });
        // Announced at the next slot, not at the next refresh.
        self.provided.1 = None;
        // The other computer finds this one by the move's protocol in identify, which is
        // listed only to peers shown it: while the move goes on, those on this local network
        // (it may be one of them, found by mDNS with no DHT) and those found under the code.
        if self.seeking.is_some() {
            let nearby: Vec<PeerId> = self
                .connections
                .values()
                .filter(|c| c.path == crate::Path::Lan)
                .map(|c| c.peer)
                .collect();
            for peer in nearby {
                self.show_protocols(peer);
            }
        }
    }

    pub(super) fn seek_announce_secrets(&self) -> Vec<[u8; 32]> {
        self.seeking.iter().map(|s| s.secret).collect()
    }

    /// The key local addresses are sealed under and opened with while seeking.
    pub(super) fn seek_lan_key(&self) -> Option<[u8; 32]> {
        self.seeking
            .as_ref()
            .and_then(|s| s.lan.as_ref())
            .map(crate::lan::key_of)
    }

    /// Connected peers found under the move's code (see [`crate::NodeHandle::move_candidates`]).
    pub(super) fn move_candidates(&self) -> Vec<PeerId> {
        if !self.is_seeking() {
            return Vec::new();
        }
        self.candidate_found
            .iter()
            .filter(|(peer, (budget, _))| {
                *budget == super::candidates::Budget::Move && self.swarm.is_connected(peer)
            })
            .map(|(peer, _)| *peer)
            .collect()
    }

    pub(super) fn is_seeking(&self) -> bool {
        self.seeking.is_some()
    }

    /// Looks the rendezvous point up again at the next slot (once this node's own
    /// announcement went through: the other computer may have announced meanwhile).
    pub(super) fn seek_again(&mut self) {
        if let Some(seeking) = self.seeking.as_mut() {
            seeking.last_lookup = None;
        }
    }

    pub(super) fn seek_slot(&mut self, now_secs: u64) {
        let Some(seeking) = self.seeking.as_mut() else {
            return;
        };
        if !self.swarm.behaviour().kad.is_enabled()
            || seeking
                .last_lookup
                .is_some_and(|t| t.elapsed() < LOOKUP_EVERY)
        {
            return;
        }
        seeking.last_lookup = Some(Instant::now());
        seeking.keys = rendezvous::lookup_keys(&[seeking.secret], now_secs)
            .into_iter()
            .collect();
        let keys: Vec<RecordKey> = seeking.keys.iter().cloned().collect();
        self.lookup_dials.clear();
        for key in keys {
            self.look_up(key);
        }
    }

    pub(super) fn is_seek_key(&self, key: &RecordKey) -> bool {
        self.seeking.as_ref().is_some_and(|s| s.keys.contains(key))
    }

    /// Peers announced under the rendezvous point: dialled now, and at every punch slot
    /// from then on, so that two computers behind NATs open them for each other.
    pub(super) fn on_seek_providers(&mut self, key: &RecordKey, providers: HashSet<PeerId>) {
        let local = self.local_peer();
        let mut room = super::candidates::MAX_NEW_PER_ANSWER;
        for peer in providers {
            if peer == local {
                continue;
            }
            let new = !self.candidates.contains_key(&peer);
            // Anyone can announce under the rendezvous point, or a DHT node name anyone.
            if !self.take_named(peer, super::candidates::Budget::Move, key, &mut room) {
                continue;
            }
            if new {
                self.info(format!(
                    "found a computer under the code ({})",
                    crate::short_peer(&peer)
                ));
            }
            self.candidates.entry(peer).or_default();
            // Told the move's protocol, which it looks for (see `seek`).
            self.show_protocols(peer);
            if !self.swarm.is_connected(&peer) && !self.lookup_dials.contains(&peer) {
                self.dial_found(peer);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::DeviceIdentity;
    use libp2p::{Multiaddr, StreamProtocol};

    use super::*;
    use crate::{
        NodeConfig,
        node::{candidates::Budget, start, tests::inspect},
    };

    /// A move needs only one computer to find the other under the code: the old one found
    /// the new one and reached it, the new one has not found the old one, so it does not
    /// list the move's protocol to it; the old one still has it among the computers found
    /// under the code, opens the move's stream, and the new one answers it.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_move_needs_only_one_computer_to_find_the_other() {
        use futures::{AsyncReadExt as _, AsyncWriteExt as _, StreamExt as _};

        let protocol = StreamProtocol::new("/fogata/transfer-test/1");
        let config = |port: u16| NodeConfig {
            identity: Arc::new(DeviceIdentity::generate().unwrap()),
            groups: Vec::new(),
            friends: None,
            listen_port: port,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
        };
        let port = std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let at: Multiaddr = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
            .parse()
            .unwrap();
        // The new computer, which takes the move's protocol, and the old one. The new one
        // does not seek here: over the loopback interface it would take the old one for a
        // computer of its local network, which it lists the protocol to.
        let (new, _new_events) = start(config(port)).unwrap();
        let mut incoming = new.streams().accept(protocol.clone()).unwrap();
        let (old, _old_events) = start(config(0)).unwrap();
        old.seek(Some([7; 32]), None).await.unwrap();
        let new_peer = new.local_peer();
        let old_peer = old.local_peer();
        // Found under the code by the old computer alone, and reached.
        inspect(&old, move |n| {
            n.candidates.entry(new_peer).or_default();
            n.candidate_found
                .insert(new_peer, (Budget::Move, Instant::now()));
        })
        .await;
        old.dial(vec![at], None).await.unwrap();
        let identified = tokio::time::timeout(Duration::from_secs(20), async {
            while !old.identified_peers().contains(&new_peer) {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        assert!(
            identified.is_ok(),
            "the old computer never reached the new one"
        );
        // A moment for a stranger's connection to be closed, were it closed.
        tokio::time::sleep(Duration::from_secs(3)).await;
        assert!(
            !old.peers_taking(&protocol).contains(&new_peer),
            "the new computer listed the move's protocol to a computer it did not find"
        );
        assert_eq!(old.move_candidates().await.unwrap(), vec![new_peer]);

        let mut stream = old
            .streams()
            .open_stream(new_peer, protocol.clone())
            .await
            .expect("the new computer answers the move's protocol");
        stream.write_all(b"start").await.unwrap();
        stream.close().await.unwrap();
        let (from, mut answered) = tokio::time::timeout(Duration::from_secs(10), incoming.next())
            .await
            .expect("the stream came")
            .expect("the node runs");
        assert_eq!(from, old_peer);
        let mut said = Vec::new();
        answered.read_to_end(&mut said).await.unwrap();
        assert_eq!(said, b"start");

        // A candidate that does not take the move's protocol refuses the stream before
        // anything is said: the code is not tried on it.
        let stranger_port = std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let (stranger, _stranger_events) = start(config(stranger_port)).unwrap();
        let stranger_peer = stranger.local_peer();
        let stranger_at: Vec<Multiaddr> = vec![
            format!("/ip4/127.0.0.1/udp/{stranger_port}/quic-v1")
                .parse()
                .unwrap(),
        ];
        inspect(&old, move |n| {
            n.candidates.entry(stranger_peer).or_default();
            n.candidate_found
                .insert(stranger_peer, (Budget::Move, Instant::now()));
        })
        .await;
        old.dial(stranger_at, None).await.unwrap();
        tokio::time::timeout(Duration::from_secs(20), async {
            while !old
                .move_candidates()
                .await
                .unwrap()
                .contains(&stranger_peer)
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("the old computer reached the stranger");
        let refused = old
            .streams()
            .open_stream(stranger_peer, protocol.clone())
            .await;
        assert!(
            matches!(
                refused,
                Err(libp2p_stream::OpenStreamError::UnsupportedProtocol(_))
            ),
            "{refused:?}"
        );

        // Another code, or none: what the earlier one found is no candidate.
        old.seek(Some([8; 32]), None).await.unwrap();
        assert!(old.move_candidates().await.unwrap().is_empty());
        old.seek(None, None).await.unwrap();
        assert!(old.move_candidates().await.unwrap().is_empty());
        stranger.shutdown().await.unwrap();
        old.shutdown().await.unwrap();
        new.shutdown().await.unwrap();
    }
}
