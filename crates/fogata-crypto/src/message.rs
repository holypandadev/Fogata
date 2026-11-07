use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};

use crate::{
    CryptoError, DeviceIdentity, DevicePublicKey, GroupId, GroupKeys, SIGNATURE_LEN, random_bytes,
};

/// Version of the sealed message format. From 3 on, the signature covers [`SIGNING_LABEL`]
/// first.
pub const MESSAGE_VERSION: u8 = 3;

/// What a message's signature starts with, so that it cannot be taken for the signature of
/// anything else the device's identity signs, nor anything else for a message's.
const SIGNING_LABEL: &[u8] = b"fogata/v1/group-message\0";

/// Length of an XChaCha20-Poly1305 nonce in bytes.
pub const NONCE_LEN: usize = 24;

const HEADER_LEN: usize = 1 + 16 + 4 + 32 + 8 + 16;

/// Unencrypted part of a group message. It is authenticated as associated data and covered
/// by the sender's signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageHeader {
    pub version: u8,
    pub group_id: GroupId,
    /// Key epoch the message is encrypted for.
    pub epoch: u32,
    pub sender: DevicePublicKey,
    pub timestamp_ms: u64,
    /// Random id used to drop duplicates when a message reaches a device by several paths.
    pub message_id: [u8; 16],
}

impl MessageHeader {
    fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[0] = self.version;
        out[1..17].copy_from_slice(self.group_id.as_bytes());
        out[17..21].copy_from_slice(&self.epoch.to_be_bytes());
        out[21..53].copy_from_slice(self.sender.as_bytes());
        out[53..61].copy_from_slice(&self.timestamp_ms.to_be_bytes());
        out[61..77].copy_from_slice(&self.message_id);
        out
    }
}

/// A group message encrypted with the group's message key and signed by its author.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedMessage {
    pub header: MessageHeader,
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
    pub signature: [u8; SIGNATURE_LEN],
}

impl SealedMessage {
    /// Encrypts and signs `plaintext` for every member of the group.
    pub fn seal(
        keys: &GroupKeys,
        identity: &DeviceIdentity,
        timestamp_ms: u64,
        plaintext: &[u8],
    ) -> Result<Self, CryptoError> {
        let mut message_id = [0u8; 16];
        random_bytes(&mut message_id)?;
        let mut nonce = [0u8; NONCE_LEN];
        random_bytes(&mut nonce)?;
        let header = MessageHeader {
            version: MESSAGE_VERSION,
            group_id: keys.group_id(),
            epoch: keys.epoch(),
            sender: identity.public_key(),
            timestamp_ms,
            message_id,
        };
        let header_bytes = header.to_bytes();
        let ciphertext = cipher(keys)
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad: &header_bytes,
                },
            )
            .map_err(|_| CryptoError::Malformed)?;
        let signature = identity.sign(&signed_bytes(&header_bytes, &nonce, &ciphertext));
        Ok(Self {
            header,
            nonce,
            ciphertext,
            signature,
        })
    }

    /// Checks the author's signature and decrypts the message.
    pub fn open(&self, keys: &GroupKeys) -> Result<Vec<u8>, CryptoError> {
        if self.header.version != MESSAGE_VERSION {
            return Err(CryptoError::UnsupportedVersion(self.header.version));
        }
        if self.header.group_id != keys.group_id() {
            return Err(CryptoError::WrongGroup);
        }
        if self.header.epoch != keys.epoch() {
            return Err(CryptoError::WrongEpoch(self.header.epoch));
        }
        let header_bytes = self.header.to_bytes();
        self.header.sender.verify(
            &signed_bytes(&header_bytes, &self.nonce, &self.ciphertext),
            &self.signature,
        )?;
        cipher(keys)
            .decrypt(
                &XNonce::from(self.nonce),
                Payload {
                    msg: &self.ciphertext,
                    aad: &header_bytes,
                },
            )
            .map_err(|_| CryptoError::Decrypt)
    }
}

fn cipher(keys: &GroupKeys) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(&(*keys.message_key()).into())
}

fn signed_bytes(header: &[u8], nonce: &[u8], ciphertext: &[u8]) -> Vec<u8> {
    let mut out =
        Vec::with_capacity(SIGNING_LABEL.len() + header.len() + nonce.len() + ciphertext.len());
    out.extend_from_slice(SIGNING_LABEL);
    out.extend_from_slice(header);
    out.extend_from_slice(nonce);
    out.extend_from_slice(ciphertext);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GroupSecret;

    fn keys(byte: u8) -> GroupKeys {
        GroupKeys::derive(
            crate::GroupId::from_bytes(&[byte; 16]).unwrap(),
            0,
            &GroupSecret::from_bytes(&[byte; 32]).unwrap(),
        )
    }

    #[test]
    fn sealed_message_opens_for_the_group() {
        let keys = keys(3);
        let identity = DeviceIdentity::generate().unwrap();
        let sealed = SealedMessage::seal(&keys, &identity, 42, b"hi").unwrap();
        assert_eq!(sealed.open(&keys).unwrap(), b"hi");
        assert_eq!(sealed.header.sender, identity.public_key());
        assert_ne!(sealed.ciphertext, b"hi");
    }

    #[test]
    fn other_groups_cannot_open_it() {
        let identity = DeviceIdentity::generate().unwrap();
        let sealed = SealedMessage::seal(&keys(3), &identity, 42, b"hi").unwrap();
        assert_eq!(sealed.open(&keys(4)), Err(CryptoError::WrongGroup));
        let mut relabelled = sealed.clone();
        relabelled.header.group_id = keys(4).group_id();
        assert!(relabelled.open(&keys(4)).is_err());
    }

    #[test]
    fn tampering_is_detected() {
        let keys = keys(3);
        let identity = DeviceIdentity::generate().unwrap();
        let sealed = SealedMessage::seal(&keys, &identity, 42, b"hi").unwrap();

        let mut changed_text = sealed.clone();
        changed_text.ciphertext[0] ^= 1;
        assert_eq!(changed_text.open(&keys), Err(CryptoError::BadSignature));

        let mut changed_time = sealed.clone();
        changed_time.header.timestamp_ms += 1;
        assert_eq!(changed_time.open(&keys), Err(CryptoError::BadSignature));

        let mut forged_sender = sealed;
        forged_sender.header.sender = DeviceIdentity::generate().unwrap().public_key();
        assert_eq!(forged_sender.open(&keys), Err(CryptoError::BadSignature));
    }

    #[test]
    fn the_signature_is_one_of_a_group_message_only() {
        let keys = keys(3);
        let identity = DeviceIdentity::generate().unwrap();
        let sealed = SealedMessage::seal(&keys, &identity, 42, b"hi").unwrap();
        let bare = |message: &SealedMessage| {
            [
                &message.header.to_bytes()[..],
                &message.nonce,
                &message.ciphertext,
            ]
            .concat()
        };
        // Not a signature of the bare bytes: anything else the identity signs that starts
        // as a message's header does could not pass for a message.
        assert_eq!(
            identity
                .public_key()
                .verify(&bare(&sealed), &sealed.signature),
            Err(CryptoError::BadSignature)
        );
        // A message signed as before, without the label, is refused.
        let mut earlier = sealed.clone();
        earlier.header.version = 2;
        earlier.signature = identity.sign(&bare(&earlier));
        assert_eq!(earlier.open(&keys), Err(CryptoError::UnsupportedVersion(2)));
        assert_eq!(sealed.open(&keys).unwrap(), b"hi");
    }

    #[test]
    fn other_epochs_need_their_own_keys() {
        let id = crate::GroupId::from_bytes(&[5u8; 16]).unwrap();
        let identity = DeviceIdentity::generate().unwrap();
        let epoch0 = GroupKeys::derive(id, 0, &GroupSecret::from_bytes(&[1u8; 32]).unwrap());
        let epoch1 = GroupKeys::derive(id, 1, &GroupSecret::from_bytes(&[2u8; 32]).unwrap());
        let sealed = SealedMessage::seal(&epoch0, &identity, 1, b"x").unwrap();
        assert_eq!(sealed.open(&epoch1), Err(CryptoError::WrongEpoch(0)));
        assert_eq!(sealed.open(&epoch0).unwrap(), b"x");
    }
}
