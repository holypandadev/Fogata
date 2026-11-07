//! The friend secret, and the keyed functions built on it.

use std::fmt;

use fogata_crypto::{CryptoError, DeviceIdentity, DevicePublicKey, random_bytes};
use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::FriendError;

const SALT: &[u8] = b"fogata/v1/friend-secret";
const PROBE_LABEL: &[u8] = b"fogata/v1/friend-probe";
const PAIR_LABEL: &[u8] = b"fogata/v1/friend-pair";

/// A device's friend secret: random, and known only to the device and its friends. The
/// device announces itself on the DHT under rendezvous points derived from it, and friends
/// prove they know it before the device says who it is.
#[derive(Clone)]
pub struct FriendSecret(Zeroizing<[u8; 32]>);

impl FriendSecret {
    pub fn generate() -> Result<Self, CryptoError> {
        let mut bytes = Zeroizing::new([0u8; 32]);
        random_bytes(bytes.as_mut())?;
        Ok(Self(bytes))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FriendError> {
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| FriendError::Malformed)?;
        Ok(Self(Zeroizing::new(bytes)))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The secret this device and `friend` share with nobody else, worked out from their two
    /// device keys ([`DeviceIdentity::pair_secret`]): the same on both sides, and unknown to
    /// their other friends, who know each one's own friend secret. What concerns the two of
    /// them alone (where to find each other to deliver a new secret or a removal, a greeting
    /// to a friend whose secret changed) goes under it.
    pub fn pair(identity: &DeviceIdentity, friend: &DevicePublicKey) -> Result<Self, FriendError> {
        Ok(Self(identity.pair_secret(friend, PAIR_LABEL)?))
    }

    /// The secret behind the rendezvous points this device announces itself under.
    pub fn rendezvous(&self) -> [u8; 32] {
        *derive(SALT, self.as_bytes(), b"rendezvous")
    }

    pub(crate) fn hello_key(&self) -> Zeroizing<[u8; 32]> {
        derive(SALT, self.as_bytes(), b"hello")
    }

    fn probe_key(&self) -> Zeroizing<[u8; 32]> {
        derive(SALT, self.as_bytes(), b"probe")
    }

    /// Answer to a probe from `asker_peer` on the local network. It looks random to anyone
    /// but this device's friends.
    pub fn probe_tag(&self, nonce: &[u8], local_peer: &[u8], asker_peer: &[u8]) -> [u8; 32] {
        mac(
            &self.probe_key(),
            &[PROBE_LABEL, nonce, local_peer, asker_peer],
        )
    }

    /// Whether `tag`, received from `friend_peer`, comes from the device whose friend secret
    /// this is.
    pub fn probe_matches(
        &self,
        nonce: &[u8],
        friend_peer: &[u8],
        local_peer: &[u8],
        tag: &[u8],
    ) -> bool {
        check(
            &self.probe_key(),
            &[PROBE_LABEL, nonce, friend_peer, local_peer],
            tag,
        )
    }
}

impl fmt::Debug for FriendSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FriendSecret(..)")
    }
}

pub(crate) fn derive(salt: &[u8], ikm: &[u8], info: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut out = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(Some(salt), ikm)
        .expand(info, out.as_mut())
        .expect("HKDF output length is far below the limit");
    out
}

fn hmac(key: &[u8; 32], parts: &[&[u8]]) -> Hmac<Sha256> {
    let mut mac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC accepts keys of any length");
    for part in parts {
        mac.update(&(part.len() as u32).to_be_bytes());
        mac.update(part);
    }
    mac
}

/// HMAC-SHA256 over length-prefixed parts.
pub(crate) fn mac(key: &[u8; 32], parts: &[&[u8]]) -> [u8; 32] {
    hmac(key, parts).finalize().into_bytes().into()
}

/// Constant-time check of a [`mac`].
pub(crate) fn check(key: &[u8; 32], parts: &[&[u8]], tag: &[u8]) -> bool {
    hmac(key, parts).verify_slice(tag).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_tags_identify_only_to_friends() {
        let secret = FriendSecret::generate().unwrap();
        let other = FriendSecret::generate().unwrap();
        let tag = secret.probe_tag(b"nonce", b"friend", b"asker");
        assert!(secret.probe_matches(b"nonce", b"friend", b"asker", &tag));
        assert!(!other.probe_matches(b"nonce", b"friend", b"asker", &tag));
        assert!(!secret.probe_matches(b"other", b"friend", b"asker", &tag));
        assert!(!secret.probe_matches(b"nonce", b"asker", b"friend", &tag));
    }

    #[test]
    fn a_pair_secret_is_the_two_friends_own() {
        let ana = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let cid = DeviceIdentity::generate().unwrap();
        let ab = FriendSecret::pair(&ana, &bea.public_key()).unwrap();
        let ba = FriendSecret::pair(&bea, &ana.public_key()).unwrap();
        assert_eq!(ab.rendezvous(), ba.rendezvous());
        assert_ne!(
            ab.rendezvous(),
            FriendSecret::pair(&cid, &bea.public_key())
                .unwrap()
                .rendezvous()
        );
    }

    #[test]
    fn derived_keys_differ() {
        let secret = FriendSecret::generate().unwrap();
        assert_ne!(secret.rendezvous(), *secret.hello_key());
        assert_ne!(*secret.hello_key(), *secret.probe_key());
        assert_eq!(format!("{secret:?}"), "FriendSecret(..)");
    }
}
