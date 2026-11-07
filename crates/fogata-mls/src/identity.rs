//! A device's place in MLS: its MLS signing key and the credential that ties that key to the
//! device key the group's log knows it by.
//!
//! The credential's identity is the device key followed by the device key's signature over
//! the MLS signing key. Every leaf of every tree must carry one that checks, so a leaf
//! belongs to the device it names and to no other. The MLS signing key is derived from the
//! device key's secret: never the same key, so an MLS signature can never pass for a Fogata
//! one, but always the same for one device, so a device that lost its MLS state comes back
//! with the same credential, and MLS replaces its old leaf instead of keeping two.

use ed25519_dalek::{Signer as _, SigningKey};
use fogata_crypto::{DeviceIdentity, DevicePublicKey, SIGNATURE_LEN, derive_secret};
use openmls::prelude::{BasicCredential, Credential, CredentialWithKey};
use openmls_traits::{
    signatures::{Signer, SignerError},
    types::SignatureScheme,
};
use zeroize::Zeroizing;

const SIGNING_KEY_LABEL: &[u8] = b"fogata/v3/mls-signing-key";
const BINDING_LABEL: &[u8] = b"fogata/v3/mls-leaf";

/// Length of a credential's identity: the device key and its signature.
const IDENTITY_LEN: usize = 32 + SIGNATURE_LEN;

/// A device's MLS signing key (Ed25519).
pub struct MlsSigner {
    key: SigningKey,
}

impl std::fmt::Debug for MlsSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MlsSigner(..)")
    }
}

impl MlsSigner {
    /// The MLS signing key of `device`.
    pub fn of(device: &DeviceIdentity) -> Self {
        let seed = Zeroizing::new(derive_secret(
            SIGNING_KEY_LABEL,
            device.secret_bytes().as_ref(),
            b"ed25519",
        ));
        Self {
            key: SigningKey::from_bytes(&seed),
        }
    }

    pub fn public(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }

    /// The credential of `device`, whose MLS signing key this is.
    pub fn credential(&self, device: &DeviceIdentity) -> CredentialWithKey {
        let public = self.public();
        let mut identity = device.public_key().as_bytes().to_vec();
        identity.extend_from_slice(&device.sign(&binding(&public)));
        CredentialWithKey {
            credential: BasicCredential::new(identity).into(),
            signature_key: public.to_vec().into(),
        }
    }
}

impl Signer for MlsSigner {
    fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, SignerError> {
        Ok(self.key.sign(payload).to_bytes().to_vec())
    }

    fn signature_scheme(&self) -> SignatureScheme {
        SignatureScheme::ED25519
    }
}

fn binding(signature_key: &[u8]) -> Vec<u8> {
    [BINDING_LABEL, signature_key].concat()
}

/// The device a leaf belongs to: the one its credential names, if the credential is a basic
/// one of the right length and that device signed the leaf's signature key.
pub fn device_of(credential: &Credential, signature_key: &[u8]) -> Option<DevicePublicKey> {
    let basic = BasicCredential::try_from(credential.clone()).ok()?;
    let identity = basic.identity();
    if identity.len() != IDENTITY_LEN {
        return None;
    }
    let device = DevicePublicKey::from_bytes(&identity[..32]).ok()?;
    device
        .verify(&binding(signature_key), &identity[32..])
        .ok()?;
    Some(device)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_credential_names_its_device_and_only_with_its_key() {
        let ana = DeviceIdentity::generate().unwrap();
        let signer = MlsSigner::of(&ana);
        assert_eq!(signer.public(), MlsSigner::of(&ana).public());
        assert_ne!(signer.public(), *ana.public_key().as_bytes());
        let mine = signer.credential(&ana);
        let key = mine.signature_key.as_slice();
        assert_eq!(device_of(&mine.credential, key), Some(ana.public_key()));
        // Another signing key under Ana's credential: no.
        let leo = DeviceIdentity::generate().unwrap();
        assert_eq!(
            device_of(&mine.credential, &MlsSigner::of(&leo).public()),
            None
        );
        // A credential claiming Ana, signed by Leo: no.
        let mut identity = ana.public_key().as_bytes().to_vec();
        identity.extend_from_slice(&leo.sign(&binding(key)));
        let forged: Credential = BasicCredential::new(identity).into();
        assert_eq!(device_of(&forged, key), None);
        // Cut short, or plain bytes: no.
        let short: Credential = BasicCredential::new(ana.public_key().as_bytes().to_vec()).into();
        assert_eq!(device_of(&short, key), None);
    }
}
