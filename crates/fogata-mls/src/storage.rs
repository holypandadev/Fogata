//! Where OpenMLS keeps a group's state: in memory, as the key/value pairs of
//! `openmls_memory_storage`, written out whole as bytes (which the store seals with the
//! folder's data key and writes atomically, one file per group). Nothing is kept anywhere
//! else: no journal, no cache, so what a new state leaves out is gone from the disk once
//! the file is replaced, which is what forward secrecy rests on.

use std::collections::HashMap;

use fogata_crypto::codec::{DecodeError, Reader, encode_secret};
use openmls_memory_storage::MemoryStorage;
use openmls_traits::OpenMlsProvider;
use zeroize::{Zeroize, Zeroizing};

use crate::provider::FogataCrypto;

/// What OpenMLS stores, as one map.
pub(crate) type Values = HashMap<Vec<u8>, Vec<u8>>;

/// One device's crypto and stored state for one group.
#[derive(Debug, Default)]
pub struct Provider {
    crypto: FogataCrypto,
    storage: MemoryStorage,
}

impl Provider {
    pub(crate) fn with_values(values: Values) -> Self {
        let provider = Self::default();
        *provider.values_mut() = values;
        provider
    }

    fn values_mut(&self) -> std::sync::RwLockWriteGuard<'_, Values> {
        self.storage
            .values
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// What [`Self::encoded`] wrote, stored again.
    pub fn from_encoded(bytes: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self::with_values(decode_values(bytes)?))
    }

    /// Everything stored, encoded.
    pub fn encoded(&self) -> Zeroizing<Vec<u8>> {
        encode_values(
            &self
                .storage
                .values
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    /// Replaces everything stored with `values`, wiping what was there.
    pub(crate) fn restore(&self, values: Values) {
        let mut stored = self.values_mut();
        wipe(&mut stored);
        *stored = values;
    }

    /// Bytes stored, keys and values.
    pub fn stored_len(&self) -> usize {
        self.storage
            .values
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|(k, v)| k.len() + v.len())
            .sum()
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        wipe(&mut self.values_mut());
    }
}

impl OpenMlsProvider for Provider {
    type CryptoProvider = FogataCrypto;
    type RandProvider = FogataCrypto;
    type StorageProvider = MemoryStorage;

    fn storage(&self) -> &Self::StorageProvider {
        &self.storage
    }

    fn crypto(&self) -> &Self::CryptoProvider {
        &self.crypto
    }

    fn rand(&self) -> &Self::RandProvider {
        &self.crypto
    }
}

fn wipe(values: &mut Values) {
    for (mut key, mut value) in values.drain() {
        key.zeroize();
        value.zeroize();
    }
}

/// `values` as bytes, in an order that does not depend on the map's.
pub(crate) fn encode_values(values: &Values) -> Zeroizing<Vec<u8>> {
    let mut pairs: Vec<(&Vec<u8>, &Vec<u8>)> = values.iter().collect();
    pairs.sort();
    // Measured first, so no copy of a secret is left behind as the buffer grows.
    encode_secret(0, |w| {
        w.u32(pairs.len() as u32);
        for (key, value) in &pairs {
            w.bytes(key).bytes(value);
        }
    })
}

pub(crate) fn read_values(r: &mut Reader<'_>) -> Result<Values, DecodeError> {
    let n = r.count(8)?;
    let mut values = Values::with_capacity(n);
    for _ in 0..n {
        let key = r.bytes()?.to_vec();
        let value = r.bytes()?.to_vec();
        if values.insert(key, value).is_some() {
            return Err(DecodeError);
        }
    }
    Ok(values)
}

pub(crate) fn decode_values(bytes: &[u8]) -> Result<Values, DecodeError> {
    let mut r = Reader::new(bytes);
    let values = read_values(&mut r)?;
    r.finish()?;
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_values_read_back_and_damage_is_refused() {
        let values: Values = [(b"a".to_vec(), vec![1; 40]), (b"bb".to_vec(), vec![])]
            .into_iter()
            .collect();
        let bytes = encode_values(&values);
        assert_eq!(decode_values(&bytes).unwrap(), values);
        // Written where it stays: no copy of a value is left behind as the buffer grows.
        assert_eq!(bytes.capacity(), bytes.len());
        let provider = Provider::with_values(values.clone());
        assert_eq!(*provider.encoded(), *bytes);
        assert_eq!(provider.stored_len(), 1 + 40 + 2);
        provider.restore(Values::new());
        assert_eq!(provider.stored_len(), 0);
        for cut in 0..bytes.len() {
            assert!(decode_values(&bytes[..cut]).is_err());
        }
        // The same key twice: refused.
        let mut w = fogata_crypto::codec::Writer::default();
        w.u32(2).bytes(b"a").bytes(b"1").bytes(b"a").bytes(b"2");
        assert!(decode_values(&w.into_bytes()).is_err());
    }
}
