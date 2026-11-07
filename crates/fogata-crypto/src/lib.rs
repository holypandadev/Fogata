//! Cryptographic building blocks for Fogata: device identity, the group key schedule,
//! membership proofs, sealed group messages, and what the device keeps sealed on its disk.
//!
//! Nothing in this crate performs I/O apart from reading the operating system's random
//! number generator, and the readers and writers it is handed to seal or open a stream. It also holds what every other crate checks names with ([`names`]),
//! being the one they all build on.

#![forbid(unsafe_code)]

pub mod at_rest;
pub mod codec;
mod error;
mod group;
mod identity;
mod message;
pub mod names;
mod proof;
pub mod recovery;
mod seal;
mod short;

pub use error::CryptoError;
pub use group::{
    GROUP_SECRET_LEN, GroupId, GroupKeys, GroupSecret, WINDOW_SECS, derive_secret,
    rendezvous_point, window_at,
};
pub use identity::crockford_encode;
pub use identity::{DeviceIdentity, DevicePublicKey, SIGNATURE_LEN};
pub use message::{MESSAGE_VERSION, MessageHeader, NONCE_LEN, SealedMessage};
pub use proof::{
    CHALLENGE_LEN, GROUP_TAG_LEN, MembershipProof, SEALED_PROOF_LEN, SealedProof, group_tag,
    group_tag_matches, new_challenge,
};
pub use seal::Sealed;
pub use short::{SHORT_MAX, SHORT_OVERHEAD, open_short, seal_short};

/// Fills `buf` with bytes from the operating system's secure random number generator.
pub fn random_bytes(buf: &mut [u8]) -> Result<(), CryptoError> {
    getrandom::fill(buf).map_err(|_| CryptoError::Random)
}
