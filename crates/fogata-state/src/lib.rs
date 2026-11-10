//! Group state for Fogata: a log of signed membership changes that every member keeps, and
//! the rules that turn it into the current members, their ladder and key epoch; and the group's
//! room, which lives only in memory. Messages and files are never part of the log. Nothing
//! here performs I/O.

#![forbid(unsafe_code)]

mod activity;
mod banner;
mod budget;
mod cache;
pub mod chain;
mod entry;
mod log;
mod picture;
mod record;
mod room;
mod state;
#[cfg(any(test, feature = "counts"))]
#[doc(hidden)]
pub mod work;

pub use activity::{Activity, Did, UndoneBy};
pub use banner::{BannerKind, GroupBanner, MAX_BANNER_BYTES, is_banner_picture};
pub use budget::{MAX_RUN_BYTES, MAX_RUN_ENTRIES, PAST_RUN};
pub use chain::{Chain, Link, MIN_UPDATE_GAP_MS, Verdict};
pub use entry::{
    Body, Changes, Entry, EntryHash, EpochId, EpochLink, FIRST_MLS_KIND, FIRST_ROOM, InviteToken,
    MAX_KEY_PACKAGE_BYTES, PayloadDigest, Rotation, Ticket, VoiceRoomId, commitment, epoch_number,
    invite_rendezvous, payload_digest,
};
pub use fogata_crypto::codec::DecodeError;
pub use log::{
    GroupLog, JOIN_ON_SPARE, KeyProblem, RAISE_LEVEL_ONLY, StateError, TICKET_EXPIRED, Taken,
    Verdicts,
};
pub use picture::{
    GroupPicture, MAX_RECORD_PICTURE_BYTES, PictureKind, is_group_picture, is_record_picture,
};
pub use record::{GroupRecord, MAX_RECORD_AHEAD_MS, RecordKind, RecordStamp};
pub use room::{
    AWAY_MS, ChatLine, EVENT_SLACK_BYTES, FileInfo, GRACE_MS, HERE_INTERVAL_MS, MAX_CHAT_CHARS,
    MAX_JOIN_BYTES, MAX_SHARE_BYTES, MAX_SHARES_EACH, MAX_THUMB_BYTES, Participant, Room,
    RoomChange, RoomEvent, RoomFile, RoomId, RoomInput, RoomTracker, VideoPreview,
};
pub use state::{
    Barrier, Caps, Epoch, GroupState, KeptOut, LEVEL_NOT_HIGHER, LEVEL_UNKNOWN, Level,
    MAX_GROUP_NAME_CHARS, MAX_IN_ROOM, MAX_MEMBER_NAME_CHARS, MAX_MEMBERS, MAX_PICTURE_BYTES,
    MAX_ROOM_NAME_CHARS, MAX_ROOMS, MLS_NOT_YET, Managers, Member, ROTATE_NOT_NEEDED, RoomRules,
    VoiceRoom, WELCOME_NOT_MANAGER, fair_name, hiding_mark,
};
