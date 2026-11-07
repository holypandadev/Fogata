//! Sealing data to one device: only the holder of that device key can open it.
//!
//! An ephemeral X25519 key agrees a secret with the device's X25519 key (the birationally
//! equivalent form of its Ed25519 key); HKDF-SHA256 turns it into an XChaCha20-Poly1305 key.
//! `context` binds the box to its purpose (for example a group id and epoch), so a box cannot
//! be moved to another place where the same recipient would accept it.

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::{CryptoError, DeviceIdentity, DevicePublicKey, NONCE_LEN, random_bytes};

const LABEL: &[u8] = b"fogata/v1/seal";

/// Data sealed to one device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sealed {
    pub ephemeral: [u8; 32],
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
}

fn key(
    shared: &x25519_dalek::SharedSecret,
    ephemeral: &[u8; 32],
    recipient: &DevicePublicKey,
    context: &[u8],
) -> Zeroizing<[u8; 32]> {
    let mut info = Vec::with_capacity(64 + context.len());
    info.extend_from_slice(ephemeral);
    info.extend_from_slice(recipient.as_bytes());
    info.extend_from_slice(context);
    let mut out = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(Some(LABEL), shared.as_bytes())
        .expand(&info, out.as_mut())
        .expect("HKDF output length is far below the limit");
    out
}

impl Sealed {
    /// Seals `plaintext` so that only `recipient` can open it.
    pub fn seal(
        recipient: &DevicePublicKey,
        context: &[u8],
        plaintext: &[u8],
    ) -> Result<Self, CryptoError> {
        let mut ephemeral_secret = Zeroizing::new([0u8; 32]);
        random_bytes(ephemeral_secret.as_mut())?;
        let ephemeral_secret = x25519_dalek::StaticSecret::from(*ephemeral_secret);
        let ephemeral = x25519_dalek::PublicKey::from(&ephemeral_secret).to_bytes();
        let shared = ephemeral_secret.diffie_hellman(&recipient.x25519_public()?);
        if !shared.was_contributory() {
            return Err(CryptoError::InvalidKey);
        }
        let key = key(&shared, &ephemeral, recipient, context);
        let mut nonce = [0u8; NONCE_LEN];
        random_bytes(&mut nonce)?;
        let ciphertext = XChaCha20Poly1305::new(&(*key).into())
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad: context,
                },
            )
            .map_err(|_| CryptoError::Malformed)?;
        Ok(Self {
            ephemeral,
            nonce,
            ciphertext,
        })
    }

    /// Opens a box sealed to `identity` with the same `context`.
    pub fn open(&self, identity: &DeviceIdentity, context: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let shared = identity
            .x25519_secret()
            .diffie_hellman(&x25519_dalek::PublicKey::from(self.ephemeral));
        if !shared.was_contributory() {
            return Err(CryptoError::Decrypt);
        }
        let key = key(&shared, &self.ephemeral, &identity.public_key(), context);
        XChaCha20Poly1305::new(&(*key).into())
            .decrypt(
                &XNonce::from(self.nonce),
                Payload {
                    msg: &self.ciphertext,
                    aad: context,
                },
            )
            .map_err(|_| CryptoError::Decrypt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_recipient_opens_it() {
        let alice = DeviceIdentity::generate().unwrap();
        let bob = DeviceIdentity::generate().unwrap();
        let sealed = Sealed::seal(&alice.public_key(), b"ctx", b"secret").unwrap();
        assert_eq!(sealed.open(&alice, b"ctx").unwrap(), b"secret");
        assert_eq!(sealed.open(&bob, b"ctx"), Err(CryptoError::Decrypt));
    }

    #[test]
    fn context_and_contents_are_bound() {
        let alice = DeviceIdentity::generate().unwrap();
        let sealed = Sealed::seal(&alice.public_key(), b"group-1", b"secret").unwrap();
        assert_eq!(sealed.open(&alice, b"group-2"), Err(CryptoError::Decrypt));
        let mut changed = sealed.clone();
        changed.ciphertext[0] ^= 1;
        assert_eq!(changed.open(&alice, b"group-1"), Err(CryptoError::Decrypt));
    }

    #[test]
    fn each_seal_is_different() {
        let alice = DeviceIdentity::generate().unwrap();
        let a = Sealed::seal(&alice.public_key(), b"c", b"same").unwrap();
        let b = Sealed::seal(&alice.public_key(), b"c", b"same").unwrap();
        assert_ne!(a.ciphertext, b.ciphertext);
        assert_ne!(a.ephemeral, b.ephemeral);
    }
}
