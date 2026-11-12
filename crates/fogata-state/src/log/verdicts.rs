//! What a log judged, to take the same entries again without judging them again.
//!
//! Judging a log whole costs as much as its entries were made to cost: 4096 entries each
//! signed on parents nobody else is signed on take seconds in a release build, and a member
//! may bring a group there a few entries at a time. A device reads its logs back at every
//! start; judging them again there would cost as much each time. So what it judged of a log
//! is kept with it ([`Verdicts`]): which of the entries read were refused and which still wait
//! for parents. Read back with the same entries, by a version that judges as the one that
//! wrote it, the others are taken as judged, and none is judged again. With any other entries
//! (a log changed, cut short or written by another version), or another way of judging, the
//! verdicts count for nothing and the entries are judged as they come.
//!
//! The verdicts say what is kept: whoever may write them may make this device keep entries
//! their authors could not make. Their keeper seals them (see `fogata-core`'s store).

use std::collections::{BTreeSet, HashSet};

use fogata_crypto::{
    GroupId,
    codec::{DecodeError, Reader, Writer},
};
use sha2::{Digest, Sha256};

use super::{GroupLog, Taken, Work, level_ahead_of};
use crate::{entry::Entry, entry::EntryHash, state::Level};

/// How judging goes, apart from the levels: raised whenever what any level keeps of the same
/// entries changes, so that verdicts written before count for nothing.
const JUDGING: u32 = 1;

/// What [`Verdicts`] start with, and their format.
const MAGIC: &[u8; 8] = b"fgjudged";
const FORMAT: u8 = 1;

/// What a log judged of the entries it was read from or written with
/// ([`GroupLog::verdicts`]), to read them back without judging them again
/// ([`GroupLog::read_judged`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdicts {
    /// The highest level the code that judged them knows, and how it judges apart from that.
    level: u32,
    judging: u32,
    group_id: GroupId,
    /// A digest of the hashes of the entries read, whatever their order.
    read: [u8; 32],
    /// Entries read that the log did not keep.
    refused: BTreeSet<EntryHash>,
    /// Entries read that the log kept without judging them: their parents never came.
    waiting: BTreeSet<EntryHash>,
}

impl Verdicts {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.raw(MAGIC)
            .u8(FORMAT)
            .u32(self.level)
            .u32(self.judging)
            .group(&self.group_id)
            .raw(&self.read);
        for set in [&self.refused, &self.waiting] {
            w.u32(set.len() as u32);
            for hash in set {
                w.raw(hash);
            }
        }
        w.into_bytes()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        if r.take(MAGIC.len())? != MAGIC || r.u8()? != FORMAT {
            return Err(DecodeError);
        }
        let level = r.u32()?;
        let judging = r.u32()?;
        let group_id = r.group()?;
        let read = r.array()?;
        let mut set = || -> Result<BTreeSet<EntryHash>, DecodeError> {
            let n = r.count(32)?;
            (0..n).map(|_| r.array()).collect()
        };
        let refused = set()?;
        let waiting = set()?;
        r.finish()?;
        Ok(Self {
            level,
            judging,
            group_id,
            read,
            refused,
            waiting,
        })
    }

    /// Whether these are verdicts on `read`, entries of `group_id`, judged as this code
    /// judges.
    fn on(&self, group_id: GroupId, read: &BTreeSet<EntryHash>) -> bool {
        self.level == Level::CURRENT.number()
            && self.judging == JUDGING
            && self.group_id == group_id
            && self.read == digest(group_id, read)
    }
}

/// A digest of the set of hashes `read`, of entries of `group_id`.
fn digest(group_id: GroupId, read: &BTreeSet<EntryHash>) -> [u8; 32] {
    let mut sha = Sha256::new();
    sha.update(MAGIC);
    sha.update(group_id.as_bytes());
    sha.update((read.len() as u64).to_be_bytes());
    for hash in read {
        sha.update(hash);
    }
    sha.finalize().into()
}

impl GroupLog {
    /// What this log judged of `read`, the hashes of the entries it was read from (all of its
    /// entries, for a log about to be written whole).
    pub fn verdicts(&self, read: impl IntoIterator<Item = EntryHash>) -> Verdicts {
        let read: BTreeSet<EntryHash> = read.into_iter().collect();
        Verdicts {
            level: Level::CURRENT.number(),
            judging: JUDGING,
            group_id: self.group_id,
            read: digest(self.group_id, &read),
            refused: read
                .iter()
                .filter(|h| !self.entries.contains_key(*h))
                .copied()
                .collect(),
            waiting: read
                .iter()
                .filter(|h| self.unjudged.contains(*h))
                .copied()
                .collect(),
        }
    }

    /// The log of `group_id` made of `entries`, as [`Self::insert_all`] makes it from nothing.
    /// When `verdicts` are this code's on the very same entries, those it kept are taken
    /// without judging them, and with no signature checked again when `signed` (the verdicts
    /// came from where nobody else writes); those it refused are left out. Otherwise each is
    /// judged as it comes, and the verdicts on them are returned, to keep in their place.
    pub fn read_judged(
        group_id: GroupId,
        entries: Vec<Entry>,
        verdicts: Option<&Verdicts>,
        signed: bool,
    ) -> (Self, Option<Verdicts>) {
        let mut log = Self::empty(group_id);
        let read: BTreeSet<EntryHash> = entries.iter().map(Entry::hash).collect();
        let Some(verdicts) = verdicts.filter(|v| v.on(group_id, &read)) else {
            log.insert_all(entries);
            let verdicts = log.verdicts(read);
            return (log, Some(verdicts));
        };
        let judged = read
            .iter()
            .filter(|h| !verdicts.refused.contains(*h) && !verdicts.waiting.contains(*h))
            .copied()
            .collect();
        // A `Level` entry above this version's levels is judged again, though it was
        // refused: refused again, it is what tells the person to update
        // ([`GroupLog::level_ahead`]). One entry, judged once.
        let entries = entries
            .into_iter()
            .filter(|e| !verdicts.refused.contains(&e.hash()) || level_ahead_of(e).is_some());
        log.insert_batch(
            entries,
            Work {
                judged,
                signed,
                ..Work::unbounded()
            },
        );
        (log, None)
    }

    /// Takes the entries of `from`, a log of the same group: those it judged and kept are
    /// taken without judging them again, nor checking their signatures (whatever a log holds
    /// was judged or is waiting to be, in this process). Judging depends on nothing but an
    /// entry and those before it, so they are kept here as there. The others (those waiting
    /// there for parents this log may have) are judged as a batch from another device is
    /// ([`Self::insert_some`]).
    pub fn insert_judged(&mut self, from: &GroupLog) -> Taken {
        let judged: HashSet<EntryHash> = from
            .entries
            .keys()
            .filter(|h| !from.unjudged.contains(*h))
            .copied()
            .collect();
        self.insert_batch(
            from.entries_in_order().into_iter().cloned(),
            Work {
                views: super::MAX_VIEWS_PER_BATCH,
                judged,
                signed: true,
                ..Work::unbounded()
            },
        )
    }
}
