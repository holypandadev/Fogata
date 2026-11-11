//! What identify says to each peer: Fogata's own protocols are listed only to peers this
//! device chose to reach or that proved themselves, and told again a moment later; and the
//! capability level each friend and member says it speaks.

use std::time::{Duration, Instant};

use libp2p::PeerId;

use super::Node;
use crate::{NodeEvent, caps};

/// How long after a peer is first shown Fogata's protocols they are told again.
const PUSH_AGAIN: Duration = Duration::from_secs(5);

impl Node {
    /// Lists Fogata's own protocols to `peer` in identify from now on, for as long as it
    /// stays connected, and tells it them at once if they were not: for a peer this device
    /// chose to reach, or that proved it is a member or a friend (see behaviour.rs).
    pub(super) fn show_protocols(&mut self, peer: PeerId) {
        if crate::locked(&self.shown).insert(peer) {
            self.swarm.behaviour_mut().identify.push([peer]);
            // A push on a connection whose first identify has not ended is dropped at the
            // other end, which would then learn them only at the next periodic identify,
            // minutes away: told again a moment later.
            self.push_again.insert(peer, Instant::now());
        }
    }

    /// Tells the protocols again to peers shown them a moment ago (see `show_protocols`),
    /// and forgets those shown that never connected (a peer found under a move's code).
    pub(super) fn push_shown_again(&mut self) {
        let now = Instant::now();
        let due: Vec<PeerId> = self
            .push_again
            .iter()
            .filter(|(_, at)| now.duration_since(**at) >= PUSH_AGAIN)
            .map(|(peer, _)| *peer)
            .collect();
        let connected: Vec<PeerId> = due
            .iter()
            .copied()
            .filter(|p| self.swarm.is_connected(p))
            .collect();
        for peer in &due {
            self.push_again.remove(peer);
        }
        if !connected.is_empty() {
            self.swarm.behaviour_mut().identify.push(connected);
        }
        let swarm = &self.swarm;
        let waiting = &self.push_again;
        crate::locked(&self.shown).retain(|p| swarm.is_connected(p) || waiting.contains_key(p));
    }

    /// The capability level `peer` says it speaks (see caps.rs), once it identified itself.
    pub(super) fn peer_level(&self, peer: &PeerId) -> Option<u32> {
        crate::locked(&self.advertised)
            .get(peer)
            .map(caps::level_listed)
    }

    /// The capability level `peer` lists, once it shows this device Fogata's protocols at
    /// all: a device shows them only to the peers it chose to reach and to those that proved
    /// something, and one that shows none here says nothing of its version.
    pub(super) fn shown_level(&self, peer: &PeerId) -> Option<u32> {
        crate::locked(&self.advertised)
            .get(peer)
            .filter(|protocols| !protocols.is_empty())
            .map(caps::level_listed)
    }

    /// Raises the capability level remembered for the device of `peer` to the one it says
    /// now, once both are known: the device from a proof of membership or from a friend's
    /// session, the level from identify. Only ever raised (see caps.rs).
    pub(super) fn note_level(&mut self, peer: PeerId) {
        let device = self
            .members
            .get(&peer)
            .map(|m| m.device)
            .or_else(|| self.friend_device(&peer));
        let level = crate::locked(&self.advertised)
            .get(&peer)
            .map(caps::level_listed);
        let (Some(device), Some(level)) = (device, level) else {
            return;
        };
        let raised = {
            let mut levels = crate::locked(&self.levels);
            let at = levels.entry(device).or_insert(caps::FIRST_LEVEL);
            let raised = level > *at;
            *at = (*at).max(level);
            raised
        };
        if raised {
            self.emit(NodeEvent::LevelSeen { device, level });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::DeviceIdentity;
    use fogata_state::GroupLog;
    use libp2p::{Multiaddr, StreamProtocol};

    use super::*;
    use crate::{
        NodeConfig,
        group::GroupSetup,
        node::{start, tests::inspect},
    };

    /// A peer that connects is not told Fogata's protocols until it proves itself (here
    /// the node is told so, as a proof of membership or a friend's session tells it), and
    /// then at once; once it went, it is a stranger again.
    #[tokio::test(flavor = "multi_thread")]
    async fn fogata_protocols_are_listed_to_a_peer_once_it_proved_itself() {
        use futures::StreamExt as _;
        use libp2p::{SwarmBuilder, identify, swarm::SwarmEvent};

        let port = std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let (node, _events) = start(NodeConfig {
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
        })
        .unwrap();
        let mut peer = SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_quic()
            .with_behaviour(|key| {
                identify::Behaviour::new(identify::Config::new(
                    "ipfs/0.1.0".to_string(),
                    key.public(),
                ))
            })
            .unwrap()
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();
        let me = *peer.local_peer_id();
        let address: Multiaddr = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
            .parse()
            .unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        // What the peer is told next, Fogata's protocols among it or not.
        async fn told(
            peer: &mut libp2p::Swarm<identify::Behaviour>,
        ) -> (bool, Vec<StreamProtocol>) {
            tokio::time::timeout(Duration::from_secs(20), async {
                loop {
                    if let SwarmEvent::Behaviour(identify::Event::Received { info, .. }) =
                        peer.select_next_some().await
                    {
                        let fogata = info
                            .protocols
                            .iter()
                            .any(|p| p.as_ref().starts_with(crate::behaviour::FOGATA_PROTOCOLS));
                        return (fogata, info.protocols);
                    }
                }
            })
            .await
            .expect("the node said who it is")
        }

        peer.dial(address.clone()).unwrap();
        let (fogata, listed) = told(&mut peer).await;
        assert!(!fogata, "a stranger was told {listed:?}");
        inspect(&node, move |n| n.show_protocols(me)).await;
        let (fogata, listed) = told(&mut peer).await;
        assert!(fogata, "a peer that proved itself was told only {listed:?}");
        assert!(listed.contains(&StreamProtocol::new(fogata_proto::MEMBER_PROTOCOL)));

        // Gone and back: a stranger again until it proves itself anew.
        let node_peer = inspect(&node, |n| n.local_peer()).await;
        let _ = peer.disconnect_peer_id(node_peer);
        tokio::time::timeout(Duration::from_secs(10), async {
            while !matches!(
                peer.select_next_some().await,
                SwarmEvent::ConnectionClosed {
                    num_established: 0,
                    ..
                }
            ) {}
        })
        .await
        .expect("the connection closed");
        let shown = || inspect(&node, move |n| crate::locked(&n.shown).contains(&me));
        for _ in 0..50 {
            if !shown().await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(!shown().await);
        peer.dial(address).unwrap();
        let (fogata, listed) = told(&mut peer).await;
        assert!(!fogata, "a peer that came back was told {listed:?}");
        node.shutdown().await.unwrap();
    }

    /// A burst of changes on a node is told a connected peer in a push or two, never one
    /// per change: a peer takes only a few streams at once on a connection and drops the
    /// rest, warning of each. Addresses listened on that come and go (a network joined, a
    /// sleep ended, a slot at a member's relay) are not said at all, so not pushed; addresses
    /// confirmed from outside are, and what is told last is right. (A burst of protocols
    /// taken and dropped: `pushes_on_a_connection_are_one_at_a_time` in the vendored
    /// libp2p-identify, and `members_push_to_each_other_a_bounded_number_of_times`.)
    #[tokio::test(flavor = "multi_thread")]
    async fn a_burst_of_changes_is_pushed_in_a_push_or_two() {
        use futures::StreamExt as _;
        use libp2p::{SwarmBuilder, identify, swarm::SwarmEvent};

        let port = std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let (node, _events) = start(NodeConfig {
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
        })
        .unwrap();
        let mut peer = SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_quic()
            .with_behaviour(|key| {
                identify::Behaviour::new(identify::Config::new(
                    "ipfs/0.1.0".to_string(),
                    key.public(),
                ))
            })
            .unwrap()
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();
        let me = *peer.local_peer_id();
        let address: Multiaddr = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
            .parse()
            .unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        // How many times the peer is told who the node is within `within`, or until `done`
        // takes what it was told, and what it was told last.
        async fn told(
            peer: &mut libp2p::Swarm<identify::Behaviour>,
            within: Duration,
            done: impl Fn(&identify::Info) -> bool,
        ) -> (usize, Option<identify::Info>) {
            let mut times = 0;
            let mut last = None;
            let _ = tokio::time::timeout(within, async {
                loop {
                    if let SwarmEvent::Behaviour(identify::Event::Received { info, .. }) =
                        peer.select_next_some().await
                    {
                        times += 1;
                        let enough = done(&info);
                        last = Some(info);
                        if enough {
                            return;
                        }
                    }
                }
            })
            .await;
            (times, last)
        }
        let never = |_: &identify::Info| false;
        peer.dial(address).unwrap();
        let (times, _) = told(&mut peer, Duration::from_secs(3), never).await;
        assert!(times >= 1, "the node never said who it is");
        inspect(&node, move |n| n.show_protocols(me)).await;
        // Told it is shown the protocols, and once more a moment later (see
        // `push_shown_again`).
        let (times, _) = told(&mut peer, PUSH_AGAIN + Duration::from_secs(3), never).await;
        assert!(
            (1..=2).contains(&times),
            "told {times} times on being shown"
        );

        // Thirty addresses listened on, then dropped, at once.
        let listeners = inspect(&node, |n| {
            (0..30)
                .filter_map(|_| {
                    n.swarm
                        .listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap())
                        .ok()
                })
                .collect::<Vec<_>>()
        })
        .await;
        assert_eq!(listeners.len(), 30);
        tokio::time::sleep(Duration::from_millis(300)).await;
        inspect(&node, move |n| {
            for listener in listeners {
                n.swarm.remove_listener(listener);
            }
        })
        .await;
        let (times, _) = told(&mut peer, Duration::from_secs(3), never).await;
        assert_eq!(times, 0, "addresses never said were pushed {times} times");

        // Fifteen addresses confirmed from outside at once (identify says twenty at most).
        let confirmed: Vec<Multiaddr> = (1..=15)
            .map(|i| {
                format!("/ip4/203.0.113.{i}/udp/4001/quic-v1")
                    .parse()
                    .unwrap()
            })
            .collect();
        let added = confirmed.clone();
        inspect(&node, move |n| {
            for address in added {
                n.swarm.add_external_address(address);
            }
        })
        .await;
        let all = confirmed.clone();
        let (times, last) = told(&mut peer, Duration::from_secs(5), move |info| {
            all.iter().all(|a| info.listen_addrs.contains(a))
        })
        .await;
        assert!(
            (1..=2).contains(&times),
            "fifteen addresses pushed {times} times"
        );
        let last = last.expect("told the addresses");
        assert!(confirmed.iter().all(|a| last.listen_addrs.contains(a)));

        node.shutdown().await.unwrap();
    }

    /// Two members that connect push to each other a bounded number of times, a burst of
    /// protocols taken and dropped included, and never on each push of the other: each
    /// tells the other its protocols when it proved itself, and once more a moment later
    /// (see `show_protocols`), and then only when something it says changes.
    #[tokio::test(flavor = "multi_thread")]
    async fn members_push_to_each_other_a_bounded_number_of_times() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        let config = |identity: Arc<DeviceIdentity>, port: u16, peers: Vec<Multiaddr>| NodeConfig {
            identity,
            groups: vec![GroupSetup::Member(log.clone())],
            friends: None,
            listen_port: port,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers,
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
        let (node_a, _events_a) = start(config(ana.clone(), port, Vec::new())).unwrap();
        let (node_b, mut events_b) = start(config(bea.clone(), 0, vec![at])).unwrap();
        let ana_peer = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                match events_b.recv().await {
                    Some(NodeEvent::MemberConnected { device, peer, .. })
                        if device == ana.public_key() =>
                    {
                        return peer;
                    }
                    Some(_) => {}
                    None => panic!("Bea's node stopped"),
                }
            }
        })
        .await
        .expect("Bea reached Ana");
        tokio::spawn(async move { while events_b.recv().await.is_some() {} });
        let bea_peer = inspect(&node_b, |n| n.local_peer()).await;
        // A protocol of Fogata's taken and dropped thirty times on each, the connection
        // running in between, and kept at last.
        for node in [&node_a, &node_b] {
            let streams = node.streams();
            let kept = StreamProtocol::new("/fogata/burst/1");
            for _ in 0..30 {
                drop(streams.clone().accept(kept.clone()).unwrap());
                let _ = node.addresses().await;
            }
            std::mem::forget(streams.clone().accept(kept).unwrap());
        }
        tokio::time::sleep(PUSH_AGAIN * 2 + Duration::from_secs(2)).await;
        let to_bea = inspect(&node_a, move |n| n.pushed.get(&bea_peer).copied()).await;
        let to_ana = inspect(&node_b, move |n| n.pushed.get(&ana_peer).copied()).await;
        // Shown, again a moment later, and the burst in a push or two (one open, one more).
        for (pushed, to) in [(to_bea, "Bea"), (to_ana, "Ana")] {
            let pushed = pushed.unwrap_or(0);
            assert!(
                (1..=6).contains(&pushed),
                "pushed {pushed} times to {to} in {}s",
                (PUSH_AGAIN * 2).as_secs() + 2
            );
        }
        // And nothing more while nothing changes.
        tokio::time::sleep(PUSH_AGAIN * 2).await;
        let again = inspect(&node_a, move |n| n.pushed.get(&bea_peer).copied()).await;
        assert_eq!(again, to_bea, "pushed to Bea while nothing changed");
        node_a.shutdown().await.unwrap();
        node_b.shutdown().await.unwrap();
    }
}
