//! Entry points for fuzzing what peers send this crate: each takes any bytes, reads them as
//! the node would and must neither panic nor take long. The seeds are well-formed examples
//! to start from. Built only with the `fuzzing` feature; the runs live in fogata-core.

use std::{net::Ipv4Addr, str::FromStr, sync::Arc};

use fogata_crypto::{
    DeviceIdentity, DevicePublicKey, GroupId, GroupKeys, GroupSecret, SealedMessage, SealedProof,
};
use fogata_proto::{
    FRIEND_PROTOCOL, FriendRequest, FriendResponse, GroupPayload, MEMBER_PROTOCOL, MemberRequest,
    MemberResponse, WireMessage, WireProof,
};
use fogata_state::{
    Body, Entry, GroupBanner, GroupLog, GroupPicture, RoomEvent, RoomTracker, Ticket, VoiceRoomId,
};
use futures::{executor::block_on, io::Cursor};
use libp2p::{Multiaddr, PeerId, StreamProtocol, request_response::Codec as _};
use serde_bytes::ByteBuf;

use crate::{
    TypingSignal, addr,
    behaviour::message_codec,
    group::{GroupContext, GroupSetup},
    lan,
    node::{room_input, room_part},
};

/// The clock the runs read: fixed, so the same input always does the same.
const NOW_MS: u64 = 1_800_000_000_000;

/// A device of the runs, the same in every run: 0 is this device.
pub fn device(n: u8) -> DeviceIdentity {
    DeviceIdentity::from_secret_bytes(&[n.wrapping_add(1); 32])
}

/// Group keys the runs use for every group and epoch.
fn keys() -> GroupKeys {
    GroupKeys::derive(
        GroupId::from_bytes(&[9; 16]).expect("16 bytes"),
        0,
        &GroupSecret::from_bytes(&[9; 32]).expect("32 bytes"),
    )
}

/// Entries one after the other, each after its length (4 bytes, big-endian), as group log
/// files keep them.
pub fn framed(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for entry in entries {
        out.extend_from_slice(&(entry.len() as u32).to_be_bytes());
        out.extend_from_slice(entry);
    }
    out
}

/// The pieces of `data` framed as [`framed`] writes them, up to the first that does not fit.
fn unframed(mut data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    while let Some((len, rest)) = data.split_at_checked(4) {
        let len = u32::from_be_bytes(len.try_into().expect("4 bytes")) as usize;
        let Some((piece, rest)) = rest.split_at_checked(len) else {
            break;
        };
        out.push(piece.to_vec());
        data = rest;
    }
    out
}

fn entries_of<B: AsRef<[u8]>>(list: &[B]) {
    for bytes in list.iter().take(4096).map(AsRef::as_ref) {
        if let Ok(entry) = Entry::decode(bytes) {
            assert_eq!(Entry::decode(&entry.encode()).as_ref(), Ok(&entry));
            let _ = entry.signature_valid();
        }
    }
}

fn sealed_of(wire: &WireMessage) {
    if let Ok(sealed) = SealedMessage::try_from(wire) {
        assert_eq!(WireMessage::from(&sealed).ciphertext, wire.ciphertext);
        let _ = sealed.open(&keys());
    }
}

fn addresses_of(list: &[ByteBuf]) {
    for bytes in list.iter().take(64) {
        if let Ok(address) = Multiaddr::try_from(bytes.to_vec()) {
            let _ =
                addr::is_quic(&address) && !addr::is_relayed(&address) && addr::is_lan(&address);
            let _ = lan::is_name(&address);
        }
    }
}

/// What the node reads out of a member request's fields.
fn member_request(request: &MemberRequest) {
    match request {
        MemberRequest::Challenge { tags, .. } => assert!(tags.len() <= 1 << 20),
        MemberRequest::Deliver(wire) => sealed_of(wire),
        MemberRequest::Spread { message, reached } => {
            sealed_of(message);
            let _ = reached.as_chunks::<{ fogata_proto::SHORT_PEER_BYTES }>();
        }
        MemberRequest::Roster {
            group_id,
            heads,
            banner,
            picture,
        } => {
            let _ = GroupId::from_bytes(group_id);
            let _ = (heads, banner, picture);
        }
        MemberRequest::Sync { have, .. } => {
            let _ = have.iter().filter(|h| h.len() == 32).count();
        }
        MemberRequest::Push { entries, .. } => entries_of(entries),
        MemberRequest::Admitted { entries, .. } => entries_of(entries),
        MemberRequest::JoinRequest { ticket } => {
            if let Ok(ticket) = Ticket::decode(ticket) {
                assert_eq!(Ticket::decode(&ticket.encode()).as_ref(), Ok(&ticket));
                let _ = ticket.signature_valid();
            }
        }
        MemberRequest::JoinCommit { entry, binding } => {
            entries_of(&[ByteBuf::from(entry.clone())]);
            if let (Ok(entry), Some(binding)) = (Entry::decode(entry), binding) {
                let _ = entry.join_bound(binding, b"joiner", b"manager");
            }
        }
        MemberRequest::Profile { profile, animation } => {
            let _ = fogata_friends::Profile::decode(profile).map(|p| p.verify());
            if let Some(animation) = animation {
                let _ = fogata_friends::Animation::decode(animation).map(|a| a.verify());
            }
        }
        MemberRequest::RoomSync { .. }
        | MemberRequest::Presence { .. }
        | MemberRequest::OtherSession
        | MemberRequest::KeyProblem { .. }
        | MemberRequest::Signal { .. } => {}
        // Opened only with this device's key, on the node.
        MemberRequest::FriendAsk { sealed } => {
            let _ = sealed.len();
        }
        MemberRequest::Addresses { addresses } => addresses_of(addresses),
        MemberRequest::JoinDeclined { revoke, .. } => {
            if let Some(revoke) = revoke {
                entries_of(std::slice::from_ref(revoke));
            }
        }
        MemberRequest::Direct { messages, .. } => messages.iter().take(1024).for_each(sealed_of),
        MemberRequest::GroupBanner { banner, .. } => {
            if let Ok(banner) = GroupBanner::decode(banner) {
                assert_eq!(GroupBanner::decode(&banner.encode()).as_ref(), Ok(&banner));
                let _ = banner.verify();
            }
        }
        MemberRequest::GroupPicture { picture, .. } => {
            if let Ok(picture) = GroupPicture::decode(picture) {
                assert_eq!(
                    GroupPicture::decode(&picture.encode()).as_ref(),
                    Ok(&picture)
                );
                let _ = picture.verify();
            }
        }
        MemberRequest::RoomEvents { ids, .. } => {
            let _ = ids.as_chunks::<16>();
        }
    }
}

/// What the node reads out of a member answer's fields.
fn member_response(response: &MemberResponse) {
    match response {
        MemberResponse::Proof(wire) => {
            if let Ok(proof) = SealedProof::try_from(wire) {
                let _ = fogata_crypto::MembershipProof::open(
                    &proof,
                    &keys(),
                    b"prover",
                    b"verifier",
                    &[3; fogata_crypto::CHALLENGE_LEN],
                );
            }
        }
        MemberResponse::Roster { peers } => {
            for peer in peers.iter().take(1024) {
                let _ = PeerId::from_bytes(peer);
            }
        }
        MemberResponse::SyncReply { entries, want } => {
            entries_of(entries);
            let _ = want.len();
        }
        MemberResponse::JoinOffer { heads } => {
            let _ = heads.iter().filter(|h| h.len() == 32).count();
        }
        MemberResponse::JoinAccepted { entries } => entries_of(entries),
        MemberResponse::RoomLog { messages, .. } => {
            messages.iter().take(1024).for_each(sealed_of);
            let all: Vec<&WireMessage> = messages.iter().collect();
            let (part, more) = room_part(&all, 0);
            assert!(!part.is_empty() || all.is_empty());
            assert!(more.is_none_or(|m| m as usize <= all.len()));
        }
        MemberResponse::NotMember
        | MemberResponse::Ack
        | MemberResponse::Refused
        | MemberResponse::JoinWaiting
        | MemberResponse::JoinDeclined => {}
    }
}

/// A member request, and an answer, as the member protocol reads them off a stream, and
/// what the node reads out of them; what reads must write and read back the same.
pub fn member_wire(data: &[u8]) {
    let protocol = StreamProtocol::new(MEMBER_PROTOCOL);
    let mut codec = message_codec::<MemberRequest, MemberResponse>();
    block_on(async {
        if let Ok(request) = codec.read_request(&protocol, &mut Cursor::new(data)).await {
            member_request(&request);
            let mut wire = Vec::new();
            codec
                .write_request(&protocol, &mut wire, request)
                .await
                .expect("written");
            codec
                .read_request(&protocol, &mut Cursor::new(wire))
                .await
                .expect("what was read writes and reads back");
        }
        if let Ok(response) = codec.read_response(&protocol, &mut Cursor::new(data)).await {
            member_response(&response);
            let mut wire = Vec::new();
            codec
                .write_response(&protocol, &mut wire, response)
                .await
                .expect("written");
            codec
                .read_response(&protocol, &mut Cursor::new(wire))
                .await
                .expect("what was read writes and reads back");
        }
    });
}

/// A member request handled by a running node, against its real state: this device is
/// device 0 of [`sample_log`], and the request comes from a member (device 1) when the
/// first byte is odd, else from a stranger. The node is started once per process.
pub fn node_request(data: &[u8]) {
    use std::sync::OnceLock;
    static NODE: OnceLock<(tokio::runtime::Runtime, crate::NodeHandle)> = OnceLock::new();
    let Some(first) = data.first() else {
        return;
    };
    let protocol = StreamProtocol::new(MEMBER_PROTOCOL);
    let mut codec = message_codec::<MemberRequest, MemberResponse>();
    let Ok(request) = block_on(codec.read_request(&protocol, &mut Cursor::new(&data[1..]))) else {
        return;
    };
    let (runtime, node) = NODE.get_or_init(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("a runtime");
        let node = runtime.block_on(async {
            crate::start(crate::NodeConfig {
                identity: Arc::new(device(0)),
                groups: vec![GroupSetup::Member(sample_log())],
                friends: None,
                listen_port: 0,
                bootstrap: Vec::new(),
                use_dht: false,
                use_mdns: false,
                bridge: true,
                peers: Vec::new(),
                remembered: Vec::new(),
                hidden: false,
                spill_dir: None,
            })
            .expect("a node")
            .0
        });
        (runtime, node)
    });
    runtime.block_on(async {
        let (reply, done) = tokio::sync::oneshot::channel();
        let command = crate::Command::Fuzz {
            request: Box::new(request),
            from_member: first % 2 == 1,
            reply,
        };
        if node.commands.send(command).await.is_ok() {
            // A node that stopped (it panicked three times) drops the reply.
            done.await.expect("the node handled the request");
        }
    });
}

/// One step of [`node_join`]: a request, or an answer to this device's ask to join (its
/// invite shown, or its signed join).
#[derive(Debug)]
pub enum JoinStep {
    Request(MemberRequest),
    Offered(MemberResponse),
    Committed(MemberResponse),
}

/// Most steps read from one input of the stateful runs.
const MAX_STEPS: usize = 64;

/// The steps of a stateful run: each a byte (its low two bits the peer it comes from, the
/// next two what it is), its length (2 bytes, big-endian) and that many bytes.
fn steps(mut data: &[u8]) -> Vec<(u8, &[u8])> {
    let mut out = Vec::new();
    while out.len() < MAX_STEPS
        && let Some((head, rest)) = data.split_at_checked(3)
    {
        let len = usize::from(u16::from_be_bytes([head[1], head[2]]));
        let Some((body, rest)) = rest.split_at_checked(len) else {
            break;
        };
        out.push((head[0], body));
        data = rest;
    }
    out
}

/// A step as [`steps`] reads it.
fn step(kind: u8, body: &[u8]) -> Vec<u8> {
    let len = u16::try_from(body.len()).expect("a step fits");
    [vec![kind], len.to_be_bytes().to_vec(), body.to_vec()].concat()
}

/// A node of the stateful runs, started once per process with `groups` and `friends`.
fn fuzz_node(
    cell: &'static std::sync::OnceLock<(tokio::runtime::Runtime, crate::NodeHandle)>,
    identity: DeviceIdentity,
    friends: Option<fogata_friends::FriendBook>,
) -> &'static (tokio::runtime::Runtime, crate::NodeHandle) {
    cell.get_or_init(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("a runtime");
        let node = runtime.block_on(async {
            crate::start(crate::NodeConfig {
                identity: Arc::new(identity),
                groups: Vec::new(),
                friends,
                listen_port: 0,
                bootstrap: Vec::new(),
                use_dht: false,
                use_mdns: false,
                bridge: false,
                peers: Vec::new(),
                remembered: Vec::new(),
                hidden: false,
                spill_dir: None,
            })
            .expect("a node")
            .0
        });
        (runtime, node)
    })
}

/// Sends `command` to a node of the stateful runs and waits until it was handled.
fn run_on(
    node: &(tokio::runtime::Runtime, crate::NodeHandle),
    command: impl FnOnce(tokio::sync::oneshot::Sender<()>) -> crate::Command,
) {
    let (runtime, node) = node;
    runtime.block_on(async {
        let (reply, done) = tokio::sync::oneshot::channel();
        if node.commands.send(command(reply)).await.is_ok() {
            // A node that stopped (it panicked three times) drops the reply.
            done.await.expect("the node handled the steps");
        }
    });
}

/// The group the joining device of [`node_join`] was invited to: [`sample_log`] with a
/// new invite from its creator, and that invite.
pub fn joinable_log() -> (GroupLog, Ticket) {
    let mut log = sample_log();
    let (ticket, _) = log
        .invite(&device(0), NOW_MS + 86_400_000, NOW_MS)
        .expect("invite");
    (log, ticket)
}

/// Steps from a few peers against a node joining a group with an invite (device 5, invited
/// to [`joinable_log`]), started once per process; the group is made anew for every input,
/// with peers 1 to 3 the managers it sent its signed join to. A step's kind: 0 a member
/// request, 1 an answer to the invite it showed, 2 an answer to its signed join (answers
/// as the member protocol reads them). The node must never panic, and what it holds of logs
/// in parts must stay within its bounds.
pub fn node_join(data: &[u8]) {
    use std::sync::OnceLock;
    static NODE: OnceLock<(tokio::runtime::Runtime, crate::NodeHandle)> = OnceLock::new();
    static TICKET: OnceLock<Ticket> = OnceLock::new();
    let ticket = TICKET.get_or_init(|| joinable_log().1).clone();
    let protocol = StreamProtocol::new(MEMBER_PROTOCOL);
    let mut codec = message_codec::<MemberRequest, MemberResponse>();
    let steps: Vec<(u8, JoinStep)> = steps(data)
        .into_iter()
        .filter_map(|(kind, body)| {
            let step = match (kind >> 2) & 3 {
                0 | 3 => JoinStep::Request(
                    block_on(codec.read_request(&protocol, &mut Cursor::new(body))).ok()?,
                ),
                1 => JoinStep::Offered(
                    block_on(codec.read_response(&protocol, &mut Cursor::new(body))).ok()?,
                ),
                _ => JoinStep::Committed(
                    block_on(codec.read_response(&protocol, &mut Cursor::new(body))).ok()?,
                ),
            };
            Some((kind & 3, step))
        })
        .collect();
    if steps.is_empty() {
        return;
    }
    let setup = GroupSetup::Joining {
        group_id: ticket.group_id,
        ticket,
        rendezvous: [3; 32],
        name: "Eva".into(),
    };
    let node = fuzz_node(&NODE, device(5), None);
    run_on(node, |reply| crate::Command::FuzzJoin {
        setup: Box::new(setup),
        steps,
        reply,
    });
}

/// Inputs of [`node_join`]: the log sent whole, in parts as a manager does, with parts
/// sent again larger, from several managers at once, an offer to sign on, and requests a
/// joining device does not take.
pub fn node_join_seeds() -> Vec<Vec<u8>> {
    let (log, _) = joinable_log();
    let group = log.group_id().as_bytes().to_vec();
    let entries: Vec<ByteBuf> = encoded(&log).into_iter().map(ByteBuf::from).collect();
    let protocol = StreamProtocol::new(MEMBER_PROTOCOL);
    let request = |request: MemberRequest| {
        let mut codec = message_codec::<MemberRequest, MemberResponse>();
        let mut wire = Vec::new();
        block_on(codec.write_request(&protocol, &mut wire, request)).expect("written");
        wire
    };
    let response = |response: MemberResponse| {
        let mut codec = message_codec::<MemberRequest, MemberResponse>();
        let mut wire = Vec::new();
        block_on(codec.write_response(&protocol, &mut wire, response)).expect("written");
        wire
    };
    let admitted = |entries: &[ByteBuf], part: Option<(u32, u32)>| MemberRequest::Admitted {
        group_id: group.clone(),
        entries: entries.to_vec(),
        part,
    };
    let halves = entries.split_at(entries.len() / 2);
    let mut seeds = Vec::new();
    // Whole, from a manager.
    seeds.push(step(1, &request(admitted(&entries, None))));
    // In two parts, the first sent twice, from two managers at once.
    seeds.push(
        [
            step(1, &request(admitted(halves.0, Some((0, 2))))),
            step(2, &request(admitted(halves.0, Some((0, 2))))),
            step(1, &request(admitted(halves.0, Some((0, 2))))),
            step(1, &request(admitted(halves.1, Some((1, 2))))),
        ]
        .concat(),
    );
    // A part sent again much larger, and parts from a stranger.
    let large = vec![ByteBuf::from(vec![7; 60_000])];
    seeds.push(
        [
            step(3, &request(admitted(halves.0, Some((0, 64))))),
            step(3, &request(admitted(&large, Some((0, 64))))),
            step(0, &request(admitted(halves.1, Some((1, 2))))),
        ]
        .concat(),
    );
    // An offer to sign on, and the log in the answer to the join.
    seeds.push(
        [
            step(
                1 | 1 << 2,
                &response(MemberResponse::JoinOffer {
                    heads: log
                        .heads()
                        .into_iter()
                        .map(|h| ByteBuf::from(h.to_vec()))
                        .collect(),
                }),
            ),
            step(1 | 2 << 2, &response(MemberResponse::JoinWaiting)),
            step(
                1 | 2 << 2,
                &response(MemberResponse::JoinAccepted {
                    entries: entries.clone(),
                }),
            ),
        ]
        .concat(),
    );
    // A refusal, and what only members send.
    seeds.push(
        [
            step(
                2,
                &request(MemberRequest::JoinDeclined {
                    group_id: group.clone(),
                    revoke: None,
                }),
            ),
            step(
                2,
                &request(MemberRequest::Sync {
                    group_id: group.clone(),
                    have: Vec::new(),
                }),
            ),
        ]
        .concat(),
    );
    seeds
}

/// Friend requests from a few peers against a node with friends (device 6, friends with
/// devices 1 and 2), started once per process; its friends are taken anew for every
/// input, peers 1 and 3 sessions of device 1 and peer 2 of device 2, each recognised on
/// its connection, and peer 0 a stranger. A step's kind is left unread. The node must
/// never panic, and what offers in parts hold must stay within their bound.
pub fn node_friend(data: &[u8]) {
    use std::sync::OnceLock;
    static NODE: OnceLock<(tokio::runtime::Runtime, crate::NodeHandle)> = OnceLock::new();
    static BOOK: OnceLock<fogata_friends::FriendBook> = OnceLock::new();
    let book = BOOK.get_or_init(friend_book).clone();
    let protocol = StreamProtocol::new(FRIEND_PROTOCOL);
    let mut codec = message_codec::<FriendRequest, FriendResponse>();
    let steps: Vec<(u8, FriendRequest)> = steps(data)
        .into_iter()
        .filter_map(|(kind, body)| {
            let request = block_on(codec.read_request(&protocol, &mut Cursor::new(body))).ok()?;
            Some((kind & 3, request))
        })
        .collect();
    if steps.is_empty() {
        return;
    }
    let node = fuzz_node(&NODE, device(6), Some(book.clone()));
    run_on(node, |reply| crate::Command::FuzzFriend {
        book: Box::new(book),
        steps,
        reply,
    });
}

/// The book of the node of [`node_friend`]: device 6, friends with devices 1 and 2.
fn friend_book() -> fogata_friends::FriendBook {
    use fogata_friends::{Friend, FriendBook};
    let me = device(6);
    let mut book = FriendBook::new(&me, "Fay", NOW_MS).expect("a book");
    for (n, name) in [(1, "Bea"), (2, "Cid")] {
        let friend = device(n);
        let theirs = FriendBook::new(&friend, name, NOW_MS).expect("a book");
        book.friends.insert(
            friend.public_key(),
            Friend {
                profile: theirs.profile,
                secret: theirs.secret,
                since_ms: NOW_MS,
                delivered: true,
                banner: None,
                bio: None,
            },
        );
    }
    book
}

/// Inputs of [`node_friend`]: a group offered whole and in parts by both friends at once,
/// parts sent again larger, offers that stop half way, and the friend protocol's other
/// requests from friends and a stranger.
pub fn node_friend_seeds() -> Vec<Vec<u8>> {
    let entries: Vec<ByteBuf> = encoded(&sample_log())
        .into_iter()
        .map(ByteBuf::from)
        .collect();
    let protocol = StreamProtocol::new(FRIEND_PROTOCOL);
    let mut codec = message_codec::<FriendRequest, FriendResponse>();
    let mut request = |request: FriendRequest| {
        let mut wire = Vec::new();
        block_on(codec.write_request(&protocol, &mut wire, request)).expect("written");
        wire
    };
    let offer = |entries: &[ByteBuf], part: Option<(u32, u32)>| FriendRequest::Group {
        entries: entries.to_vec(),
        declined: None,
        part,
    };
    let halves = entries.split_at(entries.len() / 2);
    let mut seeds = Vec::new();
    seeds.push(step(1, &request(offer(&entries, None))));
    seeds.push(
        [
            step(1, &request(offer(halves.0, Some((0, 2))))),
            step(2, &request(offer(halves.0, Some((0, 2))))),
            step(3, &request(offer(halves.0, Some((0, 2))))),
            step(1, &request(offer(halves.1, Some((1, 2))))),
            step(2, &request(offer(halves.1, Some((1, 2))))),
        ]
        .concat(),
    );
    let mut large = halves.0.to_vec();
    large.push(ByteBuf::from(vec![7; 60_000]));
    seeds.push(
        [
            step(1, &request(offer(halves.0, Some((0, 64))))),
            step(1, &request(offer(&large, Some((0, 64))))),
            step(2, &request(offer(&large, Some((5, 64))))),
            step(0, &request(offer(halves.1, Some((1, 2))))),
        ]
        .concat(),
    );
    for wire in friend_wire_seeds().into_iter().take(10) {
        seeds.push([step(0, &wire), step(1, &wire), step(2, &wire)].concat());
    }
    seeds.push(step(
        1,
        &request(FriendRequest::Unfriend {
            sealed: vec![12; 120],
        }),
    ));
    seeds
}

/// The member protocol's seeds, each from a member and from a stranger.
pub fn node_request_seeds() -> Vec<Vec<u8>> {
    member_wire_seeds()
        .into_iter()
        .flat_map(|seed| [0u8, 1].map(|from| [vec![from], seed.clone()].concat()))
        .collect()
}

/// A friend request, and an answer, as the friend protocol reads them off a stream. Their
/// sealed fields are opened by fogata-friends (its own entry point).
pub fn friend_wire(data: &[u8]) {
    let protocol = StreamProtocol::new(FRIEND_PROTOCOL);
    let mut codec = message_codec::<FriendRequest, FriendResponse>();
    block_on(async {
        if let Ok(request) = codec.read_request(&protocol, &mut Cursor::new(data)).await {
            if let FriendRequest::Group { entries, .. } | FriendRequest::Direct { entries } =
                &request
            {
                entries_of(entries);
            }
            let mut wire = Vec::new();
            codec
                .write_request(&protocol, &mut wire, request)
                .await
                .expect("written");
            codec
                .read_request(&protocol, &mut Cursor::new(wire))
                .await
                .expect("what was read writes and reads back");
        }
        if let Ok(response) = codec.read_response(&protocol, &mut Cursor::new(data)).await {
            let mut wire = Vec::new();
            codec
                .write_response(&protocol, &mut wire, response)
                .await
                .expect("written");
            codec
                .read_response(&protocol, &mut Cursor::new(wire))
                .await
                .expect("what was read writes and reads back");
        }
    });
}

/// A typing signal.
pub fn typing(data: &[u8]) {
    if let Some(signal) = TypingSignal::decode(data) {
        assert_eq!(TypingSignal::decode(&signal.encode()), Some(signal));
    }
}

/// The key that seals the names of the runs.
const NAME_KEY: [u8; 32] = [5; 32];

/// A sealed name from the DHT: as an address in bytes (first byte 0) or in text (1), or
/// local addresses to seal and open again (2: a port, then IPv4 addresses).
pub fn sealed_name(data: &[u8]) {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let opened = |address: &Multiaddr| {
        if lan::is_name(address) {
            for found in lan::open(address, &[[1; 32], NAME_KEY]) {
                assert!(addr::is_lan(&found));
            }
        }
    };
    match mode % 3 {
        0 => {
            if let Ok(address) = Multiaddr::try_from(rest.to_vec()) {
                opened(&address);
            }
        }
        1 => {
            if let Ok(Ok(address)) = std::str::from_utf8(rest).map(Multiaddr::from_str) {
                opened(&address);
            }
        }
        _ => {
            let Some((port, ips)) = rest.split_at_checked(2) else {
                return;
            };
            let port = u16::from_be_bytes([port[0], port[1]]);
            let ips: Vec<Ipv4Addr> = ips
                .as_chunks::<4>()
                .0
                .iter()
                .map(|o| Ipv4Addr::from(*o))
                .collect();
            if let Some(name) = lan::seal(&NAME_KEY, port, &ips) {
                let found = lan::open(&name, &[NAME_KEY]);
                if !lan::PORTS.contains(&port) {
                    assert!(found.is_empty());
                }
                assert!(found.len() <= ips.len());
            }
        }
    }
}

/// A group log, framed as [`framed`] writes it, taken in by this device ([`device`] 0) as
/// entries from members and room rules, then what the node asks of the group after that.
/// The same entries in the opposite order make the same group.
pub fn log(data: &[u8]) {
    let pieces = unframed(data);
    let Some(group) = pieces
        .iter()
        .find_map(|bytes| Entry::decode(bytes).ok())
        .map(|e| e.group_id)
    else {
        return;
    };
    let me = Arc::new(device(0));
    let mut ctx = GroupContext::new(me.clone(), GroupSetup::Member(GroupLog::empty(group)));
    let _ = ctx.my_removal(&pieces);
    ctx.insert_encoded(&pieces);
    let _ = ctx.diff(&ctx.heads());
    let _ = (ctx.heads_digest(), ctx.banner_digest());
    let _ = (ctx.picture_digest(), ctx.encoded_picture());
    let _ = (ctx.known_epochs(), ctx.broken_epoch(), ctx.secret_leaked());
    let _ = ctx.rendezvous_secrets(NOW_MS);
    let _ = ctx.lookup_secrets(NOW_MS);
    let _ = ctx.lan_secret();
    let state = ctx.log().state();
    for voice in state.rooms.iter().map(|r| r.id).take(16) {
        for member in state.members.keys().take(64) {
            let _ = ctx.barrier(&voice, member, NOW_MS);
        }
    }
    let _ = ctx.welcome_lacking(NOW_MS);
    let _ = ctx.rotate_if_leaked(NOW_MS, false);
    let _ = ctx.confirm_revokes(NOW_MS);
    let _ = ctx.mend_key(NOW_MS, false);
    let _ = ctx.until_removed(&me.public_key());
    // What a joining device asks of the log of the group it was invited to, for each
    // invite in it, and what an issuer asks before it cancels its invites.
    let log = ctx.log();
    let tickets: Vec<Ticket> = log
        .entries()
        .filter_map(|e| match &e.body {
            Body::Invite { ticket, .. } => Some(ticket.clone()),
            body => fogata_state::NamedInvite::of(body).map(|i| i.ticket),
        })
        .take(16)
        .collect();
    for ticket in &tickets {
        let _ = log.invited_with(ticket);
        for member in state.members.keys().take(8).chain([&me.public_key()]) {
            let _ = log.admits_as_invited(ticket, member);
        }
        let _ = log.invites_a_manager_left_with(&ticket.issuer, NOW_MS);
    }

    // Taken in any order, entries make the same group.
    let decoded: Vec<Entry> = pieces
        .iter()
        .filter_map(|bytes| Entry::decode(bytes).ok())
        .collect();
    if decoded.len() <= 200 {
        let mut forward = GroupLog::empty(group);
        forward.insert_all(decoded.iter().cloned());
        let mut backward = GroupLog::empty(group);
        backward.insert_all(decoded.into_iter().rev());
        assert_eq!(
            format!("{:?}", forward.state()),
            format!("{:?}", backward.state())
        );
    }
}

/// Room events of a group, one after the other: who sent it (its low two bits; with its
/// top bit, that device disconnects first), how many seconds after the one before, then the
/// length (2 bytes) and the JSON of the group message, applied to this device's view of the
/// first voice room.
pub fn room_event(data: &[u8]) {
    let mut tracker: RoomTracker<WireMessage> = RoomTracker::new(device(0).public_key());
    let senders: Vec<DevicePublicKey> = (0..4).map(|n| device(n).public_key()).collect();
    let mut now = NOW_MS;
    let mut rest = data;
    let mut count: u32 = 0;
    while let Some((head, tail)) = rest.split_at_checked(4) {
        let len = usize::from(u16::from_be_bytes([head[2], head[3]]));
        let Some((json, tail)) = tail.split_at_checked(len) else {
            break;
        };
        rest = tail;
        count += 1;
        now += u64::from(head[1]) * 1000;
        let sender = senders[usize::from(head[0] & 3)];
        if head[0] & 0x80 != 0 {
            let _ = tracker.drop_device(&sender);
        }
        let _ = tracker.tick(now);
        let Ok(payload) = GroupPayload::decode(json) else {
            continue;
        };
        assert_eq!(
            GroupPayload::decode(&payload.encode()).as_ref(),
            Ok(&payload)
        );
        let Some((voice, room, input)) = room_input(payload) else {
            continue;
        };
        let _: VoiceRoomId = voice;
        let mut message_id = [0u8; 16];
        message_id[..4].copy_from_slice(&count.to_be_bytes());
        let wire = WireMessage {
            version: 2,
            group_id: vec![9; 16],
            epoch: 0,
            sender: sender.as_bytes().to_vec(),
            timestamp_ms: now,
            message_id: message_id.to_vec(),
            nonce: vec![0; 24],
            ciphertext: json.to_vec(),
            signature: vec![0; 64],
        };
        let event = RoomEvent {
            room,
            sender,
            timestamp_ms: now.saturating_sub(u64::from(head[1] & 7) * 1000),
            message_id,
            input,
        };
        let _ = tracker.apply(event, wire, now);
        let _ = tracker.take_earlier_session();
    }
    let snapshot = tracker.snapshot();
    let all: Vec<&WireMessage> = snapshot.iter().collect();
    let mut from = 0;
    while let (part, Some(more)) = room_part(&all, from) {
        assert!(!part.is_empty() && more > from);
        from = more;
    }
    let _ = tracker.close();
}

/// A group of this device (its creator) with a member it added, one that joined with an
/// invite, a move up the ladder and a managers setting, voice rooms and their rules, a new key and a removal: entries of every
/// kind.
pub fn sample_log() -> GroupLog {
    let (me, bea, cid, dan) = (device(0), device(1), device(2), device(3));
    let mut t = NOW_MS - 100_000;
    let mut tick = || {
        t += 1000;
        t
    };
    let mut log = GroupLog::create(&me, "friends", tick()).expect("created");
    log.add(&me, bea.public_key(), "Bea", tick())
        .expect("added");
    log.append(&me, Body::SetManagers { first: Some(2) }, tick())
        .expect("managers");
    log.append(
        &me,
        Body::Promote {
            member: bea.public_key(),
        },
        tick(),
    )
    .expect("promote");
    let (ticket, _) = log
        .invite(&me, NOW_MS + 86_400_000, tick())
        .expect("invite");
    let join = Entry::sign(
        &cid,
        log.group_id(),
        log.heads(),
        tick(),
        Body::Join {
            ticket,
            name: "Cid".into(),
        },
    );
    log.insert(join).expect("joined");
    log.welcome(&me, cid.public_key(), tick())
        .expect("welcomed");
    log.append(
        &me,
        Body::Rename {
            name: "Friends".into(),
        },
        tick(),
    )
    .expect("renamed");
    log.append(&me, Body::SetIcon { icon: 3 }, tick())
        .expect("icon");
    log.append(
        &me,
        Body::SetRoom {
            id: [4; 8],
            name: "Music".into(),
        },
        tick(),
    )
    .expect("room");
    log.set_room_access(&me, [4; 8], Some(vec![bea.public_key()]), tick())
        .expect("access");
    log.keep_out(
        &me,
        fogata_state::FIRST_ROOM,
        bea.public_key(),
        Some(NOW_MS + 60_000),
        tick(),
    )
    .expect("kept out");
    log.add(&me, dan.public_key(), "Dan", tick())
        .expect("added");
    log.rotate(&bea, tick()).expect("rotated");
    log.remove(&me, dan.public_key(), tick()).expect("removed");
    log.append(&cid, Body::Leave, tick()).expect("left");
    log
}

/// A group raised to level three: two managers (Ana, this device, above Bea), an invite
/// of Bea's that names Ana and one of Ana's that names nobody, Cid let in with Bea's by
/// Ana, a manager it names, and Dan with Ana's by Ana herself; then a rotation and a
/// demotion. Also a `Welcome` of Bea's letting Dan in with Ana's invite, which does not
/// name her, and which the group therefore refuses (its box is made up, which judging
/// cannot tell).
pub fn sample_log_at_three() -> (GroupLog, Entry) {
    let (me, bea, cid, dan) = (device(0), device(1), device(2), device(3));
    let mut t = NOW_MS - 100_000;
    let mut tick = || {
        t += 1000;
        t
    };
    let mut log = GroupLog::create(&me, "level three", tick()).expect("created");
    log.add(&me, bea.public_key(), "Bea", tick())
        .expect("added");
    log.raise_level(&me, fogata_state::Level::Three, tick())
        .expect("raised");
    let join = |log: &mut GroupLog, who: &DeviceIdentity, ticket: Ticket, time: u64| {
        let join = Entry::sign(
            who,
            log.group_id(),
            log.heads(),
            time,
            Body::Join {
                ticket,
                name: "Someone".into(),
            },
        );
        log.insert(join.clone()).expect("joined");
        join
    };
    let (by_bea, _) = log
        .invite(&bea, NOW_MS + 86_400_000, tick())
        .expect("invite");
    let (by_ana, _) = log
        .invite(&me, NOW_MS + 86_400_000, tick())
        .expect("invite");
    join(&mut log, &cid, by_bea, tick());
    log.welcome(&me, cid.public_key(), tick())
        .expect("welcomed by a manager the invite names");
    let dan_join = join(&mut log, &dan, by_ana, tick());
    let epoch = log.state().current_epoch().expect("an epoch").id;
    let mut parents = log.heads();
    parents.push(dan_join.hash());
    parents.sort_unstable();
    let unnamed = Entry::sign(
        &bea,
        log.group_id(),
        parents,
        tick(),
        Body::Welcome {
            member: dan.public_key(),
            epoch,
            secret: fogata_crypto::Sealed::seal(&dan.public_key(), b"fuzz", &[7; 32])
                .expect("sealed"),
        },
    );
    log.welcome(&me, dan.public_key(), tick())
        .expect("welcomed by the issuer");
    log.rotate(&bea, tick()).expect("rotated");
    log.append(
        &me,
        Body::Demote {
            member: bea.public_key(),
        },
        tick(),
    )
    .expect("demoted");
    (log, unnamed)
}

/// The entries of `log`, encoded, in order.
pub fn encoded(log: &GroupLog) -> Vec<Vec<u8>> {
    log.entries_in_order()
        .into_iter()
        .map(Entry::encode)
        .collect()
}

pub fn member_wire_seeds() -> Vec<Vec<u8>> {
    let log = sample_log();
    let entries: Vec<ByteBuf> = encoded(&log).into_iter().map(ByteBuf::from).collect();
    let group = log.group_id().as_bytes().to_vec();
    let me = device(0);
    let keys = GroupKeys::derive(
        log.group_id(),
        0,
        &GroupSecret::from_bytes(&[9; 32]).expect("32 bytes"),
    );
    let payload = GroupPayload::RoomChat {
        voice: [0; 8],
        room: [1; 16],
        nick: "Ana".into(),
        text: "hi".into(),
    };
    let wire = WireMessage::from(
        &SealedMessage::seal(&keys, &me, NOW_MS, &payload.encode()).expect("sealed"),
    );
    let ticket = log.issue_ticket(&me, NOW_MS + 1000).expect("ticket");
    let banner = GroupBanner::sign(
        &me,
        log.group_id(),
        b"\x89PNG\r\n\x1a\n0000".to_vec(),
        NOW_MS,
    )
    .expect("banner");
    let picture = GroupPicture::sign(
        &me,
        log.group_id(),
        b"\x89PNG\r\n\x1a\n0000".to_vec(),
        NOW_MS,
    )
    .expect("picture");
    let profile = fogata_friends::Profile::sign(&me, "Ana", fogata_friends::Avatar::Preset(1), 5)
        .expect("profile");
    let address = Multiaddr::from_str("/ip4/192.168.1.20/udp/24000/quic-v1").expect("address");
    let requests = vec![
        MemberRequest::Challenge {
            challenge: vec![3; 32],
            tags: vec![ByteBuf::from(vec![4; 32])],
        },
        MemberRequest::Deliver(wire.clone()),
        MemberRequest::Spread {
            message: wire.clone(),
            reached: vec![7; 24],
        },
        MemberRequest::Roster {
            group_id: group.clone(),
            heads: Some(ByteBuf::from(vec![1; 16])),
            banner: Some(ByteBuf::from(vec![])),
            picture: Some(ByteBuf::from(vec![])),
        },
        MemberRequest::Sync {
            group_id: group.clone(),
            have: vec![ByteBuf::from(vec![5; 32])],
        },
        MemberRequest::Push {
            group_id: group.clone(),
            entries: entries.iter().map(|e| e.to_vec().into()).collect(),
        },
        MemberRequest::JoinRequest {
            ticket: ticket.encode(),
        },
        MemberRequest::JoinCommit {
            entry: entries[0].to_vec(),
            binding: Some(ByteBuf::from(vec![6; 64])),
        },
        MemberRequest::Profile {
            profile: profile.encode(),
            animation: None,
        },
        MemberRequest::RoomSync {
            group_id: group.clone(),
            from: 3,
        },
        MemberRequest::Addresses {
            addresses: vec![ByteBuf::from(address.to_vec())],
        },
        MemberRequest::Signal {
            group_id: group.clone(),
            payload: br#"{"type":"offer","sdp":"v=0"}"#.to_vec(),
        },
        MemberRequest::Presence {
            hidden: true,
            away: false,
        },
        MemberRequest::Presence {
            hidden: false,
            away: true,
        },
        MemberRequest::Admitted {
            part: None,
            group_id: group.clone(),
            entries: entries.clone(),
        },
        MemberRequest::JoinDeclined {
            group_id: group.clone(),
            revoke: Some(entries[1].clone()),
        },
        MemberRequest::OtherSession,
        MemberRequest::Direct {
            group_id: group.clone(),
            messages: vec![wire.clone()],
        },
        MemberRequest::GroupBanner {
            group_id: group.clone(),
            banner: banner.encode(),
        },
        MemberRequest::GroupPicture {
            group_id: group.clone(),
            picture: picture.encode(),
        },
        MemberRequest::RoomEvents {
            group_id: group.clone(),
            ids: vec![8; 32],
        },
        MemberRequest::KeyProblem {
            group_id: group,
            epoch: 7,
        },
        MemberRequest::FriendAsk {
            sealed: vec![9; 160],
        },
    ];
    let proof = fogata_crypto::MembershipProof::create(
        &keys,
        &me,
        b"prover",
        b"verifier",
        &[3; fogata_crypto::CHALLENGE_LEN],
    )
    .seal(
        &keys,
        b"prover",
        b"verifier",
        &[3; fogata_crypto::CHALLENGE_LEN],
    )
    .expect("sealed");
    let responses = vec![
        MemberResponse::Proof(WireProof::from(&proof)),
        MemberResponse::Ack,
        MemberResponse::Refused,
        MemberResponse::Roster {
            peers: vec![ByteBuf::from(PeerId::random().to_bytes())],
        },
        MemberResponse::SyncReply {
            entries: entries.clone(),
            want: vec![ByteBuf::from(vec![6; 32])],
        },
        MemberResponse::JoinOffer {
            heads: vec![ByteBuf::from(vec![6; 32])],
        },
        MemberResponse::JoinAccepted { entries },
        MemberResponse::JoinWaiting,
        MemberResponse::RoomLog {
            messages: vec![wire.clone(), wire],
            more: Some(2),
        },
    ];
    let protocol = StreamProtocol::new(MEMBER_PROTOCOL);
    let mut codec = message_codec::<MemberRequest, MemberResponse>();
    block_on(async {
        let mut out = Vec::new();
        for request in requests {
            let mut wire = Vec::new();
            codec
                .write_request(&protocol, &mut wire, request)
                .await
                .expect("written");
            out.push(wire);
        }
        for response in responses {
            let mut wire = Vec::new();
            codec
                .write_response(&protocol, &mut wire, response)
                .await
                .expect("written");
            out.push(wire);
        }
        out
    })
}

pub fn friend_wire_seeds() -> Vec<Vec<u8>> {
    let entries: Vec<ByteBuf> = encoded(&sample_log())
        .into_iter()
        .map(ByteBuf::from)
        .collect();
    let requests = vec![
        FriendRequest::Probe { nonce: vec![1; 16] },
        FriendRequest::Hello {
            mac: vec![2; 32],
            sealed: vec![3; 120],
        },
        FriendRequest::CodeStart {
            gate: vec![4; 32],
            spake: vec![5; 33],
        },
        FriendRequest::Group {
            entries: entries.clone(),
            declined: None,
            part: None,
        },
        FriendRequest::Group {
            entries: entries.clone(),
            declined: Some(serde_bytes::ByteBuf::from(vec![8; 32])),
            part: None,
        },
        FriendRequest::Group {
            entries: entries.clone(),
            declined: None,
            part: Some((1, 3)),
        },
        FriendRequest::Heartbeat,
        FriendRequest::Call {
            sealed: vec![6; 90],
        },
        FriendRequest::Direct { entries },
        FriendRequest::Bio {
            sealed: vec![7; 80],
        },
    ];
    let responses = vec![
        FriendResponse::ProbeReply { tag: vec![8; 32] },
        FriendResponse::CodeReply {
            spake: vec![9; 33],
            sealed: vec![10; 64],
        },
        FriendResponse::Pending,
        FriendResponse::Mutual {
            sealed: vec![11; 70],
        },
        FriendResponse::Unfriended {
            sealed: vec![12; 120],
        },
    ];
    let protocol = StreamProtocol::new(FRIEND_PROTOCOL);
    let mut codec = message_codec::<FriendRequest, FriendResponse>();
    block_on(async {
        let mut out = Vec::new();
        for request in requests {
            let mut wire = Vec::new();
            codec
                .write_request(&protocol, &mut wire, request)
                .await
                .expect("written");
            out.push(wire);
        }
        for response in responses {
            let mut wire = Vec::new();
            codec
                .write_response(&protocol, &mut wire, response)
                .await
                .expect("written");
            out.push(wire);
        }
        out
    })
}

pub fn typing_seeds() -> Vec<Vec<u8>> {
    let group = GroupId::from_bytes(&[9; 16]).expect("16 bytes");
    [true, false]
        .into_iter()
        .map(|typing| {
            TypingSignal {
                group,
                voice: [1; 8],
                typing,
                seq: 7,
            }
            .encode()
            .to_vec()
        })
        .collect()
}

pub fn sealed_name_seeds() -> Vec<Vec<u8>> {
    let ips = [Ipv4Addr::new(192, 168, 1, 20), Ipv4Addr::new(10, 0, 0, 7)];
    let name = lan::seal(&NAME_KEY, 24000, &ips).expect("sealed");
    let mut out = vec![
        [&[0u8][..], &name.to_vec()].concat(),
        [&[1u8][..], name.to_string().as_bytes()].concat(),
    ];
    let mut plain = vec![2u8];
    plain.extend_from_slice(&24000u16.to_be_bytes());
    for ip in ips {
        plain.extend_from_slice(&ip.octets());
    }
    out.push(plain);
    out
}

pub fn log_seeds() -> Vec<Vec<u8>> {
    let log = sample_log();
    let all = encoded(&log);
    // This device as a member only, and as a stranger.
    let bea = device(1);
    let mut other = GroupLog::create(&bea, "others", NOW_MS).expect("created");
    other
        .add(&bea, device(0).public_key(), "Ana", NOW_MS + 1)
        .expect("added");
    let stranger = GroupLog::create(&device(2), "far away", NOW_MS).expect("created");
    let (three, unnamed) = sample_log_at_three();
    let mut at_three = encoded(&three);
    at_three.push(unnamed.encode());
    vec![
        framed(&all),
        framed(&all.iter().rev().cloned().collect::<Vec<_>>()),
        framed(&encoded(&other)),
        framed(&encoded(&stranger)),
        framed(&at_three),
        framed(&at_three.iter().rev().cloned().collect::<Vec<_>>()),
    ]
}

pub fn room_event_seeds() -> Vec<Vec<u8>> {
    let room = [1u8; 16];
    let payloads = [
        (
            1,
            GroupPayload::RoomJoin {
                voice: [0; 8],
                room,
                started_ms: NOW_MS,
                nick: "Bea".into(),
            },
        ),
        (
            2,
            GroupPayload::RoomJoin {
                voice: [0; 8],
                room,
                started_ms: NOW_MS,
                nick: "Cid".into(),
            },
        ),
        (
            1,
            GroupPayload::RoomChat {
                voice: [0; 8],
                room,
                nick: "Bea".into(),
                text: "hi".into(),
            },
        ),
        (
            2,
            GroupPayload::RoomShare {
                voice: [0; 8],
                room,
                id: [5; 32],
                name: "video.mp4".into(),
                size: 1000,
                video: Some(fogata_proto::WireVideo {
                    thumb: vec![0xff, 0xd8, 0xff],
                    width: 640,
                    height: 360,
                    duration_ms: 4000,
                }),
            },
        ),
        (
            3,
            GroupPayload::RoomHere {
                voice: [0; 8],
                room,
                started_ms: NOW_MS,
                nick: "Dan".into(),
                recent: vec![7; 32],
            },
        ),
        (
            2,
            GroupPayload::RoomUnshare {
                voice: [0; 8],
                room,
                id: [5; 32],
            },
        ),
        (
            1,
            GroupPayload::RoomRemove {
                voice: [0; 8],
                room,
                member: *device(3).public_key().as_bytes(),
            },
        ),
        (
            2,
            GroupPayload::RoomLeave {
                voice: [0; 8],
                room,
            },
        ),
    ];
    let mut seed = Vec::new();
    for (sender, payload) in payloads {
        let json = payload.encode();
        seed.extend_from_slice(&[sender, 2]);
        seed.extend_from_slice(&(json.len() as u16).to_be_bytes());
        seed.extend_from_slice(&json);
    }
    vec![seed]
}
