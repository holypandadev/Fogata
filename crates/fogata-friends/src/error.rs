use fogata_crypto::{CryptoError, codec::DecodeError};

/// Errors from profiles, invites and friend handshakes.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FriendError {
    #[error("a name needs 1 to 40 characters that show, and no control characters")]
    BadName,
    #[error(
        "a bio has at most 190 characters on 4 lines, and no control characters but line breaks"
    )]
    BadBio,
    #[error("unknown preset avatar")]
    BadAvatar,
    #[error("the picture must be a PNG or JPEG of at most 64 KiB")]
    BadPhoto,
    #[error("an animation must be a GIF or WebP of at most 550 KiB")]
    BadAnimation,
    #[error("this is not a Fogata friend link")]
    NotALink,
    #[error("this friend link is damaged")]
    DamagedLink,
    #[error("this friend link needs a newer version of Fogata")]
    NewerLink,
    #[error("a friend code looks like FGT-XXXXX-XXXXX")]
    BadCode,
    #[error("the code is wrong or no longer valid")]
    WrongCode,
    #[error("the other device did not prove who it is")]
    NotProved,
    #[error("malformed data")]
    Malformed,
    #[error("saved by a newer version of Fogata")]
    NewerVersion,
    #[error(transparent)]
    Crypto(#[from] CryptoError),
}

impl From<DecodeError> for FriendError {
    fn from(_: DecodeError) -> Self {
        FriendError::Malformed
    }
}
