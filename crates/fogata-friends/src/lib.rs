//! Profiles and friends for Fogata: the signed profile a device shows, the friend secret that
//! lets friends find it, invite links and short codes, and the messages that make two devices
//! friends and let them recognise each other later.
//!
//! Nothing in this crate performs I/O apart from reading the operating system's random number
//! generator.

#![forbid(unsafe_code)]

mod animation;
mod banner;
mod bio;
mod book;
mod code;
mod error;
mod handshake;
mod link;
mod mutual;
mod profile;
mod secret;
mod transfer;

/// Entry points for fuzzing what friends send (see fogata-core's `fuzzing`).
#[cfg(feature = "fuzzing")]
pub use handshake::fuzz as fuzzing;

pub use animation::{
    Animated, Animation, MAX_ANIMATION_FRAMES, MAX_BANNER_ANIMATION_BYTES,
    MAX_BANNER_ANIMATION_PIXELS, MAX_PHOTO_ANIMATION_BYTES, MAX_PHOTO_ANIMATION_PIXELS,
    is_animation, still_digest,
};
pub use banner::{Banner, MAX_BANNER_BYTES};
pub use bio::{Bio, MAX_BIO_CHARS, MAX_BIO_LINES, clean_bio};
pub use book::{
    Departing, Friend, FriendBook, Incoming, MAX_PREVIOUS, Outgoing, Previous, RequestOutcome,
};
pub use code::{CODE_LIFETIME_MS, CodeSecrets, FriendCode, MAX_FAILED_RUNS};
pub use error::FriendError;
pub use handshake::{
    CallMessage, CallSignal, CodeInviter, CodeReply, CodeRequester, CodeStart, Envelope, HelloFrom,
    HelloReply, Introduction, Status, accept, animation_update, banner_update, bio_update,
    call_message, code_gate_ok, code_reply, code_start, hello, hello_reply, link_request,
    member_request, mutual_ask, mutual_reply, open_accept, open_animation_update,
    open_banner_update, open_bio_update, open_call_message, open_hello, open_hello_reply,
    open_link_request, open_member_request, open_mutual_ask, open_mutual_reply,
    open_profile_update, open_status, open_unfriend, open_unfriend_ack, profile_update,
    removal_digest, status, unfriend, unfriend_ack,
};
pub use link::{FriendLink, LINK_LIFETIME_MS, LINK_PREFIX, MAX_LINK_ADDRS};
pub use mutual::{MAX_MUTUAL_POINTS, MutualAnswer, MutualAsk, Point, mutual_answer};
pub use profile::{
    Avatar, MAX_NAME_CHARS, MAX_PHOTO_BYTES, PRESET_AVATARS, Profile, clean_name, is_picture,
};
pub use secret::FriendSecret;
pub use transfer::{
    CodeProblem, FrameOpener, FrameSealer, MAX_FAILED_TRANSFER_RUNS, ReceiverRun, RunError,
    RunKeys, RunReply, RunStart, SenderRun, TRANSFER_CODE_LIFETIME_MS, TransferCode, run_reply,
    run_start,
};
