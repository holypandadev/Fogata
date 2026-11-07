use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::{
    CryptoError, DeviceIdentity, DevicePublicKey, GroupKeys, NONCE_LEN, SIGNATURE_LEN, random_bytes,
};

/// Length of a membership challenge in bytes.
pub const CHALLENGE_LEN: usize = 32;
/// Length of a sealed proof's ciphertext: the device key, the MAC, the signature and the
/// authentication tag.
pub const SEALED_PROOF_LEN: usize = 32 + 32 + SIGNATURE_LEN + 16;

/// Length of a [`group_tag`].
pub const GROUP_TAG_LEN: usize = 16;

const LABEL: &[u8] = b"fogata/v1/hello";
const WHICH_LABEL: &[u8] = b"fogata/v1/which";
const SEAL_LABEL: &[u8] = b"fogata/v1/proof";

/// Proof that the sender knows the group secret and owns a device key, bound to one
/// transport connection.
///
/// `prover_peer` and `verifier_peer` are the transport identities (libp2p peer ids) of the
/// two ends, as authenticated by the transport handshake. Binding them prevents a proof from
/// being replayed on another connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MembershipProof {
    /// Key epoch whose `auth_key` the proof uses.
    pub epoch: u32,
    pub device_key: DevicePublicKey,
    pub mac: [u8; 32],
    pub signature: [u8; SIGNATURE_LEN],
}

/// Creates a fresh random challenge.
pub fn new_challenge() -> Result<[u8; CHALLENGE_LEN], CryptoError> {
    let mut challenge = [0u8; CHALLENGE_LEN];
    random_bytes(&mut challenge)?;
    Ok(challenge)
}

fn transcript(
    keys: &GroupKeys,
    prover_peer: &[u8],
    verifier_peer: &[u8],
    challenge: &[u8; CHALLENGE_LEN],
    device_key: &DevicePublicKey,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(LABEL.len() + 16 + 2 * 48 + CHALLENGE_LEN + 32 + 8);
    out.extend_from_slice(LABEL);
    out.extend_from_slice(keys.group_id().as_bytes());
    out.extend_from_slice(&keys.epoch().to_be_bytes());
    for part in [prover_peer, verifier_peer] {
        out.extend_from_slice(&(part.len() as u32).to_be_bytes());
        out.extend_from_slice(part);
    }
    out.extend_from_slice(challenge);
    out.extend_from_slice(device_key.as_bytes());
    out
}

/// A membership proof readable only with the group key of its epoch. Whoever asks for a proof
/// without that key (a stranger that learnt the group id) sees bytes that tell nothing: not
/// the device key, not even whether the answer is a proof at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedProof {
    pub epoch: u32,
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
}

impl SealedProof {
    /// Something that looks like a sealed proof but is random, for answering about groups
    /// or epochs this device does not have.
    pub fn decoy(epoch: u32) -> Result<Self, CryptoError> {
        let mut nonce = [0u8; NONCE_LEN];
        random_bytes(&mut nonce)?;
        let mut ciphertext = vec![0u8; SEALED_PROOF_LEN];
        random_bytes(&mut ciphertext)?;
        Ok(Self {
            epoch,
            nonce,
            ciphertext,
        })
    }
}

/// The key that seals a proof: from the epoch's `auth_key` and the challenge.
fn seal_key(keys: &GroupKeys, challenge: &[u8; CHALLENGE_LEN]) -> Zeroizing<[u8; 32]> {
    let mut out = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(Some(SEAL_LABEL), keys.auth_key())
        .expand(challenge, out.as_mut())
        .expect("HKDF output length is far below the limit");
    out
}

/// What a sealed proof is bound to: everything but the device key.
fn seal_aad(
    keys: &GroupKeys,
    prover_peer: &[u8],
    verifier_peer: &[u8],
    challenge: &[u8; CHALLENGE_LEN],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(SEAL_LABEL.len() + 16 + 4 + 2 * 48 + CHALLENGE_LEN);
    out.extend_from_slice(SEAL_LABEL);
    out.extend_from_slice(keys.group_id().as_bytes());
    out.extend_from_slice(&keys.epoch().to_be_bytes());
    for part in [prover_peer, verifier_peer] {
        out.extend_from_slice(&(part.len() as u32).to_be_bytes());
        out.extend_from_slice(part);
    }
    out.extend_from_slice(challenge);
    out
}

fn mac(keys: &GroupKeys) -> Hmac<Sha256> {
    <Hmac<Sha256> as KeyInit>::new_from_slice(keys.auth_key())
        .expect("HMAC accepts keys of any length")
}

fn which(
    keys: &GroupKeys,
    challenger_peer: &[u8],
    responder_peer: &[u8],
    challenge: &[u8; CHALLENGE_LEN],
) -> Hmac<Sha256> {
    let mut hmac = mac(keys);
    hmac.update(WHICH_LABEL);
    hmac.update(keys.group_id().as_bytes());
    hmac.update(&keys.epoch().to_be_bytes());
    for part in [challenger_peer, responder_peer] {
        hmac.update(&(part.len() as u32).to_be_bytes());
        hmac.update(part);
    }
    hmac.update(challenge);
    hmac
}

/// Tells which group and key epoch a challenge is about to whoever holds that epoch's key,
/// and to nobody else: an HMAC under the epoch's `auth_key` over the group, the epoch, both
/// transport identities and the challenge. With a fresh challenge every time, tags cannot be
/// linked to a group or to each other.
pub fn group_tag(
    keys: &GroupKeys,
    challenger_peer: &[u8],
    responder_peer: &[u8],
    challenge: &[u8; CHALLENGE_LEN],
) -> [u8; GROUP_TAG_LEN] {
    let full = which(keys, challenger_peer, responder_peer, challenge)
        .finalize()
        .into_bytes();
    let mut tag = [0u8; GROUP_TAG_LEN];
    tag.copy_from_slice(&full[..GROUP_TAG_LEN]);
    tag
}

/// Whether `tag` is [`group_tag`] for these keys, compared in constant time.
pub fn group_tag_matches(
    keys: &GroupKeys,
    challenger_peer: &[u8],
    responder_peer: &[u8],
    challenge: &[u8; CHALLENGE_LEN],
    tag: &[u8],
) -> bool {
    tag.len() == GROUP_TAG_LEN
        && which(keys, challenger_peer, responder_peer, challenge)
            .verify_truncated_left(tag)
            .is_ok()
}

impl MembershipProof {
    /// Answers `challenge` sent by `verifier_peer`.
    pub fn create(
        keys: &GroupKeys,
        identity: &DeviceIdentity,
        prover_peer: &[u8],
        verifier_peer: &[u8],
        challenge: &[u8; CHALLENGE_LEN],
    ) -> Self {
        let device_key = identity.public_key();
        let transcript = transcript(keys, prover_peer, verifier_peer, challenge, &device_key);
        let mut hmac = mac(keys);
        hmac.update(&transcript);
        let mac: [u8; 32] = hmac.finalize().into_bytes().into();
        let mut signed = transcript;
        signed.extend_from_slice(&mac);
        Self {
            epoch: keys.epoch(),
            device_key,
            mac,
            signature: identity.sign(&signed),
        }
    }

    /// Seals the proof so that only holders of the epoch's key can read it.
    pub fn seal(
        &self,
        keys: &GroupKeys,
        prover_peer: &[u8],
        verifier_peer: &[u8],
        challenge: &[u8; CHALLENGE_LEN],
    ) -> Result<SealedProof, CryptoError> {
        if self.epoch != keys.epoch() {
            return Err(CryptoError::WrongEpoch(self.epoch));
        }
        let mut plaintext = Zeroizing::new(Vec::with_capacity(32 + 32 + SIGNATURE_LEN));
        plaintext.extend_from_slice(self.device_key.as_bytes());
        plaintext.extend_from_slice(&self.mac);
        plaintext.extend_from_slice(&self.signature);
        let mut nonce = [0u8; NONCE_LEN];
        random_bytes(&mut nonce)?;
        let ciphertext = XChaCha20Poly1305::new(&(*seal_key(keys, challenge)).into())
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: &plaintext,
                    aad: &seal_aad(keys, prover_peer, verifier_peer, challenge),
                },
            )
            .map_err(|_| CryptoError::Malformed)?;
        Ok(SealedProof {
            epoch: self.epoch,
            nonce,
            ciphertext,
        })
    }

    /// Opens a sealed proof received from `prover_peer` in answer to our `challenge` and
    /// checks it. Fails for anything not sealed with `keys`, decoys included.
    pub fn open(
        sealed: &SealedProof,
        keys: &GroupKeys,
        prover_peer: &[u8],
        verifier_peer: &[u8],
        challenge: &[u8; CHALLENGE_LEN],
    ) -> Result<DevicePublicKey, CryptoError> {
        if sealed.epoch != keys.epoch() {
            return Err(CryptoError::WrongEpoch(sealed.epoch));
        }
        let plaintext = XChaCha20Poly1305::new(&(*seal_key(keys, challenge)).into())
            .decrypt(
                &XNonce::from(sealed.nonce),
                Payload {
                    msg: &sealed.ciphertext,
                    aad: &seal_aad(keys, prover_peer, verifier_peer, challenge),
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| CryptoError::BadProof)?;
        if plaintext.len() != 32 + 32 + SIGNATURE_LEN {
            return Err(CryptoError::BadProof);
        }
        let proof = Self {
            epoch: sealed.epoch,
            device_key: DevicePublicKey::from_bytes(&plaintext[..32])?,
            mac: plaintext[32..64].try_into().expect("32 bytes"),
            signature: plaintext[64..].try_into().expect("64 bytes"),
        };
        proof.verify(keys, prover_peer, verifier_peer, challenge)?;
        Ok(proof.device_key)
    }

    /// Checks a proof received from `prover_peer` in answer to our `challenge`.
    pub fn verify(
        &self,
        keys: &GroupKeys,
        prover_peer: &[u8],
        verifier_peer: &[u8],
        challenge: &[u8; CHALLENGE_LEN],
    ) -> Result<(), CryptoError> {
        if self.epoch != keys.epoch() {
            return Err(CryptoError::WrongEpoch(self.epoch));
        }
        let transcript = transcript(
            keys,
            prover_peer,
            verifier_peer,
            challenge,
            &self.device_key,
        );
        let mut hmac = mac(keys);
        hmac.update(&transcript);
        hmac.verify_slice(&self.mac)
            .map_err(|_| CryptoError::BadProof)?;
        let mut signed = transcript;
        signed.extend_from_slice(&self.mac);
        self.device_key.verify(&signed, &self.signature)
    }
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
    fn a_group_tag_names_one_group_epoch_and_connection() {
        let keys1 = keys(1);
        let challenge = new_challenge().unwrap();
        let tag = group_tag(&keys1, b"asker", b"member", &challenge);
        assert!(group_tag_matches(
            &keys1, b"asker", b"member", &challenge, &tag
        ));
        assert!(!group_tag_matches(
            &keys(2),
            b"asker",
            b"member",
            &challenge,
            &tag
        ));
        let later = GroupKeys::derive(
            crate::GroupId::from_bytes(&[1; 16]).unwrap(),
            1,
            &GroupSecret::from_bytes(&[1; 32]).unwrap(),
        );
        assert!(!group_tag_matches(
            &later, b"asker", b"member", &challenge, &tag
        ));
        assert!(!group_tag_matches(
            &keys1, b"member", b"asker", &challenge, &tag
        ));
        assert!(!group_tag_matches(
            &keys1, b"asker", b"other", &challenge, &tag
        ));
        assert!(!group_tag_matches(
            &keys1, b"asker", b"member", &[0; 32], &tag
        ));
        assert!(!group_tag_matches(
            &keys1,
            b"asker",
            b"member",
            &challenge,
            &tag[..8]
        ));
        // Another challenge, another tag.
        assert_ne!(
            group_tag(&keys1, b"asker", b"member", &new_challenge().unwrap()),
            tag
        );
    }

    #[test]
    fn valid_proof_verifies() {
        let keys = keys(1);
        let identity = DeviceIdentity::generate().unwrap();
        let challenge = new_challenge().unwrap();
        let proof = MembershipProof::create(&keys, &identity, b"prover", b"verifier", &challenge);
        assert_eq!(
            proof.verify(&keys, b"prover", b"verifier", &challenge),
            Ok(())
        );
    }

    #[test]
    fn proof_from_another_group_is_rejected() {
        let identity = DeviceIdentity::generate().unwrap();
        let challenge = new_challenge().unwrap();
        let proof = MembershipProof::create(&keys(1), &identity, b"p", b"v", &challenge);
        assert_eq!(
            proof.verify(&keys(2), b"p", b"v", &challenge),
            Err(CryptoError::BadProof)
        );
    }

    #[test]
    fn proof_cannot_be_replayed_on_another_connection() {
        let keys = keys(1);
        let identity = DeviceIdentity::generate().unwrap();
        let challenge = new_challenge().unwrap();
        let proof = MembershipProof::create(&keys, &identity, b"p", b"v", &challenge);
        assert_eq!(
            proof.verify(&keys, b"p", b"other", &challenge),
            Err(CryptoError::BadProof)
        );
        assert_eq!(
            proof.verify(&keys, b"p", b"v", &new_challenge().unwrap()),
            Err(CryptoError::BadProof)
        );
    }

    #[test]
    fn substituted_device_key_is_rejected() {
        let keys = keys(1);
        let challenge = new_challenge().unwrap();
        let mut proof = MembershipProof::create(
            &keys,
            &DeviceIdentity::generate().unwrap(),
            b"p",
            b"v",
            &challenge,
        );
        proof.device_key = DeviceIdentity::generate().unwrap().public_key();
        assert!(proof.verify(&keys, b"p", b"v", &challenge).is_err());
    }

    #[test]
    fn sealed_proofs_open_only_with_the_epoch_key() {
        let group = keys(1);
        let identity = DeviceIdentity::generate().unwrap();
        let challenge = new_challenge().unwrap();
        let proof = MembershipProof::create(&group, &identity, b"p", b"v", &challenge);
        let sealed = proof.seal(&group, b"p", b"v", &challenge).unwrap();
        assert_eq!(sealed.ciphertext.len(), SEALED_PROOF_LEN);
        assert_eq!(
            MembershipProof::open(&sealed, &group, b"p", b"v", &challenge).unwrap(),
            identity.public_key()
        );
        // The device key is not in the clear.
        let key = identity.public_key();
        assert!(!sealed.ciphertext.windows(32).any(|w| w == key.as_bytes()));
        // Another group's key, another connection or another challenge: nothing opens.
        assert_eq!(
            MembershipProof::open(&sealed, &keys(2), b"p", b"v", &challenge),
            Err(CryptoError::BadProof)
        );
        assert_eq!(
            MembershipProof::open(&sealed, &group, b"p", b"other", &challenge),
            Err(CryptoError::BadProof)
        );
        assert_eq!(
            MembershipProof::open(&sealed, &group, b"p", b"v", &new_challenge().unwrap()),
            Err(CryptoError::BadProof)
        );
        // A decoy has the same shape and opens with nothing.
        let decoy = SealedProof::decoy(group.epoch()).unwrap();
        assert_eq!(decoy.ciphertext.len(), sealed.ciphertext.len());
        assert_eq!(
            MembershipProof::open(&decoy, &group, b"p", b"v", &challenge),
            Err(CryptoError::BadProof)
        );
    }
}
