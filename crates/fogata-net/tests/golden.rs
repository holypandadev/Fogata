//! Frozen bytes of every member and friend protocol message, as the CBOR codec of the two
//! protocols writes them (`tests/data/wire.txt`).
//!
//! Each message must still encode to the bytes frozen there, and those bytes must still
//! decode to it: what this version sends is what versions already handed out read, and what
//! they send is still read here. A line that changes is a change of the wire. It needs a
//! capability level (a marker the peer advertises, or a field older versions skip), not a
//! new fixture: only write the file anew on purpose, once that is done, with
//!
//! ```text
//! FOGATA_WRITE_FIXTURES=1 cargo test -p fogata-net --test golden
//! ```
//!
//! A message that leaves out what an older version did not send is frozen in both forms,
//! with that part and without it. The bytes inside (sealed messages, entries, profiles) are
//! opaque here; the group log entries and records are frozen in `fogata-state`, the signed
//! profiles in `fogata-friends` and the plaintexts of messages in `fogata-proto`.

#[path = "../../fogata-proto/tests/frozen/mod.rs"]
mod frozen;

use fogata_proto::{
    FRIEND_PROTOCOL, FriendRequest, FriendResponse, MEMBER_PROTOCOL, MemberRequest, MemberResponse,
    WireMessage, WireProof,
};
use frozen::{case, frozen};
use libp2p::{
    StreamProtocol,
    request_response::{Codec as _, cbor::codec::Codec},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_bytes::ByteBuf;

fn request_bytes<Req, Resp>(protocol: &'static str, request: Req) -> Vec<u8>
where
    Req: Serialize + DeserializeOwned + Send + 'static,
    Resp: Serialize + DeserializeOwned + Send + 'static,
{
    futures::executor::block_on(async {
        let mut wire = Vec::new();
        Codec::<Req, Resp>::default()
            .write_request(&StreamProtocol::new(protocol), &mut wire, request)
            .await
            .unwrap();
        wire
    })
}

fn read_request<Req, Resp>(protocol: &'static str, bytes: &[u8]) -> Option<Req>
where
    Req: Serialize + DeserializeOwned + Send + 'static,
    Resp: Serialize + DeserializeOwned + Send + 'static,
{
    futures::executor::block_on(async {
        Codec::<Req, Resp>::default()
            .read_request(
                &StreamProtocol::new(protocol),
                &mut futures::io::Cursor::new(bytes.to_vec()),
            )
            .await
            .ok()
    })
}

fn response_bytes<Req, Resp>(protocol: &'static str, response: Resp) -> Vec<u8>
where
    Req: Serialize + DeserializeOwned + Send + 'static,
    Resp: Serialize + DeserializeOwned + Send + 'static,
{
    futures::executor::block_on(async {
        let mut wire = Vec::new();
        Codec::<Req, Resp>::default()
            .write_response(&StreamProtocol::new(protocol), &mut wire, response)
            .await
            .unwrap();
        wire
    })
}

fn read_response<Req, Resp>(protocol: &'static str, bytes: &[u8]) -> Option<Resp>
where
    Req: Serialize + DeserializeOwned + Send + 'static,
    Resp: Serialize + DeserializeOwned + Send + 'static,
{
    futures::executor::block_on(async {
        Codec::<Req, Resp>::default()
            .read_response(
                &StreamProtocol::new(protocol),
                &mut futures::io::Cursor::new(bytes.to_vec()),
            )
            .await
            .ok()
    })
}

fn buf(byte: u8, len: usize) -> ByteBuf {
    ByteBuf::from(vec![byte; len])
}

fn message(byte: u8) -> WireMessage {
    WireMessage {
        version: 1,
        group_id: vec![1; 16],
        epoch: 3,
        sender: vec![byte; 32],
        timestamp_ms: 1_700_000_000_000,
        message_id: vec![byte.wrapping_add(1); 16],
        nonce: vec![byte.wrapping_add(2); 24],
        ciphertext: vec![byte.wrapping_add(3); 40],
        signature: vec![byte.wrapping_add(4); 64],
    }
}

/// The name of each kind of member request. A kind added fails to build here: freeze it in
/// [`member_requests`] too.
fn member_request_kind(request: &MemberRequest) -> &'static str {
    match request {
        MemberRequest::Challenge { .. } => "member-request-challenge",
        MemberRequest::Deliver(_) => "member-request-deliver",
        MemberRequest::Spread { .. } => "member-request-spread",
        MemberRequest::Roster { .. } => "member-request-roster",
        MemberRequest::Sync { .. } => "member-request-sync",
        MemberRequest::Push { .. } => "member-request-push",
        MemberRequest::JoinRequest { .. } => "member-request-join-request",
        MemberRequest::JoinCommit { .. } => "member-request-join-commit",
        MemberRequest::Profile { .. } => "member-request-profile",
        MemberRequest::RoomSync { .. } => "member-request-room-sync",
        MemberRequest::Addresses { .. } => "member-request-addresses",
        MemberRequest::Signal { .. } => "member-request-signal",
        MemberRequest::Presence { .. } => "member-request-presence",
        MemberRequest::Admitted { .. } => "member-request-admitted",
        MemberRequest::JoinDeclined { .. } => "member-request-join-declined",
        MemberRequest::OtherSession => "member-request-other-session",
        MemberRequest::Direct { .. } => "member-request-direct",
        MemberRequest::GroupBanner { .. } => "member-request-group-banner",
        MemberRequest::GroupPicture { .. } => "member-request-group-picture",
        MemberRequest::RoomEvents { .. } => "member-request-room-events",
        MemberRequest::KeyProblem { .. } => "member-request-key-problem",
        MemberRequest::FriendAsk { .. } => "member-request-friend-ask",
    }
}

fn member_requests() -> Vec<(String, MemberRequest)> {
    let group_id = || vec![1u8; 16];
    [
        (
            "",
            MemberRequest::Challenge {
                challenge: vec![2; 32],
                tags: vec![buf(3, 16), buf(4, 16)],
            },
        ),
        ("", MemberRequest::Deliver(message(5))),
        (
            "",
            MemberRequest::Spread {
                message: message(6),
                reached: [[7u8; 8], [8; 8]].concat(),
            },
        ),
        (
            "-older",
            MemberRequest::Roster {
                group_id: group_id(),
                heads: None,
                banner: None,
                picture: None,
            },
        ),
        (
            "",
            MemberRequest::Roster {
                group_id: group_id(),
                heads: Some(buf(9, 32)),
                banner: Some(buf(10, 16)),
                picture: Some(ByteBuf::new()),
            },
        ),
        (
            "",
            MemberRequest::Sync {
                group_id: group_id(),
                have: vec![buf(11, 32), buf(12, 32)],
            },
        ),
        (
            "",
            MemberRequest::Push {
                group_id: group_id(),
                entries: vec![vec![13; 40].into(), vec![14; 3].into()],
            },
        ),
        (
            "",
            MemberRequest::JoinRequest {
                ticket: vec![15; 136],
            },
        ),
        (
            "-older",
            MemberRequest::JoinCommit {
                entry: vec![16; 60],
                binding: None,
            },
        ),
        (
            "",
            MemberRequest::JoinCommit {
                entry: vec![16; 60],
                binding: Some(buf(17, 64)),
            },
        ),
        (
            "-older",
            MemberRequest::Profile {
                profile: vec![18; 50],
                animation: None,
            },
        ),
        (
            "",
            MemberRequest::Profile {
                profile: vec![18; 50],
                animation: Some(buf(19, 30)),
            },
        ),
        (
            "-older",
            MemberRequest::RoomSync {
                group_id: group_id(),
                from: 0,
            },
        ),
        (
            "",
            MemberRequest::RoomSync {
                group_id: group_id(),
                from: 40,
            },
        ),
        (
            "",
            MemberRequest::Addresses {
                addresses: vec![buf(20, 12), buf(21, 8)],
            },
        ),
        (
            "",
            MemberRequest::Signal {
                group_id: group_id(),
                payload: b"{\"offer\":\"v=0\"}".to_vec(),
            },
        ),
        (
            "-older",
            MemberRequest::Presence {
                hidden: true,
                away: false,
            },
        ),
        (
            "",
            MemberRequest::Presence {
                hidden: false,
                away: true,
            },
        ),
        (
            "-older",
            MemberRequest::Admitted {
                group_id: group_id(),
                entries: vec![buf(22, 40)],
                part: None,
            },
        ),
        (
            "",
            MemberRequest::Admitted {
                group_id: group_id(),
                entries: vec![buf(22, 40)],
                part: Some((2, 5)),
            },
        ),
        (
            "-older",
            MemberRequest::JoinDeclined {
                group_id: group_id(),
                revoke: None,
            },
        ),
        (
            "",
            MemberRequest::JoinDeclined {
                group_id: group_id(),
                revoke: Some(buf(23, 40)),
            },
        ),
        ("", MemberRequest::OtherSession),
        (
            "",
            MemberRequest::Direct {
                group_id: group_id(),
                messages: vec![message(24), message(25)],
            },
        ),
        (
            "",
            MemberRequest::GroupBanner {
                group_id: group_id(),
                banner: vec![26; 50],
            },
        ),
        (
            "",
            MemberRequest::GroupPicture {
                group_id: group_id(),
                picture: vec![27; 50],
            },
        ),
        (
            "",
            MemberRequest::RoomEvents {
                group_id: group_id(),
                ids: [[28u8; 16], [29; 16]].concat(),
            },
        ),
        (
            "",
            MemberRequest::KeyProblem {
                group_id: group_id(),
                epoch: 4,
            },
        ),
        (
            "",
            MemberRequest::FriendAsk {
                sealed: vec![30; 80],
            },
        ),
    ]
    .into_iter()
    .map(|(form, request)| (format!("{}{form}", member_request_kind(&request)), request))
    .collect()
}

/// The name of each kind of member response, as [`member_request_kind`].
fn member_response_kind(response: &MemberResponse) -> &'static str {
    match response {
        MemberResponse::Proof(_) => "member-response-proof",
        MemberResponse::NotMember => "member-response-not-member",
        MemberResponse::Ack => "member-response-ack",
        MemberResponse::Refused => "member-response-refused",
        MemberResponse::Roster { .. } => "member-response-roster",
        MemberResponse::SyncReply { .. } => "member-response-sync-reply",
        MemberResponse::JoinOffer { .. } => "member-response-join-offer",
        MemberResponse::JoinAccepted { .. } => "member-response-join-accepted",
        MemberResponse::JoinWaiting => "member-response-join-waiting",
        MemberResponse::JoinDeclined => "member-response-join-declined",
        MemberResponse::RoomLog { .. } => "member-response-room-log",
    }
}

fn member_responses() -> Vec<(String, MemberResponse)> {
    [
        (
            "",
            MemberResponse::Proof(WireProof {
                epoch: 3,
                nonce: vec![40; 24],
                ciphertext: vec![41; 48],
            }),
        ),
        ("", MemberResponse::NotMember),
        ("", MemberResponse::Ack),
        ("", MemberResponse::Refused),
        (
            "",
            MemberResponse::Roster {
                peers: vec![buf(42, 38), buf(43, 38)],
            },
        ),
        (
            "",
            MemberResponse::SyncReply {
                entries: vec![buf(44, 40)],
                want: vec![buf(45, 32)],
            },
        ),
        (
            "",
            MemberResponse::JoinOffer {
                heads: vec![buf(46, 32)],
            },
        ),
        (
            "",
            MemberResponse::JoinAccepted {
                entries: vec![buf(47, 40), buf(48, 40)],
            },
        ),
        ("", MemberResponse::JoinWaiting),
        ("", MemberResponse::JoinDeclined),
        (
            "-older",
            MemberResponse::RoomLog {
                messages: vec![message(49)],
                more: None,
            },
        ),
        (
            "",
            MemberResponse::RoomLog {
                messages: vec![message(50)],
                more: Some(12),
            },
        ),
    ]
    .into_iter()
    .map(|(form, response)| {
        (
            format!("{}{form}", member_response_kind(&response)),
            response,
        )
    })
    .collect()
}

/// The name of each kind of friend request, as [`member_request_kind`].
fn friend_request_kind(request: &FriendRequest) -> &'static str {
    match request {
        FriendRequest::Probe { .. } => "friend-request-probe",
        FriendRequest::Hello { .. } => "friend-request-hello",
        FriendRequest::LinkRequest { .. } => "friend-request-link-request",
        FriendRequest::CodeStart { .. } => "friend-request-code-start",
        FriendRequest::CodeFinish { .. } => "friend-request-code-finish",
        FriendRequest::Accept { .. } => "friend-request-accept",
        FriendRequest::Profile { .. } => "friend-request-profile",
        FriendRequest::Group { .. } => "friend-request-group",
        FriendRequest::Status { .. } => "friend-request-status",
        FriendRequest::Heartbeat => "friend-request-heartbeat",
        FriendRequest::Call { .. } => "friend-request-call",
        FriendRequest::Banner { .. } => "friend-request-banner",
        FriendRequest::Unfriend { .. } => "friend-request-unfriend",
        FriendRequest::Mutual { .. } => "friend-request-mutual",
        FriendRequest::Direct { .. } => "friend-request-direct",
        FriendRequest::Animation { .. } => "friend-request-animation",
        FriendRequest::Bio { .. } => "friend-request-bio",
    }
}

fn friend_requests() -> Vec<(String, FriendRequest)> {
    let sealed = |byte: u8| vec![byte; 60];
    [
        (
            "",
            FriendRequest::Probe {
                nonce: vec![60; 32],
            },
        ),
        (
            "",
            FriendRequest::Hello {
                mac: vec![61; 32],
                sealed: sealed(62),
            },
        ),
        (
            "",
            FriendRequest::LinkRequest {
                mac: vec![63; 32],
                sealed: sealed(64),
            },
        ),
        (
            "",
            FriendRequest::CodeStart {
                gate: vec![65; 32],
                spake: vec![66; 33],
            },
        ),
        ("", FriendRequest::CodeFinish { sealed: sealed(67) }),
        (
            "",
            FriendRequest::Accept {
                mac: vec![68; 32],
                sealed: sealed(69),
            },
        ),
        ("", FriendRequest::Profile { sealed: sealed(70) }),
        (
            "-older",
            FriendRequest::Group {
                entries: vec![buf(71, 40)],
                declined: None,
                part: None,
            },
        ),
        (
            "",
            FriendRequest::Group {
                entries: vec![buf(71, 40)],
                declined: Some(buf(72, 32)),
                part: Some((1, 3)),
            },
        ),
        ("", FriendRequest::Status { sealed: sealed(73) }),
        ("", FriendRequest::Heartbeat),
        ("", FriendRequest::Call { sealed: sealed(74) }),
        ("", FriendRequest::Banner { sealed: sealed(75) }),
        ("", FriendRequest::Unfriend { sealed: sealed(76) }),
        ("", FriendRequest::Mutual { sealed: sealed(77) }),
        (
            "",
            FriendRequest::Direct {
                entries: vec![buf(78, 40)],
            },
        ),
        ("", FriendRequest::Animation { sealed: sealed(79) }),
        ("", FriendRequest::Bio { sealed: sealed(80) }),
    ]
    .into_iter()
    .map(|(form, request)| (format!("{}{form}", friend_request_kind(&request)), request))
    .collect()
}

/// The name of each kind of friend response, as [`member_request_kind`].
fn friend_response_kind(response: &FriendResponse) -> &'static str {
    match response {
        FriendResponse::ProbeReply { .. } => "friend-response-probe-reply",
        FriendResponse::HelloReply { .. } => "friend-response-hello-reply",
        FriendResponse::CodeReply { .. } => "friend-response-code-reply",
        FriendResponse::Pending => "friend-response-pending",
        FriendResponse::Ack => "friend-response-ack",
        FriendResponse::Refused => "friend-response-refused",
        FriendResponse::Mutual { .. } => "friend-response-mutual",
        FriendResponse::Unfriended { .. } => "friend-response-unfriended",
    }
}

fn friend_responses() -> Vec<(String, FriendResponse)> {
    [
        FriendResponse::ProbeReply { tag: vec![90; 16] },
        FriendResponse::HelloReply {
            sealed: vec![91; 60],
        },
        FriendResponse::CodeReply {
            spake: vec![92; 33],
            sealed: vec![93; 60],
        },
        FriendResponse::Pending,
        FriendResponse::Ack,
        FriendResponse::Refused,
        FriendResponse::Mutual {
            sealed: vec![94; 60],
        },
        FriendResponse::Unfriended {
            sealed: vec![95; 60],
        },
    ]
    .into_iter()
    .map(|response| (friend_response_kind(&response).to_string(), response))
    .collect()
}

#[test]
fn every_wire_message_is_sent_and_read_as_frozen() {
    const M: &str = MEMBER_PROTOCOL;
    const F: &str = FRIEND_PROTOCOL;
    // One file for all of them, written anew at once.
    let mut cases = Vec::new();
    for (name, request) in member_requests() {
        cases.push(case(
            name,
            &request,
            |r| request_bytes::<_, MemberResponse>(M, r.clone()),
            |b| read_request::<_, MemberResponse>(M, b),
        ));
    }
    for (name, response) in member_responses() {
        cases.push(case(
            name,
            &response,
            |r| response_bytes::<MemberRequest, _>(M, r.clone()),
            |b| read_response::<MemberRequest, _>(M, b),
        ));
    }
    for (name, request) in friend_requests() {
        cases.push(case(
            name,
            &request,
            |r| request_bytes::<_, FriendResponse>(F, r.clone()),
            |b| read_request::<_, FriendResponse>(F, b),
        ));
    }
    for (name, response) in friend_responses() {
        cases.push(case(
            name,
            &response,
            |r| response_bytes::<FriendRequest, _>(F, r.clone()),
            |b| read_response::<FriendRequest, _>(F, b),
        ));
    }
    frozen("wire.txt", &cases);
}
