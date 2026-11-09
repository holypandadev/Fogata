//! The messages that make two devices friends and let friends recognise each other later.
//!
//! Every message is bound to the transport identities of the two ends of the connection it
//! travels on (`local` and `remote`, as authenticated by the transport), so that it cannot be
//! replayed on another connection. Who a device is never travels in the clear: it is sealed
//! to the other device's key, or encrypted with a key only the two ends of a code run share.

use std::fmt;

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use fogata_crypto::{
    DeviceIdentity, DevicePublicKey, NONCE_LEN, SIGNATURE_LEN, Sealed,
    codec::{Reader, Writer, encode_secret},
    random_bytes,
};
use sha2::{Digest, Sha256};
use spake2::{Ed25519Group, Identity, Password, Spake2};
use zeroize::Zeroizing;

use crate::{
    Animation, Banner, Bio, CodeSecrets, FriendError, FriendLink, FriendSecret, MAX_MUTUAL_POINTS,
    MutualAnswer, Point, Profile,
    secret::{check, derive, mac},
};

#[cfg(feature = "fuzzing")]
pub mod fuzz;

const HELLO: &[u8] = b"fogata/v1/friend-hello";
const HELLO_REPLY: &[u8] = b"fogata/v1/friend-hello-reply";
const HELLO_KNOWS: &[u8] = b"fogata/v1/friend-hello-knows";
const LINK_REQUEST: &[u8] = b"fogata/v1/friend-link-request";
const MEMBER_REQUEST: &[u8] = b"fogata/v1/friend-member-request";
const ACCEPT: &[u8] = b"fogata/v1/friend-accept";
const PROFILE_UPDATE: &[u8] = b"fogata/v1/friend-profile";
const STATUS: &[u8] = b"fogata/v1/friend-status";
const BANNER: &[u8] = b"fogata/v1/friend-banner";
const BIO: &[u8] = b"fogata/v1/friend-bio";
const ANIMATION: &[u8] = b"fogata/v1/friend-animation";
const UNFRIEND: &[u8] = b"fogata/v1/friend-unfriend";
const UNFRIEND_ACK: &[u8] = b"fogata/v1/friend-unfriend-ack";
const MUTUAL_ASK: &[u8] = b"fogata/v1/friend-mutual-ask";
const MUTUAL_ANSWER: &[u8] = b"fogata/v1/friend-mutual-answer";
const CALL: &[u8] = b"fogata/v1/friend-call";
const CODE_GATE: &[u8] = b"fogata/v1/friend-code-start";
const CODE_TRANSCRIPT: &[u8] = b"fogata/v1/friend-code-transcript";
const CODE_RUN: &[u8] = b"fogata/v1/friend-code-run";
const CODE_REPLY: &[u8] = b"fogata/v1/friend-code-reply";
const CODE_FINISH: &[u8] = b"fogata/v1/friend-code-finish";
const SPAKE_REQUESTER: &[u8] = b"fogata/v1/requester";
const SPAKE_INVITER: &[u8] = b"fogata/v1/inviter";

/// A keyed check that the sender knows a shared secret, and a body sealed to the receiver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Envelope {
    pub mac: [u8; 32],
    pub sealed: Vec<u8>,
}

/// A friend request or acceptance, opened and checked: the sender's profile and friend
/// secret.
#[derive(Clone, Debug)]
pub struct Introduction {
    pub profile: Profile,
    pub secret: FriendSecret,
}

/// A checked hello: the sender owns `device`. Whether it is a friend is up to the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelloFrom {
    pub device: DevicePublicKey,
    /// Which of this device's friend secrets (by its place in the list given to
    /// [`open_hello`]) the sender has, if any: the first, the current one, tells that it got
    /// the newest.
    pub knows: Option<usize>,
    /// Version of this device's profile the sender has.
    pub knows_version: u64,
    /// The sender appears offline: it asks to be shown as such.
    pub hidden: bool,
}

/// A checked answer to a hello.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelloReply {
    /// Version of this device's profile the friend has.
    pub knows_version: u64,
    /// The friend's profile, if ours was older.
    pub profile: Option<Profile>,
    /// The friend appears offline: it asks to be shown as such.
    pub hidden: bool,
}

/// Label, the sender's and receiver's transport identities and the receiver's device key:
/// what signatures cover and what sealed bodies are bound to.
fn bound(
    label: &[u8],
    sender_peer: &[u8],
    receiver_peer: &[u8],
    receiver: &DevicePublicKey,
) -> Vec<u8> {
    let mut w = Writer::default();
    w.raw(label)
        .bytes(sender_peer)
        .bytes(receiver_peer)
        .key(receiver);
    w.into_bytes()
}

fn encode_sealed(sealed: &Sealed) -> Vec<u8> {
    let mut w = Writer::default();
    w.sealed(sealed);
    w.into_bytes()
}

fn open_sealed(
    identity: &DeviceIdentity,
    context: &[u8],
    bytes: &[u8],
) -> Result<Zeroizing<Vec<u8>>, FriendError> {
    let mut r = Reader::new(bytes);
    let sealed = r.sealed()?;
    r.finish()?;
    Ok(Zeroizing::new(sealed.open(identity, context)?))
}

/// A flag that versions before it do not send: their messages end without it.
fn trailing_flag(r: &mut Reader) -> Result<bool, FriendError> {
    if r.is_empty() {
        return Ok(false);
    }
    match r.u8()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(FriendError::Malformed),
    }
}

fn verify(device: &DevicePublicKey, message: &[u8], signature: &[u8]) -> Result<(), FriendError> {
    device
        .verify(message, signature)
        .map_err(|_| FriendError::NotProved)
}

// ----- Friends meeting again -----

/// First message to a friend on a new connection: an HMAC keyed by the secret the two share
/// (`pair`, see [`FriendSecret::pair`]), so that the friend answers only a device it knows
/// and nobody else (another friend of it, which knows its own secret) can tell from the hello
/// whom this device is friends with; and, sealed so that only the friend can read it, which
/// device it is, and an HMAC keyed by the friend's secret as this device knows it, which tells
/// the friend whether this device has its newest one. `knows_version` is the version of the
/// friend's profile this device has; `hidden` asks the friend to show this device as offline.
#[allow(clippy::too_many_arguments)]
pub fn hello(
    identity: &DeviceIdentity,
    friend: &DevicePublicKey,
    pair: &FriendSecret,
    friend_secret: &FriendSecret,
    local: &[u8],
    remote: &[u8],
    knows_version: u64,
    hidden: bool,
) -> Result<Envelope, FriendError> {
    let context = bound(HELLO, local, remote, friend);
    let mut w = Writer::default();
    w.key(&identity.public_key())
        .raw(&identity.sign(&context))
        .u64(knows_version)
        .u8(hidden.into())
        .raw(&mac(
            &friend_secret.hello_key(),
            &[HELLO_KNOWS, local, remote],
        ));
    let sealed = Sealed::seal(friend, &context, &w.into_bytes())?;
    Ok(Envelope {
        mac: mac(&pair.hello_key(), &[HELLO, local, remote]),
        sealed: encode_sealed(&sealed),
    })
}

/// Checks a hello sent to this device by the friend it shares `pair` with; `own_secrets`
/// are this device's friend secrets, current first (see [`HelloFrom::knows`]).
pub fn open_hello(
    identity: &DeviceIdentity,
    pair: &FriendSecret,
    own_secrets: &[&FriendSecret],
    local: &[u8],
    remote: &[u8],
    mac: &[u8],
    sealed: &[u8],
) -> Result<HelloFrom, FriendError> {
    if !check(&pair.hello_key(), &[HELLO, remote, local], mac) {
        return Err(FriendError::NotProved);
    }
    let context = bound(HELLO, remote, local, &identity.public_key());
    let body = open_sealed(identity, &context, sealed)?;
    let mut r = Reader::new(&body);
    let device = r.key()?;
    let signature: [u8; SIGNATURE_LEN] = r.array()?;
    let knows_version = r.u64()?;
    let hidden = match r.u8()? {
        0 => false,
        1 => true,
        _ => return Err(FriendError::Malformed),
    };
    let knows_secret: [u8; 32] = r.array()?;
    r.finish()?;
    verify(&device, &context, &signature)?;
    let knows = own_secrets.iter().position(|secret| {
        check(
            &secret.hello_key(),
            &[HELLO_KNOWS, remote, local],
            &knows_secret,
        )
    });
    Ok(HelloFrom {
        device,
        knows,
        knows_version,
        hidden,
    })
}

/// Answer to a friend's hello, with this device's profile if the friend's copy is older;
/// `hidden` asks the friend to show this device as offline.
pub fn hello_reply(
    identity: &DeviceIdentity,
    friend: &DevicePublicKey,
    local: &[u8],
    remote: &[u8],
    knows_version: u64,
    profile: Option<&Profile>,
    hidden: bool,
) -> Result<Vec<u8>, FriendError> {
    let context = bound(HELLO_REPLY, local, remote, friend);
    let mut w = Writer::default();
    w.raw(&identity.sign(&context)).u64(knows_version);
    match profile {
        Some(profile) => w.u8(1).bytes(&profile.encode()),
        None => w.u8(0),
    };
    w.u8(hidden.into());
    Ok(encode_sealed(&Sealed::seal(
        friend,
        &context,
        &w.into_bytes(),
    )?))
}

/// Checks a friend's answer to our hello.
pub fn open_hello_reply(
    identity: &DeviceIdentity,
    friend: &DevicePublicKey,
    local: &[u8],
    remote: &[u8],
    sealed: &[u8],
) -> Result<HelloReply, FriendError> {
    let context = bound(HELLO_REPLY, remote, local, &identity.public_key());
    let body = open_sealed(identity, &context, sealed)?;
    let mut r = Reader::new(&body);
    let signature: [u8; SIGNATURE_LEN] = r.array()?;
    let knows_version = r.u64()?;
    let profile = match r.u8()? {
        0 => None,
        1 => Some(Profile::decode(r.bytes()?)?),
        _ => return Err(FriendError::Malformed),
    };
    let hidden = trailing_flag(&mut r)?;
    r.finish()?;
    verify(friend, &context, &signature)?;
    if profile.as_ref().is_some_and(|p| p.device != *friend) {
        return Err(FriendError::NotProved);
    }
    Ok(HelloReply {
        knows_version,
        profile,
        hidden,
    })
}

/// A newer profile for a friend we are connected to.
pub fn profile_update(
    friend: &DevicePublicKey,
    profile: &Profile,
    local: &[u8],
    remote: &[u8],
) -> Result<Vec<u8>, FriendError> {
    let context = bound(PROFILE_UPDATE, local, remote, friend);
    Ok(encode_sealed(&Sealed::seal(
        friend,
        &context,
        &profile.encode(),
    )?))
}

/// Opens a profile update from `friend`.
pub fn open_profile_update(
    identity: &DeviceIdentity,
    friend: &DevicePublicKey,
    local: &[u8],
    remote: &[u8],
    sealed: &[u8],
) -> Result<Profile, FriendError> {
    let context = bound(PROFILE_UPDATE, remote, local, &identity.public_key());
    let profile = Profile::decode(&open_sealed(identity, &context, sealed)?)?;
    if profile.device != *friend {
        return Err(FriendError::NotProved);
    }
    Ok(profile)
}

/// What a friend is doing: whether it is in a room, and the room's group when the receiver
/// is a member of it too, or whether it is away from its computer; or that it appears
/// offline, and then nothing else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub in_room: bool,
    pub group: Option<[u8; 16]>,
    pub hidden: bool,
    /// Nobody used the friend's computer for a while.
    pub away: bool,
}

/// Tells a friend we are connected to what this device is doing.
pub fn status(
    friend: &DevicePublicKey,
    status: &Status,
    local: &[u8],
    remote: &[u8],
) -> Result<Vec<u8>, FriendError> {
    let context = bound(STATUS, local, remote, friend);
    let mut w = Writer::default();
    w.u8(status.in_room.into());
    match &status.group {
        Some(group) => w.u8(1).raw(group),
        None => w.u8(0),
    };
    w.u8(status.hidden.into());
    // Only when away, so that versions before it, which take nothing after `hidden`, still
    // read every other status.
    if status.away {
        w.u8(1);
    }
    Ok(encode_sealed(&Sealed::seal(
        friend,
        &context,
        &w.into_bytes(),
    )?))
}

/// Opens a status from the friend on the other end of the connection.
pub fn open_status(
    identity: &DeviceIdentity,
    local: &[u8],
    remote: &[u8],
    sealed: &[u8],
) -> Result<Status, FriendError> {
    let context = bound(STATUS, remote, local, &identity.public_key());
    let body = open_sealed(identity, &context, sealed)?;
    let mut r = Reader::new(&body);
    let in_room = r.u8()? != 0;
    let group = match r.u8()? {
        0 => None,
        1 => Some(r.array()?),
        _ => return Err(FriendError::Malformed),
    };
    let hidden = trailing_flag(&mut r)?;
    let away = trailing_flag(&mut r)?;
    r.finish()?;
    Ok(Status {
        in_room,
        group,
        hidden,
        away,
    })
}

// ----- Banners, leaving and friends in common -----

/// This device's banner, for a friend we are connected to.
pub fn banner_update(
    friend: &DevicePublicKey,
    banner: &Banner,
    local: &[u8],
    remote: &[u8],
) -> Result<Vec<u8>, FriendError> {
    let context = bound(BANNER, local, remote, friend);
    Ok(encode_sealed(&Sealed::seal(
        friend,
        &context,
        &banner.encode(),
    )?))
}

/// Opens a banner from `friend`.
pub fn open_banner_update(
    identity: &DeviceIdentity,
    friend: &DevicePublicKey,
    local: &[u8],
    remote: &[u8],
    sealed: &[u8],
) -> Result<Banner, FriendError> {
    let context = bound(BANNER, remote, local, &identity.public_key());
    let banner = Banner::decode(&open_sealed(identity, &context, sealed)?)?;
    if banner.device != *friend {
        return Err(FriendError::NotProved);
    }
    Ok(banner)
}

/// This device's bio, for a friend we are connected to.
pub fn bio_update(
    friend: &DevicePublicKey,
    bio: &Bio,
    local: &[u8],
    remote: &[u8],
) -> Result<Vec<u8>, FriendError> {
    let context = bound(BIO, local, remote, friend);
    Ok(encode_sealed(&Sealed::seal(
        friend,
        &context,
        &bio.encode(),
    )?))
}

/// Opens a bio from `friend`.
pub fn open_bio_update(
    identity: &DeviceIdentity,
    friend: &DevicePublicKey,
    local: &[u8],
    remote: &[u8],
    sealed: &[u8],
) -> Result<Bio, FriendError> {
    let context = bound(BIO, remote, local, &identity.public_key());
    let bio = Bio::decode(&open_sealed(identity, &context, sealed)?)?;
    if bio.device != *friend {
        return Err(FriendError::NotProved);
    }
    Ok(bio)
}

/// This device's animation of its picture or banner, for a friend we are connected to.
pub fn animation_update(
    friend: &DevicePublicKey,
    animation: &Animation,
    local: &[u8],
    remote: &[u8],
) -> Result<Vec<u8>, FriendError> {
    let context = bound(ANIMATION, local, remote, friend);
    Ok(encode_sealed(&Sealed::seal(
        friend,
        &context,
        &animation.encode(),
    )?))
}

/// Opens an animation from `friend`.
pub fn open_animation_update(
    identity: &DeviceIdentity,
    friend: &DevicePublicKey,
    local: &[u8],
    remote: &[u8],
    sealed: &[u8],
) -> Result<Animation, FriendError> {
    let context = bound(ANIMATION, remote, local, &identity.public_key());
    let animation = Animation::decode(&open_sealed(identity, &context, sealed)?)?;
    if animation.device != *friend {
        return Err(FriendError::NotProved);
    }
    Ok(animation)
}

/// Tells a former friend that this device removed it, so it forgets this device too. Signed,
/// since it may go to a device that has not greeted us on this connection.
pub fn unfriend(
    identity: &DeviceIdentity,
    friend: &DevicePublicKey,
    local: &[u8],
    remote: &[u8],
) -> Result<Vec<u8>, FriendError> {
    let context = bound(UNFRIEND, local, remote, friend);
    let mut w = Writer::default();
    w.key(&identity.public_key()).raw(&identity.sign(&context));
    Ok(encode_sealed(&Sealed::seal(
        friend,
        &context,
        &w.into_bytes(),
    )?))
}

/// Opens a removal: the device that no longer counts this one as a friend.
pub fn open_unfriend(
    identity: &DeviceIdentity,
    local: &[u8],
    remote: &[u8],
    sealed: &[u8],
) -> Result<DevicePublicKey, FriendError> {
    let context = bound(UNFRIEND, remote, local, &identity.public_key());
    let body = open_sealed(identity, &context, sealed)?;
    let mut r = Reader::new(&body);
    let device = r.key()?;
    let signature: [u8; SIGNATURE_LEN] = r.array()?;
    r.finish()?;
    verify(&device, &context, &signature)?;
    Ok(device)
}

/// The SHA-256 of a removal as sent ([`unfriend`]), which its acknowledgment names.
pub fn removal_digest(removal: &[u8]) -> [u8; 32] {
    Sha256::digest(removal).into()
}

/// What a former friend signs to acknowledge a removal: both transport ids, the key of the
/// device that removed it and the removal's digest ([`removal_digest`]).
fn unfriend_ack_context(
    sender_peer: &[u8],
    receiver_peer: &[u8],
    remover: &DevicePublicKey,
    removal: &[u8; 32],
) -> Vec<u8> {
    let mut context = bound(UNFRIEND_ACK, sender_peer, receiver_peer, remover);
    context.extend_from_slice(removal);
    context
}

/// A former friend's answer to a removal (`removal`, its digest, as it came from `remover`):
/// its signature over both transport ids and the removal, sealed to the device that removed it.
/// Only the former friend's key makes it, so whoever else answers where the former friend
/// is looked for (another of its friends, which knows its secret) cannot make the device
/// stop telling it.
pub fn unfriend_ack(
    identity: &DeviceIdentity,
    remover: &DevicePublicKey,
    local: &[u8],
    remote: &[u8],
    removal: &[u8; 32],
) -> Result<Vec<u8>, FriendError> {
    let context = unfriend_ack_context(local, remote, remover, removal);
    Ok(encode_sealed(&Sealed::seal(
        remover,
        &context,
        &identity.sign(&context),
    )?))
}

/// Checks that `friend` (the former friend) acknowledged the removal sent from here whose
/// digest is `removal`.
pub fn open_unfriend_ack(
    identity: &DeviceIdentity,
    friend: &DevicePublicKey,
    local: &[u8],
    remote: &[u8],
    removal: &[u8; 32],
    sealed: &[u8],
) -> Result<(), FriendError> {
    let context = unfriend_ack_context(remote, local, &identity.public_key(), removal);
    let body = open_sealed(identity, &context, sealed)?;
    verify(friend, &context, &body)
}

fn write_points(w: &mut Writer, points: &[Point]) {
    w.u32(points.len() as u32);
    for point in points {
        w.raw(point);
    }
}

fn read_points(r: &mut Reader<'_>) -> Result<Vec<Point>, FriendError> {
    let count = r.count(32)?;
    if count > MAX_MUTUAL_POINTS {
        return Err(FriendError::Malformed);
    }
    (0..count).map(|_| Ok(r.array()?)).collect()
}

/// Asks a friend we are connected to which friends we have in common, with the points of a
/// [`crate::MutualAsk`].
pub fn mutual_ask(
    friend: &DevicePublicKey,
    points: &[Point],
    local: &[u8],
    remote: &[u8],
) -> Result<Vec<u8>, FriendError> {
    let context = bound(MUTUAL_ASK, local, remote, friend);
    let mut w = Writer::default();
    write_points(&mut w, points);
    Ok(encode_sealed(&Sealed::seal(
        friend,
        &context,
        &w.into_bytes(),
    )?))
}

/// Opens a friend's question about friends in common.
pub fn open_mutual_ask(
    identity: &DeviceIdentity,
    local: &[u8],
    remote: &[u8],
    sealed: &[u8],
) -> Result<Vec<Point>, FriendError> {
    let context = bound(MUTUAL_ASK, remote, local, &identity.public_key());
    let body = open_sealed(identity, &context, sealed)?;
    let mut r = Reader::new(&body);
    let points = read_points(&mut r)?;
    r.finish()?;
    Ok(points)
}

/// The answer to a friend's question about friends in common.
pub fn mutual_reply(
    friend: &DevicePublicKey,
    answer: &MutualAnswer,
    local: &[u8],
    remote: &[u8],
) -> Result<Vec<u8>, FriendError> {
    let context = bound(MUTUAL_ANSWER, local, remote, friend);
    let mut w = Writer::default();
    write_points(&mut w, &answer.doubled);
    write_points(&mut w, &answer.hidden);
    Ok(encode_sealed(&Sealed::seal(
        friend,
        &context,
        &w.into_bytes(),
    )?))
}

/// Opens a friend's answer about friends in common.
pub fn open_mutual_reply(
    identity: &DeviceIdentity,
    local: &[u8],
    remote: &[u8],
    sealed: &[u8],
) -> Result<MutualAnswer, FriendError> {
    let context = bound(MUTUAL_ANSWER, remote, local, &identity.public_key());
    let body = open_sealed(identity, &context, sealed)?;
    let mut r = Reader::new(&body);
    let doubled = read_points(&mut r)?;
    let hidden = read_points(&mut r)?;
    r.finish()?;
    Ok(MutualAnswer { doubled, hidden })
}

// ----- Direct calls -----

/// Most group log entries a ring carries: a call's group is made with its two members.
const MAX_RING_ENTRIES: usize = 16;
/// Most group log entries an invitation carries: a call of up to eight, each added and
/// made an admin, with room to spare.
const MAX_INVITE_ENTRIES: usize = 64;

/// A step of a direct call between two friends. `call` is the id of the call's group, which
/// the caller makes with the two of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallMessage {
    pub call: [u8; 16],
    pub signal: CallSignal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallSignal {
    /// The caller rings, with the entries of the call's group log.
    Ring { log: Vec<Vec<u8>> },
    /// The one called answers.
    Accept,
    /// The one called cannot talk now.
    Decline,
    /// The one called is in another call.
    Busy,
    /// The caller stops ringing.
    Cancel,
    /// Either of them hangs up.
    End,
    /// Another session of the one called (the same identity open on another computer)
    /// answered or declined first: this one stops ringing, or leaves. Older devices cannot
    /// read it and refuse it.
    Elsewhere,
    /// Someone in a call rings a friend into it, with the entries of the call's group log,
    /// which has more members than the two of a call's start. Older devices cannot read it
    /// and refuse it.
    Invite { log: Vec<Vec<u8>> },
    /// This device takes calls of more than two: invitations, and coming back into a call
    /// left a moment ago. Older devices cannot read it and refuse it.
    GroupCalls,
    /// This device is still in the call, which started `age_ms` ago and had two or more
    /// people in it for its first `together_ms` (all of it, while it still has). Goes to
    /// those who left the call, now and then, and to someone who comes in.
    Going { age_ms: u64, together_ms: u64 },
}

/// A call step for a friend we are connected to, sealed to it.
pub fn call_message(
    friend: &DevicePublicKey,
    message: &CallMessage,
    local: &[u8],
    remote: &[u8],
) -> Result<Vec<u8>, FriendError> {
    let context = bound(CALL, local, remote, friend);
    let mut w = Writer::default();
    let kind = match &message.signal {
        CallSignal::Ring { .. } => 1,
        CallSignal::Accept => 2,
        CallSignal::Decline => 3,
        CallSignal::Busy => 4,
        CallSignal::Cancel => 5,
        CallSignal::End => 6,
        CallSignal::Elsewhere => 7,
        CallSignal::Invite { .. } => 8,
        CallSignal::GroupCalls => 9,
        CallSignal::Going { .. } => 10,
    };
    w.u8(kind).raw(&message.call);
    if let CallSignal::Going {
        age_ms,
        together_ms,
    } = &message.signal
    {
        w.u64(*age_ms).u64(*together_ms);
    }
    if let CallSignal::Ring { log } | CallSignal::Invite { log } = &message.signal {
        let max = if kind == 1 {
            MAX_RING_ENTRIES
        } else {
            MAX_INVITE_ENTRIES
        };
        if log.len() > max {
            return Err(FriendError::Malformed);
        }
        w.u32(log.len() as u32);
        for entry in log {
            w.bytes(entry);
        }
    }
    Ok(encode_sealed(&Sealed::seal(
        friend,
        &context,
        &w.into_bytes(),
    )?))
}

/// Opens a call step from the friend on the other end of the connection.
pub fn open_call_message(
    identity: &DeviceIdentity,
    local: &[u8],
    remote: &[u8],
    sealed: &[u8],
) -> Result<CallMessage, FriendError> {
    let context = bound(CALL, remote, local, &identity.public_key());
    let body = open_sealed(identity, &context, sealed)?;
    let mut r = Reader::new(&body);
    let kind = r.u8()?;
    let call: [u8; 16] = r.array()?;
    let signal = match kind {
        1 | 8 => {
            let count = r.u32()? as usize;
            let max = if kind == 1 {
                MAX_RING_ENTRIES
            } else {
                MAX_INVITE_ENTRIES
            };
            if count > max {
                return Err(FriendError::Malformed);
            }
            let mut log = Vec::with_capacity(count);
            for _ in 0..count {
                log.push(r.bytes()?.to_vec());
            }
            if kind == 1 {
                CallSignal::Ring { log }
            } else {
                CallSignal::Invite { log }
            }
        }
        2 => CallSignal::Accept,
        3 => CallSignal::Decline,
        4 => CallSignal::Busy,
        5 => CallSignal::Cancel,
        6 => CallSignal::End,
        7 => CallSignal::Elsewhere,
        9 => CallSignal::GroupCalls,
        10 => CallSignal::Going {
            age_ms: r.u64()?,
            together_ms: r.u64()?,
        },
        _ => return Err(FriendError::Malformed),
    };
    r.finish()?;
    Ok(CallMessage { call, signal })
}

// ----- Requests and acceptances -----

fn introduction_body(
    identity: &DeviceIdentity,
    profile: &Profile,
    secret: &FriendSecret,
    signed: &[u8],
) -> Zeroizing<Vec<u8>> {
    let (profile, signature) = (profile.encode(), identity.sign(signed));
    // Measured first, so the secret is written where it stays.
    encode_secret(0, |w| {
        w.bytes(&profile).raw(secret.as_bytes()).raw(&signature);
    })
}

fn read_introduction(body: &[u8], signed: &[u8]) -> Result<Introduction, FriendError> {
    let mut r = Reader::new(body);
    let profile = Profile::decode(r.bytes()?)?;
    let secret = FriendSecret::from_bytes(r.take(32)?)?;
    let signature: [u8; SIGNATURE_LEN] = r.array()?;
    r.finish()?;
    verify(&profile.device, signed, &signature)?;
    Ok(Introduction { profile, secret })
}

/// A friend request to the device that made `link`, carrying this device's profile and
/// friend secret.
pub fn link_request(
    identity: &DeviceIdentity,
    own_profile: &Profile,
    own_secret: &FriendSecret,
    link: &FriendLink,
    local: &[u8],
    remote: &[u8],
) -> Result<Envelope, FriendError> {
    let context = bound(LINK_REQUEST, local, remote, &link.device);
    let body = introduction_body(identity, own_profile, own_secret, &context);
    Ok(Envelope {
        mac: mac(&link.request_key(), &[LINK_REQUEST, local, remote]),
        sealed: encode_sealed(&Sealed::seal(&link.device, &context, &body)?),
    })
}

/// Opens a friend request made with `link`, a link this device made.
pub fn open_link_request(
    identity: &DeviceIdentity,
    link: &FriendLink,
    local: &[u8],
    remote: &[u8],
    mac: &[u8],
    sealed: &[u8],
) -> Result<Introduction, FriendError> {
    if !check(&link.request_key(), &[LINK_REQUEST, remote, local], mac) {
        return Err(FriendError::NotProved);
    }
    let context = bound(LINK_REQUEST, remote, local, &identity.public_key());
    read_introduction(&open_sealed(identity, &context, sealed)?, &context)
}

/// A friend request to `member`, a member of a group this device is in, sent on a connection
/// where it proved to be that member: this device's profile and request secret, signed and
/// sealed to it. No shared secret keys it: the receiver takes it only from the peer that
/// proved to be a member, and checks that the profile inside is that member's device.
pub fn member_request(
    identity: &DeviceIdentity,
    own_profile: &Profile,
    own_secret: &FriendSecret,
    member: &DevicePublicKey,
    local: &[u8],
    remote: &[u8],
) -> Result<Vec<u8>, FriendError> {
    let context = bound(MEMBER_REQUEST, local, remote, member);
    let body = introduction_body(identity, own_profile, own_secret, &context);
    Ok(encode_sealed(&Sealed::seal(member, &context, &body)?))
}

/// Opens a friend request from a member of a group this device is in. Whose it is (the
/// profile's device) is for the caller to compare with the member the peer proved to be.
pub fn open_member_request(
    identity: &DeviceIdentity,
    local: &[u8],
    remote: &[u8],
    sealed: &[u8],
) -> Result<Introduction, FriendError> {
    let context = bound(MEMBER_REQUEST, remote, local, &identity.public_key());
    read_introduction(&open_sealed(identity, &context, sealed)?, &context)
}

/// Accepts a friend request, or gives a friend a new friend secret: this device's profile
/// and friend secret, sealed to the friend, with an HMAC keyed by the secret the two share
/// (`pair`), which nobody else can check.
pub fn accept(
    identity: &DeviceIdentity,
    own_profile: &Profile,
    own_secret: &FriendSecret,
    friend: &DevicePublicKey,
    pair: &FriendSecret,
    local: &[u8],
    remote: &[u8],
) -> Result<Envelope, FriendError> {
    let context = bound(ACCEPT, local, remote, friend);
    let body = introduction_body(identity, own_profile, own_secret, &context);
    Ok(Envelope {
        mac: mac(&pair.hello_key(), &[ACCEPT, local, remote]),
        sealed: encode_sealed(&Sealed::seal(friend, &context, &body)?),
    })
}

/// Opens an acceptance from the device this one shares `pair` with.
pub fn open_accept(
    identity: &DeviceIdentity,
    pair: &FriendSecret,
    local: &[u8],
    remote: &[u8],
    mac: &[u8],
    sealed: &[u8],
) -> Result<Introduction, FriendError> {
    if !check(&pair.hello_key(), &[ACCEPT, remote, local], mac) {
        return Err(FriendError::NotProved);
    }
    let context = bound(ACCEPT, remote, local, &identity.public_key());
    read_introduction(&open_sealed(identity, &context, sealed)?, &context)
}

// ----- Short codes -----

/// First message of a code run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeStart {
    /// Proves knowledge of the stretched code, so that strangers cannot use up the inviting
    /// device's runs.
    pub gate: [u8; 32],
    pub spake: Vec<u8>,
}

/// The inviting device's answer: its SPAKE2 message and its profile, encrypted with the
/// run's key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeReply {
    pub spake: Vec<u8>,
    pub sealed: Vec<u8>,
}

/// The device where the code was typed, waiting for the inviting device's answer.
pub struct CodeRequester {
    spake: Spake2<Ed25519Group>,
    outbound: Vec<u8>,
    local: Vec<u8>,
    remote: Vec<u8>,
}

impl fmt::Debug for CodeRequester {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CodeRequester(..)")
    }
}

/// The inviting device, waiting for the requester's last message.
pub struct CodeInviter {
    finish_key: Zeroizing<[u8; 32]>,
    transcript: [u8; 32],
}

impl fmt::Debug for CodeInviter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CodeInviter(..)")
    }
}

fn transcript(
    requester_peer: &[u8],
    inviter_peer: &[u8],
    requester_msg: &[u8],
    inviter_msg: &[u8],
) -> [u8; 32] {
    let mut w = Writer::default();
    w.raw(CODE_TRANSCRIPT)
        .bytes(requester_peer)
        .bytes(inviter_peer)
        .bytes(requester_msg)
        .bytes(inviter_msg);
    Sha256::digest(w.into_bytes()).into()
}

/// Keys for the inviter's answer and for the requester's last message.
fn run_keys(key: &[u8], transcript: &[u8; 32]) -> (Zeroizing<[u8; 32]>, Zeroizing<[u8; 32]>) {
    let mut reply = b"reply".to_vec();
    reply.extend_from_slice(transcript);
    let mut finish = b"finish".to_vec();
    finish.extend_from_slice(transcript);
    (
        derive(CODE_RUN, key, &reply),
        derive(CODE_RUN, key, &finish),
    )
}

fn signed(label: &[u8], transcript: &[u8; 32]) -> Vec<u8> {
    [label, transcript].concat()
}

fn encrypt(key: &[u8; 32], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, FriendError> {
    let mut nonce = [0u8; NONCE_LEN];
    random_bytes(&mut nonce)?;
    let ciphertext = XChaCha20Poly1305::new(&(*key).into())
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| FriendError::Malformed)?;
    Ok([nonce.as_slice(), &ciphertext].concat())
}

fn decrypt(key: &[u8; 32], aad: &[u8], data: &[u8]) -> Result<Zeroizing<Vec<u8>>, FriendError> {
    let (nonce, ciphertext) = data
        .split_at_checked(NONCE_LEN)
        .ok_or(FriendError::Malformed)?;
    let nonce: [u8; NONCE_LEN] = nonce.try_into().expect("split at the nonce length");
    XChaCha20Poly1305::new(&(*key).into())
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| FriendError::WrongCode)
}

fn spake_password(secrets: &CodeSecrets) -> Password {
    Password::new(secrets.password().as_slice())
}

/// Starts a code run towards `remote`, a device that may have shown the code.
pub fn code_start(
    secrets: &CodeSecrets,
    local: &[u8],
    remote: &[u8],
) -> (CodeRequester, CodeStart) {
    let (spake, outbound) = Spake2::<Ed25519Group>::start_a(
        &spake_password(secrets),
        &Identity::new(SPAKE_REQUESTER),
        &Identity::new(SPAKE_INVITER),
    );
    let start = CodeStart {
        gate: mac(&secrets.gate_key(), &[CODE_GATE, local, remote]),
        spake: outbound.clone(),
    };
    let requester = CodeRequester {
        spake,
        outbound,
        local: local.to_vec(),
        remote: remote.to_vec(),
    };
    (requester, start)
}

/// Inviting device: whether a run started by `remote` knows the stretched code. Checked
/// before the run counts against the code.
pub fn code_gate_ok(secrets: &CodeSecrets, local: &[u8], remote: &[u8], gate: &[u8]) -> bool {
    check(&secrets.gate_key(), &[CODE_GATE, remote, local], gate)
}

/// Inviting device: answers a run started by `remote` with this device's profile, readable
/// only if the requester used the same code.
pub fn code_reply(
    identity: &DeviceIdentity,
    own_profile: &Profile,
    secrets: &CodeSecrets,
    local: &[u8],
    remote: &[u8],
    start: &CodeStart,
) -> Result<(CodeInviter, CodeReply), FriendError> {
    let (spake, outbound) = Spake2::<Ed25519Group>::start_b(
        &spake_password(secrets),
        &Identity::new(SPAKE_REQUESTER),
        &Identity::new(SPAKE_INVITER),
    );
    let key = Zeroizing::new(
        spake
            .finish(&start.spake)
            .map_err(|_| FriendError::Malformed)?,
    );
    let transcript = transcript(remote, local, &start.spake, &outbound);
    let (reply_key, finish_key) = run_keys(&key, &transcript);
    let mut w = Writer::default();
    w.bytes(&own_profile.encode())
        .raw(&identity.sign(&signed(CODE_REPLY, &transcript)));
    let sealed = encrypt(&reply_key, &transcript, &w.into_bytes())?;
    Ok((
        CodeInviter {
            finish_key,
            transcript,
        },
        CodeReply {
            spake: outbound,
            sealed,
        },
    ))
}

impl CodeRequester {
    /// Reads the inviting device's answer and makes the friend request that closes the run.
    /// Fails with [`FriendError::WrongCode`] if the two devices used different codes.
    pub fn finish(
        self,
        identity: &DeviceIdentity,
        own_profile: &Profile,
        own_secret: &FriendSecret,
        reply: &CodeReply,
    ) -> Result<(Profile, Vec<u8>), FriendError> {
        let key = Zeroizing::new(
            self.spake
                .finish(&reply.spake)
                .map_err(|_| FriendError::WrongCode)?,
        );
        let transcript = transcript(&self.local, &self.remote, &self.outbound, &reply.spake);
        let (reply_key, finish_key) = run_keys(&key, &transcript);
        let body = decrypt(&reply_key, &transcript, &reply.sealed)?;
        let mut r = Reader::new(&body);
        let inviter = Profile::decode(r.bytes()?)?;
        let signature: [u8; SIGNATURE_LEN] = r.array()?;
        r.finish()?;
        verify(
            &inviter.device,
            &signed(CODE_REPLY, &transcript),
            &signature,
        )?;
        let body = introduction_body(
            identity,
            own_profile,
            own_secret,
            &signed(CODE_FINISH, &transcript),
        );
        Ok((inviter, encrypt(&finish_key, &transcript, &body)?))
    }
}

impl CodeInviter {
    /// Opens the requester's last message: its friend request.
    pub fn finish(self, sealed: &[u8]) -> Result<Introduction, FriendError> {
        let body = decrypt(&self.finish_key, &self.transcript, sealed)?;
        read_introduction(&body, &signed(CODE_FINISH, &self.transcript))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Avatar, FriendCode};

    struct Device {
        identity: DeviceIdentity,
        profile: Profile,
        secret: FriendSecret,
        peer: Vec<u8>,
    }

    fn device(name: &str, peer: &[u8]) -> Device {
        let identity = DeviceIdentity::generate().unwrap();
        let profile = Profile::sign(&identity, name, Avatar::Preset(1), 5).unwrap();
        Device {
            identity,
            profile,
            secret: FriendSecret::generate().unwrap(),
            peer: peer.to_vec(),
        }
    }

    /// A friend request's body, which holds the friend secret, is written where it stays:
    /// a buffer that grew as it was written would have left a copy of it behind.
    #[test]
    fn a_requests_secret_is_written_without_copies_left_behind() {
        let ana = device("Ana", b"peer-a");
        let body = introduction_body(&ana.identity, &ana.profile, &ana.secret, b"signed");
        assert_eq!(body.capacity(), body.len());
        let intro = read_introduction(&body, b"signed").unwrap();
        assert_eq!(intro.secret.as_bytes(), ana.secret.as_bytes());
    }

    #[test]
    fn friends_recognise_each_other() {
        let a = device("Ana", b"peer-a");
        let b = device("Bea", b"peer-b");
        let a_key = a.identity.public_key();
        let b_key = b.identity.public_key();

        let pair = FriendSecret::pair(&b.identity, &a_key).unwrap();
        let env = hello(
            &b.identity,
            &a_key,
            &pair,
            &a.secret,
            &b.peer,
            &a.peer,
            4,
            true,
        )
        .unwrap();
        let earlier = FriendSecret::generate().unwrap();
        let from = open_hello(
            &a.identity,
            &pair,
            &[&a.secret, &earlier],
            &a.peer,
            &b.peer,
            &env.mac,
            &env.sealed,
        )
        .unwrap();
        assert_eq!(
            from,
            HelloFrom {
                device: b_key,
                knows: Some(0),
                knows_version: 4,
                hidden: true,
            }
        );
        // A friend that has only an earlier secret of this device's says so.
        let env = hello(
            &b.identity,
            &a_key,
            &pair,
            &earlier,
            &b.peer,
            &a.peer,
            4,
            false,
        )
        .unwrap();
        let from = open_hello(
            &a.identity,
            &pair,
            &[&a.secret, &earlier],
            &a.peer,
            &b.peer,
            &env.mac,
            &env.sealed,
        )
        .unwrap();
        assert_eq!(from.knows, Some(1));
        let reply = hello_reply(
            &a.identity,
            &b_key,
            &a.peer,
            &b.peer,
            5,
            Some(&a.profile),
            false,
        )
        .unwrap();
        let reply = open_hello_reply(&b.identity, &a_key, &b.peer, &a.peer, &reply).unwrap();
        assert_eq!(reply.knows_version, 5);
        assert_eq!(reply.profile, Some(a.profile.clone()));
        assert!(!reply.hidden);

        let update = profile_update(&a_key, &b.profile, &b.peer, &a.peer).unwrap();
        assert_eq!(
            open_profile_update(&a.identity, &b_key, &a.peer, &b.peer, &update).unwrap(),
            b.profile
        );

        let busy = Status {
            in_room: true,
            group: Some([7; 16]),
            hidden: false,
            away: false,
        };
        let sealed = status(&a_key, &busy, &b.peer, &a.peer).unwrap();
        assert_eq!(
            open_status(&a.identity, &a.peer, &b.peer, &sealed).unwrap(),
            busy
        );
        assert!(open_status(&a.identity, &a.peer, b"peer-x", &sealed).is_err());

        let away = Status {
            in_room: false,
            group: None,
            hidden: false,
            away: true,
        };
        let sealed = status(&a_key, &away, &b.peer, &a.peer).unwrap();
        assert_eq!(
            open_status(&a.identity, &a.peer, &b.peer, &sealed).unwrap(),
            away
        );
    }

    #[test]
    fn banners_bios_leaving_and_friends_in_common_reach_only_the_friend() {
        let a = device("Ana", b"peer-a");
        let b = device("Bea", b"peer-b");
        let a_key = a.identity.public_key();
        let b_key = b.identity.public_key();

        let banner = Banner::sign(&b.identity, 0xFF3D21AD, None, 9).unwrap();
        let sealed = banner_update(&a_key, &banner, &b.peer, &a.peer).unwrap();
        assert_eq!(
            open_banner_update(&a.identity, &b_key, &a.peer, &b.peer, &sealed).unwrap(),
            banner
        );
        // Only as the friend's own banner, and only on its connection.
        assert_eq!(
            open_banner_update(&a.identity, &a_key, &a.peer, &b.peer, &sealed),
            Err(FriendError::NotProved)
        );
        assert!(open_banner_update(&a.identity, &b_key, &a.peer, b"peer-x", &sealed).is_err());

        let bio = Bio::sign(&b.identity, "Hey there!", 9).unwrap();
        let sealed = bio_update(&a_key, &bio, &b.peer, &a.peer).unwrap();
        assert_eq!(
            open_bio_update(&a.identity, &b_key, &a.peer, &b.peer, &sealed).unwrap(),
            bio
        );
        assert_eq!(
            open_bio_update(&a.identity, &a_key, &a.peer, &b.peer, &sealed),
            Err(FriendError::NotProved)
        );
        assert!(open_bio_update(&a.identity, &b_key, &a.peer, b"peer-x", &sealed).is_err());
        // Not a banner, nor a banner a bio.
        assert!(open_banner_update(&a.identity, &b_key, &a.peer, &b.peer, &sealed).is_err());

        let sealed = unfriend(&b.identity, &a_key, &b.peer, &a.peer).unwrap();
        assert_eq!(
            open_unfriend(&a.identity, &a.peer, &b.peer, &sealed).unwrap(),
            b_key
        );
        assert!(open_unfriend(&a.identity, &a.peer, b"peer-x", &sealed).is_err());
        // Only the former friend acknowledges it, and only that removal on that connection.
        let removal = removal_digest(&sealed);
        let ack = unfriend_ack(&a.identity, &b_key, &a.peer, &b.peer, &removal).unwrap();
        assert!(open_unfriend_ack(&b.identity, &a_key, &b.peer, &a.peer, &removal, &ack).is_ok());
        let other = DeviceIdentity::generate().unwrap();
        let forged = unfriend_ack(&other, &b_key, &a.peer, &b.peer, &removal).unwrap();
        assert_eq!(
            open_unfriend_ack(&b.identity, &a_key, &b.peer, &a.peer, &removal, &forged),
            Err(FriendError::NotProved)
        );
        assert!(
            open_unfriend_ack(&b.identity, &a_key, &b.peer, b"peer-x", &removal, &ack).is_err()
        );
        let another = removal_digest(b"another");
        assert!(open_unfriend_ack(&b.identity, &a_key, &b.peer, &a.peer, &another, &ack).is_err());

        let shared = DeviceIdentity::generate().unwrap().public_key();
        let (ask, points) = crate::MutualAsk::start(&[shared, b_key]).unwrap();
        let sealed = mutual_ask(&b_key, &points, &a.peer, &b.peer).unwrap();
        let asked = open_mutual_ask(&b.identity, &b.peer, &a.peer, &sealed).unwrap();
        assert_eq!(asked, points);
        let answer = crate::mutual_answer(&asked, &[shared, a_key]).unwrap();
        let sealed = mutual_reply(&a_key, &answer, &b.peer, &a.peer).unwrap();
        let answer = open_mutual_reply(&a.identity, &a.peer, &b.peer, &sealed).unwrap();
        assert_eq!(ask.finish(&answer).unwrap(), vec![shared]);
    }

    #[test]
    fn call_steps_reach_only_the_friend() {
        let a = device("Ana", b"peer-a");
        let b = device("Bea", b"peer-b");
        let a_key = a.identity.public_key();
        let ring = CallMessage {
            call: [9; 16],
            signal: CallSignal::Ring {
                log: vec![vec![1, 2, 3], vec![4; 300]],
            },
        };
        let sealed = call_message(&a_key, &ring, &b.peer, &a.peer).unwrap();
        assert_eq!(
            open_call_message(&a.identity, &a.peer, &b.peer, &sealed).unwrap(),
            ring
        );
        // Not on another connection, and not for another device.
        assert!(open_call_message(&a.identity, &a.peer, b"peer-x", &sealed).is_err());
        assert!(open_call_message(&b.identity, &a.peer, &b.peer, &sealed).is_err());
        for signal in [
            CallSignal::Accept,
            CallSignal::Decline,
            CallSignal::Busy,
            CallSignal::Cancel,
            CallSignal::End,
            CallSignal::Elsewhere,
            CallSignal::GroupCalls,
            CallSignal::Going {
                age_ms: 61_000,
                together_ms: u64::MAX,
            },
            CallSignal::Invite {
                log: vec![vec![7; 40]],
            },
        ] {
            let message = CallMessage {
                call: [1; 16],
                signal,
            };
            let sealed = call_message(&a_key, &message, &b.peer, &a.peer).unwrap();
            assert_eq!(
                open_call_message(&a.identity, &a.peer, &b.peer, &sealed).unwrap(),
                message
            );
        }
    }

    #[test]
    fn a_status_from_an_older_version_reads_as_visible() {
        let a = device("Ana", b"peer-a");
        let b = device("Bea", b"peer-b");
        let a_key = a.identity.public_key();
        let context = bound(STATUS, &b.peer, &a.peer, &a_key);
        let mut w = Writer::default();
        w.u8(1).u8(0);
        let sealed = encode_sealed(&Sealed::seal(&a_key, &context, &w.into_bytes()).unwrap());
        assert_eq!(
            open_status(&a.identity, &a.peer, &b.peer, &sealed).unwrap(),
            Status {
                in_room: true,
                group: None,
                hidden: false,
                away: false,
            }
        );
    }

    #[test]
    fn a_status_that_is_not_away_reads_as_before() {
        // Versions before `away` stop after `hidden`: a status that is not away is all
        // they read, byte for byte.
        let a = device("Ana", b"peer-a");
        let b = device("Bea", b"peer-b");
        let a_key = a.identity.public_key();
        let context = bound(STATUS, &b.peer, &a.peer, &a_key);
        let here = Status {
            in_room: false,
            group: None,
            hidden: false,
            away: false,
        };
        let sealed = status(&a_key, &here, &b.peer, &a.peer).unwrap();
        let body = open_sealed(&a.identity, &context, &sealed).unwrap();
        assert_eq!(&body[..], &[0, 0, 0]);
        let away = Status { away: true, ..here };
        let sealed = status(&a_key, &away, &b.peer, &a.peer).unwrap();
        let body = open_sealed(&a.identity, &context, &sealed).unwrap();
        assert_eq!(&body[..], &[0, 0, 0, 1]);
    }

    #[test]
    fn hellos_cannot_be_moved_or_forged() {
        let a = device("Ana", b"peer-a");
        let b = device("Bea", b"peer-b");
        let a_key = a.identity.public_key();
        let pair = FriendSecret::pair(&b.identity, &a_key).unwrap();
        let env = hello(
            &b.identity,
            &a_key,
            &pair,
            &a.secret,
            &b.peer,
            &a.peer,
            0,
            false,
        )
        .unwrap();
        // Another connection.
        assert_eq!(
            open_hello(
                &a.identity,
                &pair,
                &[&a.secret],
                &a.peer,
                b"peer-x",
                &env.mac,
                &env.sealed
            ),
            Err(FriendError::NotProved)
        );
        // Someone who knows Ana's friend secret (another friend of hers) but not the secret
        // Ana shares with Bea: its hello is refused, and Bea's tells it nothing.
        let c = device("Cid", b"peer-c");
        let cid_pair = FriendSecret::pair(&c.identity, &a_key).unwrap();
        assert_ne!(cid_pair.as_bytes(), pair.as_bytes());
        assert!(!check(
            &a.secret.hello_key(),
            &[HELLO, &b.peer, &a.peer],
            &env.mac
        ));
        let env = hello(
            &c.identity,
            &a_key,
            &cid_pair,
            &a.secret,
            &b.peer,
            &a.peer,
            0,
            false,
        )
        .unwrap();
        assert_eq!(
            open_hello(
                &a.identity,
                &pair,
                &[&a.secret],
                &a.peer,
                &b.peer,
                &env.mac,
                &env.sealed
            ),
            Err(FriendError::NotProved)
        );
        // A reply signed by someone else.
        let reply = hello_reply(
            &c.identity,
            &b.identity.public_key(),
            &a.peer,
            &b.peer,
            0,
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            open_hello_reply(&b.identity, &a_key, &b.peer, &a.peer, &reply),
            Err(FriendError::NotProved)
        );
    }

    #[test]
    fn member_requests_open_only_on_their_connection() {
        let a = device("Ana", b"peer-a");
        let b = device("Bea", b"peer-b");
        let a_key = a.identity.public_key();
        let sealed =
            member_request(&b.identity, &b.profile, &b.secret, &a_key, &b.peer, &a.peer).unwrap();
        let intro = open_member_request(&a.identity, &a.peer, &b.peer, &sealed).unwrap();
        assert_eq!(intro.profile, b.profile);
        assert_eq!(intro.secret.as_bytes(), b.secret.as_bytes());
        // Replayed on another connection, or to another device, it does not open.
        assert!(open_member_request(&a.identity, &a.peer, b"peer-c", &sealed).is_err());
        let c = device("Cid", b"peer-a");
        assert!(open_member_request(&c.identity, &c.peer, &b.peer, &sealed).is_err());
        // Nor is it a link request.
        let link = FriendLink::new(a_key, "Ana", vec![], vec![], 0).unwrap();
        assert!(
            open_link_request(&a.identity, &link, &a.peer, &b.peer, &[0; 32], &sealed).is_err()
        );
    }

    #[test]
    fn link_requests_and_acceptances() {
        let a = device("Ana", b"peer-a");
        let b = device("Bea", b"peer-b");
        let link = FriendLink::new(a.identity.public_key(), "Ana", vec![], vec![], 0).unwrap();
        let env =
            link_request(&b.identity, &b.profile, &b.secret, &link, &b.peer, &a.peer).unwrap();
        let intro =
            open_link_request(&a.identity, &link, &a.peer, &b.peer, &env.mac, &env.sealed).unwrap();
        assert_eq!(intro.profile, b.profile);
        assert_eq!(intro.secret.as_bytes(), b.secret.as_bytes());

        // A request made with another link is refused.
        let other = FriendLink::new(a.identity.public_key(), "Ana", vec![], vec![], 0).unwrap();
        assert_eq!(
            open_link_request(&a.identity, &other, &a.peer, &b.peer, &env.mac, &env.sealed)
                .unwrap_err(),
            FriendError::NotProved
        );

        let pair = FriendSecret::pair(&a.identity, &b.identity.public_key()).unwrap();
        let env = accept(
            &a.identity,
            &a.profile,
            &a.secret,
            &b.identity.public_key(),
            &pair,
            &a.peer,
            &b.peer,
        )
        .unwrap();
        let back = open_accept(
            &b.identity,
            &FriendSecret::pair(&b.identity, &a.identity.public_key()).unwrap(),
            &b.peer,
            &a.peer,
            &env.mac,
            &env.sealed,
        )
        .unwrap();
        // Not with the secret of the request, which the acceptance no longer uses.
        assert!(
            open_accept(
                &b.identity,
                &intro.secret,
                &b.peer,
                &a.peer,
                &env.mac,
                &env.sealed
            )
            .is_err()
        );
        assert_eq!(back.profile, a.profile);
        assert_eq!(back.secret.as_bytes(), a.secret.as_bytes());
    }

    #[test]
    fn code_runs_need_the_same_code() {
        let a = device("Ana", b"peer-a");
        let b = device("Bea", b"peer-b");
        let now = 1_793_106_000;
        let code = FriendCode::generate(now).unwrap();
        let secrets = CodeSecrets::stretch(&code, now);

        let (requester, start) = code_start(&secrets, &b.peer, &a.peer);
        assert!(code_gate_ok(&secrets, &a.peer, &b.peer, &start.gate));
        assert!(!code_gate_ok(&secrets, &a.peer, b"peer-x", &start.gate));
        let (inviter, reply) =
            code_reply(&a.identity, &a.profile, &secrets, &a.peer, &b.peer, &start).unwrap();
        let (found, finish) = requester
            .finish(&b.identity, &b.profile, &b.secret, &reply)
            .unwrap();
        assert_eq!(found, a.profile);
        let intro = inviter.finish(&finish).unwrap();
        assert_eq!(intro.profile, b.profile);
        assert_eq!(intro.secret.as_bytes(), b.secret.as_bytes());

        // A requester who typed another code learns nothing and cannot finish. (The gate
        // would already stop it; run the exchange anyway to check SPAKE2 itself.)
        let wrong = CodeSecrets::stretch(&FriendCode::generate(now).unwrap(), now);
        let (requester, start) = code_start(&wrong, &b.peer, &a.peer);
        let (inviter, reply) =
            code_reply(&a.identity, &a.profile, &secrets, &a.peer, &b.peer, &start).unwrap();
        assert_eq!(
            requester
                .finish(&b.identity, &b.profile, &b.secret, &reply)
                .unwrap_err(),
            FriendError::WrongCode
        );
        let forged = encrypt(&[9; 32], &[0; 32], b"x").unwrap();
        assert_eq!(inviter.finish(&forged).unwrap_err(), FriendError::WrongCode);
    }
}
