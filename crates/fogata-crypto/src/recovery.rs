//! The key that opens copies of an account made to be restored without the system keychain,
//! and the encryption of such a copy.
//!
//! The recovery key is the secret half of an X25519 pair: 32 random bytes, shown once to the
//! person as 52 Crockford base32 characters in 13 groups of four and a check character, and
//! never kept by the device. The public half stays in the data folder, so a copy is made
//! without the key and only opening one asks for it. Being random and 256 bits long, the key
//! needs no slow hashing: there is no dictionary to try against it.
//!
//! A copy starts with a header in the clear ([`CopyHeader`]): the magic, the version, a
//! fingerprint of the public key (to tell a copy made for another key), the public half of a
//! pair made for this copy alone, and the day it was made; nothing of the account. The
//! copy's key is HKDF-SHA256 of the X25519 agreement between that pair and the recovery key,
//! salted with the whole header, and every frame after it is XChaCha20-Poly1305 under that
//! key, numbered by its nonce and with the whole header as associated data: a byte of the
//! header or of a frame changed, a frame dropped, repeated or put in another order, and the
//! rest does not open.

use std::fmt;

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

use crate::{CryptoError, random_bytes};

/// The start of every copy.
pub const COPY_MAGIC: [u8; 10] = *b"FOGATACOPY";
/// The version of copies this code writes and reads.
pub const COPY_VERSION: u8 = 1;
/// Bytes of a fingerprint of a recovery key's public half.
pub const FINGERPRINT_LEN: usize = 16;
/// Bytes of the header in the clear.
pub const HEADER_LEN: usize = COPY_MAGIC.len() + 1 + FINGERPRINT_LEN + 32 + 4;
/// What a frame adds to what it carries: the tag.
pub const FRAME_OVERHEAD: usize = 16;

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// Characters of a key, without its check character: 52 carry 260 bits, the key's 256 and
/// four zeros.
const KEY_CHARS: usize = 52;
const GROUP: usize = 4;
const FINGERPRINT: &[u8] = b"fogata/v1/recovery-fingerprint";
const COPY_KEY: &[u8] = b"fogata/v1/recovery-copy";
const AUTH: &[u8] = b"fogata/v1/recovery-auth";
/// Bytes of a copy's proof.
pub const PROOF_LEN: usize = 32;

/// What a device that holds the account's [`RecoveryKey::auth_key`] puts in a copy whose
/// header is `header`: an HMAC-SHA256 of the header, and with it of the pair made for that
/// copy alone, whose secret only the device that made it knew. Someone with the public half
/// only can make a copy of its own, but not this proof for it.
pub fn copy_proof(auth: &[u8; 32], header: &[u8]) -> [u8; PROOF_LEN] {
    let mut mac =
        <Hmac<Sha256> as hmac::KeyInit>::new_from_slice(auth).expect("HMAC takes any key length");
    mac.update(header);
    mac.finalize().into_bytes().into()
}

/// Whether `proof` is [`copy_proof`] of `header` under `auth`, compared in a time that does
/// not depend on where they differ.
pub fn proves(auth: &[u8; 32], header: &[u8], proof: &[u8]) -> bool {
    let mut mac =
        <Hmac<Sha256> as hmac::KeyInit>::new_from_slice(auth).expect("HMAC takes any key length");
    mac.update(header);
    mac.verify_slice(proof).is_ok()
}

/// Why typed text is not a recovery key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeyProblem {
    #[error("a recovery key has 53 letters and digits, in groups of four and one at the end")]
    NotAKey,
    #[error("that isn't the whole recovery key: check the letters and digits")]
    Mistyped,
}

/// Why a copy does not open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CopyProblem {
    #[error("this is not a copy of a Fogata account")]
    NotACopy,
    #[error("this copy was made by a newer version of Fogata; update Fogata to open it")]
    NewerVersion,
    #[error("this copy was made for another recovery key")]
    OtherKey,
    #[error("this copy is damaged or was changed")]
    Damaged,
    /// It opens with the key, but was not made on a computer that held the account: someone
    /// who knew only the key's public half made it.
    #[error("this copy was not made by this account")]
    Unproven,
}

/// The secret half of a recovery pair. Wiped from memory when dropped.
#[derive(Clone, PartialEq, Eq)]
pub struct RecoveryKey(Zeroizing<[u8; 32]>);

impl fmt::Debug for RecoveryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryKey(..)")
    }
}

impl RecoveryKey {
    pub fn generate() -> Result<Self, CryptoError> {
        let mut bytes = Zeroizing::new([0u8; 32]);
        random_bytes(bytes.as_mut())?;
        Ok(Self(bytes))
    }

    /// The public half, which the data folder keeps.
    pub fn public(&self) -> RecoveryPublic {
        RecoveryPublic(x25519_dalek::PublicKey::from(&self.secret()).to_bytes())
    }

    /// The X25519 secret, which wipes itself when dropped; the copy of the bytes it is made
    /// from is wiped here.
    fn secret(&self) -> x25519_dalek::StaticSecret {
        let mut bytes = *self.0;
        let secret = x25519_dalek::StaticSecret::from(bytes);
        bytes.zeroize();
        secret
    }

    /// The key that proves a copy was made by a device that holds the account
    /// ([`copy_proof`]): the device keeps it sealed with its data key beside the public half,
    /// so the public half alone, which a backup of the folder may show, makes no copy that a
    /// restore takes.
    pub fn auth_key(&self) -> Zeroizing<[u8; 32]> {
        let mut out = Zeroizing::new([0u8; 32]);
        Hkdf::<Sha256>::new(Some(AUTH), &self.0[..])
            .expand(b"auth", out.as_mut())
            .expect("HKDF output length is far below the limit");
        out
    }

    /// The key as the person writes it down: 13 groups of four characters and the check
    /// character, `XXXX XXXX ... XXXX C`.
    pub fn text(&self) -> Zeroizing<String> {
        let chars = encode(&self.0);
        let mut text = Zeroizing::new(String::with_capacity(KEY_CHARS + KEY_CHARS / GROUP + 1));
        for (i, c) in chars.iter().enumerate() {
            if i > 0 && i % GROUP == 0 {
                text.push(' ');
            }
            text.push(*c as char);
        }
        text.push(' ');
        text.push(check_char(&chars[..]) as char);
        text
    }

    /// Reads a key as people type it: spaces and dashes anywhere, any case, and O, I and L
    /// read as 0, 1 and 1.
    pub fn parse(text: &str) -> Result<Self, KeyProblem> {
        // Wiped when dropped; never grown, so no copy is left behind either.
        let mut chars = Zeroizing::new(Vec::with_capacity(KEY_CHARS + 1));
        for c in text.chars() {
            if c == '-' || c.is_whitespace() {
                continue;
            }
            if chars.len() == KEY_CHARS + 1 {
                return Err(KeyProblem::NotAKey);
            }
            let c = match c.to_ascii_uppercase() {
                'O' => b'0',
                'I' | 'L' => b'1',
                c if c.is_ascii() && CROCKFORD.contains(&(c as u8)) => c as u8,
                _ => return Err(KeyProblem::NotAKey),
            };
            chars.push(c);
        }
        if chars.len() != KEY_CHARS + 1 {
            return Err(KeyProblem::NotAKey);
        }
        if check_char(&chars[..KEY_CHARS]) != chars[KEY_CHARS] {
            return Err(KeyProblem::Mistyped);
        }
        decode(&chars[..KEY_CHARS])
            .map(Self)
            .ok_or(KeyProblem::Mistyped)
    }
}

/// The public half of a recovery pair.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RecoveryPublic(pub [u8; 32]);

impl fmt::Debug for RecoveryPublic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RecoveryPublic({})", hex(&self.fingerprint()))
    }
}

impl RecoveryPublic {
    /// What a copy's header names its key by.
    pub fn fingerprint(&self) -> [u8; FINGERPRINT_LEN] {
        let digest = Sha256::new()
            .chain_update(FINGERPRINT)
            .chain_update(self.0)
            .finalize();
        digest[..FINGERPRINT_LEN].try_into().expect("16 bytes")
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The 52 characters of a key: its 256 bits and four zeros, five bits a character.
fn encode(key: &[u8; 32]) -> Zeroizing<[u8; KEY_CHARS]> {
    let mut out = Zeroizing::new([0u8; KEY_CHARS]);
    let mut acc: u32 = 0;
    let mut bits = 0;
    let mut at = 0;
    for byte in key.iter().chain(std::iter::once(&0u8)) {
        acc = (acc << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 && at < KEY_CHARS {
            bits -= 5;
            out[at] = CROCKFORD[((acc >> bits) & 31) as usize];
            at += 1;
        }
        acc &= (1 << bits) - 1;
    }
    acc.zeroize();
    out
}

/// The key of 52 characters, when the four bits past it are zeros.
fn decode(chars: &[u8]) -> Option<Zeroizing<[u8; 32]>> {
    let mut out = Zeroizing::new([0u8; 32]);
    let mut acc: u32 = 0;
    let mut bits = 0;
    let mut at = 0;
    for c in chars {
        let value = CROCKFORD.iter().position(|x| x == c)? as u32;
        acc = (acc << 5) | value;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            if at < 32 {
                out[at] = (acc >> bits) as u8;
                at += 1;
            }
            acc &= (1 << bits) - 1;
        }
    }
    let rest = acc;
    acc.zeroize();
    (at == 32 && bits == 4 && rest == 0).then_some(out)
}

/// The check character of a key's characters, as for the code that moves an account: their
/// values weighted 1, 3, 5, ..., summed modulo 32. Odd weights catch every single wrong
/// character, and most pairs of neighbours swapped.
fn check_char(chars: &[u8]) -> u8 {
    let sum: usize = chars
        .iter()
        .enumerate()
        .map(|(i, c)| (2 * i + 1) * CROCKFORD.iter().position(|x| x == c).unwrap_or(0))
        .sum();
    CROCKFORD[sum % 32]
}

/// What a copy says in the clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CopyHeader {
    /// [`RecoveryPublic::fingerprint`] of the key it was made for.
    pub fingerprint: [u8; FINGERPRINT_LEN],
    /// The public half of the pair made for this copy alone.
    pub ephemeral: [u8; 32],
    /// The day it was made, counted from 1 January 1970.
    pub day: u32,
}

impl CopyHeader {
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        let mut at = 0;
        for part in [
            &COPY_MAGIC[..],
            &[COPY_VERSION],
            &self.fingerprint,
            &self.ephemeral,
            &self.day.to_be_bytes(),
        ] {
            out[at..at + part.len()].copy_from_slice(part);
            at += part.len();
        }
        out
    }

    /// Reads a header; `bytes` may be shorter than one (a file that is not a copy).
    pub fn decode(bytes: &[u8]) -> Result<Self, CopyProblem> {
        let (magic, rest) = bytes
            .split_at_checked(COPY_MAGIC.len())
            .ok_or(CopyProblem::NotACopy)?;
        if magic != COPY_MAGIC {
            return Err(CopyProblem::NotACopy);
        }
        match rest.first() {
            Some(&COPY_VERSION) => {}
            Some(v) if *v > COPY_VERSION => return Err(CopyProblem::NewerVersion),
            _ => return Err(CopyProblem::NotACopy),
        }
        if bytes.len() < HEADER_LEN {
            return Err(CopyProblem::Damaged);
        }
        let rest = &rest[1..];
        Ok(Self {
            fingerprint: rest[..16].try_into().expect("16 bytes"),
            ephemeral: rest[16..48].try_into().expect("32 bytes"),
            day: u32::from_be_bytes(rest[48..52].try_into().expect("4 bytes")),
        })
    }
}

fn copy_key(shared: &x25519_dalek::SharedSecret, header: &[u8; HEADER_LEN]) -> XChaCha20Poly1305 {
    let mut key = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(Some(header), shared.as_bytes())
        .expand(COPY_KEY, key.as_mut())
        .expect("HKDF output length is far below the limit");
    XChaCha20Poly1305::new_from_slice(&key[..]).expect("a key of 32 bytes")
}

fn frame_nonce(n: u64) -> XNonce {
    let mut nonce = [0u8; 24];
    nonce[16..].copy_from_slice(&n.to_be_bytes());
    XNonce::from(nonce)
}

/// Seals the frames of a new copy, in order.
pub struct CopySealer {
    cipher: XChaCha20Poly1305,
    header: [u8; HEADER_LEN],
    next: u64,
}

impl fmt::Debug for CopySealer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CopySealer(..)")
    }
}

impl CopySealer {
    /// A new copy for the key whose public half is `public`, made on `day`: its sealer and
    /// the header that starts it.
    pub fn new(public: &RecoveryPublic, day: u32) -> Result<Self, CryptoError> {
        let mut secret = Zeroizing::new([0u8; 32]);
        random_bytes(secret.as_mut())?;
        let mut bytes = *secret;
        let secret = x25519_dalek::StaticSecret::from(bytes);
        bytes.zeroize();
        let shared = secret.diffie_hellman(&x25519_dalek::PublicKey::from(public.0));
        if !shared.was_contributory() {
            return Err(CryptoError::InvalidKey);
        }
        let header = CopyHeader {
            fingerprint: public.fingerprint(),
            ephemeral: x25519_dalek::PublicKey::from(&secret).to_bytes(),
            day,
        }
        .encode();
        Ok(Self {
            cipher: copy_key(&shared, &header),
            header,
            next: 0,
        })
    }

    /// The header the copy starts with.
    pub fn header(&self) -> &[u8; HEADER_LEN] {
        &self.header
    }

    pub fn seal(&mut self, plain: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let nonce = frame_nonce(self.next);
        self.next += 1;
        self.cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plain,
                    aad: &self.header,
                },
            )
            .map_err(|_| CryptoError::Malformed)
    }
}

/// Opens the frames of a copy, in order.
pub struct CopyOpener {
    cipher: XChaCha20Poly1305,
    header: [u8; HEADER_LEN],
    next: u64,
}

impl fmt::Debug for CopyOpener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CopyOpener(..)")
    }
}

impl CopyOpener {
    /// The opener of the copy that starts with `header`, with `key`.
    pub fn new(header: &[u8], key: &RecoveryKey) -> Result<Self, CopyProblem> {
        let read = CopyHeader::decode(header)?;
        if read.fingerprint != key.public().fingerprint() {
            return Err(CopyProblem::OtherKey);
        }
        let header: [u8; HEADER_LEN] = header[..HEADER_LEN].try_into().expect("checked");
        let secret = key.secret();
        let shared = secret.diffie_hellman(&x25519_dalek::PublicKey::from(read.ephemeral));
        if !shared.was_contributory() {
            return Err(CopyProblem::Damaged);
        }
        Ok(Self {
            cipher: copy_key(&shared, &header),
            header,
            next: 0,
        })
    }

    /// The header the copy starts with.
    pub fn header(&self) -> &[u8; HEADER_LEN] {
        &self.header
    }

    pub fn open(&mut self, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, CopyProblem> {
        let plain = self
            .cipher
            .decrypt(
                &frame_nonce(self.next),
                Payload {
                    msg: sealed,
                    aad: &self.header,
                },
            )
            .map_err(|_| CopyProblem::Damaged)?;
        self.next += 1;
        Ok(Zeroizing::new(plain))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_reads_back_as_written_and_a_slip_is_told() {
        for _ in 0..50 {
            let key = RecoveryKey::generate().unwrap();
            let text = key.text();
            assert_eq!(text.len(), 13 * 5 + 1);
            assert_eq!(text.split(' ').count(), 14);
            assert_eq!(RecoveryKey::parse(&text).unwrap(), key);
            // As people type it.
            let typed = text.to_lowercase().replace(' ', "-");
            assert_eq!(RecoveryKey::parse(&typed).unwrap(), key);
            // Every single character wrong is told.
            let chars: Vec<char> = text.chars().filter(|c| *c != ' ').collect();
            for at in 0..chars.len() {
                let mut wrong = chars.clone();
                wrong[at] = if wrong[at] == 'Z' { 'Y' } else { 'Z' };
                let wrong: String = wrong.into_iter().collect();
                assert_eq!(RecoveryKey::parse(&wrong), Err(KeyProblem::Mistyped));
            }
            // A character left out or one too many.
            assert_eq!(
                RecoveryKey::parse(&text[1..]),
                Err(KeyProblem::NotAKey),
                "{}",
                &text[..]
            );
            assert_eq!(
                RecoveryKey::parse(&format!("{}0", &text[..])),
                Err(KeyProblem::NotAKey)
            );
        }
        assert_eq!(RecoveryKey::parse("hello"), Err(KeyProblem::NotAKey));
        assert_eq!(RecoveryKey::parse(""), Err(KeyProblem::NotAKey));
    }

    /// The four bits past the key are zeros: a last character that sets them is not a key,
    /// so two texts never stand for one key.
    #[test]
    fn only_one_text_stands_for_a_key() {
        let key = RecoveryKey::generate().unwrap();
        let mut chars: Vec<u8> = key
            .text()
            .bytes()
            .filter(|c| *c != b' ')
            .take(KEY_CHARS)
            .collect();
        let last = CROCKFORD.iter().position(|c| *c == chars[51]).unwrap();
        assert_eq!(last & 0b1111, 0);
        chars[51] = CROCKFORD[last | 1];
        let check = check_char(&chars);
        chars.push(check);
        let text = String::from_utf8(chars).unwrap();
        assert_eq!(RecoveryKey::parse(&text), Err(KeyProblem::Mistyped));
    }

    fn sealed_copy(key: &RecoveryKey) -> (Vec<u8>, Vec<Vec<u8>>) {
        let mut sealer = CopySealer::new(&key.public(), 20_000).unwrap();
        let frames = (0..3u8)
            .map(|i| sealer.seal(&[i; 40]).unwrap())
            .collect::<Vec<_>>();
        (sealer.header().to_vec(), frames)
    }

    #[test]
    fn a_copy_opens_with_its_key_only_and_as_it_was_written() {
        let key = RecoveryKey::generate().unwrap();
        let (header, frames) = sealed_copy(&key);
        let read = CopyHeader::decode(&header).unwrap();
        assert_eq!(read.day, 20_000);
        assert_eq!(read.fingerprint, key.public().fingerprint());
        let mut opener = CopyOpener::new(&header, &key).unwrap();
        for (i, frame) in frames.iter().enumerate() {
            assert_eq!(&opener.open(frame).unwrap()[..], &[i as u8; 40]);
        }
        // Another key.
        let other = RecoveryKey::generate().unwrap();
        assert_eq!(
            CopyOpener::new(&header, &other).unwrap_err(),
            CopyProblem::OtherKey
        );
        // Frames in another order, repeated, or one left out.
        let mut opener = CopyOpener::new(&header, &key).unwrap();
        assert!(opener.open(&frames[1]).is_err());
        let mut opener = CopyOpener::new(&header, &key).unwrap();
        opener.open(&frames[0]).unwrap();
        assert!(opener.open(&frames[0]).is_err());
        assert!(opener.open(&frames[2]).is_err());
        // Any byte of a frame changed.
        let mut changed = frames[0].clone();
        changed[5] ^= 1;
        assert!(
            CopyOpener::new(&header, &key)
                .unwrap()
                .open(&changed)
                .is_err()
        );
        // Any byte of the header changed: not a copy, or nothing opens with it. Even the day,
        // which is bound to every frame.
        for at in 0..HEADER_LEN {
            let mut changed = header.clone();
            changed[at] ^= 1;
            let opened = CopyOpener::new(&changed, &key).and_then(|mut o| o.open(&frames[0]));
            assert!(opened.is_err(), "header byte {at}");
        }
    }

    #[test]
    fn only_the_key_s_holder_proves_a_copy() {
        let key = RecoveryKey::generate().unwrap();
        let auth = key.auth_key();
        assert_eq!(*auth, *RecoveryKey::parse(&key.text()).unwrap().auth_key());
        assert_ne!(*auth, *RecoveryKey::generate().unwrap().auth_key());
        assert_ne!(&auth[..], &key.public().0[..]);
        let (header, _) = sealed_copy(&key);
        let proof = copy_proof(&auth, &header);
        assert!(proves(&auth, &header, &proof));
        // Another copy's header, another key, or a proof changed or cut.
        let (other, _) = sealed_copy(&key);
        assert!(!proves(&auth, &other, &proof));
        assert!(!proves(
            &RecoveryKey::generate().unwrap().auth_key(),
            &header,
            &proof
        ));
        let mut changed = proof;
        changed[0] ^= 1;
        assert!(!proves(&auth, &header, &changed));
        assert!(!proves(&auth, &header, &proof[..16]));
    }

    #[test]
    fn the_header_says_nothing_but_what_it_must() {
        let key = RecoveryKey::generate().unwrap();
        let (header, _) = sealed_copy(&key);
        assert_eq!(header.len(), HEADER_LEN);
        assert!(header.starts_with(b"FOGATACOPY\x01"));
        // A file that is not a copy, a newer one, and one cut inside its header.
        assert_eq!(
            CopyHeader::decode(b"PK\x03\x04").unwrap_err(),
            CopyProblem::NotACopy
        );
        let mut newer = header.clone();
        newer[COPY_MAGIC.len()] = 2;
        assert_eq!(
            CopyHeader::decode(&newer).unwrap_err(),
            CopyProblem::NewerVersion
        );
        assert_eq!(
            CopyHeader::decode(&header[..20]).unwrap_err(),
            CopyProblem::Damaged
        );
        // Each copy has its own pair, so two copies share nothing past the fingerprint.
        let (again, _) = sealed_copy(&key);
        assert_eq!(header[..27], again[..27]);
        assert_ne!(header[27..59], again[27..59]);
    }
}
