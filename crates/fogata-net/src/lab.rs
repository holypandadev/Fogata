//! Pieces for the network lab, never used by the app: a minimal DHT node standing in for the
//! public Amino DHT inside an isolated test network (it can write down every announcement it
//! takes, or answer every lookup with made-up announcements), a peer that lies about the
//! addresses it sees, and a way to open sealed local addresses found in announcements with
//! the secrets of a device.

use std::{borrow::Cow, fs::File, io::Write, path::PathBuf, time::Duration};

use futures::{AsyncWriteExt, StreamExt};
use libp2p::{
    Multiaddr, PeerId, StreamProtocol, SwarmBuilder, identify,
    identity::Keypair,
    kad,
    multiaddr::Protocol,
    noise,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};

use crate::{NetError, lan, rendezvous::AMINO_PROTOCOL};

#[derive(NetworkBehaviour)]
struct LabBehaviour {
    identify: identify::Behaviour,
    kad: kad::Behaviour<Recording>,
}

/// A DHT node's store that also writes down every announcement (provider record) it takes,
/// one line each: the key in hex, the provider and the addresses it gave. With `lies`, it
/// answers every lookup with that many announcements of peers made up for that answer, all
/// at the addresses given, and never with the real ones.
struct Recording {
    store: kad::store::MemoryStore,
    file: Option<File>,
    lies: Option<(usize, Vec<Multiaddr>)>,
}

impl kad::store::RecordStore for Recording {
    type RecordsIter<'a> = <kad::store::MemoryStore as kad::store::RecordStore>::RecordsIter<'a>;
    type ProvidedIter<'a> = <kad::store::MemoryStore as kad::store::RecordStore>::ProvidedIter<'a>;

    fn get(&self, k: &kad::RecordKey) -> Option<Cow<'_, kad::Record>> {
        self.store.get(k)
    }

    fn put(&mut self, r: kad::Record) -> kad::store::Result<()> {
        self.store.put(r)
    }

    fn remove(&mut self, k: &kad::RecordKey) {
        self.store.remove(k)
    }

    fn records(&self) -> Self::RecordsIter<'_> {
        self.store.records()
    }

    fn add_provider(&mut self, record: kad::ProviderRecord) -> kad::store::Result<()> {
        if let Some(file) = &mut self.file {
            let addresses: Vec<String> = record.addresses.iter().map(|a| a.to_string()).collect();
            let _ = writeln!(
                file,
                "{} {} {}",
                hex::encode(record.key.as_ref()),
                record.provider,
                addresses.join(" ")
            );
        }
        self.store.add_provider(record)
    }

    fn providers(&self, key: &kad::RecordKey) -> Vec<kad::ProviderRecord> {
        match &self.lies {
            Some((count, addresses)) => (0..*count)
                .map(|_| kad::ProviderRecord {
                    key: key.clone(),
                    provider: PeerId::random(),
                    expires: None,
                    addresses: addresses.clone(),
                })
                .collect(),
            None => self.store.providers(key),
        }
    }

    fn provided(&self) -> Self::ProvidedIter<'_> {
        self.store.provided()
    }

    fn remove_provider(&mut self, k: &kad::RecordKey, p: &PeerId) {
        self.store.remove_provider(k, p)
    }
}

/// Runs a DHT server on `port` until the process ends. `on_listen` receives every listen
/// address (with the peer id) so a test script can hand it to the clients. `peers` are
/// other lab DHT nodes to connect to. With `records`, every announcement the node takes is
/// added to that file as it comes, as a stranger running a DHT node would see it; with
/// `identified`, the protocols each peer lists in identify, one line each: the peer, then
/// the protocols. With `lies`, every lookup it is asked gets that many made-up peers, new
/// ones each time, at the addresses given (see [`Recording`]): what a node on the way to a
/// key can answer.
pub async fn run_lab_dht(
    port: u16,
    peers: Vec<Multiaddr>,
    records: Option<PathBuf>,
    identified: Option<PathBuf>,
    lies: Option<(usize, Vec<Multiaddr>)>,
    on_listen: impl Fn(Multiaddr),
) -> Result<(), NetError> {
    let open = |path: PathBuf| File::options().create(true).append(true).open(path);
    let file = records
        .map(open)
        .transpose()
        .map_err(|e| NetError::Setup(e.to_string()))?;
    let mut identified = identified
        .map(open)
        .transpose()
        .map_err(|e| NetError::Setup(e.to_string()))?;
    let setup = |e: &dyn std::fmt::Display| NetError::Setup(e.to_string());
    let mut swarm = SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .map_err(|e| setup(&e))?
        .with_quic()
        .with_behaviour(|key| {
            let peer_id = key.public().to_peer_id();
            let mut kad = kad::Behaviour::with_config(
                peer_id,
                Recording {
                    store: kad::store::MemoryStore::new(peer_id),
                    file,
                    lies,
                },
                kad::Config::new(StreamProtocol::new(AMINO_PROTOCOL)),
            );
            kad.set_mode(Some(kad::Mode::Server));
            LabBehaviour {
                identify: identify::Behaviour::new(identify::Config::new(
                    "/fogata-lab/1".into(),
                    key.public(),
                )),
                kad,
            }
        })
        .map_err(|e| setup(&e))?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(300)))
        .build();

    for listen in [
        format!("/ip4/0.0.0.0/udp/{port}/quic-v1"),
        format!("/ip4/0.0.0.0/tcp/{port}"),
    ] {
        swarm
            .listen_on(listen.parse().expect("static listen address"))
            .map_err(|e| setup(&e))?;
    }
    for peer in peers {
        if let Some(Protocol::P2p(id)) = peer.iter().last() {
            swarm.behaviour_mut().kad.add_address(&id, peer.clone());
        }
        let _ = swarm.dial(peer);
    }

    let local = *swarm.local_peer_id();
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                on_listen(address.with(Protocol::P2p(local)));
            }
            SwarmEvent::Behaviour(LabBehaviourEvent::Identify(identify::Event::Received {
                peer_id,
                info,
                ..
            })) => {
                if let Some(file) = &mut identified {
                    let listed: Vec<&str> = info.protocols.iter().map(|p| p.as_ref()).collect();
                    let _ = writeln!(file, "{peer_id} {}", listed.join(" "));
                }
                // Clients run in DHT client mode; only route to other lab servers.
                if info.protocols.iter().any(|p| p.as_ref() == AMINO_PROTOCOL) {
                    for address in info.listen_addrs {
                        swarm.behaviour_mut().kad.add_address(&peer_id, address);
                    }
                }
            }
            _ => {}
        }
    }
}

/// The identify protocol, which tells a peer the address its connection comes from.
const IDENTIFY: &str = "/ipfs/id/1.0.0";

/// Runs a peer that dials `dial` (again every 10 seconds while it has no connection) and,
/// whenever a peer asks it who it is, says that peer's address is `says`, whatever it really
/// is: what a peer posing as a DHT node can do. `on_event` receives a line for each
/// connection and each lie told.
pub async fn run_lab_liar(
    dial: Vec<Multiaddr>,
    says: Multiaddr,
    on_event: impl Fn(String),
) -> Result<(), NetError> {
    let setup = |e: &dyn std::fmt::Display| NetError::Setup(e.to_string());
    let key = Keypair::generate_ed25519();
    let answer = identify_answer(&key, &says);
    let mut swarm = SwarmBuilder::with_existing_identity(key)
        .with_tokio()
        .with_quic()
        .with_behaviour(|_| libp2p_stream::Behaviour::new())
        .map_err(|e| setup(&e))?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();
    swarm
        .listen_on(
            "/ip4/0.0.0.0/udp/0/quic-v1"
                .parse()
                .expect("static listen address"),
        )
        .map_err(|e| setup(&e))?;
    let mut asked = swarm
        .behaviour()
        .new_control()
        .accept(StreamProtocol::new(IDENTIFY))
        .map_err(|e| setup(&e))?;
    let mut tick = tokio::time::interval(Duration::from_secs(10));
    loop {
        tokio::select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                    on_event(format!("connected to {peer_id} at {}", endpoint.get_remote_address()));
                }
                SwarmEvent::OutgoingConnectionError { error, .. } => {
                    on_event(format!("could not connect: {error}"));
                }
                _ => {}
            },
            Some((peer, mut stream)) = asked.next() => {
                on_event(format!("told {peer} it is at {says}"));
                let answer = answer.clone();
                tokio::spawn(async move {
                    let _ = stream.write_all(&answer).await;
                    let _ = stream.close().await;
                });
            }
            _ = tick.tick() => {
                if swarm.connected_peers().next().is_none() {
                    for address in &dial {
                        let _ = swarm.dial(address.clone());
                    }
                }
            }
        }
    }
}

/// Runs a stranger that keeps `connections` connections open to the device at `dial`, all
/// from `listen` (an address of its own, so several copies come from several addresses), and
/// says something cheap over the member protocol every half minute: what one machine of a
/// flood of incoming connections does. `on_event` receives a line whenever the number of
/// open connections changes and for each answer.
pub async fn run_lab_flood(
    dial: Multiaddr,
    listen: Multiaddr,
    connections: usize,
    on_event: impl Fn(String),
) -> Result<(), NetError> {
    use fogata_proto::{MEMBER_PROTOCOL, MemberRequest, MemberResponse};
    use libp2p::request_response::{self, ProtocolSupport};

    let setup = |e: &dyn std::fmt::Display| NetError::Setup(e.to_string());
    let mut swarm = SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_quic()
        .with_behaviour(|_| {
            request_response::Behaviour::with_codec(
                crate::behaviour::message_codec::<MemberRequest, MemberResponse>(),
                [(StreamProtocol::new(MEMBER_PROTOCOL), ProtocolSupport::Full)],
                request_response::Config::default(),
            )
        })
        .map_err(|e| setup(&e))?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(600)))
        .build();
    swarm.listen_on(listen).map_err(|e| setup(&e))?;
    let mut open = 0usize;
    let mut dialling = 0usize;
    let mut device = None;
    let mut redial = tokio::time::interval(Duration::from_secs(2));
    let mut say = tokio::time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. }
                    if endpoint.is_dialer() =>
                {
                    dialling = dialling.saturating_sub(1);
                    open += 1;
                    on_event(format!("open {open}"));
                    if device.is_none() {
                        device = Some(peer_id);
                        say.reset_immediately();
                    }
                }
                SwarmEvent::ConnectionClosed { endpoint, .. } if endpoint.is_dialer() => {
                    open = open.saturating_sub(1);
                    on_event(format!("open {open}"));
                }
                SwarmEvent::OutgoingConnectionError { .. } => {
                    dialling = dialling.saturating_sub(1);
                }
                SwarmEvent::Behaviour(request_response::Event::Message {
                    message: request_response::Message::Response { response, .. },
                    ..
                }) => on_event(format!("answered {response:?}")),
                _ => {}
            },
            _ = redial.tick() => {
                while open + dialling < connections {
                    let opts = libp2p::swarm::dial_opts::DialOpts::unknown_peer_id()
                        .address(dial.clone())
                        .build();
                    if swarm.dial(opts).is_err() {
                        break;
                    }
                    dialling += 1;
                }
            }
            _ = say.tick() => {
                if let Some(device) = device.filter(|d| swarm.is_connected(d)) {
                    swarm.behaviour_mut().send_request(
                        &device,
                        MemberRequest::Presence {
                            hidden: false,
                            away: false,
                        },
                    );
                }
            }
        }
    }
}

/// Keeps up to `per_address` TCP connections open to `dial` from each address in `from`, never
/// sending anything on them, and opens a new one whenever one closes: what a machine trying to
/// use up a device's sockets does. `on_event` receives, every second, how many are open and
/// how many the device has closed so far.
pub async fn run_lab_silent(
    dial: std::net::SocketAddr,
    from: Vec<std::net::IpAddr>,
    per_address: usize,
    on_event: impl Fn(String),
) -> Result<(), NetError> {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::io::AsyncReadExt;

    let open = Arc::new(AtomicUsize::new(0));
    let closed = Arc::new(AtomicUsize::new(0));
    for source in from {
        let (open, closed) = (open.clone(), closed.clone());
        let slots = Arc::new(tokio::sync::Semaphore::new(per_address));
        tokio::spawn(async move {
            loop {
                let Ok(slot) = slots.clone().acquire_owned().await else {
                    return;
                };
                let socket = match source {
                    std::net::IpAddr::V4(_) => tokio::net::TcpSocket::new_v4(),
                    std::net::IpAddr::V6(_) => tokio::net::TcpSocket::new_v6(),
                };
                let Ok(socket) = socket else {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                };
                if socket.bind((source, 0).into()).is_err() {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                let Ok(Ok(mut stream)) =
                    tokio::time::timeout(Duration::from_secs(5), socket.connect(dial)).await
                else {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                };
                open.fetch_add(1, Ordering::Relaxed);
                let (open, closed) = (open.clone(), closed.clone());
                tokio::spawn(async move {
                    // Whatever the device sends, until it closes the connection.
                    let mut buffer = [0u8; 512];
                    while matches!(stream.read(&mut buffer).await, Ok(n) if n > 0) {}
                    open.fetch_sub(1, Ordering::Relaxed);
                    closed.fetch_add(1, Ordering::Relaxed);
                    drop(slot);
                });
            }
        });
    }
    let mut every = tokio::time::interval(Duration::from_secs(1));
    loop {
        every.tick().await;
        on_event(format!(
            "open {} closed {}",
            open.load(Ordering::Relaxed),
            closed.load(Ordering::Relaxed)
        ));
    }
}

/// Answers every mDNS query for libp2p peers it hears on the local network of `interface`
/// with `answers` responses, each naming `per_answer` made-up peers that never expire, and
/// also sends `unasked` such responses a second answering nothing: what a neighbour flooding
/// devices with made-up peers does. `on_event` receives, every second, how many responses
/// it sent.
pub async fn run_lab_mdns_flood(
    interface: std::net::Ipv4Addr,
    answers: usize,
    per_answer: usize,
    unasked: usize,
    on_event: impl Fn(String),
) -> Result<(), NetError> {
    let setup = |e: &dyn std::fmt::Display| NetError::Setup(e.to_string());
    let group = std::net::Ipv4Addr::new(224, 0, 0, 251);
    let socket = tokio::net::UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 5353))
        .await
        .map_err(|e| setup(&e))?;
    socket
        .join_multicast_v4(group, interface)
        .map_err(|e| setup(&e))?;
    let to = std::net::SocketAddr::from((group, 5353));
    let mut sent = 0usize;
    let mut buffer = [0u8; 4096];
    let mut every = tokio::time::interval(Duration::from_secs(1));
    let mut unasked_tick =
        tokio::time::interval(Duration::from_millis(1000 / unasked.clamp(1, 1000) as u64));
    loop {
        tokio::select! {
            got = socket.recv_from(&mut buffer) => {
                let Ok((len, _)) = got else { continue };
                if let Some(id) = mdns_query_id(&buffer[..len]) {
                    for _ in 0..answers {
                        let _ = socket.send_to(&mdns_flood_answer(id, interface, per_answer), to).await;
                        sent += 1;
                    }
                }
            }
            _ = unasked_tick.tick(), if unasked > 0 => {
                let mut id = [0u8; 2];
                let _ = fogata_crypto::random_bytes(&mut id);
                let answer = mdns_flood_answer(u16::from_le_bytes(id), interface, per_answer);
                let _ = socket.send_to(&answer, to).await;
                sent += 1;
            }
            _ = every.tick() => on_event(format!("sent {sent}")),
        }
    }
}

/// The id of an mDNS query for libp2p peers (`_p2p._udp.local`), if `packet` is one.
fn mdns_query_id(packet: &[u8]) -> Option<u16> {
    let questions = u16::from_be_bytes([*packet.get(4)?, *packet.get(5)?]);
    let flags = u16::from_be_bytes([packet[2], packet[3]]);
    let wanted = mdns_name(&[b"_p2p", b"_udp", b"local"]);
    (flags & 0x8000 == 0 && questions > 0 && packet.get(12..12 + wanted.len())? == wanted)
        .then(|| u16::from_be_bytes([packet[0], packet[1]]))
}

fn mdns_name(labels: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    for label in labels {
        out.push(label.len() as u8);
        out.extend_from_slice(label);
    }
    out.push(0);
    out
}

/// An mDNS response to query `id` naming `peers` made-up libp2p peers, each at a QUIC
/// address on `interface`, with the longest TTL there is.
fn mdns_flood_answer(id: u16, interface: std::net::Ipv4Addr, peers: usize) -> Vec<u8> {
    fn record(out: &mut Vec<u8>, name: &[u8], kind: u16, data: &[u8]) {
        out.extend_from_slice(name);
        out.extend_from_slice(&kind.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&u32::MAX.to_be_bytes());
        out.extend_from_slice(&(data.len() as u16).to_be_bytes());
        out.extend_from_slice(data);
    }
    let service = mdns_name(&[b"_p2p", b"_udp", b"local"]);
    let named: Vec<(Vec<u8>, Vec<u8>)> = (0..peers)
        .map(|_| {
            let peer = PeerId::random();
            let mut label = [0u8; 8];
            let _ = fogata_crypto::random_bytes(&mut label);
            let name = mdns_name(&[hex::encode(label).as_bytes(), b"_p2p", b"_udp", b"local"]);
            let text = format!("dnsaddr=/ip4/{interface}/udp/4001/quic-v1/p2p/{peer}");
            let mut txt = vec![text.len() as u8];
            txt.extend_from_slice(text.as_bytes());
            (name, txt)
        })
        .collect();
    let mut out = Vec::new();
    out.extend_from_slice(&id.to_be_bytes());
    out.extend_from_slice(&0x8400u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&(peers as u16).to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&(peers as u16).to_be_bytes());
    for (name, _) in &named {
        record(&mut out, &service, 12, name);
    }
    for (name, txt) in &named {
        record(&mut out, name, 16, txt);
    }
    out
}

/// An identify message (protobuf, after its length as an unsigned varint) with this peer's
/// key and `observed` as the address it sees the other at.
fn identify_answer(key: &Keypair, observed: &Multiaddr) -> Vec<u8> {
    fn varint(out: &mut Vec<u8>, mut n: usize) {
        while n >= 0x80 {
            out.push((n as u8 & 0x7f) | 0x80);
            n >>= 7;
        }
        out.push(n as u8);
    }
    fn field(out: &mut Vec<u8>, number: u8, bytes: &[u8]) {
        out.push(number << 3 | 2);
        varint(out, bytes.len());
        out.extend_from_slice(bytes);
    }
    let mut message = Vec::new();
    field(&mut message, 1, &key.public().encode_protobuf());
    field(&mut message, 4, &observed.to_vec());
    field(&mut message, 5, b"/fogata-lab/1");
    field(&mut message, 6, b"fogata-lab-liar");
    let mut out = Vec::new();
    varint(&mut out, message.len());
    out.extend_from_slice(&message);
    out
}

/// Whether `address` is a sealed name holding local addresses (see lan.rs).
pub fn lab_is_sealed(address: &Multiaddr) -> bool {
    lan::is_name(address)
}

/// The local addresses sealed in `name` for whoever knows `rendezvous_secret` (a device's
/// friend rendezvous secret, or a group's for one epoch); nothing for any other secret.
pub fn lab_open_sealed(name: &Multiaddr, rendezvous_secret: &[u8; 32]) -> Vec<Multiaddr> {
    lan::open(name, &[lan::key_of(rendezvous_secret)])
}

/// The DHT keys, in hex as a lab DHT node writes them down, under which a device looks for
/// whoever announces under `rendezvous_secret` around `unix_secs`.
pub fn lab_lookup_keys(rendezvous_secret: &[u8; 32], unix_secs: u64) -> Vec<String> {
    crate::rendezvous::lookup_keys(&[*rendezvous_secret], unix_secs)
        .iter()
        .map(|key| hex::encode(key.as_ref()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lie_reads_back_as_an_identify_message() {
        let key = Keypair::generate_ed25519();
        let says: Multiaddr = "/ip4/203.0.113.66/udp/41000/quic-v1".parse().unwrap();
        let answer = identify_answer(&key, &says);
        let (length, message) = (usize::from(answer[0]), &answer[1..]);
        assert_eq!(length, message.len());
        // The public key first, then the address said, as fields 1 and 4.
        let public = key.public().encode_protobuf();
        assert_eq!(message[0], 1 << 3 | 2);
        assert_eq!(&message[2..2 + public.len()], public.as_slice());
        let at = 2 + public.len();
        assert_eq!(message[at], 4 << 3 | 2);
        let observed = &message[at + 2..at + 2 + usize::from(message[at + 1])];
        assert_eq!(Multiaddr::try_from(observed.to_vec()).unwrap(), says);
    }

    #[test]
    fn a_sealed_name_opens_only_with_its_secret() {
        let secret = [3u8; 32];
        let ip = std::net::Ipv4Addr::new(192, 168, 2, 123);
        let name = lan::seal(&lan::key_of(&secret), 31000, &[ip]).unwrap();
        assert!(lab_is_sealed(&name));
        assert!(!name.to_string().contains("192.168"));
        let opened = lab_open_sealed(&name, &secret);
        assert_eq!(opened.len(), 1);
        assert!(
            opened[0]
                .to_string()
                .starts_with("/ip4/192.168.2.123/udp/31000")
        );
        assert!(lab_open_sealed(&name, &[4u8; 32]).is_empty());
    }
}
