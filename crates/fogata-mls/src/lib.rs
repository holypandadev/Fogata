//! MLS (RFC 9420) for Fogata's group keys, in "exporter mode": OpenMLS keeps each group's
//! ratchet tree and moves it from epoch to epoch with Commits that ride in the group's log,
//! and Fogata takes each epoch's secret from MLS's exporter; every key above it (messages,
//! proofs, rendezvous, rooms) is derived as before. Nothing here performs I/O: the state of
//! a group comes and goes as bytes, which the store seals with the folder's data key.

#![forbid(unsafe_code)]

mod identity;
#[cfg(test)]
mod interop;
mod keys;
mod member;
mod provider;
mod storage;

pub use identity::{MlsSigner, device_of};
pub use keys::{EXPORT_LABEL, EpochSecret, KEPT_EPOCHS, KEPT_FOR_MS, KeptSecrets};
pub use member::{
    ChainLink, Followed, GroupMember, MAX_SNAPSHOTS, MlsError, Proposed, SNAPSHOT_FOR_MS,
    read_key_package,
};
pub use provider::{CIPHERSUITE, FogataCrypto, RandError};
pub use storage::Provider;
