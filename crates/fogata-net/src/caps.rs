//! Capability levels: which version of Fogata's protocols a device speaks.
//!
//! Every device says its level when it connects, in the protocols it lists (identify), as
//! [`caps_protocol`] of its level: older copies, which list none, are level 1. Nothing else
//! goes on the wire, so older copies only see one more protocol name they ignore.
//!
//! The node remembers the highest level each friend and member was seen at, once it knows
//! which device a peer is (a proof of membership, a friend's session); the application keeps
//! that list across restarts ([`crate::NodeEvent::LevelSeen`],
//! [`crate::NodeHandle::remember_levels`]). A feature that keeps something safe asks
//! [`decide`] whether to use it with a peer: a device that once spoke a level is never
//! believed when it later says it is older, so a modified copy cannot pretend to be old to
//! get the weaker behaviour. A group's level is the lowest of its members'.

use std::collections::HashSet;

use libp2p::StreamProtocol;

/// The level of this version. A level of judgment of group logs comes with the capability
/// level of its number (see `fogata_state::Level`), so the level a device lists also says
/// the levels it judges at: every one up to it. That is how a manager knows that every
/// member's copy judges at a level before it raises a group to it.
pub const CAPS_LEVEL: u32 = 3;

// This version judges at every level up to the one it lists.
const _: () = assert!(fogata_state::Level::CURRENT.number() <= CAPS_LEVEL);

/// Whether a device seen at capability level `seen` judges at `level`.
pub fn judges_at(seen: u32, level: fogata_state::Level) -> bool {
    seen >= level.number()
}

/// The level of a device that does not say one: every version before levels.
pub const FIRST_LEVEL: u32 = 1;

/// Highest level taken from what a peer lists; anything above is noise, not a version.
pub(crate) const MAX_LEVEL: u32 = 1000;

const PREFIX: &str = "/fogata/caps/";

/// The protocol a device of `level` lists.
pub fn caps_protocol(level: u32) -> StreamProtocol {
    StreamProtocol::try_from_owned(format!("{PREFIX}{level}")).expect("starts with a slash")
}

/// The level a peer says it speaks, from the protocols it listed: the highest it names, or
/// [`FIRST_LEVEL`] for a peer that names none.
pub(crate) fn level_listed(protocols: &HashSet<StreamProtocol>) -> u32 {
    protocols
        .iter()
        .filter_map(|p| p.as_ref().strip_prefix(PREFIX))
        .filter_map(|n| n.parse::<u32>().ok())
        .filter(|n| (FIRST_LEVEL..=MAX_LEVEL).contains(n))
        .max()
        .unwrap_or(FIRST_LEVEL)
}

/// What to do about a feature that came with level `since`, with a peer that says `now`
/// and whose device was seen at `seen` before.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The peer has it: use it.
    Use,
    /// Neither the peer nor its device ever had it: do as older versions do.
    Older,
    /// The device had it before and now says it has not: do neither. For a feature that
    /// keeps something safe, the older way is exactly what someone pretending to be old
    /// wants.
    Refuse,
}

/// See [`Decision`]. `now` is `None` until the peer said what it speaks.
pub fn decide(seen: u32, now: Option<u32>, since: u32) -> Decision {
    match now {
        _ if since <= FIRST_LEVEL => Decision::Use,
        Some(now) if now >= since => Decision::Use,
        _ if seen >= since => Decision::Refuse,
        Some(_) => Decision::Older,
        // Not known yet: nothing to fall back on.
        None => Decision::Refuse,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(names: &[&str]) -> HashSet<StreamProtocol> {
        names
            .iter()
            .map(|n| StreamProtocol::try_from_owned(n.to_string()).unwrap())
            .collect()
    }

    #[test]
    fn a_peer_speaks_the_highest_level_it_names() {
        assert_eq!(level_listed(&listed(&["/fogata/member/1"])), FIRST_LEVEL);
        assert_eq!(
            level_listed(&listed(&["/fogata/member/1", "/fogata/caps/2"])),
            2
        );
        assert_eq!(
            level_listed(&listed(&["/fogata/caps/3", "/fogata/caps/2"])),
            3
        );
        // Noise is no level.
        assert_eq!(
            level_listed(&listed(&[
                "/fogata/caps/x",
                "/fogata/caps/0",
                "/fogata/caps/99999999",
                "/fogata/caps/-2",
                "/fogata/caps/"
            ])),
            FIRST_LEVEL
        );
        assert_eq!(caps_protocol(CAPS_LEVEL).as_ref(), "/fogata/caps/3");
        assert_eq!(
            level_listed(&listed(&[caps_protocol(CAPS_LEVEL).as_ref()])),
            CAPS_LEVEL
        );
    }

    #[test]
    fn a_device_seen_newer_is_never_taken_for_older() {
        // A feature of level 3.
        assert_eq!(decide(1, Some(3), 3), Decision::Use);
        assert_eq!(decide(3, Some(3), 3), Decision::Use);
        assert_eq!(decide(1, Some(2), 3), Decision::Older);
        assert_eq!(decide(2, Some(1), 3), Decision::Older);
        // Seen at 3 before, says 2 now.
        assert_eq!(decide(3, Some(2), 3), Decision::Refuse);
        assert_eq!(decide(4, Some(1), 3), Decision::Refuse);
        // Has not said yet.
        assert_eq!(decide(1, None, 3), Decision::Refuse);
        // What every version had needs no level.
        assert_eq!(decide(5, Some(1), FIRST_LEVEL), Decision::Use);
        assert_eq!(decide(5, None, FIRST_LEVEL), Decision::Use);
    }
}
