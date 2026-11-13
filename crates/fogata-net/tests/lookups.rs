// A DHT node on the way to a key can answer a lookup with any number of made-up providers, at
// an address that never answers. libp2p-kad as published walked those providers as if they
// were DHT nodes: those nearer the key than the nodes that keep the real announcement were
// dialled first, each holding one of the lookup's few places for as long as a dial may take,
// and the real answer came only once they were over (vendor/libp2p-kad/FOGATA.md).

use std::{collections::HashSet, time::Duration};

use futures::StreamExt as _;
use libp2p::{
    Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder,
    kad::{
        self, GetProvidersOk, KBucketKey, ProviderRecord, QueryResult, RecordKey,
        store::{MemoryStore, RecordStore as _},
    },
    noise,
    swarm::SwarmEvent,
    tcp, yamux,
};

/// Providers the node on the way makes up, as many as one answer of the lab's lying nodes.
const MADE_UP: usize = 8;

fn dht_node(mode: kad::Mode) -> Swarm<kad::Behaviour<MemoryStore>> {
    let mut swarm = SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .unwrap()
        .with_behaviour(|key| {
            let peer = key.public().to_peer_id();
            kad::Behaviour::with_config(
                peer,
                MemoryStore::new(peer),
                kad::Config::new(StreamProtocol::new("/ipfs/kad/1.0.0")),
            )
        })
        .unwrap()
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();
    swarm.behaviour_mut().set_mode(Some(mode));
    swarm
}

async fn listen(swarm: &mut Swarm<kad::Behaviour<MemoryStore>>) -> Multiaddr {
    swarm
        .listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap())
        .unwrap();
    loop {
        if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
            return address;
        }
    }
}

fn serve(mut swarm: Swarm<kad::Behaviour<MemoryStore>>) {
    tokio::spawn(async move {
        loop {
            swarm.select_next_some().await;
        }
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lookup_asks_the_nodes_an_answer_names_not_the_providers_it_names() {
    // Where the made-up providers are: connections are taken there and never answered, so a
    // dial to one lasts until it times out.
    let silent = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = silent.local_addr().unwrap().port();
    let silent_address: Multiaddr = format!("/ip4/127.0.0.1/tcp/{port}").parse().unwrap();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = silent.accept().await {
            held.push(socket);
        }
    });

    let mut keeper = dht_node(kad::Mode::Server);
    let mut liar = dht_node(kad::Mode::Server);
    let mut client = dht_node(kad::Mode::Client);
    let keeper_id = *keeper.local_peer_id();
    let liar_id = *liar.local_peer_id();
    let keeper_address = listen(&mut keeper).await;
    let liar_address = listen(&mut liar).await;

    // A key every made-up provider is nearer to than the node that keeps the announcement,
    // as a liar choosing its made-up peers would make them.
    let made_up: Vec<PeerId> = (0..MADE_UP).map(|_| PeerId::random()).collect();
    let key = (0u64..)
        .map(|i| RecordKey::new(&i.to_be_bytes()))
        .find(|key| {
            let target = KBucketKey::new(key.clone());
            let to_keeper = target.distance(&KBucketKey::from(keeper_id));
            made_up
                .iter()
                .all(|p| target.distance(&KBucketKey::from(*p)) < to_keeper)
        })
        .unwrap();

    let member = PeerId::random();
    keeper
        .behaviour_mut()
        .store_mut()
        .add_provider(ProviderRecord::new(
            key.clone(),
            member,
            vec!["/ip4/127.0.0.1/tcp/9".parse().unwrap()],
        ))
        .unwrap();
    for peer in &made_up {
        liar.behaviour_mut()
            .store_mut()
            .add_provider(ProviderRecord::new(
                key.clone(),
                *peer,
                vec![silent_address.clone()],
            ))
            .unwrap();
    }
    // The liar names the keeper as a node nearer the key, as any node on the way does.
    liar.behaviour_mut().add_address(&keeper_id, keeper_address);
    serve(keeper);
    serve(liar);

    client.behaviour_mut().add_address(&liar_id, liar_address);
    let query = client.behaviour_mut().get_providers(key);
    let mut dialled = HashSet::new();
    let found = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match client.select_next_some().await {
                SwarmEvent::Dialing {
                    peer_id: Some(peer),
                    ..
                } => {
                    dialled.insert(peer);
                }
                SwarmEvent::Behaviour(kad::Event::OutboundQueryProgressed {
                    id,
                    result:
                        QueryResult::GetProviders(Ok(GetProvidersOk::FoundProviders {
                            providers, ..
                        })),
                    ..
                }) if id == query && providers.contains(&member) => return,
                _ => {}
            }
        }
    })
    .await;
    let walked = made_up.iter().filter(|p| dialled.contains(p)).count();
    assert!(
        found.is_ok(),
        "the member was not found in 5 s; {walked} made-up providers were dialled as DHT nodes"
    );
    assert_eq!(walked, 0, "made-up providers were dialled as DHT nodes");
}
