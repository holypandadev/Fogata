//! Canonical binary encoding for signed data such as group log entries and profiles: the
//! same value always encodes to the same bytes, which is what signatures and hashes are
//! computed over. Also used on disk and on the wire.

use crate::{DevicePublicKey, GroupId, NONCE_LEN, Sealed};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("malformed data")]
pub struct DecodeError;

/// Writes values in the canonical encoding. One made with [`Writer::secret`] holds secrets:
/// it never leaves a copy of them behind as it grows (see [`Writer::measure`]).
#[derive(Debug, Default)]
pub struct Writer {
    bytes: Vec<u8>,
    /// Only counts what would be written ([`Writer::measure`]).
    counting: bool,
    counted: usize,
    /// What it held is wiped when it grows.
    wipe: bool,
}

impl Writer {
    /// A writer that only counts how many bytes an encoding takes, so that one of secrets
    /// can be written into a [`Writer::secret`] of exactly that size, which never grows.
    pub fn measure() -> Self {
        Self {
            counting: true,
            ..Self::default()
        }
    }

    /// A writer for an encoding of secrets, with room for `len` bytes: past that it grows,
    /// wiping what it held where it was.
    pub fn secret(len: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(len),
            wipe: true,
            ..Self::default()
        }
    }

    /// How many bytes are written (or counted) so far.
    pub fn len(&self) -> usize {
        if self.counting {
            self.counted
        } else {
            self.bytes.len()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Adds `v`, the one place bytes go in.
    fn put(&mut self, v: &[u8]) -> &mut Self {
        if self.counting {
            self.counted += v.len();
            return self;
        }
        let needed = self.bytes.len() + v.len();
        if self.wipe && needed > self.bytes.capacity() {
            let mut grown = Vec::with_capacity(needed.max(2 * self.bytes.capacity()));
            grown.extend_from_slice(&self.bytes);
            zeroize::Zeroize::zeroize(&mut self.bytes);
            self.bytes = grown;
        }
        self.bytes.extend_from_slice(v);
        self
    }

    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.put(&[v])
    }

    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.put(&v.to_be_bytes())
    }

    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.put(&v.to_be_bytes())
    }

    pub fn raw(&mut self, v: &[u8]) -> &mut Self {
        self.put(v)
    }

    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.u32(v.len() as u32).raw(v)
    }

    pub fn str(&mut self, v: &str) -> &mut Self {
        self.bytes(v.as_bytes())
    }

    pub fn key(&mut self, v: &DevicePublicKey) -> &mut Self {
        self.raw(v.as_bytes())
    }

    pub fn group(&mut self, v: &GroupId) -> &mut Self {
        self.raw(v.as_bytes())
    }

    pub fn sealed(&mut self, v: &Sealed) -> &mut Self {
        self.raw(&v.ephemeral).raw(&v.nonce).bytes(&v.ciphertext)
    }
}

/// Encodes secrets with `write`, called twice: once to measure them, then into a buffer of
/// exactly that size and `extra` bytes more, so no copy of them is ever left behind.
pub fn encode_secret(extra: usize, write: impl Fn(&mut Writer)) -> zeroize::Zeroizing<Vec<u8>> {
    let mut size = Writer::measure();
    write(&mut size);
    let mut w = Writer::secret(size.len() + extra);
    write(&mut w);
    zeroize::Zeroizing::new(w.into_bytes())
}

#[derive(Debug)]
pub struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }

    pub fn finish(&self) -> Result<(), DecodeError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(DecodeError)
        }
    }

    /// Whether everything was read.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// How many bytes are left to read.
    pub fn remaining(&self) -> usize {
        self.0.len()
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.0.len() < n {
            return Err(DecodeError);
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }

    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        Ok(self.take(N)?.try_into().expect("exact length"))
    }

    pub fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.array::<1>()?[0])
    }

    pub fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    pub fn bytes(&mut self) -> Result<&'a [u8], DecodeError> {
        let n = self.u32()? as usize;
        self.take(n)
    }

    pub fn str(&mut self) -> Result<String, DecodeError> {
        String::from_utf8(self.bytes()?.to_vec()).map_err(|_| DecodeError)
    }

    pub fn key(&mut self) -> Result<DevicePublicKey, DecodeError> {
        DevicePublicKey::from_bytes(&self.array::<32>()?).map_err(|_| DecodeError)
    }

    pub fn group(&mut self) -> Result<GroupId, DecodeError> {
        GroupId::from_bytes(&self.array::<16>()?).map_err(|_| DecodeError)
    }

    pub fn sealed(&mut self) -> Result<Sealed, DecodeError> {
        Ok(Sealed {
            ephemeral: self.array()?,
            nonce: self.array::<NONCE_LEN>()?,
            ciphertext: self.bytes()?.to_vec(),
        })
    }

    /// Reads a count, refusing counts that cannot fit in the remaining bytes.
    pub fn count(&mut self, min_item_len: usize) -> Result<usize, DecodeError> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min_item_len.max(1)) > self.0.len() {
            return Err(DecodeError);
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(w: &mut Writer) {
        w.u8(1)
            .u32(2)
            .u64(3)
            .bytes(b"secreto")
            .str("Marta")
            .raw(&[9; 40]);
    }

    /// Measured first, an encoding of secrets goes where it stays, of exactly its size; one
    /// past the room it was given still comes out whole.
    #[test]
    fn secrets_are_written_where_they_stay() {
        let mut plain = Writer::default();
        write(&mut plain);
        let plain = plain.into_bytes();
        let mut size = Writer::measure();
        write(&mut size);
        assert_eq!(size.len(), plain.len());
        assert!(size.into_bytes().is_empty());
        let mut secret = Writer::secret(plain.len());
        write(&mut secret);
        let bytes = secret.into_bytes();
        assert_eq!((bytes.capacity(), &bytes), (plain.len(), &plain));
        let encoded = encode_secret(3, write);
        assert_eq!(encoded.as_slice(), plain.as_slice());
        assert_eq!(encoded.capacity(), plain.len() + 3);
        // Too little room: it grows, and what it had moves whole.
        let mut small = Writer::secret(4);
        write(&mut small);
        assert_eq!(small.into_bytes(), plain);
    }
}
