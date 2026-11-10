//! Frozen bytes of the signed records a device hands its friends and the members of its
//! groups: its profile, banner, bio and animations (`tests/data/records.txt`). See
//! `fogata-proto/tests/frozen/mod.rs` for what a change of them means and how to write them
//! anew on purpose.
//!
//! They are signed with a fixed key, and ed25519 signs the same bytes alike, so the
//! signatures are frozen too; decoding checks them. What friends seal to each other (hellos,
//! requests, acceptances, statuses, calls) is sealed with fresh random nonces and keys and
//! is not frozen here; the book a device keeps is (`book-v5.bin`).

#[path = "../../fogata-proto/tests/frozen/mod.rs"]
mod frozen;

use fogata_crypto::DeviceIdentity;
use fogata_friends::{Animated, Animation, Avatar, Banner, Bio, Profile};
use frozen::{case, frozen};

const TIME_MS: u64 = 1_700_000_000_000;

#[test]
fn every_signed_record_is_written_and_read_as_frozen() {
    let identity = DeviceIdentity::from_secret_bytes(&[2; 32]);
    let png = b"\x89PNG\r\n\x1a\n tiny".to_vec();
    let mut cases = Vec::new();
    for (name, avatar) in [
        ("profile-preset", Avatar::Preset(3)),
        ("profile-photo", Avatar::Photo(png.clone())),
    ] {
        let profile = Profile::sign(&identity, "Ana \u{1f525}", avatar, TIME_MS).unwrap();
        cases.push(case(name.into(), &profile, Profile::encode, |b| {
            Profile::decode(b).ok()
        }));
    }
    for (name, picture) in [("banner", Some(png.clone())), ("banner-color", None)] {
        let banner = Banner::sign(&identity, 0x00ff_8800, picture, TIME_MS).unwrap();
        cases.push(case(name.into(), &banner, Banner::encode, |b| {
            Banner::decode(b).ok()
        }));
    }
    let bio = Bio::sign(&identity, "Hola.\nSounds at night.", TIME_MS).unwrap();
    cases.push(case("bio".into(), &bio, Bio::encode, |b| {
        Bio::decode(b).ok()
    }));
    // An animation taken away: one that moves needs a whole GIF or WebP, and its encoding
    // holds the picture as it holds any bytes.
    for (name, of) in [
        ("animation-photo-none", Animated::Photo),
        ("animation-banner-none", Animated::Banner),
    ] {
        let animation = Animation::sign(&identity, of, &png, Vec::new(), TIME_MS).unwrap();
        cases.push(case(name.into(), &animation, Animation::encode, |b| {
            Animation::decode(b).ok()
        }));
    }
    frozen("records.txt", &cases);
}
