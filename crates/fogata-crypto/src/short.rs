//! Sealing a few bytes into as little room as possible, for values that must fit where
//! there is no space for a full AEAD nonce and tag (such as one DNS label): an 8-byte random
//! nonce, the bytes XORed with a keystream, and an 8-byte tag over the nonce and the result.
//! Keystream and tag are HMAC-SHA256 under two keys derived from one 32-byte secret: the
//! keystream's first 32 bytes are the HMAC of the nonce, the next 32 that of the nonce and a
//! one.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::{CryptoError, derive_secret, random_bytes};

/// Most bytes [`seal_short`] takes: a signature.
pub const SHORT_MAX: usize = 64;
/// Bytes [`seal_short`] adds.
pub const SHORT_OVERHEAD: usize = NONCE + TAG;

const NONCE: usize = 8;
const TAG: usize = 8;

fn mac(key: &[u8; 32]) -> Hmac<Sha256> {
    <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC takes any key length")
}

/// [`SHORT_MAX`] bytes of keystream for `nonce`.
fn keystream(stream_key: &[u8; 32], nonce: &[u8]) -> Zeroizing<[u8; SHORT_MAX]> {
    let mut pad = Zeroizing::new([0u8; SHORT_MAX]);
    for (i, block) in pad.chunks_mut(32).enumerate() {
        let mut stream = mac(stream_key);
        stream.update(nonce);
        if i > 0 {
            stream.update(&[i as u8]);
        }
        block.copy_from_slice(&stream.finalize().into_bytes());
    }
    pad
}

fn keys(secret: &[u8; 32], context: &[u8]) -> (Zeroizing<[u8; 32]>, Zeroizing<[u8; 32]>) {
    let mut info = context.to_vec();
    info.extend_from_slice(b"/stream");
    let stream = Zeroizing::new(derive_secret(b"fogata/v1/short", secret, &info));
    info.truncate(context.len());
    info.extend_from_slice(b"/tag");
    let tag = Zeroizing::new(derive_secret(b"fogata/v1/short", secret, &info));
    (stream, tag)
}

/// Seals `plain` (at most [`SHORT_MAX`] bytes) under `secret` for `context`.
pub fn seal_short(secret: &[u8; 32], context: &[u8], plain: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if plain.len() > SHORT_MAX {
        return Err(CryptoError::Malformed);
    }
    let (stream_key, tag_key) = keys(secret, context);
    let mut out = vec![0u8; NONCE];
    random_bytes(&mut out)?;
    let pad = keystream(&stream_key, &out);
    out.extend(plain.iter().zip(pad.iter()).map(|(p, k)| p ^ k));
    let mut tag = mac(&tag_key);
    tag.update(&out);
    out.extend_from_slice(&tag.finalize().into_bytes()[..TAG]);
    Ok(out)
}

/// Opens what [`seal_short`] made with the same secret and context.
pub fn open_short(
    secret: &[u8; 32],
    context: &[u8],
    sealed: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if sealed.len() < SHORT_OVERHEAD || sealed.len() > SHORT_OVERHEAD + SHORT_MAX {
        return Err(CryptoError::Malformed);
    }
    let (body, tag) = sealed.split_at(sealed.len() - TAG);
    let (stream_key, tag_key) = keys(secret, context);
    let mut check = mac(&tag_key);
    check.update(body);
    check
        .verify_truncated_left(tag)
        .map_err(|_| CryptoError::Decrypt)?;
    let (nonce, cipher) = body.split_at(NONCE);
    let pad = keystream(&stream_key, nonce);
    Ok(cipher.iter().zip(pad.iter()).map(|(c, k)| c ^ k).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_with_the_same_secret_and_context_only() {
        let secret = [7u8; 32];
        let sealed = seal_short(&secret, b"lan", b"\xc0\xa8\x28\x03").unwrap();
        assert_eq!(sealed.len(), 4 + SHORT_OVERHEAD);
        assert_eq!(
            open_short(&secret, b"lan", &sealed).unwrap(),
            b"\xc0\xa8\x28\x03"
        );
        assert!(open_short(&[8u8; 32], b"lan", &sealed).is_err());
        assert!(open_short(&secret, b"other", &sealed).is_err());
        let mut changed = sealed.clone();
        changed[NONCE] ^= 1;
        assert!(open_short(&secret, b"lan", &changed).is_err());
        // A fresh nonce every time: the same bytes do not seal the same.
        assert_ne!(
            seal_short(&secret, b"lan", b"\xc0\xa8\x28\x03").unwrap(),
            sealed
        );
    }

    #[test]
    fn sizes_are_bounded() {
        let secret = [1u8; 32];
        assert!(seal_short(&secret, b"x", &[0; SHORT_MAX + 1]).is_err());
        let full = seal_short(&secret, b"x", &[9; SHORT_MAX]).unwrap();
        assert_eq!(open_short(&secret, b"x", &full).unwrap(), [9; SHORT_MAX]);
        assert!(open_short(&secret, b"x", &full[..SHORT_OVERHEAD - 1]).is_err());
        let empty = seal_short(&secret, b"x", &[]).unwrap();
        assert!(open_short(&secret, b"x", &empty).unwrap().is_empty());
        // The second half of a long one is not the first's keystream again.
        let zeros = seal_short(&secret, b"x", &[0; SHORT_MAX]).unwrap();
        assert_ne!(zeros[NONCE..NONCE + 32], zeros[NONCE + 32..NONCE + 64]);
    }
}
