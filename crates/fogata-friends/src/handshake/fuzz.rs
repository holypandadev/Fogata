//! Entry points for fuzzing what a friend seals to this device: any body, sealed as a friend
//! would seal it, must open without a panic and in little time. Built only with the
//! `fuzzing` feature; the runs live in fogata-core.

use fogata_crypto::{DeviceIdentity, Sealed, codec::Writer};

use super::*;

/// A GIF of one pixel.
const ONE_PIXEL_GIF: &[u8] =
    b"GIF89a\x01\x00\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";

/// The transport ids of the two ends of the runs' connection.
const LOCAL: &[u8] = b"this device";
const REMOTE: &[u8] = b"its friend";

/// This device (0) and its friend (1), the same in every run.
fn device(n: u8) -> DeviceIdentity {
    DeviceIdentity::from_secret_bytes(&[n.wrapping_add(1); 32])
}

/// This device's friend secret.
fn own_secret() -> FriendSecret {
    FriendSecret::from_bytes(&[7; 32]).expect("32 bytes")
}

/// What each kind of message is bound to.
const LABELS: [&[u8]; 13] = [
    HELLO,
    HELLO_REPLY,
    PROFILE_UPDATE,
    STATUS,
    BANNER,
    BIO,
    ANIMATION,
    UNFRIEND,
    MUTUAL_ASK,
    MUTUAL_ANSWER,
    CALL,
    ACCEPT,
    UNFRIEND_ACK,
];

/// What the friend signs for a message of `label` to this device.
fn context(label: &[u8]) -> Vec<u8> {
    if label == UNFRIEND_ACK {
        return unfriend_ack_context(REMOTE, LOCAL, &device(0).public_key(), REMOVAL);
    }
    bound(label, REMOTE, LOCAL, &device(0).public_key())
}

/// The digest of the removal this device sent, which the friend's acknowledgment names.
const REMOVAL: &[u8; 32] = &[9; 32];

/// A message from this device's friend: its kind (the first byte, among [`LABELS`]), then
/// its body, which is sealed to this device as the friend would and opened.
pub fn friend_sealed(data: &[u8]) {
    let Some((&kind, body)) = data.split_first() else {
        return;
    };
    let kind = usize::from(kind) % LABELS.len();
    let label = LABELS[kind];
    let me = device(0);
    let friend = device(1).public_key();
    let Ok(sealed) = Sealed::seal(&me.public_key(), &context(label), body) else {
        return;
    };
    let sealed = encode_sealed(&sealed);
    let secret = own_secret();
    let tag = mac(&secret.hello_key(), &[label, REMOTE, LOCAL]);
    let _ = match kind {
        0 => open_hello(&me, &secret, &[&secret], LOCAL, REMOTE, &tag, &sealed).map(drop),
        1 => open_hello_reply(&me, &friend, LOCAL, REMOTE, &sealed).map(drop),
        2 => open_profile_update(&me, &friend, LOCAL, REMOTE, &sealed).map(drop),
        3 => open_status(&me, LOCAL, REMOTE, &sealed).map(drop),
        4 => open_banner_update(&me, &friend, LOCAL, REMOTE, &sealed).map(drop),
        5 => open_bio_update(&me, &friend, LOCAL, REMOTE, &sealed).map(drop),
        6 => open_animation_update(&me, &friend, LOCAL, REMOTE, &sealed).map(drop),
        7 => open_unfriend(&me, LOCAL, REMOTE, &sealed).map(drop),
        8 => open_mutual_ask(&me, LOCAL, REMOTE, &sealed).map(|points| {
            assert!(points.len() <= MAX_MUTUAL_POINTS);
        }),
        9 => open_mutual_reply(&me, LOCAL, REMOTE, &sealed).map(drop),
        10 => open_call_message(&me, LOCAL, REMOTE, &sealed).map(drop),
        11 => open_accept(&me, &secret, LOCAL, REMOTE, &tag, &sealed).map(drop),
        _ => open_unfriend_ack(&me, &friend, LOCAL, REMOTE, REMOVAL, &sealed),
    };
}

pub fn friend_sealed_seeds() -> Vec<Vec<u8>> {
    let friend = device(1);
    let signed = |label: &[u8]| friend.sign(&context(label));
    let profile = Profile::sign(
        &friend,
        "Bea",
        crate::profile::Avatar::Photo(b"\x89PNG\r\n\x1a\n0000".to_vec()),
        5,
    )
    .expect("profile");
    let points = |n: u8| {
        let mut w = Writer::default();
        w.u32(u32::from(n));
        for i in 0..n {
            w.raw(&[i; 32]);
        }
        w.into_bytes()
    };
    let mut bodies: Vec<Vec<u8>> = Vec::new();
    let mut w = Writer::default();
    w.key(&friend.public_key())
        .raw(&signed(HELLO))
        .u64(4)
        .u8(1)
        .raw(&[3; 32]);
    bodies.push(w.into_bytes());
    let mut w = Writer::default();
    w.raw(&signed(HELLO_REPLY))
        .u64(4)
        .u8(1)
        .bytes(&profile.encode())
        .u8(0);
    bodies.push(w.into_bytes());
    bodies.push(profile.encode());
    bodies.push(vec![
        1, 1, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 0,
    ]);
    bodies.push(
        Banner::sign(&friend, 0xFF3D21AD, Some(b"\x89PNG\r\n\x1a\n".to_vec()), 6)
            .expect("banner")
            .encode(),
    );
    bodies.push(Bio::sign(&friend, "hi", 7).expect("bio").encode());
    bodies.push(
        Animation::sign(
            &friend,
            crate::Animated::Photo,
            b"\x89PNG\r\n\x1a\n0000",
            ONE_PIXEL_GIF.to_vec(),
            8,
        )
        .expect("animation")
        .encode(),
    );
    let mut w = Writer::default();
    w.key(&friend.public_key()).raw(&signed(UNFRIEND));
    bodies.push(w.into_bytes());
    bodies.push(points(3));
    bodies.push([points(2), points(1)].concat());
    let mut w = Writer::default();
    w.u8(1)
        .raw(&[5; 16])
        .u32(2)
        .bytes(&[6; 120])
        .bytes(&[7; 90]);
    bodies.push(w.into_bytes());
    let mut w = Writer::default();
    w.bytes(&profile.encode())
        .raw(&[8; 32])
        .raw(&signed(ACCEPT));
    bodies.push(w.into_bytes());
    bodies.push(friend.sign(&context(UNFRIEND_ACK)).to_vec());
    bodies
        .into_iter()
        .enumerate()
        .map(|(kind, body)| [&[kind as u8][..], &body].concat())
        .chain([vec![10, 2, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5]])
        .collect()
}
