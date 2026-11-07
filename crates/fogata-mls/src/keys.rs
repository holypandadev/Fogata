//! Fogata's keys from MLS epochs ("exporter mode"): each epoch's secret is exported from MLS
//! and every key above it is derived as for any group ([`GroupKeys::derive`]). The epoch's
//! id is the start of MLS's epoch authenticator, which no two epochs share, not even two
//! branches of a fork with the same epoch number.
//!
//! The secrets of past epochs are kept for a while, so that messages sealed under them still
//! open (late ones, or ones sent on a branch that lost), and then deleted for good: that
//! window is all a copy of the device's data reads of the past.

use fogata_crypto::{GroupId, GroupKeys, GroupSecret};
use fogata_state::{EpochId, epoch_number};

/// The label MLS's exporter is asked for an epoch's secret under.
pub const EXPORT_LABEL: &str = "fogata/v3/epoch";

/// Past epochs whose keys are kept, besides the current one.
pub const KEPT_EPOCHS: usize = 3;

/// How long a past epoch's keys are kept once the next one started.
pub const KEPT_FOR_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// One epoch's secret, as exported from MLS.
#[derive(Debug)]
pub struct EpochSecret {
    pub id: EpochId,
    pub secret: GroupSecret,
    /// When this device reached the epoch.
    pub since_ms: u64,
}

impl EpochSecret {
    /// The keys of this epoch for `group`.
    pub fn keys(&self, group: GroupId) -> GroupKeys {
        GroupKeys::derive(group, epoch_number(&self.id), &self.secret)
    }
}

/// The current epoch's secret and the past ones still kept, newest last.
#[derive(Debug, Default)]
pub struct KeptSecrets {
    secrets: Vec<EpochSecret>,
}

impl KeptSecrets {
    pub fn current(&self) -> Option<&EpochSecret> {
        self.secrets.last()
    }

    pub fn all(&self) -> &[EpochSecret] {
        &self.secrets
    }

    /// The device reached epoch `secret` (again, after undoing a branch that lost: it then
    /// moves to the end, the current one).
    pub fn reached(&mut self, secret: EpochSecret) {
        self.secrets.retain(|s| s.id != secret.id);
        self.secrets.push(secret);
    }

    /// Deletes what is past keeping at `now_ms`: all but the newest [`KEPT_EPOCHS`] past
    /// epochs, and any past epoch the next one replaced more than [`KEPT_FOR_MS`] ago.
    pub fn prune(&mut self, now_ms: u64) {
        let n = self.secrets.len();
        let keep: Vec<bool> = (0..n)
            .map(|i| match self.secrets.get(i + 1) {
                None => true,
                Some(next) => {
                    i + 1 + KEPT_EPOCHS >= n && now_ms.saturating_sub(next.since_ms) < KEPT_FOR_MS
                }
            })
            .collect();
        let mut keep = keep.into_iter();
        self.secrets.retain(|_| keep.next().unwrap_or(true));
    }

    pub(crate) fn from_vec(secrets: Vec<EpochSecret>) -> Self {
        Self { secrets }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(n: u8, since_ms: u64) -> EpochSecret {
        EpochSecret {
            id: [n; 16],
            secret: GroupSecret::from_bytes(&[n; 32]).unwrap(),
            since_ms,
        }
    }

    #[test]
    fn past_epochs_go_after_three_more_or_a_week() {
        let day = 24 * 60 * 60 * 1000;
        let mut kept = KeptSecrets::default();
        for n in 0..6u8 {
            kept.reached(secret(n, u64::from(n) * day));
            kept.prune(u64::from(n) * day);
        }
        let ids = |k: &KeptSecrets| k.all().iter().map(|s| s.id[0]).collect::<Vec<_>>();
        assert_eq!(ids(&kept), vec![2, 3, 4, 5]);
        // A week after epoch 5 replaced 4, only 5 is left: the current one always stays.
        kept.prune(5 * day + KEPT_FOR_MS - 1);
        assert_eq!(ids(&kept), vec![4, 5]);
        kept.prune(5 * day + KEPT_FOR_MS);
        assert_eq!(ids(&kept), vec![5]);
        kept.prune(u64::MAX);
        assert_eq!(ids(&kept), vec![5]);
        // Back on an epoch it had left: it is the current one again.
        kept.reached(secret(9, 6 * day));
        kept.reached(secret(5, 7 * day));
        assert_eq!(ids(&kept), vec![9, 5]);
        let keys = kept
            .current()
            .unwrap()
            .keys(GroupId::from_bytes(&[1; 16]).unwrap());
        assert_eq!(keys.epoch(), epoch_number(&[5; 16]));
    }
}
