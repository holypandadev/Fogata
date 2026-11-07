//! Who did what in a group, from its log: every member has the same log, so every member
//! sees the same list. An entry that applied is something done; one outranked by a loss of
//! its author (a removal, a move to the end or a leave it had not seen, see
//! [`GroupLog::state`]) was done and then undone, and says by which loss. Entries refused for
//! any other reason never happened, and neither do keys handed out or rotations.

use fogata_crypto::DevicePublicKey;

use crate::{
    budget::PAST_RUN,
    entry::{Body, EntryHash, VoiceRoomId},
    log::GroupLog,
    state::{GroupState, Managers},
};

/// Something a member did in the group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Did {
    /// Started the group, with this name.
    Created {
        name: String,
    },
    Renamed {
        name: String,
    },
    /// Chose one of the preset icons.
    SetIcon,
    /// Set the picture with an entry, as versions before picture records did; or took it
    /// away.
    SetPicture {
        taken_away: bool,
    },
    Added {
        member: DevicePublicKey,
    },
    /// Joined with an invite of `invited_by`.
    Joined {
        invited_by: DevicePublicKey,
    },
    /// Let in a member that joined with an invite: its first key.
    LetIn {
        member: DevicePublicKey,
    },
    Removed {
        member: DevicePublicKey,
    },
    Left,
    /// Moved `member` up to just below itself, to `place` (0 for the first).
    MovedUp {
        member: DevicePublicKey,
        place: usize,
    },
    /// Moved `member` to the end.
    MovedDown {
        member: DevicePublicKey,
    },
    Managers {
        managers: Managers,
    },
    Limits {
        members: u8,
        room: u8,
    },
    /// Opened a voice room called `name`.
    RoomOpened {
        name: String,
    },
    /// Renamed voice room `from` to `name`.
    RoomRenamed {
        from: String,
        name: String,
    },
    RoomClosed {
        name: String,
    },
    /// Wrote an invite.
    Invited,
    /// Cancelled an invite.
    Revoked,
    /// Who may enter voice room `room`: everyone, or only `members` (and the author and
    /// those above it).
    RoomAccess {
        room: String,
        members: Option<Vec<DevicePublicKey>>,
    },
    /// Kept `member` out of voice room `room` until `until_ms` by the author's clock, or
    /// until let back.
    KeptOut {
        room: String,
        member: DevicePublicKey,
        until_ms: Option<u64>,
    },
    LetBack {
        room: String,
        member: DevicePublicKey,
    },
}

/// How a loss undid what its device did at the same time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndoneBy {
    /// The author was removed, by `by`.
    Removed { by: DevicePublicKey },
    /// The author left.
    Left,
    /// The author was moved to the end, by `by`.
    MovedDown { by: DevicePublicKey },
}

/// One line of a group's activity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Activity {
    pub entry: EntryHash,
    pub author: DevicePublicKey,
    /// When, by the author's clock.
    pub time_ms: u64,
    pub did: Did,
    /// Done, then undone by a loss of its author it had not seen.
    pub undone: Option<UndoneBy>,
}

impl GroupLog {
    /// What the members did, in the common order, the oldest first (see
    /// [`crate::activity`]).
    pub fn activity(&self) -> Vec<Activity> {
        let (settled, outranked) = self.settled();
        let loss = |hash: &EntryHash| {
            let entry = self.get(hash)?;
            Some(match &entry.body {
                Body::Remove { .. } => UndoneBy::Removed { by: entry.author },
                Body::Leave => UndoneBy::Left,
                Body::Demote { .. } => UndoneBy::MovedDown { by: entry.author },
                _ => return None,
            })
        };
        // The room rules, as they settled against the whole group, each told where it is
        // in the common order.
        let rules = self.state();
        let room = |id: &VoiceRoomId| {
            rules
                .rooms
                .iter()
                .find(|r| r.id == *id)
                .map(|r| r.name.clone())
                .unwrap_or_default()
        };
        let mut rule_did: std::collections::BTreeMap<EntryHash, Did> = Default::default();
        for hash in &rules.applied {
            let Some(entry) = self.get(hash) else {
                continue;
            };
            let did = match &entry.body {
                Body::RoomAccess { room: r, members } => Did::RoomAccess {
                    room: room(r),
                    members: members.clone(),
                },
                Body::KeepOut {
                    room: r,
                    member,
                    until_ms,
                } => Did::KeptOut {
                    room: room(r),
                    member: *member,
                    until_ms: *until_ms,
                },
                Body::LetBack { room: r, member } => Did::LetBack {
                    room: room(r),
                    member: *member,
                },
                _ => continue,
            };
            rule_did.insert(*hash, did);
        }
        let mut out = Vec::new();
        // The main entries, step by step, each against the group just before it: the same
        // steps as `settled`, which they are checked against.
        let over = &self.full().budget.over;
        let mut state = GroupState::default();
        for hash in self.ordered() {
            let entry = self.get(&hash).expect("ordered entries are in the log");
            if entry.body.is_room_rule() {
                if let Some(did) = rule_did.remove(&hash) {
                    out.push(Activity {
                        entry: hash,
                        author: entry.author,
                        time_ms: entry.time_ms,
                        did,
                        undone: None,
                    });
                }
                continue;
            }
            let before = state.clone();
            let undone = outranked.get(&hash).and_then(loss);
            if over.contains(&hash) {
                state.refuse(hash, PAST_RUN);
                continue;
            }
            if undone.is_some() {
                state.refuse(hash, "signed apart from its author's removal");
            } else {
                let needed = state
                    .rotation_to_judge(entry)
                    .then(|| self.rotation_needed(&hash));
                let place = &self.full().order.place;
                let seen = |h: &EntryHash| self.is_before(place, &entry.parents, h);
                state.apply(hash, entry, needed, &seen);
                if state.applied.last() != Some(&hash) {
                    continue;
                }
                // A `Welcome` that let a device in applied its `Join` just before it.
                if state.applied.len() == before.applied.len() + 2 {
                    let hash = state.applied[state.applied.len() - 2];
                    if let Some(join) = self.get(&hash)
                        && let Body::Join { ticket, .. } = &join.body
                    {
                        out.push(Activity {
                            entry: hash,
                            author: join.author,
                            time_ms: join.time_ms,
                            did: Did::Joined {
                                invited_by: ticket.issuer,
                            },
                            undone: None,
                        });
                    }
                }
            }
            // An outranked entry is told as it would have been, on the group before it.
            let after = if undone.is_some() {
                let mut tried = before.clone();
                let place = &self.full().order.place;
                let seen = |h: &EntryHash| self.is_before(place, &entry.parents, h);
                tried.apply(hash, entry, Some(true), &seen);
                tried
            } else {
                state.clone()
            };
            if let Some(did) = did(&entry.body, &before, &after) {
                out.push(Activity {
                    entry: hash,
                    author: entry.author,
                    time_ms: entry.time_ms,
                    did,
                    undone,
                });
            }
        }
        debug_assert_eq!(state.applied, settled.applied);
        out
    }
}

/// What `body` did, given the group just `before` it and just `after`.
fn did(body: &Body, before: &GroupState, after: &GroupState) -> Option<Did> {
    let room_name = |state: &GroupState, id: &VoiceRoomId| {
        state
            .rooms
            .iter()
            .find(|r| r.id == *id)
            .map(|r| r.name.clone())
    };
    Some(match body {
        Body::Create { .. } => Did::Created {
            name: after.name.clone(),
        },
        Body::Rename { .. } => Did::Renamed {
            name: after.name.clone(),
        },
        Body::SetIcon { .. } => Did::SetIcon,
        Body::SetPicture { picture } => Did::SetPicture {
            taken_away: picture.is_none(),
        },
        Body::Add { member, .. } => Did::Added { member: *member },
        Body::Join { ticket, .. } => Did::Joined {
            invited_by: ticket.issuer,
        },
        // Only the key that lets in a member that joined: the others are upkeep.
        Body::Welcome { member, .. } if !before.keyed(member) => Did::LetIn { member: *member },
        Body::Remove { member, .. } => Did::Removed { member: *member },
        Body::Leave => Did::Left,
        Body::Promote { member } => Did::MovedUp {
            member: *member,
            place: after.rank(member).unwrap_or_default(),
        },
        Body::Demote { member } => Did::MovedDown { member: *member },
        Body::SetManagers { .. } => Did::Managers {
            managers: after.managers,
        },
        Body::SetLimits { members, room } => Did::Limits {
            members: *members,
            room: *room,
        },
        Body::SetRoom { id, .. } => {
            let name = room_name(after, id).unwrap_or_default();
            match room_name(before, id) {
                None => Did::RoomOpened { name },
                Some(from) => Did::RoomRenamed { from, name },
            }
        }
        Body::RemoveRoom { id } => Did::RoomClosed {
            name: room_name(before, id).unwrap_or_default(),
        },
        Body::Invite { .. } => Did::Invited,
        Body::Revoke { .. } => Did::Revoked,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use fogata_crypto::DeviceIdentity;

    use super::*;
    use crate::entry::FIRST_ROOM;

    fn ids(n: usize) -> Vec<DeviceIdentity> {
        (0..n)
            .map(|_| DeviceIdentity::generate().unwrap())
            .collect()
    }

    #[test]
    fn the_activity_tells_who_did_what_in_order() {
        let d = ids(4);
        let k: Vec<DevicePublicKey> = d.iter().map(|i| i.public_key()).collect();
        let mut log = GroupLog::create(&d[0], "Sundays", 1).unwrap();
        log.add(&d[0], k[1], "Bea", 2).unwrap();
        log.add(&d[0], k[2], "Leo", 3).unwrap();
        let (ticket, _) = log.invite(&d[1], 1_000, 4).unwrap();
        let join = crate::entry::Entry::sign(
            &d[3],
            log.group_id(),
            log.heads(),
            5,
            Body::Join {
                ticket,
                name: "Sofía".into(),
            },
        );
        log.insert(join).unwrap();
        log.welcome(&d[1], k[3], 6).unwrap();
        log.append(&d[0], Body::Promote { member: k[2] }, 7)
            .unwrap();
        log.append(
            &d[0],
            Body::Rename {
                name: "Mondays".into(),
            },
            8,
        )
        .unwrap();
        let room = |id: u8, name: &str| Body::SetRoom {
            id: [id; 8],
            name: name.into(),
        };
        log.append(&d[0], room(1, "Fun"), 9).unwrap();
        log.append(&d[0], room(1, "Games"), 10).unwrap();
        log.append(&d[0], Body::SetManagers { first: Some(2) }, 11)
            .unwrap();
        log.keep_out(&d[1], FIRST_ROOM, k[3], Some(500), 12)
            .unwrap();
        log.remove(&d[2], k[3], 13).unwrap();
        log.append(&d[1], Body::Leave, 14).unwrap();
        let did: Vec<(DevicePublicKey, Did)> = log
            .activity()
            .into_iter()
            .map(|a| {
                assert!(a.undone.is_none());
                (a.author, a.did)
            })
            .collect();
        assert_eq!(
            did,
            vec![
                (
                    k[0],
                    Did::Created {
                        name: "Sundays".into()
                    }
                ),
                (k[0], Did::Added { member: k[1] }),
                (k[0], Did::Added { member: k[2] }),
                (k[1], Did::Invited),
                (k[3], Did::Joined { invited_by: k[1] }),
                (k[1], Did::LetIn { member: k[3] }),
                (
                    k[0],
                    Did::MovedUp {
                        member: k[2],
                        place: 1
                    }
                ),
                (
                    k[0],
                    Did::Renamed {
                        name: "Mondays".into()
                    }
                ),
                (k[0], Did::RoomOpened { name: "Fun".into() }),
                (
                    k[0],
                    Did::RoomRenamed {
                        from: "Fun".into(),
                        name: "Games".into()
                    }
                ),
                (
                    k[0],
                    Did::Managers {
                        managers: Managers::First(2)
                    }
                ),
                (k[2], Did::Removed { member: k[3] }),
                (k[1], Did::Left),
                (
                    k[1],
                    Did::KeptOut {
                        room: String::new(),
                        member: k[3],
                        until_ms: Some(500)
                    }
                ),
            ]
            .into_iter()
            // The time out of a member removed since counts for nothing any more.
            .filter(|(_, d)| !matches!(d, Did::KeptOut { .. }))
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn what_a_loss_undid_shows_as_undone_and_by_whom() {
        let d = ids(4);
        let k: Vec<DevicePublicKey> = d.iter().map(|i| i.public_key()).collect();
        let mut base = GroupLog::create(&d[0], "Sundays", 1).unwrap();
        for (i, key) in k.iter().enumerate().skip(1) {
            base.add(&d[0], *key, "m", 1 + i as u64).unwrap();
        }
        let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
        at_ana
            .append(&d[0], Body::Demote { member: k[1] }, 10)
            .unwrap();
        at_bea.remove(&d[1], k[3], 11).unwrap();
        at_bea
            .append(
                &d[1],
                Body::Rename {
                    name: "Taken".into(),
                },
                12,
            )
            .unwrap();
        at_ana.insert_all(at_bea.entries().cloned());
        let activity = at_ana.activity();
        let undone: Vec<&Activity> = activity.iter().filter(|a| a.undone.is_some()).collect();
        assert_eq!(undone.len(), 2);
        for a in undone {
            assert_eq!(a.author, k[1]);
            assert_eq!(a.undone, Some(UndoneBy::MovedDown { by: k[0] }));
        }
        assert!(
            activity
                .iter()
                .any(|a| a.did == Did::Removed { member: k[3] })
        );
        assert!(activity.iter().any(|a| a.did
            == Did::Renamed {
                name: "Taken".into()
            }));
        assert!(
            activity
                .iter()
                .any(|a| a.did == Did::MovedDown { member: k[1] } && a.undone.is_none())
        );
    }
}
