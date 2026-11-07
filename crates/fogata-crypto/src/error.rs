/// Errors returned by the cryptographic operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CryptoError {
    #[error("the operating system's random number generator failed")]
    Random,
    #[error("invalid public key")]
    InvalidKey,
    #[error("invalid signature")]
    BadSignature,
    #[error("membership proof does not match the group")]
    BadProof,
    #[error("message belongs to another group")]
    WrongGroup,
    #[error("keys of epoch {0} are needed")]
    WrongEpoch(u32),
    #[error("unsupported message version {0}")]
    UnsupportedVersion(u8),
    #[error("message could not be decrypted")]
    Decrypt,
    #[error("malformed input")]
    Malformed,
}
