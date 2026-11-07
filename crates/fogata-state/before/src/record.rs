//! Pictures of a group that a manager signs apart from the log: its banner
//! ([`crate::GroupBanner`]) and its picture ([`crate::GroupPicture`]). Only the newest of each
//! is kept, so changing one often grows nothing, and one taken away is gone from every device
//! that gets the change. They go to the members that say they take them (see
//! `fogata_proto::MemberRequest::Roster`).

use std::marker::PhantomData;

use fogata_crypto::{
    DeviceIdentity, DevicePublicKey, GroupId, SIGNATURE_LEN,
    codec::{DecodeError, Reader, Writer},
};
use sha2::{Digest, Sha256};

const ENCODING_VERSION: u8 = 1;

/// What one kind of record is: what its signature covers first, and the pictures it holds.
pub trait RecordKind {
    /// Signed before the record's fields, so that a record of one kind never reads as one
    /// of another.
    const LABEL: &'static [u8];
    /// Largest picture, in bytes.
    const MAX_BYTES: usize;
    /// Whether `bytes` start like a picture this kind holds.
    fn takes(bytes: &[u8]) -> bool;
}

/// A group's picture of kind `K`, signed by the manager who chose it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupRecord<K> {
    pub group_id: GroupId,
    pub author: DevicePublicKey,
    /// When it was chosen, by the author's clock: the newest one shows.
    pub time_ms: u64,
    /// The picture; empty when the manager took it away.
    pub picture: Vec<u8>,
    pub signature: [u8; SIGNATURE_LEN],
    kind: PhantomData<K>,
}

impl<K: RecordKind> GroupRecord<K> {
    pub fn sign(
        identity: &DeviceIdentity,
        group_id: GroupId,
        picture: Vec<u8>,
        time_ms: u64,
    ) -> Result<Self, DecodeError> {
        let mut record = Self {
            group_id,
            author: identity.public_key(),
            time_ms,
            picture,
            signature: [0; SIGNATURE_LEN],
            kind: PhantomData,
        };
        if !record.fits() {
            return Err(DecodeError);
        }
        record.signature = identity.sign(&record.signed_bytes());
        Ok(record)
    }

    fn fits(&self) -> bool {
        self.picture.is_empty() || (self.picture.len() <= K::MAX_BYTES && K::takes(&self.picture))
    }

    fn write_body(&self, w: &mut Writer) {
        w.group(&self.group_id)
            .key(&self.author)
            .u64(self.time_ms)
            .bytes(&self.picture);
    }

    fn signed_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.raw(K::LABEL);
        self.write_body(&mut w);
        w.into_bytes()
    }

    /// Whether the author signed it and the picture is one of this kind. Whether the author
    /// may choose it is the log's to say.
    pub fn verify(&self) -> bool {
        self.fits()
            && self
                .author
                .verify(&self.signed_bytes(), &self.signature)
                .is_ok()
    }

    /// Whether this one shows rather than `other`: the newer, or of two at the same time the
    /// one with the larger signature, so every member settles on the same.
    pub fn replaces(&self, other: &Self) -> bool {
        (self.time_ms, self.signature) > (other.time_ms, other.signature)
    }

    /// A short name of this record: two members with the same one have the same picture.
    pub fn digest(&self) -> [u8; 16] {
        let hash = Sha256::digest(self.encode());
        hash[..16].try_into().expect("16 of 32 bytes")
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u8(ENCODING_VERSION);
        self.write_body(&mut w);
        w.raw(&self.signature);
        w.into_bytes()
    }

    /// Decodes a record and checks its signature.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        if r.u8()? != ENCODING_VERSION {
            return Err(DecodeError);
        }
        let record = Self {
            group_id: r.group()?,
            author: r.key()?,
            time_ms: r.u64()?,
            picture: r.bytes()?.to_vec(),
            signature: r.array()?,
            kind: PhantomData,
        };
        r.finish()?;
        if !record.verify() {
            return Err(DecodeError);
        }
        Ok(record)
    }
}
