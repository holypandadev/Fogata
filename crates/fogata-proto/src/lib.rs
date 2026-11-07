//! Messages exchanged between Fogata devices over the member and friend protocols. The
//! network layer serialises them with CBOR; nothing here performs I/O.

#![forbid(unsafe_code)]

use fogata_crypto::{
    CryptoError, DevicePublicKey, GroupId, MessageHeader, SealedMessage, SealedProof,
};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

mod direct;

pub use direct::{
    DirectFile, DirectFileKind, DirectPayload, MAX_DIRECT_BATCH, MAX_DIRECT_PAYLOAD_BYTES,
    MAX_VOICE_BYTES, MAX_VOICE_MS, VOICE_BARS, WireVoice,
};

/// libp2p protocol name of the member protocol.
pub const MEMBER_PROTOCOL: &str = "/fogata/member/1";

/// Bytes every copy of a message shares, sent as a byte string like
/// [`serde_bytes::ByteBuf`]: the same entries pushed to every member of a group, or queued
/// for several peers at once, are held once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedBytes(Arc<[u8]>);

impl SharedBytes {
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl std::ops::Deref for SharedBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl AsRef<[u8]> for SharedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for SharedBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes.into())
    }
}

impl Serialize for SharedBytes {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for SharedBytes {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        serde_bytes::ByteBuf::deserialize(deserializer).map(|bytes| bytes.into_vec().into())
    }
}

/// A request sent to another device.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MemberRequest {
    /// Asks the peer to prove that it belongs to a group. The group is not named: `tags` has
    /// one group tag per key epoch the asker knows, newest first, which only holders of that
    /// epoch's key recognise. The answer is always a sealed proof; only holders of the
    /// epoch's key can read it or tell it from a decoy.
    Challenge {
        #[serde(with = "serde_bytes")]
        challenge: Vec<u8>,
        tags: Vec<serde_bytes::ByteBuf>,
    },
    /// Delivers a sealed group message. The receiver forwards it to the other members it is
    /// connected to if it has not seen it before.
    Deliver(WireMessage),
    /// Delivers a sealed group message like [`MemberRequest::Deliver`], naming the devices
    /// it went to already: the receiver passes it on only to members it is connected to that
    /// are not among them (see [`SPREAD_MARKER`]). Older devices cannot read it, so it goes
    /// only to devices that advertise the marker.
    Spread {
        message: WireMessage,
        /// Short ids of the devices that have the message or are being sent it, one after
        /// the other: each one the last [`SHORT_PEER_BYTES`] of a transport peer id.
        #[serde(with = "serde_bytes")]
        reached: Vec<u8>,
    },
    /// Asks which other members of `group_id` the peer is directly connected to, so that
    /// they can be reached through it.
    Roster {
        #[serde(with = "serde_bytes")]
        group_id: Vec<u8>,
        /// A digest of the asker's group log heads: an answerer whose own differ starts a
        /// sync, so an entry lost on the way is not missing until the next connection.
        #[serde(default)]
        heads: Option<serde_bytes::ByteBuf>,
        /// A short digest of the asker's group banner, empty for none, which goes apart from
        /// the log ([`MemberRequest::GroupBanner`]): an answerer whose own differs sends it
        /// its banner. Older versions neither send nor read it, and are never sent banners.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        banner: Option<serde_bytes::ByteBuf>,
        /// A short digest of the asker's group picture record, empty for none, which goes
        /// apart from the log like the banner ([`MemberRequest::GroupPicture`]). Older
        /// versions neither send nor read it, and are never sent pictures this way.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        picture: Option<serde_bytes::ByteBuf>,
    },
    /// Lists the hashes of the group log entries the sender has; the answer carries the
    /// entries it lacks and names the ones the answerer lacks.
    Sync {
        #[serde(with = "serde_bytes")]
        group_id: Vec<u8>,
        have: Vec<serde_bytes::ByteBuf>,
    },
    /// New or requested group log entries (canonical encoding).
    Push {
        #[serde(with = "serde_bytes")]
        group_id: Vec<u8>,
        entries: Vec<SharedBytes>,
    },
    /// A device that is not a member yet presents an invite ticket.
    JoinRequest {
        #[serde(with = "serde_bytes")]
        ticket: Vec<u8>,
    },
    /// The signed `Join` entry, built on the heads from the offer.
    JoinCommit {
        #[serde(with = "serde_bytes")]
        entry: Vec<u8>,
        /// The joining device's signature binding the entry to this connection's two
        /// transport ids (`fogata_state::Entry::join_binding`): the manager sends the log
        /// only to the peer it names. Earlier versions send none, and are refused.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        binding: Option<serde_bytes::ByteBuf>,
    },
    /// The sender's signed profile, so members see its name and picture.
    Profile {
        #[serde(with = "serde_bytes")]
        profile: Vec<u8>,
        /// The signed animation of the profile's picture (`fogata_friends::Animation`), when
        /// it has one. Older versions do not read it and show the still picture.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        animation: Option<serde_bytes::ByteBuf>,
    },
    /// Asks for the events of the group's current room, as sealed messages.
    RoomSync {
        #[serde(with = "serde_bytes")]
        group_id: Vec<u8>,
        /// How many events earlier parts of the answer gave (see
        /// [`MemberResponse::RoomLog`]); none at first.
        #[serde(default, skip_serializing_if = "is_zero")]
        from: u32,
    },
    /// The sender's addresses on its local networks, QUIC first, for members only: a member
    /// on the same network that did not see it there (another subnet, a virtual machine)
    /// can then reach it directly, which calls need for their media.
    Addresses {
        addresses: Vec<serde_bytes::ByteBuf>,
    },
    /// Setting up a call in the group's room: an offer, an answer or a network candidate
    /// for the receiver's WebRTC engine, opaque to the node.
    Signal {
        #[serde(with = "serde_bytes")]
        group_id: Vec<u8>,
        #[serde(with = "serde_bytes")]
        payload: Vec<u8>,
    },
    /// Whether the sender appears offline: members then show it as offline, except in a room
    /// they share; or, when not, whether it is away from its computer. Older versions do not
    /// read `away`.
    Presence {
        hidden: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        away: bool,
    },
    /// From an admin to a device that asked to join with an invite: an admin let it in,
    /// and here is the whole log, including the `Welcome` with the key.
    Admitted {
        #[serde(with = "serde_bytes")]
        group_id: Vec<u8>,
        entries: Vec<serde_bytes::ByteBuf>,
        /// A long log comes in parts, one after the other, each with its number and how
        /// many there are; the device takes the log once it has them all. Left out for a
        /// log in one part.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        part: Option<(u32, u32)>,
    },
    /// From an admin to a device that asked to join with an invite: an admin said no, and
    /// the invite no longer works.
    JoinDeclined {
        #[serde(with = "serde_bytes")]
        group_id: Vec<u8>,
        /// The `Revoke` log entry that cancelled the invite (canonical encoding), the
        /// issuing admin's when the log has it. The device believes the refusal only when
        /// that admin signed it; older devices send none and take it without.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        revoke: Option<serde_bytes::ByteBuf>,
    },
    /// From a member or friend that saw two live sessions of the receiver's own device at
    /// once, to each of them: the same identity is open on another computer too. Older
    /// devices cannot read it, which only fails this request.
    OtherSession,
    /// Messages of a direct chat, sealed with the key of the chat's two-person group, for
    /// the other person only (never forwarded); see [`DirectPayload`].
    Direct {
        #[serde(with = "serde_bytes")]
        group_id: Vec<u8>,
        messages: Vec<WireMessage>,
    },
    /// The group's banner, signed by the admin who chose it (`fogata_state::GroupBanner`).
    /// Only for devices that send a [`MemberRequest::Roster`] `banner`.
    GroupBanner {
        #[serde(with = "serde_bytes")]
        group_id: Vec<u8>,
        #[serde(with = "serde_bytes")]
        banner: Vec<u8>,
    },
    /// The group's picture, signed by the admin who chose it (`fogata_state::GroupPicture`).
    /// Only for devices that send a [`MemberRequest::Roster`] `picture`.
    GroupPicture {
        #[serde(with = "serde_bytes")]
        group_id: Vec<u8>,
        #[serde(with = "serde_bytes")]
        picture: Vec<u8>,
    },
    /// Asks for room events of the group by message id, 16 bytes each one after the other:
    /// ones a participant's presence named as recent (`GroupPayload::RoomHere`) that the
    /// asker lacks. The answer is a [`MemberResponse::RoomLog`] with those the peer still
    /// keeps. Only for devices that advertise [`SPREAD_MARKER`].
    RoomEvents {
        #[serde(with = "serde_bytes")]
        group_id: Vec<u8>,
        #[serde(with = "serde_bytes")]
        ids: Vec<u8>,
    },
    /// From a member to an admin: the box of key epoch `epoch` (the current one) that the
    /// log gave the sender does not open, so it cannot read or write in the group; an admin
    /// may start a new epoch. Older devices cannot read it, which only fails this request.
    KeyProblem {
        #[serde(with = "serde_bytes")]
        group_id: Vec<u8>,
        epoch: u32,
    },
}

/// Protocol a device advertises (identify) when it takes [`MemberRequest::Spread`] and
/// [`MemberRequest::RoomEvents`]. Nothing is ever sent over it.
pub const SPREAD_MARKER: &str = "/fogata/spread/1";

/// Bytes of a short peer id in [`MemberRequest::Spread`].
pub const SHORT_PEER_BYTES: usize = 8;

/// The answer to a [`MemberRequest`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MemberResponse {
    /// A sealed membership proof, or something that looks like one.
    Proof(WireProof),
    /// Kept for older peers; this version always answers a challenge with `Proof`.
    NotMember,
    /// The request was accepted.
    Ack,
    /// The request was refused because the sender has not proved membership.
    Refused,
    Roster {
        /// Transport peer ids, in their binary form.
        peers: Vec<serde_bytes::ByteBuf>,
    },
    SyncReply {
        entries: Vec<serde_bytes::ByteBuf>,
        want: Vec<serde_bytes::ByteBuf>,
    },
    /// The ticket is acceptable; sign a `Join` on top of these heads.
    JoinOffer { heads: Vec<serde_bytes::ByteBuf> },
    /// The join was applied; here is the whole log, including the `Welcome` with the key.
    JoinAccepted { entries: Vec<serde_bytes::ByteBuf> },
    /// The join waits for an admin to let the device in; the admin says so with
    /// [`MemberRequest::Admitted`] or [`MemberRequest::JoinDeclined`].
    JoinWaiting,
    /// An admin said no to joining with this invite. Nothing proves it, so devices that
    /// check refusals wait for [`MemberRequest::JoinDeclined`] with the signed `Revoke`,
    /// which this version sends along.
    JoinDeclined,
    /// The events of the current room (empty if there is none), or a part of them that
    /// fits one answer: then `more` says from which event to ask for the rest. Older
    /// devices give all of them at once and leave it out.
    RoomLog {
        messages: Vec<WireMessage>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        more: Option<u32>,
    },
}

/// libp2p protocol name of the friend protocol.
pub const FRIEND_PROTOCOL: &str = "/fogata/friend/1";

/// A request between devices that are friends or are becoming friends. The fields are opaque
/// here; `fogata-friends` builds and checks them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FriendRequest {
    /// Asks a device on the local network for a tag that only its friends recognise.
    Probe {
        #[serde(with = "serde_bytes")]
        nonce: Vec<u8>,
    },
    /// A friend says who it is on a new connection.
    Hello {
        #[serde(with = "serde_bytes")]
        mac: Vec<u8>,
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    /// A friend request made with an invite link.
    LinkRequest {
        #[serde(with = "serde_bytes")]
        mac: Vec<u8>,
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    /// Starts a run with a short code.
    CodeStart {
        #[serde(with = "serde_bytes")]
        gate: Vec<u8>,
        #[serde(with = "serde_bytes")]
        spake: Vec<u8>,
    },
    /// Last message of a code run: the friend request.
    CodeFinish {
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    /// Accepts a friend request the receiver made earlier.
    Accept {
        #[serde(with = "serde_bytes")]
        mac: Vec<u8>,
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    /// A newer profile of the sender.
    Profile {
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    /// The log of a group the receiver was added to and may not know yet, or entries of a
    /// group the sender does not run (the `Leave` of an invitation it declined).
    Group {
        entries: Vec<serde_bytes::ByteBuf>,
        /// The sender declined the invitation that the entry with this hash (the `Add` of
        /// the sender, or its `Join`) made, and `entries` are the `Leave` it signed for it:
        /// the member that invited it can say so. Older versions neither send nor read it,
        /// and take the entries as before.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        declined: Option<serde_bytes::ByteBuf>,
        /// A long log comes in parts, one after the other, each with its number and how many
        /// there are; the receiver takes the log once it has them all. Left out for a log in
        /// one part.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        part: Option<(u32, u32)>,
    },
    /// Whether the sender is in a room.
    Status {
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    /// Keeps the connection open while both friends are online.
    Heartbeat,
    /// A step of a direct call, sealed to the receiver.
    Call {
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    /// The sender's banner, sealed to the receiver.
    Banner {
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    /// The sender removed the receiver from its friends, signed and sealed to the receiver.
    Unfriend {
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    /// Asks which friends the two have in common, sealed to the receiver.
    Mutual {
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    /// The log of the two-person group a direct chat between the two runs over, for a friend
    /// that may not have it yet.
    Direct { entries: Vec<serde_bytes::ByteBuf> },
    /// The sender's animation of its picture or banner, sealed to the receiver. Older
    /// devices cannot read it, which only fails this request: they show the still picture.
    Animation {
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    /// The sender's bio, sealed to the receiver. Older devices cannot read it, which only
    /// fails this request: their cards show no bio.
    Bio {
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
}

/// The answer to a [`FriendRequest`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FriendResponse {
    ProbeReply {
        #[serde(with = "serde_bytes")]
        tag: Vec<u8>,
    },
    HelloReply {
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    CodeReply {
        #[serde(with = "serde_bytes")]
        spake: Vec<u8>,
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    /// The friend request arrived and waits for the person to answer.
    Pending,
    Ack,
    Refused,
    /// The answer to [`FriendRequest::Mutual`], sealed to the asker.
    Mutual {
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
    /// The answer to [`FriendRequest::Unfriend`]: the former friend's signature over both
    /// transport ids and the removal, sealed to the device that removed it.
    Unfriended {
        #[serde(with = "serde_bytes")]
        sealed: Vec<u8>,
    },
}

/// Wire form of a [`SealedProof`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WireProof {
    pub epoch: u32,
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub ciphertext: Vec<u8>,
}

impl From<&SealedProof> for WireProof {
    fn from(proof: &SealedProof) -> Self {
        Self {
            epoch: proof.epoch,
            nonce: proof.nonce.to_vec(),
            ciphertext: proof.ciphertext.clone(),
        }
    }
}

impl TryFrom<&WireProof> for SealedProof {
    type Error = CryptoError;

    fn try_from(wire: &WireProof) -> Result<Self, Self::Error> {
        Ok(Self {
            epoch: wire.epoch,
            nonce: array(&wire.nonce)?,
            ciphertext: wire.ciphertext.clone(),
        })
    }
}

/// Wire form of a [`SealedMessage`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WireMessage {
    pub version: u8,
    #[serde(with = "serde_bytes")]
    pub group_id: Vec<u8>,
    pub epoch: u32,
    #[serde(with = "serde_bytes")]
    pub sender: Vec<u8>,
    pub timestamp_ms: u64,
    #[serde(with = "serde_bytes")]
    pub message_id: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub nonce: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub ciphertext: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
}

impl From<&SealedMessage> for WireMessage {
    fn from(message: &SealedMessage) -> Self {
        Self {
            version: message.header.version,
            group_id: message.header.group_id.as_bytes().to_vec(),
            epoch: message.header.epoch,
            sender: message.header.sender.as_bytes().to_vec(),
            timestamp_ms: message.header.timestamp_ms,
            message_id: message.header.message_id.to_vec(),
            nonce: message.nonce.to_vec(),
            ciphertext: message.ciphertext.clone(),
            signature: message.signature.to_vec(),
        }
    }
}

impl TryFrom<&WireMessage> for SealedMessage {
    type Error = CryptoError;

    fn try_from(wire: &WireMessage) -> Result<Self, Self::Error> {
        Ok(Self {
            header: MessageHeader {
                version: wire.version,
                group_id: GroupId::from_bytes(&wire.group_id)?,
                epoch: wire.epoch,
                sender: DevicePublicKey::from_bytes(&wire.sender)?,
                timestamp_ms: wire.timestamp_ms,
                message_id: array(&wire.message_id)?,
            },
            nonce: array(&wire.nonce)?,
            ciphertext: wire.ciphertext.clone(),
            signature: array(&wire.signature)?,
        })
    }
}

/// Plaintext of a group message, before sealing. Group messages only exist inside the room:
/// everything said in a group is said in its room.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GroupPayload {
    RoomJoin {
        /// The voice room; the group's first room when left out.
        #[serde(default, skip_serializing_if = "is_first_room")]
        voice: [u8; 8],
        room: [u8; 16],
        started_ms: u64,
        nick: String,
    },
    RoomHere {
        /// The voice room; the group's first room when left out.
        #[serde(default, skip_serializing_if = "is_first_room")]
        voice: [u8; 8],
        room: [u8; 16],
        started_ms: u64,
        nick: String,
        /// Ids of the room's latest events the sender has (joins, leaves, chat, files), 16
        /// bytes each one after the other: a member that lacks one asks for it. Older
        /// devices send none and ignore it.
        #[serde(default, skip_serializing_if = "Vec::is_empty", with = "base64_text")]
        recent: Vec<u8>,
    },
    RoomLeave {
        /// The voice room; the group's first room when left out.
        #[serde(default, skip_serializing_if = "is_first_room")]
        voice: [u8; 8],
        room: [u8; 16],
    },
    RoomChat {
        /// The voice room; the group's first room when left out.
        #[serde(default, skip_serializing_if = "is_first_room")]
        voice: [u8; 8],
        room: [u8; 16],
        nick: String,
        text: String,
    },
    /// The sender stops serving file `id`.
    RoomUnshare {
        /// The voice room; the group's first room when left out.
        #[serde(default, skip_serializing_if = "is_first_room")]
        voice: [u8; 8],
        room: [u8; 16],
        id: [u8; 32],
    },
    /// The sender has this file (BLAKE3 hash `id`) and serves it while it is in the room.
    RoomShare {
        /// The voice room; the group's first room when left out.
        #[serde(default, skip_serializing_if = "is_first_room")]
        voice: [u8; 8],
        room: [u8; 16],
        id: [u8; 32],
        name: String,
        size: u64,
        /// A video's first picture and length, as the sharer saw them.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        video: Option<WireVideo>,
    },
    /// An admin takes `member` (a device key) out of the voice room now; it may come back
    /// at once. Versions from before it cannot read it and drop it.
    RoomRemove {
        /// The voice room; the group's first room when left out.
        #[serde(default, skip_serializing_if = "is_first_room")]
        voice: [u8; 8],
        room: [u8; 16],
        member: [u8; 32],
    },
}

/// What a shared video looks like before it is fetched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireVideo {
    /// A small JPEG of the first picture.
    #[serde(with = "base64_text")]
    pub thumb: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
}

/// Bytes as base64 text, so a picture stays compact inside a JSON payload.
pub(crate) mod base64_text {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        STANDARD.decode(text).map_err(serde::de::Error::custom)
    }
}

fn is_first_room(voice: &[u8; 8]) -> bool {
    *voice == [0; 8]
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl GroupPayload {
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("plain data always serialises")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CryptoError> {
        serde_json::from_slice(bytes).map_err(|_| CryptoError::Malformed)
    }
}

fn array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], CryptoError> {
    bytes.try_into().map_err(|_| CryptoError::Malformed)
}

#[cfg(test)]
mod tests {
    use fogata_crypto::{DeviceIdentity, GroupKeys, GroupSecret};

    use super::*;

    #[test]
    fn shared_bytes_go_as_a_byte_string() {
        let shared = SharedBytes::from(vec![1, 2, 3]);
        let json = serde_json::to_string(&shared).unwrap();
        assert_eq!(
            json,
            serde_json::to_string(&serde_bytes::ByteBuf::from(vec![1, 2, 3])).unwrap()
        );
        assert_eq!(serde_json::from_str::<SharedBytes>(&json).unwrap(), shared);
        // Copies share the bytes.
        let copy = shared.clone();
        assert!(Arc::ptr_eq(&copy.0, &shared.0));
    }

    #[test]
    fn sealed_message_survives_the_wire_form() {
        let keys = GroupKeys::derive(
            GroupId::random().unwrap(),
            7,
            &GroupSecret::generate().unwrap(),
        );
        let identity = DeviceIdentity::generate().unwrap();
        let payload = GroupPayload::RoomChat {
            voice: [0; 8],
            room: [3; 16],
            nick: "ana".into(),
            text: "hi".into(),
        };
        let sealed = SealedMessage::seal(&keys, &identity, 1, &payload.encode()).unwrap();
        let back = SealedMessage::try_from(&WireMessage::from(&sealed)).unwrap();
        assert_eq!(back, sealed);
        assert_eq!(
            GroupPayload::decode(&back.open(&keys).unwrap()).unwrap(),
            payload
        );
    }

    #[test]
    fn presence_with_recent_events_reads_with_older_devices() {
        // A presence as older devices know it.
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum Older {
            RoomHere {
                room: [u8; 16],
                started_ms: u64,
                nick: String,
            },
        }
        let here = GroupPayload::RoomHere {
            voice: [0; 8],
            room: [1; 16],
            started_ms: 5,
            nick: "Ana".into(),
            recent: [[7u8; 16], [8; 16]].concat(),
        };
        let encoded = here.encode();
        let Older::RoomHere {
            room,
            started_ms,
            nick,
        } = serde_json::from_slice(&encoded).unwrap();
        assert_eq!((room, started_ms, nick.as_str()), ([1; 16], 5, "Ana"));
        assert_eq!(GroupPayload::decode(&encoded).unwrap(), here);
        // What older devices send reads with nothing recent.
        let older = br#"{"kind":"room_here","room":[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1],"started_ms":5,"nick":"Ana"}"#;
        let GroupPayload::RoomHere { recent, .. } = GroupPayload::decode(older).unwrap() else {
            panic!("not a presence");
        };
        assert!(recent.is_empty());
    }

    #[test]
    fn the_first_voice_room_is_left_out_and_read_back() {
        let first = GroupPayload::RoomLeave {
            voice: [0; 8],
            room: [1; 16],
        };
        let text = String::from_utf8(first.encode()).unwrap();
        assert!(!text.contains("voice"));
        assert_eq!(GroupPayload::decode(text.as_bytes()).unwrap(), first);
        let other = GroupPayload::RoomLeave {
            voice: [5; 8],
            room: [1; 16],
        };
        assert_eq!(GroupPayload::decode(&other.encode()).unwrap(), other);
    }

    #[test]
    fn a_room_log_in_parts_reads_as_before_with_older_devices() {
        // The first ask is what older devices send, and their whole answer still reads.
        let first = MemberRequest::RoomSync {
            group_id: vec![1; 16],
            from: 0,
        };
        let text = serde_json::to_string(&first).unwrap();
        assert!(!text.contains("from"));
        let older: MemberResponse = serde_json::from_str(r#"{"RoomLog":{"messages":[]}}"#).unwrap();
        assert!(matches!(older, MemberResponse::RoomLog { more: None, .. }));
        // An older device ignores what it does not know.
        let later = MemberRequest::RoomSync {
            group_id: vec![1; 16],
            from: 40,
        };
        let back: MemberRequest =
            serde_json::from_str(&serde_json::to_string(&later).unwrap()).unwrap();
        assert!(matches!(back, MemberRequest::RoomSync { from: 40, .. }));
    }

    #[test]
    fn truncated_fields_are_rejected() {
        let keys = GroupKeys::derive(
            GroupId::random().unwrap(),
            7,
            &GroupSecret::generate().unwrap(),
        );
        let identity = DeviceIdentity::generate().unwrap();
        let sealed = SealedMessage::seal(&keys, &identity, 1, b"x").unwrap();
        let mut wire = WireMessage::from(&sealed);
        wire.nonce.pop();
        assert_eq!(SealedMessage::try_from(&wire), Err(CryptoError::Malformed));
    }
}
