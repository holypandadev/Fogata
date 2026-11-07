//! Entry points for fuzzing what reaches the media bridge: datagrams and media streams from
//! members, and packets to the TURN endpoint. Each takes any bytes and must neither panic
//! nor take long; the seeds are well-formed examples to start from. Built only with the
//! `fuzzing` feature; the runs live in fogata-core.

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use bytes::Bytes;
use fogata_net::PeerId;
use tokio::sync::Mutex;
use turn::{
    allocation::allocation_manager::{Manager, ManagerConfig},
    relay::RelayAddressGenerator,
    server::request::Request,
};
use webrtc_util::Conn;

use crate::{
    bridge::{self, Kind, LocalAuth, REALM},
    parts::{self, Reassembly},
};

/// Members the runs' packets come from.
fn members() -> &'static [PeerId] {
    static MEMBERS: OnceLock<Vec<PeerId>> = OnceLock::new();
    MEMBERS.get_or_init(|| (0..4).map(|_| PeerId::random()).collect())
}

fn addr(bytes: &[u8]) -> SocketAddr {
    SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])),
        u16::from_be_bytes([bytes[4], bytes[5]]),
    )
}

/// Media packets from members, as datagrams arrive. First byte 0: packets one after the
/// other, each after the member that sent it (its low two bits; its top bits, how many
/// tenths of a second after the one before) and its length (2 bytes): read whole or as parts,
/// which are put back together. Any other first byte: the largest datagram (2 bytes), the
/// packet's number (2), both addresses (6 each) and a packet to send in parts, which must
/// come back whole.
pub fn parts(data: &[u8]) {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let start = Instant::now();
    if mode == 0 {
        let mut reassembly = Reassembly::default();
        let mut now = start;
        let mut rest = rest;
        let mut largest = 0;
        while let Some((head, tail)) = rest.split_at_checked(3) {
            let len = usize::from(u16::from_be_bytes([head[1], head[2]]));
            let Some((packet, tail)) = tail.split_at_checked(len) else {
                break;
            };
            rest = tail;
            now += Duration::from_millis(100 * u64::from(head[0] >> 2));
            largest = largest.max(packet.len());
            let packet = Bytes::copy_from_slice(packet);
            if let Some((_, _, payload)) = bridge::decode(&packet) {
                assert!(payload.len() < packet.len());
            }
            if let Some(part) = parts::decode(&packet) {
                let peer = members()[usize::from(head[0] & 3)];
                if let Some((_, _, whole)) = reassembly.add(peer, part, now) {
                    assert!(whole.len() <= 4 * largest);
                }
            }
        }
        return;
    }
    let Some((head, payload)) = rest.split_at_checked(16) else {
        return;
    };
    let max = usize::from(u16::from_be_bytes([head[0], head[1]]));
    let id = u16::from_be_bytes([head[2], head[3]]);
    let (src, dst) = (addr(&head[4..10]), addr(&head[10..16]));
    // The bridge splits only packets too long for one datagram: always into two or more.
    let Some(split) = parts::split(src, dst, payload, max, id).filter(|s| s.len() > 1) else {
        return;
    };
    let mut reassembly = Reassembly::default();
    let mut whole = None;
    for part in &split {
        assert!(part.len() <= max);
        let part = parts::decode(part).expect("a part reads");
        whole = reassembly.add(members()[0], part, start);
    }
    let (from, to, packet) = whole.expect("every part came");
    assert_eq!((from, to, &packet[..]), (src, dst, payload));
}

/// The user and the password of the runs' TURN server.
const USER: &str = "fogata";
const PASSWORD: &str = "fuzz";
/// A nonce the runs' TURN server gave earlier, so requests that name it are checked further.
const NONCE: &str = "0123456789abcdef";

/// A relay that holds nothing: what is sent through it goes nowhere, and nothing arrives.
struct Nowhere(SocketAddr);

#[async_trait]
impl Conn for Nowhere {
    async fn connect(&self, _addr: SocketAddr) -> webrtc_util::Result<()> {
        Ok(())
    }

    async fn recv(&self, _buf: &mut [u8]) -> webrtc_util::Result<usize> {
        Err(webrtc_util::Error::ErrClosedListener)
    }

    async fn recv_from(&self, _buf: &mut [u8]) -> webrtc_util::Result<(usize, SocketAddr)> {
        Err(webrtc_util::Error::ErrClosedListener)
    }

    async fn send(&self, buf: &[u8]) -> webrtc_util::Result<usize> {
        Ok(buf.len())
    }

    async fn send_to(&self, buf: &[u8], _target: SocketAddr) -> webrtc_util::Result<usize> {
        Ok(buf.len())
    }

    fn local_addr(&self) -> webrtc_util::Result<SocketAddr> {
        Ok(self.0)
    }

    fn remote_addr(&self) -> Option<SocketAddr> {
        None
    }

    async fn close(&self) -> webrtc_util::Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &(dyn std::any::Any + Send + Sync) {
        self
    }
}

struct NowhereRelays;

#[async_trait]
impl RelayAddressGenerator for NowhereRelays {
    fn validate(&self) -> Result<(), turn::Error> {
        Ok(())
    }

    async fn allocate_conn(
        &self,
        _use_ipv4: bool,
        requested_port: u16,
    ) -> Result<(Arc<dyn Conn + Send + Sync>, SocketAddr), turn::Error> {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 1, 2, 1)), requested_port);
        Ok((Arc::new(Nowhere(addr)), addr))
    }
}

/// Packets to the TURN endpoint from the local WebRTC engine, one after the other, each
/// after its length (2 bytes): those the endpoint lets through go to a TURN server like the
/// bridge's.
pub fn turn(data: &[u8]) {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("a runtime")
    });
    let mut packets = Vec::new();
    let mut rest = data;
    while let Some((len, tail)) = rest.split_at_checked(2) {
        let len = usize::from(u16::from_be_bytes([len[0], len[1]]));
        let Some((packet, tail)) = tail.split_at_checked(len) else {
            break;
        };
        rest = tail;
        if bridge::kind(packet) != Kind::Neither {
            packets.push(packet.to_vec());
        }
    }
    if packets.is_empty() {
        return;
    }
    runtime.block_on(async {
        let manager = Arc::new(Manager::new(ManagerConfig {
            relay_addr_generator: Box::new(NowhereRelays),
            alloc_close_notify: None,
        }));
        let nonces = Arc::new(Mutex::new(HashMap::from([(
            NONCE.to_string(),
            tokio::time::Instant::now(),
        )])));
        let auth = Arc::new(LocalAuth::new(USER.into(), PASSWORD.into(), false));
        let endpoint: Arc<dyn Conn + Send + Sync> =
            Arc::new(Nowhere("127.0.0.1:3478".parse().expect("an address")));
        for packet in packets {
            let mut request = Request {
                conn: endpoint.clone(),
                src_addr: "127.0.0.1:50000".parse().expect("an address"),
                buff: packet,
                allocation_manager: manager.clone(),
                nonces: nonces.clone(),
                auth_handler: auth.clone(),
                realm: REALM.into(),
                channel_bind_timeout: Duration::from_secs(600),
            };
            let _ = request.handle_request().await;
        }
        let _ = manager.close().await;
    });
}

pub fn parts_seeds() -> Vec<Vec<u8>> {
    let src: SocketAddr = "10.1.2.1:40000".parse().expect("an address");
    let dst: SocketAddr = "10.3.4.1:40001".parse().expect("an address");
    let payload: Vec<u8> = (0..1500u32).map(|i| i as u8).collect();
    let mut streamed = vec![0u8];
    for (n, part) in parts::split(src, dst, &payload, 1100, 7)
        .expect("parts")
        .into_iter()
        .chain(bridge::encode(src, dst, &payload[..300]))
        .enumerate()
    {
        streamed.push(n as u8 & 3);
        streamed.extend_from_slice(&(part.len() as u16).to_be_bytes());
        streamed.extend_from_slice(&part);
    }
    let mut split = vec![1u8];
    split.extend_from_slice(&1100u16.to_be_bytes());
    split.extend_from_slice(&9u16.to_be_bytes());
    split.extend_from_slice(&[10, 1, 2, 1, 0x9c, 0x40, 10, 3, 4, 1, 0x9c, 0x41]);
    split.extend_from_slice(&payload);
    vec![streamed, split]
}

/// A STUN message of `kind` with `attributes` (type and value), the transaction id `id`.
fn stun(kind: u16, id: u8, attributes: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (kind, value) in attributes {
        body.extend_from_slice(&kind.to_be_bytes());
        body.extend_from_slice(&(value.len() as u16).to_be_bytes());
        body.extend_from_slice(value);
        body.resize(body.len().next_multiple_of(4), 0);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&kind.to_be_bytes());
    out.extend_from_slice(&(body.len() as u16).to_be_bytes());
    out.extend_from_slice(&[0x21, 0x12, 0xa4, 0x42]);
    out.extend_from_slice(&[id; 12]);
    out.extend_from_slice(&body);
    out
}

pub fn turn_seeds() -> Vec<Vec<u8>> {
    const USERNAME: u16 = 0x0006;
    const INTEGRITY: u16 = 0x0008;
    const CHANNEL: u16 = 0x000c;
    const LIFETIME: u16 = 0x000d;
    const PEER: u16 = 0x0012;
    const DATA: u16 = 0x0013;
    const REALM_ATTR: u16 = 0x0014;
    const NONCE_ATTR: u16 = 0x0015;
    const TRANSPORT: u16 = 0x0019;
    // 10.3.4.1:40001, as XOR-PEER-ADDRESS writes it.
    let peer = vec![
        0,
        1,
        0x9c ^ 0x21,
        0x41 ^ 0x12,
        10 ^ 0x21,
        3 ^ 0x12,
        4 ^ 0xa4,
        1 ^ 0x42,
    ];
    let signed = |kind: u16, id: u8, mut attributes: Vec<(u16, Vec<u8>)>| {
        attributes.push((USERNAME, bridge::member_user(USER, &[7; 32]).into_bytes()));
        attributes.push((REALM_ATTR, REALM.as_bytes().to_vec()));
        attributes.push((NONCE_ATTR, NONCE.as_bytes().to_vec()));
        attributes.push((INTEGRITY, vec![0; 20]));
        stun(kind, id, &attributes)
    };
    let messages = vec![
        stun(0x0001, 1, &[]),
        stun(0x0003, 2, &[(TRANSPORT, vec![17, 0, 0, 0])]),
        signed(0x0003, 3, vec![(TRANSPORT, vec![17, 0, 0, 0])]),
        signed(0x0004, 4, vec![(LIFETIME, 600u32.to_be_bytes().to_vec())]),
        signed(0x0008, 5, vec![(PEER, peer.clone())]),
        signed(
            0x0009,
            6,
            vec![(CHANNEL, vec![0x40, 0x01, 0, 0]), (PEER, peer.clone())],
        ),
        stun(0x0016, 7, &[(PEER, peer), (DATA, b"media".to_vec())]),
        [&[0x40, 0x01, 0, 5][..], b"media"].concat(),
    ];
    let mut together = Vec::new();
    for message in &messages {
        together.extend_from_slice(&(message.len() as u16).to_be_bytes());
        together.extend_from_slice(message);
    }
    let mut out: Vec<Vec<u8>> = messages
        .into_iter()
        .map(|m| [&(m.len() as u16).to_be_bytes()[..], &m].concat())
        .collect();
    out.push(together);
    out
}
