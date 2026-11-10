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

/// How far ahead of a member's clock a record may be dated and still be taken: clocks
/// differ by less, and one dated later would show over every newer choice until then.
pub const MAX_RECORD_AHEAD_MS: u64 = 24 * 60 * 60 * 1000;

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

/// When a record was chosen, and its signature: what orders two records of one kind (see
/// [`GroupRecord::replaces`]). [`GroupRecord::peek`] reads it without checking the
/// signature, so a device that keeps a newer record drops an older one without that work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RecordStamp {
    pub time_ms: u64,
    pub signature: [u8; SIGNATURE_LEN],
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
        self.stamp() > other.stamp()
    }

    /// When it was chosen, and its signature.
    pub fn stamp(&self) -> RecordStamp {
        RecordStamp {
            time_ms: self.time_ms,
            signature: self.signature,
        }
    }

    /// Whether it is dated more than [`MAX_RECORD_AHEAD_MS`] after `now_ms`: one a member
    /// does not take.
    pub fn from_the_future(&self, now_ms: u64) -> bool {
        self.time_ms > now_ms.saturating_add(MAX_RECORD_AHEAD_MS)
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

    /// The stamp of an encoded record, read without checking its signature or copying its
    /// picture: whether [`Self::decode`] takes it is still to be seen, but bytes it fails
    /// to read as a record fail here too.
    pub fn peek(bytes: &[u8]) -> Result<RecordStamp, DecodeError> {
        let mut r = Reader::new(bytes);
        if r.u8()? != ENCODING_VERSION {
            return Err(DecodeError);
        }
        r.group()?;
        r.key()?;
        let time_ms = r.u64()?;
        r.bytes()?;
        let signature = r.array()?;
        r.finish()?;
        Ok(RecordStamp { time_ms, signature })
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

#[cfg(test)]
mod tests {
    use fogata_crypto::{DeviceIdentity, GroupId};

    use crate::GroupBanner;

    /// A peek reads what orders records whatever the signature, and refuses what a decode
    /// cannot read as a record either.
    #[test]
    fn a_peek_reads_the_stamp_without_the_signature() {
        let identity = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        let banner = GroupBanner::sign(&identity, group, b"GIF89a".to_vec(), 5).unwrap();
        let bytes = banner.encode();
        assert_eq!(GroupBanner::peek(&bytes), Ok(banner.stamp()));
        // Signed by nobody: decoding refuses it, and a peek still tells it is older.
        let mut forged = banner.clone();
        forged.time_ms = 4;
        assert!(GroupBanner::decode(&forged.encode()).is_err());
        let stamp = GroupBanner::peek(&forged.encode()).unwrap();
        assert!(stamp < banner.stamp());
        // The order of stamps is the order of records.
        let later = GroupBanner::sign(&identity, group, Vec::new(), 5).unwrap();
        assert_eq!(later.replaces(&banner), later.stamp() > banner.stamp());
        for broken in [
            &bytes[..bytes.len() - 1],
            &[bytes.as_slice(), &[0]].concat(),
            &[&[2], &bytes[1..]].concat(),
            &[][..],
        ] {
            assert!(GroupBanner::peek(broken).is_err());
            assert!(GroupBanner::decode(broken).is_err());
        }
    }
}
