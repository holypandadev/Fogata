use std::fmt;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use zeroize::Zeroizing;

use crate::{CryptoError, random_bytes};

/// Length of an Ed25519 signature in bytes.
pub const SIGNATURE_LEN: usize = 64;

/// The long-term key pair of a device. It signs everything the device says and is never
/// used as a network identity, so the network cannot follow a device by it.
pub struct DeviceIdentity {
    signing: SigningKey,
}

impl DeviceIdentity {
    /// Creates a new random identity.
    pub fn generate() -> Result<Self, CryptoError> {
        let mut secret = Zeroizing::new([0u8; 32]);
        random_bytes(secret.as_mut())?;
        Ok(Self::from_secret_bytes(&secret))
    }

    /// Restores an identity from its 32-byte secret.
    pub fn from_secret_bytes(secret: &[u8; 32]) -> Self {
        Self {
            signing: SigningKey::from_bytes(secret),
        }
    }

    /// The 32-byte secret, for storage.
    pub fn secret_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.signing.to_bytes())
    }

    pub fn public_key(&self) -> DevicePublicKey {
        DevicePublicKey(self.signing.verifying_key().to_bytes())
    }

    pub fn sign(&self, message: &[u8]) -> [u8; SIGNATURE_LEN] {
        self.signing.sign(message).to_bytes()
    }

    /// A secret that only this device and `other` can work out, the same on both: X25519
    /// between the two device keys, through HKDF with `label` (what it is for) and both keys
    /// in order. For what concerns the two of them alone, which nobody else who knows
    /// either one's other secrets may see.
    pub fn pair_secret(
        &self,
        other: &DevicePublicKey,
        label: &[u8],
    ) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
        let shared = self.x25519_secret().diffie_hellman(&other.x25519_public()?);
        if !shared.was_contributory() {
            return Err(CryptoError::InvalidKey);
        }
        let own = self.public_key();
        let (low, high) = if own <= *other {
            (own, *other)
        } else {
            (*other, own)
        };
        let mut info = label.to_vec();
        info.extend_from_slice(low.as_bytes());
        info.extend_from_slice(high.as_bytes());
        Ok(Zeroizing::new(crate::derive_secret(
            b"fogata/v1/pair",
            shared.as_bytes(),
            &info,
        )))
    }

    /// X25519 secret for opening data sealed to this device (the Ed25519 key's scalar).
    pub(crate) fn x25519_secret(&self) -> x25519_dalek::StaticSecret {
        x25519_dalek::StaticSecret::from(self.signing.to_scalar_bytes())
    }
}

impl fmt::Debug for DeviceIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceIdentity")
            .field("public_key", &self.public_key())
            .finish_non_exhaustive()
    }
}

/// The public half of a [`DeviceIdentity`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DevicePublicKey([u8; 32]);

impl DevicePublicKey {
    /// Parses a public key, rejecting byte strings that are not valid curve points.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| CryptoError::InvalidKey)?;
        VerifyingKey::from_bytes(&bytes).map_err(|_| CryptoError::InvalidKey)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Whether the key is a point of small order: no signature verifies under it
    /// ([`Self::verify`]) and nothing can be sealed to it, so no device has it, though an
    /// entry can still name it (a group's `Add`, say).
    pub fn is_weak(&self) -> bool {
        VerifyingKey::from_bytes(&self.0).map_or(true, |key| key.is_weak())
    }

    /// Verifies an Ed25519 signature, rejecting weak keys and malleable signatures.
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), CryptoError> {
        let key = VerifyingKey::from_bytes(&self.0).map_err(|_| CryptoError::InvalidKey)?;
        let signature = Signature::from_slice(signature).map_err(|_| CryptoError::BadSignature)?;
        key.verify_strict(message, &signature)
            .map_err(|_| CryptoError::BadSignature)
    }

    /// X25519 public key matching the device's Ed25519 key, for sealing data to it.
    pub(crate) fn x25519_public(&self) -> Result<x25519_dalek::PublicKey, CryptoError> {
        let key = VerifyingKey::from_bytes(&self.0).map_err(|_| CryptoError::InvalidKey)?;
        Ok(x25519_dalek::PublicKey::from(
            key.to_montgomery().to_bytes(),
        ))
    }

    /// Human-comparable form of the key: the first 80 bits of its SHA-256 in Crockford
    /// base32, 16 characters (`7Q4MK2XR9PLD3VHC`).
    pub fn fingerprint(&self) -> String {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(self.0);
        crockford(&digest[..10])
    }

    /// The fingerprint in groups of four, as shown to people: `7Q4M K2XR 9PLD 3VHC`.
    pub fn fingerprint_grouped(&self) -> String {
        let f = self.fingerprint();
        [&f[0..4], &f[4..8], &f[8..12], &f[12..16]].join(" ")
    }

    /// The first eight characters, for lists: `7Q4M·K2XR`.
    pub fn short_fingerprint(&self) -> String {
        let f = self.fingerprint();
        format!("{}·{}", &f[0..4], &f[4..8])
    }
}

impl fmt::Debug for DevicePublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DevicePublicKey({})", self.fingerprint())
    }
}

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Crockford base32 of `bytes` (whose bit length must be a multiple of 5).
pub fn crockford_encode(bytes: &[u8]) -> String {
    crockford(bytes)
}

pub(crate) fn crockford(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 8 / 5);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for byte in bytes {
        acc = (acc << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(CROCKFORD[((acc >> bits) & 31) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_verify_and_reject_tampering() {
        let identity = DeviceIdentity::generate().unwrap();
        let public = identity.public_key();
        let signature = identity.sign(b"hello");
        assert_eq!(public.verify(b"hello", &signature), Ok(()));
        assert_eq!(
            public.verify(b"hellO", &signature),
            Err(CryptoError::BadSignature)
        );
    }

    #[test]
    fn a_pair_secret_is_the_same_on_both_sides_only() {
        let ana = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let cid = DeviceIdentity::generate().unwrap();
        let ab = ana.pair_secret(&bea.public_key(), b"x").unwrap();
        assert_eq!(ab, bea.pair_secret(&ana.public_key(), b"x").unwrap());
        assert_ne!(ab, ana.pair_secret(&cid.public_key(), b"x").unwrap());
        assert_ne!(ab, cid.pair_secret(&bea.public_key(), b"x").unwrap());
        assert_ne!(ab, ana.pair_secret(&bea.public_key(), b"y").unwrap());
        // A small-order key agrees nothing.
        let small = DevicePublicKey::from_bytes(&[
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ])
        .unwrap();
        assert_eq!(ana.pair_secret(&small, b"x"), Err(CryptoError::InvalidKey));
        assert!(small.is_weak());
        assert!(!ana.public_key().is_weak());
    }

    #[test]
    fn identity_round_trips_through_its_secret() {
        let identity = DeviceIdentity::generate().unwrap();
        let restored = DeviceIdentity::from_secret_bytes(&identity.secret_bytes());
        assert_eq!(identity.public_key(), restored.public_key());
    }

    #[test]
    fn fingerprints_are_crockford_base32() {
        assert_eq!(crockford(&[0u8; 5]), "00000000");
        assert_eq!(crockford(&[0xff; 5]), "ZZZZZZZZ");
        let key = DeviceIdentity::generate().unwrap().public_key();
        let f = key.fingerprint();
        assert_eq!(f.len(), 16);
        assert!(f.chars().all(|c| CROCKFORD.contains(&(c as u8))));
        assert_eq!(key.fingerprint_grouped().len(), 19);
        assert_eq!(key.short_fingerprint(), format!("{}·{}", &f[..4], &f[4..8]));
    }

    #[test]
    fn rejects_keys_of_the_wrong_length() {
        assert_eq!(
            DevicePublicKey::from_bytes(&[0u8; 31]),
            Err(CryptoError::InvalidKey)
        );
    }
}
