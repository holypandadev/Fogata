//! The crypto OpenMLS runs on, built only on permissively licensed crates: the `hpke` crate
//! (RFC 9180) and the RustCrypto crates the rest of Fogata uses. The providers published with
//! OpenMLS depend on `hpke-rs`, which is MPL-2.0.
//!
//! One ciphersuite only, [`CIPHERSUITE`]: X25519, ChaCha20-Poly1305, SHA-256 and Ed25519, the
//! primitives Fogata uses everywhere else. Anything else is refused, whatever OpenMLS asks.

use std::convert::Infallible;

use chacha20poly1305::{
    ChaCha20Poly1305,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use hpke::{
    Deserializable, Kem as _, OpModeR, OpModeS, Serializable, aead::ChaCha20Poly1305 as HpkeAead,
    kdf::HkdfSha256, kem::X25519HkdfSha256,
};
use openmls::prelude::Ciphersuite;
use openmls::prelude::tls_codec::SecretVLBytes;
use openmls_traits::{
    crypto::OpenMlsCrypto,
    random::OpenMlsRand,
    types::{
        AeadType, CryptoError, ExporterSecret, HashType, HpkeAeadType, HpkeCiphertext, HpkeConfig,
        HpkeKdfType, HpkeKemType, HpkeKeyPair, SignatureScheme,
    },
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// The one ciphersuite: MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519 (0x0003).
pub const CIPHERSUITE: Ciphersuite =
    Ciphersuite::MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519;

type Kem = X25519HkdfSha256;

/// Length of an X25519 key and of the keying material a key pair is made from.
const KEY_LEN: usize = 32;

/// The crypto of [`CIPHERSUITE`] for OpenMLS.
#[derive(Clone, Copy, Debug, Default)]
pub struct FogataCrypto;

fn hpke_suite(config: &HpkeConfig) -> Result<(), CryptoError> {
    match (config.0, config.1, config.2) {
        (HpkeKemType::DhKem25519, HpkeKdfType::HkdfSha256, HpkeAeadType::ChaCha20Poly1305) => {
            Ok(())
        }
        _ => Err(CryptoError::UnsupportedCiphersuite),
    }
}

fn sha256(hash: HashType) -> Result<(), CryptoError> {
    match hash {
        HashType::Sha2_256 => Ok(()),
        _ => Err(CryptoError::UnsupportedHashAlgorithm),
    }
}

fn ed25519(alg: SignatureScheme) -> Result<(), CryptoError> {
    match alg {
        SignatureScheme::ED25519 => Ok(()),
        _ => Err(CryptoError::UnsupportedSignatureScheme),
    }
}

fn chacha(
    alg: AeadType,
    key: &[u8],
    nonce: &[u8],
) -> Result<(ChaCha20Poly1305, [u8; 12]), CryptoError> {
    if alg != AeadType::ChaCha20Poly1305 {
        return Err(CryptoError::UnsupportedAeadAlgorithm);
    }
    let nonce: [u8; 12] = nonce.try_into().map_err(|_| CryptoError::InvalidLength)?;
    let cipher = ChaCha20Poly1305::new_from_slice(key).map_err(|_| CryptoError::InvalidLength)?;
    Ok((cipher, nonce))
}

/// Fresh keying material for an ephemeral HPKE key, from the operating system.
fn ephemeral_ikm() -> Result<Zeroizing<[u8; KEY_LEN]>, CryptoError> {
    let mut ikm = Zeroizing::new([0u8; KEY_LEN]);
    getrandom::fill(ikm.as_mut()).map_err(|_| CryptoError::InsufficientRandomness)?;
    Ok(ikm)
}

/// The randomness the `hpke` crate draws for a sender: one ephemeral key's keying material,
/// given beforehand, so that drawing it cannot fail (or panic) inside the crate, and so the
/// sender can be checked against RFC 9180's test vectors. Should more ever be asked of it,
/// it comes from a seed drawn from the operating system beforehand too, where a failure is
/// an error: the crate takes randomness that cannot fail.
struct Ikm {
    first: Option<Zeroizing<[u8; KEY_LEN]>>,
    more: Hkdf<Sha256>,
    drawn: u64,
}

impl Ikm {
    fn new(first: Zeroizing<[u8; KEY_LEN]>) -> Result<Self, CryptoError> {
        Ok(Self::with_seed(first, &*ephemeral_ikm()?))
    }

    fn with_seed(first: Zeroizing<[u8; KEY_LEN]>, seed: &[u8; KEY_LEN]) -> Self {
        Self {
            first: Some(first),
            more: Hkdf::<Sha256>::new(Some(b"fogata/v1/mls-sender-randomness"), seed),
            drawn: 0,
        }
    }
}

impl rand_core::TryRng for Ikm {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        let mut b = [0u8; 4];
        self.try_fill_bytes(&mut b)?;
        Ok(u32::from_le_bytes(b))
    }

    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        let mut b = [0u8; 8];
        self.try_fill_bytes(&mut b)?;
        Ok(u64::from_le_bytes(b))
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        match self.first.take() {
            Some(ikm) if dst.len() == KEY_LEN => dst.copy_from_slice(ikm.as_ref()),
            given => {
                self.first = given;
                // HKDF gives at most 255 hashes' worth for one name: each piece has its own.
                for piece in dst.chunks_mut(255 * 32) {
                    self.drawn += 1;
                    self.more
                        .expand(&self.drawn.to_be_bytes(), piece)
                        .expect("at most 255 hashes' worth");
                }
            }
        }
        Ok(())
    }
}

impl rand_core::TryCryptoRng for Ikm {}

/// HPKE base-mode single-shot sealing to `pk_r`, with the ephemeral key made from `ikm_e`.
pub(crate) fn hpke_seal_with(
    ikm_e: Zeroizing<[u8; KEY_LEN]>,
    pk_r: &[u8],
    info: &[u8],
    aad: &[u8],
    ptxt: &[u8],
) -> Result<HpkeCiphertext, CryptoError> {
    let pk =
        <Kem as hpke::Kem>::PublicKey::from_bytes(pk_r).map_err(|_| CryptoError::InvalidLength)?;
    let (enc, ct) = hpke::single_shot_seal_with_rng::<HpkeAead, HkdfSha256, Kem>(
        &OpModeS::Base,
        &pk,
        info,
        ptxt,
        aad,
        &mut Ikm::new(ikm_e)?,
    )
    .map_err(|_| CryptoError::HpkeEncryptionError)?;
    Ok(HpkeCiphertext {
        kem_output: enc.to_bytes().to_vec().into(),
        ciphertext: ct.into(),
    })
}

/// HPKE base-mode sender set up to `pk_r`, with the ephemeral key made from `ikm_e`: the
/// encapsulated key and `len` bytes exported for `context`.
pub(crate) fn hpke_export_with(
    ikm_e: Zeroizing<[u8; KEY_LEN]>,
    pk_r: &[u8],
    info: &[u8],
    context: &[u8],
    len: usize,
) -> Result<(Vec<u8>, ExporterSecret), CryptoError> {
    let pk =
        <Kem as hpke::Kem>::PublicKey::from_bytes(pk_r).map_err(|_| CryptoError::InvalidLength)?;
    let (enc, ctx) = hpke::setup_sender_with_rng::<HpkeAead, HkdfSha256, Kem>(
        &OpModeS::Base,
        &pk,
        info,
        &mut Ikm::new(ikm_e)?,
    )
    .map_err(|_| CryptoError::SenderSetupError)?;
    let mut out = Zeroizing::new(vec![0u8; len]);
    ctx.export(context, &mut out)
        .map_err(|_| CryptoError::ExporterError)?;
    Ok((enc.to_bytes().to_vec(), out.to_vec().into()))
}

fn private_key(sk_r: &[u8]) -> Result<<Kem as hpke::Kem>::PrivateKey, CryptoError> {
    <Kem as hpke::Kem>::PrivateKey::from_bytes(sk_r).map_err(|_| CryptoError::HpkeDecryptionError)
}

fn encapped(enc: &[u8]) -> Result<<Kem as hpke::Kem>::EncappedKey, CryptoError> {
    <Kem as hpke::Kem>::EncappedKey::from_bytes(enc).map_err(|_| CryptoError::HpkeDecryptionError)
}

impl OpenMlsCrypto for FogataCrypto {
    fn supports(&self, ciphersuite: Ciphersuite) -> Result<(), CryptoError> {
        if ciphersuite == CIPHERSUITE {
            Ok(())
        } else {
            Err(CryptoError::UnsupportedCiphersuite)
        }
    }

    fn supported_ciphersuites(&self) -> Vec<Ciphersuite> {
        vec![CIPHERSUITE]
    }

    fn hkdf_extract(
        &self,
        hash_type: HashType,
        salt: &[u8],
        ikm: &[u8],
    ) -> Result<SecretVLBytes, CryptoError> {
        sha256(hash_type)?;
        let (prk, _) = Hkdf::<Sha256>::extract(Some(salt), ikm);
        Ok(prk.as_slice().into())
    }

    fn hmac(
        &self,
        hash_type: HashType,
        key: &[u8],
        message: &[u8],
    ) -> Result<SecretVLBytes, CryptoError> {
        sha256(hash_type)?;
        let mut mac = <Hmac<Sha256> as hmac::KeyInit>::new_from_slice(key)
            .map_err(|_| CryptoError::CryptoLibraryError)?;
        mac.update(message);
        Ok(mac.finalize().into_bytes().as_slice().into())
    }

    fn hkdf_expand(
        &self,
        hash_type: HashType,
        prk: &[u8],
        info: &[u8],
        okm_len: usize,
    ) -> Result<SecretVLBytes, CryptoError> {
        sha256(hash_type)?;
        let hkdf =
            Hkdf::<Sha256>::from_prk(prk).map_err(|_| CryptoError::HkdfOutputLengthInvalid)?;
        let mut okm = Zeroizing::new(vec![0u8; okm_len]);
        hkdf.expand(info, &mut okm)
            .map_err(|_| CryptoError::HkdfOutputLengthInvalid)?;
        Ok(okm.as_slice().into())
    }

    fn hash(&self, hash_type: HashType, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        sha256(hash_type)?;
        Ok(Sha256::digest(data).to_vec())
    }

    fn aead_encrypt(
        &self,
        alg: AeadType,
        key: &[u8],
        data: &[u8],
        nonce: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let (cipher, nonce) = chacha(alg, key, nonce)?;
        cipher
            .encrypt(&nonce.into(), Payload { msg: data, aad })
            .map_err(|_| CryptoError::CryptoLibraryError)
    }

    fn aead_decrypt(
        &self,
        alg: AeadType,
        key: &[u8],
        ct_tag: &[u8],
        nonce: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let (cipher, nonce) = chacha(alg, key, nonce)?;
        cipher
            .decrypt(&nonce.into(), Payload { msg: ct_tag, aad })
            .map_err(|_| CryptoError::AeadDecryptionError)
    }

    fn signature_key_gen(&self, alg: SignatureScheme) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
        ed25519(alg)?;
        let mut seed = Zeroizing::new([0u8; 32]);
        getrandom::fill(seed.as_mut()).map_err(|_| CryptoError::InsufficientRandomness)?;
        let sk = ed25519_dalek::SigningKey::from_bytes(&seed);
        Ok((seed.to_vec(), sk.verifying_key().to_bytes().to_vec()))
    }

    fn verify_signature(
        &self,
        alg: SignatureScheme,
        data: &[u8],
        pk: &[u8],
        signature: &[u8],
    ) -> Result<(), CryptoError> {
        ed25519(alg)?;
        let pk: [u8; 32] = pk.try_into().map_err(|_| CryptoError::InvalidLength)?;
        let signature: [u8; 64] = signature
            .try_into()
            .map_err(|_| CryptoError::InvalidSignature)?;
        ed25519_dalek::VerifyingKey::from_bytes(&pk)
            .map_err(|_| CryptoError::InvalidSignature)?
            .verify_strict(data, &ed25519_dalek::Signature::from_bytes(&signature))
            .map_err(|_| CryptoError::InvalidSignature)
    }

    fn sign(&self, alg: SignatureScheme, data: &[u8], key: &[u8]) -> Result<Vec<u8>, CryptoError> {
        use ed25519_dalek::Signer as _;
        ed25519(alg)?;
        let seed: Zeroizing<[u8; 32]> =
            Zeroizing::new(key.try_into().map_err(|_| CryptoError::InvalidLength)?);
        Ok(ed25519_dalek::SigningKey::from_bytes(&seed)
            .sign(data)
            .to_bytes()
            .to_vec())
    }

    fn hpke_seal(
        &self,
        config: HpkeConfig,
        pk_r: &[u8],
        info: &[u8],
        aad: &[u8],
        ptxt: &[u8],
    ) -> Result<HpkeCiphertext, CryptoError> {
        hpke_suite(&config)?;
        hpke_seal_with(ephemeral_ikm()?, pk_r, info, aad, ptxt)
    }

    fn hpke_open(
        &self,
        config: HpkeConfig,
        input: &HpkeCiphertext,
        sk_r: &[u8],
        info: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        hpke_suite(&config)?;
        hpke::single_shot_open::<HpkeAead, HkdfSha256, Kem>(
            &OpModeR::Base,
            &private_key(sk_r)?,
            &encapped(input.kem_output.as_slice())?,
            info,
            input.ciphertext.as_slice(),
            aad,
        )
        .map_err(|_| CryptoError::HpkeDecryptionError)
    }

    fn hpke_setup_sender_and_export(
        &self,
        config: HpkeConfig,
        pk_r: &[u8],
        info: &[u8],
        exporter_context: &[u8],
        exporter_length: usize,
    ) -> Result<(Vec<u8>, ExporterSecret), CryptoError> {
        hpke_suite(&config)?;
        hpke_export_with(
            ephemeral_ikm()?,
            pk_r,
            info,
            exporter_context,
            exporter_length,
        )
    }

    fn hpke_setup_receiver_and_export(
        &self,
        config: HpkeConfig,
        enc: &[u8],
        sk_r: &[u8],
        info: &[u8],
        exporter_context: &[u8],
        exporter_length: usize,
    ) -> Result<ExporterSecret, CryptoError> {
        hpke_suite(&config)?;
        let ctx = hpke::setup_receiver::<HpkeAead, HkdfSha256, Kem>(
            &OpModeR::Base,
            &private_key(sk_r).map_err(|_| CryptoError::ReceiverSetupError)?,
            &encapped(enc).map_err(|_| CryptoError::ReceiverSetupError)?,
            info,
        )
        .map_err(|_| CryptoError::ReceiverSetupError)?;
        let mut out = Zeroizing::new(vec![0u8; exporter_length]);
        ctx.export(exporter_context, &mut out)
            .map_err(|_| CryptoError::ExporterError)?;
        Ok(out.to_vec().into())
    }

    fn derive_hpke_keypair(
        &self,
        config: HpkeConfig,
        ikm: &[u8],
    ) -> Result<HpkeKeyPair, CryptoError> {
        hpke_suite(&config)?;
        // RFC 9180 7.1.3: keying material at least as long as the private key.
        if ikm.len() < KEY_LEN {
            return Err(CryptoError::InsufficientRandomness);
        }
        let (sk, pk) = Kem::derive_keypair(ikm);
        Ok(HpkeKeyPair {
            private: sk.to_bytes().as_slice().into(),
            public: pk.to_bytes().to_vec(),
        })
    }
}

/// The operating system's randomness failed.
#[derive(Debug, thiserror::Error)]
#[error("no randomness from the operating system")]
pub struct RandError;

impl OpenMlsRand for FogataCrypto {
    type Error = RandError;

    fn random_array<const N: usize>(&self) -> Result<[u8; N], Self::Error> {
        let mut out = [0u8; N];
        getrandom::fill(&mut out).map_err(|_| RandError)?;
        Ok(out)
    }

    fn random_vec(&self, len: usize) -> Result<Vec<u8>, Self::Error> {
        let mut out = vec![0u8; len];
        getrandom::fill(&mut out).map_err(|_| RandError)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    //! RFC 9180, appendix A.2.1: DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, ChaCha20Poly1305,
    //! base mode, the suite MLS's ciphersuite 0x0003 uses.

    use super::*;

    fn h(s: &str) -> Vec<u8> {
        hex::decode(s).unwrap()
    }

    const INFO: &str = "4f6465206f6e2061204772656369616e2055726e";
    const IKM_E: &str = "909a9b35d3dc4713a5e72a4da274b55d3d3821a37e5d099e74a647db583a904b";
    const PK_EM: &str = "1afa08d3dec047a643885163f1180476fa7ddb54c6a8029ea33f95796bf2ac4a";
    const IKM_R: &str = "1ac01f181fdf9f352797655161c58b75c656a6cc2716dcb66372da835542e1df";
    const PK_RM: &str = "4310ee97d88cc1f088a5576c77ab0cf5c3ac797f3d95139c6c84b5429c59662a";
    const SK_RM: &str = "8057991eef8f1f1af18f4a9491d16a1ce333f695d4db8e38da75975c4478e0fb";
    /// Sequence number 0: what a single-shot seal makes.
    const PT: &str = "4265617574792069732074727574682c20747275746820626561757479";
    const AAD: &str = "436f756e742d30";
    const CT: &str = "1c5250d8034ec2b784ba2cfd69dbdb8af406cfe3ff938e131f0def8c8b60b4db\
                      21993c62ce81883d2dd1b51a28";
    /// Exported values: context, value (32 bytes each).
    const EXPORTS: [(&str, &str); 3] = [
        (
            "",
            "4bbd6243b8bb54cec311fac9df81841b6fd61f56538a775e7c80a9f40160606e",
        ),
        (
            "00",
            "8c1df14732580e5501b00f82b10a1647b40713191b7c1240ac80e2b68808ba69",
        ),
        (
            "54657374436f6e74657874",
            "5acb09211139c43b3090489a9da433e8a30ee7188ba8b0a9a1ccf0c229283e53",
        ),
    ];

    fn config() -> HpkeConfig {
        HpkeConfig(
            HpkeKemType::DhKem25519,
            HpkeKdfType::HkdfSha256,
            HpkeAeadType::ChaCha20Poly1305,
        )
    }

    fn ikm_e() -> Zeroizing<[u8; KEY_LEN]> {
        Zeroizing::new(h(IKM_E).try_into().unwrap())
    }

    /// Randomness past the ephemeral key given comes from the seed drawn beforehand, never
    /// from the operating system at the moment the crate asks: a failure there was a panic.
    #[test]
    fn more_randomness_than_the_key_comes_from_a_seed_drawn_beforehand() {
        use rand_core::TryRng as _;
        let seed = [7u8; KEY_LEN];
        let (mut a, mut b) = (
            Ikm::with_seed(ikm_e(), &seed),
            Ikm::with_seed(ikm_e(), &seed),
        );
        let mut key = [0u8; KEY_LEN];
        a.try_fill_bytes(&mut key).unwrap();
        assert_eq!(key, *ikm_e());
        b.try_fill_bytes(&mut key).unwrap();
        // Past the key, and in pieces larger than one name of HKDF gives.
        let (mut x, mut y) = (vec![0u8; 10_000], vec![0u8; 10_000]);
        a.try_fill_bytes(&mut x).unwrap();
        b.try_fill_bytes(&mut y).unwrap();
        assert_eq!(x, y);
        assert!(x.iter().any(|v| *v != 0));
        assert_ne!(x[..32], x[8160..8192]);
        let mut z = vec![0u8; 10_000];
        a.try_fill_bytes(&mut z).unwrap();
        assert_ne!(x, z);
        let mut other = Ikm::with_seed(ikm_e(), &[8u8; KEY_LEN]);
        other.try_fill_bytes(&mut key).unwrap();
        other.try_fill_bytes(&mut z).unwrap();
        assert_ne!(x, z);
    }

    #[test]
    fn key_pairs_derive_as_rfc_9180_says() {
        let pair = FogataCrypto
            .derive_hpke_keypair(config(), &h(IKM_R))
            .unwrap();
        // The private key comes clamped (RFC 7748), which X25519 does to it anyway.
        let mut clamped = h(SK_RM);
        clamped[0] &= 248;
        clamped[31] &= 127;
        clamped[31] |= 64;
        assert_eq!(&pair.private[..], clamped);
        assert_eq!(pair.public, h(PK_RM));
        assert!(
            FogataCrypto
                .derive_hpke_keypair(config(), &h(IKM_R)[..31])
                .is_err()
        );
    }

    #[test]
    fn sealing_matches_rfc_9180() {
        let sealed = hpke_seal_with(ikm_e(), &h(PK_RM), &h(INFO), &h(AAD), &h(PT)).unwrap();
        assert_eq!(sealed.kem_output.as_slice(), h(PK_EM));
        assert_eq!(sealed.ciphertext.as_slice(), h(CT));
    }

    #[test]
    fn opening_matches_rfc_9180() {
        let input = HpkeCiphertext {
            kem_output: h(PK_EM).into(),
            ciphertext: h(CT).into(),
        };
        let pt = FogataCrypto
            .hpke_open(config(), &input, &h(SK_RM), &h(INFO), &h(AAD))
            .unwrap();
        assert_eq!(pt, h(PT));
        // Another aad, another key or a changed byte: nothing opens.
        assert!(
            FogataCrypto
                .hpke_open(config(), &input, &h(SK_RM), &h(INFO), b"Count-1")
                .is_err()
        );
        let mut damaged = input.clone();
        let mut ct = h(CT);
        ct[3] ^= 1;
        damaged.ciphertext = ct.into();
        assert!(
            FogataCrypto
                .hpke_open(config(), &damaged, &h(SK_RM), &h(INFO), &h(AAD))
                .is_err()
        );
        let other = FogataCrypto
            .derive_hpke_keypair(config(), &[7; 32])
            .unwrap();
        assert!(
            FogataCrypto
                .hpke_open(config(), &input, &other.private[..], &h(INFO), &h(AAD))
                .is_err()
        );
    }

    #[test]
    fn exported_values_match_rfc_9180_on_both_sides() {
        for (context, value) in EXPORTS {
            let (enc, sent) =
                hpke_export_with(ikm_e(), &h(PK_RM), &h(INFO), &h(context), 32).unwrap();
            assert_eq!(enc, h(PK_EM));
            assert_eq!(&sent[..], h(value));
            let received = FogataCrypto
                .hpke_setup_receiver_and_export(
                    config(),
                    &enc,
                    &h(SK_RM),
                    &h(INFO),
                    &h(context),
                    32,
                )
                .unwrap();
            assert_eq!(&received[..], h(value));
        }
    }

    #[test]
    fn a_fresh_seal_opens_and_differs_each_time() {
        let pair = FogataCrypto
            .derive_hpke_keypair(config(), &[9; 32])
            .unwrap();
        let a = FogataCrypto
            .hpke_seal(config(), &pair.public, b"info", b"aad", b"hi")
            .unwrap();
        let b = FogataCrypto
            .hpke_seal(config(), &pair.public, b"info", b"aad", b"hi")
            .unwrap();
        assert_ne!(a.kem_output, b.kem_output);
        let open = |c: &HpkeCiphertext| {
            FogataCrypto.hpke_open(config(), c, &pair.private[..], b"info", b"aad")
        };
        assert_eq!(open(&a).unwrap(), b"hi");
        assert_eq!(open(&b).unwrap(), b"hi");
    }

    #[test]
    fn only_the_one_suite_is_taken() {
        use openmls_traits::types::Ciphersuite as C;
        assert!(FogataCrypto.supports(CIPHERSUITE).is_ok());
        for other in [
            C::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519,
            C::MLS_128_DHKEMP256_AES128GCM_SHA256_P256,
            C::MLS_256_DHKEMX448_CHACHA20POLY1305_SHA512_Ed448,
        ] {
            assert!(FogataCrypto.supports(other).is_err());
        }
        let aes = HpkeConfig(
            HpkeKemType::DhKem25519,
            HpkeKdfType::HkdfSha256,
            HpkeAeadType::AesGcm128,
        );
        assert!(FogataCrypto.derive_hpke_keypair(aes, &[1; 32]).is_err());
        assert!(FogataCrypto.hash(HashType::Sha2_512, b"x").is_err());
        assert!(
            FogataCrypto
                .signature_key_gen(SignatureScheme::ECDSA_SECP256R1_SHA256)
                .is_err()
        );
        assert!(
            FogataCrypto
                .aead_encrypt(AeadType::Aes128Gcm, &[0; 16], b"x", &[0; 12], b"")
                .is_err()
        );
    }

    #[test]
    fn signatures_check_strictly_and_keys_have_their_length() {
        let (sk, pk) = FogataCrypto
            .signature_key_gen(SignatureScheme::ED25519)
            .unwrap();
        let sig = FogataCrypto
            .sign(SignatureScheme::ED25519, b"data", &sk)
            .unwrap();
        let verify = |data: &[u8], pk: &[u8], sig: &[u8]| {
            FogataCrypto.verify_signature(SignatureScheme::ED25519, data, pk, sig)
        };
        assert!(verify(b"data", &pk, &sig).is_ok());
        assert!(verify(b"datas", &pk, &sig).is_err());
        assert!(verify(b"data", &pk[..31], &sig).is_err());
        assert!(verify(b"data", &pk, &sig[..63]).is_err());
        // The identity point as a key: refused, as a strict check does.
        let mut identity = [0u8; 32];
        identity[0] = 1;
        assert!(verify(b"data", &identity, &[0; 64]).is_err());
        assert!(
            FogataCrypto
                .sign(SignatureScheme::ED25519, b"data", &sk[..31])
                .is_err()
        );
    }

    #[test]
    fn a_low_order_key_seals_nothing() {
        // The all-zero X25519 point gives an all-zero shared secret, which HPKE refuses.
        assert!(
            FogataCrypto
                .hpke_seal(config(), &[0; 32], b"info", b"aad", b"hi")
                .is_err()
        );
        let input = HpkeCiphertext {
            kem_output: vec![0u8; 32].into(),
            ciphertext: h(CT).into(),
        };
        assert!(
            FogataCrypto
                .hpke_open(config(), &input, &h(SK_RM), &h(INFO), &h(AAD))
                .is_err()
        );
    }
}
