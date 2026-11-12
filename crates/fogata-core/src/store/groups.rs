//! The groups kept on this device: the list of them and each group's log, read back whole
//! or rebuilt from the logs when the list is damaged, and the groups friends offer, kept
//! waiting for an answer or declined, a few from each friend.

use std::{fs, io, path::PathBuf};

use anyhow::{Context, Result, bail};
use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
use fogata_state::{Body, Entry, EntryHash, GroupBanner, GroupLog, GroupPicture};
use serde::{Deserialize, Serialize};

use super::{
    Held, Store,
    appended::{LogFile, Stamp, read_log_file},
    aside_path, copy_aside, decode_entries, encode_log, free_name,
    judged::{judged_context, read_judged, write_judged},
    with_suffix, write_private,
};

/// Group invitations from friends kept at once: from one friend, and in all. Past them, a
/// friend's new ones are refused until the person answers some.
const MAX_OFFERS_EACH: usize = 8;
const MAX_OFFERS: usize = 32;
/// Most entries, and bytes, of the log of a group offered and not accepted yet.
const MAX_OFFERED_ENTRIES: usize = 4096;
const MAX_OFFERED_BYTES: usize = 4 * 1024 * 1024;

/// A group this device belongs to, by local name.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StoredGroup {
    pub name: String,
    /// Group id in hex.
    pub group_id: String,
    /// A friend added this device and sent the log, and the person has not accepted yet:
    /// the group is kept but not run.
    #[serde(default)]
    pub pending: bool,
    /// The friend that added this device (device key in hex), while pending.
    #[serde(default)]
    pub invited_by: Option<String>,
}

/// A group invitation the person declined. Friends in the group offer its log again each
/// time they meet this device; those offers are ignored until a member adds this device
/// again, which is an admission not among `admitted`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeclinedGroup {
    /// Group id in hex. Named apart from [`StoredGroup::group_id`], so that neither is ever
    /// read as the other (and versions that know no declined groups keep these as unread).
    pub declined_group: String,
    /// Hashes (hex) of the entries that had made this device a member when it declined.
    pub admitted: Vec<String>,
    /// This device's signed `Leave` (encoded, hex), handed to members who offer the group
    /// again: once they have it, they no longer count this device as a member.
    #[serde(default)]
    pub leave: Option<String>,
    /// Hash (hex) of the declined log's first entry. Ids can be copied: a log offered under
    /// the same id with another first entry is another group, which the decline does not
    /// cover. Declines kept by versions before this field cover every log of the id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
}

/// Declined invitations kept, the newest; an older one past this may come back once.
const MAX_DECLINED: usize = 200;

/// What became of a group a friend offered (see [`Store::save_offered_group`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Offered {
    /// Kept as an invitation waiting for an answer, under this local name.
    Pending(String),
    /// Already accepted here, under this local name: the log was only merged.
    Accepted(String),
    /// This device was removed here, but the removal was undone where the offer comes from
    /// (its author was moved down or removed at the same time): it is a member by the same
    /// entry as before, under this local name, and runs the group again.
    Restored(String),
    /// Declined here before, and not added again since: nothing was kept. The `Leave` this
    /// device signed then, to hand to the friend who offered it.
    Declined(Option<Box<DeclineLeave>>),
}

/// The `Leave` this device signed for an invitation it declined, and the entry that made
/// the invitation (an `Add` of this device, or its `Join`), so the member that made it can
/// be told.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclineLeave {
    pub leave: Entry,
    pub admission: EntryHash,
}

/// The list of groups: the groups, the invitations declined, and entries this version
/// cannot read (those of earlier prototypes, or of a later version), kept as they are when
/// the list is written back.
#[derive(Default)]
pub(super) struct GroupList {
    pub(super) groups: Vec<StoredGroup>,
    declined: Vec<DeclinedGroup>,
    unread: Vec<serde_json::Value>,
}

/// Whether `entry` made `me` a member: an add of it, or its join.
fn admits(entry: &Entry, me: &DevicePublicKey) -> bool {
    match &entry.body {
        Body::Add { member, .. } => member == me,
        Body::Join { .. } => entry.author == *me,
        _ => false,
    }
}

/// The last entry that made `me` a member in `log` and applied (the group's `Create` for
/// the device that started it), whether or not it is a member still.
fn last_admission(log: &GroupLog, me: &DevicePublicKey) -> Option<EntryHash> {
    log.state().applied.iter().rev().copied().find(|h| {
        log.get(h).is_some_and(|e| {
            admits(e, me) || (matches!(e.body, Body::Create { .. }) && e.author == *me)
        })
    })
}

/// The entries of `log` that made `me` a member: adds of it and its joins.
fn admissions(log: &GroupLog, me: &DevicePublicKey) -> Vec<String> {
    log.entries()
        .filter(|e| admits(e, me))
        .map(|e| hex::encode(e.hash()))
        .collect()
}

/// The entry of `log` that made `me` a member last: the invitation it answers.
fn latest_admission(log: &GroupLog, me: &DevicePublicKey) -> Option<EntryHash> {
    log.entries_in_order()
        .into_iter()
        .rev()
        .find(|e| admits(e, me))
        .map(Entry::hash)
}

/// Refuses an offer of group `id` from `by` that would keep more invitations than
/// [`MAX_OFFERS_EACH`] from that friend or [`MAX_OFFERS`] in all, or a log of the size of no
/// real group: a friend could otherwise fill the disk with groups made up for it.
fn check_offer(
    list: &GroupList,
    id: &str,
    by: &fogata_crypto::DevicePublicKey,
    log: &GroupLog,
) -> Result<()> {
    if log.len() > MAX_OFFERED_ENTRIES || encode_log(log).len() > MAX_OFFERED_BYTES {
        bail!("an offered group's log is too big");
    }
    let by = hex::encode(by.as_bytes());
    let known = list.groups.iter().any(|g| g.group_id == id);
    let others = || list.groups.iter().filter(|g| g.pending && g.group_id != id);
    let from_by = || {
        others()
            .filter(|g| g.invited_by.as_deref() == Some(by.as_str()))
            .count()
    };
    if !known && (others().count() >= MAX_OFFERS || from_by() >= MAX_OFFERS_EACH) {
        bail!("too many group invitations waiting for an answer");
    }
    Ok(())
}

impl Store {
    /// A local name not used by another group here: `base`, or `base (2)` and so on.
    pub fn free_group_name(&self, base: &str) -> Result<String> {
        let held = self.lock();
        Ok(free_name(&self.read_groups(&held)?.groups, base))
    }

    /// Groups of this device. Entries in the format of earlier prototypes (a bare group
    /// secret, no log) are left out, and kept in the file.
    pub fn groups(&self) -> Result<Vec<StoredGroup>> {
        let held = self.lock();
        Ok(self.read_groups(&held)?.groups)
    }

    pub(super) fn read_groups(&self, held: &Held) -> Result<GroupList> {
        let values = match fs::read(self.groups_path()) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(GroupList::default()),
            Err(e) => Err(anyhow::Error::from(e)),
            Ok(bytes) => {
                serde_json::from_slice::<Vec<serde_json::Value>>(&bytes).map_err(Into::into)
            }
        };
        let values = match values {
            Ok(values) => values,
            Err(damage) => return self.rebuild_groups(held, damage),
        };
        let mut list = GroupList::default();
        for value in values {
            if let Ok(group) = StoredGroup::deserialize(&value) {
                list.groups.push(group);
            } else if let Ok(declined) = DeclinedGroup::deserialize(&value) {
                list.declined.push(declined);
            } else {
                list.unread.push(value);
            }
        }
        Ok(list)
    }

    /// Makes the list of groups again from the logs, when it cannot be read; the damaged
    /// file is put aside first. Each group gets the name in its log, made unique. One whose
    /// log has nothing by this device was never run here, so it comes back as an invitation
    /// to answer again rather than as a group joined without asking.
    fn rebuild_groups(&self, held: &Held, damage: anyhow::Error) -> Result<GroupList> {
        let path = self.groups_path();
        let aside = aside_path(&path);
        copy_aside(&path, &aside).with_context(|| {
            format!(
                "{} cannot be read ({damage:#}) nor put aside",
                path.display()
            )
        })?;
        let me = self.own_key();
        let mut logs: Vec<PathBuf> = fs::read_dir(self.dir.join("logs"))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "log"))
            .collect();
        logs.sort();
        let mut list = GroupList::default();
        for log_path in logs {
            let Some(id) = log_path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| hex::decode(s).ok())
                .and_then(|b| GroupId::from_bytes(&b).ok())
            else {
                continue;
            };
            let Ok(log) = self.read_log(id) else {
                continue;
            };
            let ran_here = me.is_some_and(|me| log.entries().any(|e| e.author == me));
            let invited_by = me.filter(|_| !ran_here).and_then(|me| {
                log.entries().find_map(|e| match &e.body {
                    Body::Add { member, .. } if *member == me => {
                        Some(hex::encode(e.author.as_bytes()))
                    }
                    _ => None,
                })
            });
            list.groups.push(StoredGroup {
                name: free_name(&list.groups, &log.state().name),
                group_id: hex::encode(id.as_bytes()),
                pending: !ran_here,
                invited_by,
            });
        }
        if let Err(e) = self.write_groups(held, &list) {
            // The damaged file is still in its place; the next read puts it aside again.
            let _ = fs::remove_file(&aside);
            return Err(e);
        }
        tracing::warn!(
            "{} could not be read ({damage:#}): it is kept as {}, and the list was made again \
             from the logs, with {} groups",
            path.display(),
            aside.display(),
            list.groups.len()
        );
        Ok(list)
    }

    fn write_groups(&self, _held: &Held, list: &GroupList) -> Result<()> {
        let mut values = list
            .groups
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()?;
        for declined in &list.declined {
            values.push(serde_json::to_value(declined)?);
        }
        values.extend(list.unread.iter().cloned());
        write_private(&self.groups_path(), &serde_json::to_vec_pretty(&values)?)
    }

    pub fn group(&self, name: &str) -> Result<StoredGroup> {
        self.groups()?
            .into_iter()
            .find(|g| g.name == name)
            .with_context(|| format!("no group called \"{name}\" (see `fogata group list`)"))
    }

    pub fn load_log(&self, group: &StoredGroup) -> Result<GroupLog> {
        let id = hex::decode(&group.group_id)
            .ok()
            .and_then(|b| GroupId::from_bytes(&b).ok())
            .context("stored group id is damaged")?;
        self.read_log(id)
    }

    /// The log of group `id` as stored here.
    pub fn load_log_of(&self, id: &GroupId) -> Result<GroupLog> {
        self.read_log(*id)
    }

    /// A group's log. When the file is damaged (changed on the disk), the entries that still
    /// read are kept and a copy of the file is put aside; one that ends with part of an
    /// entry, as a computer that stopped while one was added leaves it, keeps those before.
    fn read_log(&self, id: GroupId) -> Result<GroupLog> {
        let path = self.log_path(&id);
        let stamp = Stamp::at(&path);
        let bytes = fs::read(&path).with_context(|| format!("cannot read {}", path.display()))?;
        let LogFile {
            entries,
            damaged,
            cut_short,
        } = read_log_file(&bytes);
        let read = entries.len();
        if cut_short {
            // An entry being added when the computer stopped: nothing of it is lost, as it was
            // never all there, and the file is written whole at the next change.
            tracing::info!(
                "{} ends with part of an entry: the {read} entries before it are kept",
                path.display()
            );
        }
        if damaged {
            let aside = with_suffix(&path, ".damaged");
            // Not even a link there: it is never written through.
            if fs::symlink_metadata(&aside).is_err() {
                let _ = copy_aside(&path, &aside);
            }
            tracing::warn!(
                "{} is damaged: the {} entries that read are kept, and a copy is in {}",
                path.display(),
                entries.len(),
                aside.display()
            );
        }
        // All at once, so that no entry is dropped for waiting on one further down the file;
        // not judged again when what was judged of them is kept.
        let mut log = match self.known_key() {
            Some(key) => read_judged(
                key,
                &self.judged_path(&id),
                &judged_context(&id),
                id,
                entries,
            ),
            None => {
                let mut log = GroupLog::empty(id);
                log.insert_all(entries);
                log
            }
        };
        // Room rules an earlier version kept apart, if any; a damaged end of that file loses
        // only rules.
        let rules = fs::read(self.rules_path(&id));
        if let Ok(bytes) = &rules {
            let (rules, _) = decode_entries(bytes);
            log.insert_all(rules.into_iter().filter(|e| e.body.is_room_rule()));
        }
        // What the file holds, to add to it: not when part of it did not read, nor with rules
        // that are not in it yet.
        if !damaged && !cut_short && rules.is_err() {
            self.note_read(id, stamp, read, &log);
        }
        // The banner, checked again against the log and the clock: one dated more than a
        // day ahead would show over every banner chosen until then.
        if let Ok(bytes) = fs::read(self.banner_path(&id))
            && let Ok(banner) = GroupBanner::decode(&bytes)
        {
            log.take_banner_at(banner, crate::now_ms());
        }
        // The picture the same way.
        if let Ok(bytes) = fs::read(self.picture_path(&id))
            && let Ok(picture) = GroupPicture::decode(&bytes)
        {
            log.take_picture_at(picture, crate::now_ms());
        }
        Ok(log)
    }

    /// The local name of the group with this id, if this device has it.
    pub fn group_name(&self, id: &GroupId) -> Option<String> {
        let id = hex::encode(id.as_bytes());
        self.groups()
            .ok()?
            .into_iter()
            .find(|g| g.group_id == id)
            .map(|g| g.name)
    }

    /// Stores the log of a group a friend added this device to, merged with the copy here if
    /// there is one; a group not known here, or one this device had been removed from or had
    /// left and is now added to again, is kept as a pending invitation from `by`. A group
    /// whose invitation was declined is not kept, unless a member added this device again
    /// since.
    pub fn save_offered_group(
        &self,
        offered: &GroupLog,
        by: &fogata_crypto::DevicePublicKey,
    ) -> Result<Offered> {
        let held = self.lock();
        let id = hex::encode(offered.group_id().as_bytes());
        let mut list = self.read_groups(&held)?;
        let me = self.own_key();
        if let Some(existing) = list.groups.iter_mut().find(|g| g.group_id == id) {
            let mut log = self.load_log(existing)?;
            let member = |log: &GroupLog| me.is_some_and(|me| log.state().is_member(&me));
            let was_member = member(&log);
            // The admission this device was let in with, before its removal here.
            let let_in_by = me.and_then(|me| last_admission(&log, &me));
            // What the node judged of the offer is not judged again: only entries that waited
            // there for parents this copy has, a few at a time as from any member. Those left
            // out come with later offers, or with syncs once the group runs.
            let whole = log.insert_judged(offered).left_out == 0;
            if existing.pending || !was_member {
                check_offer(&GroupList::default(), &id, by, &log)?;
            }
            // Run again without asking only when the offer was taken whole: an admission
            // among the entries left out would make this device a member in another way,
            // which the person is asked about.
            let restored = whole
                && !existing.pending
                && !was_member
                && member(&log)
                && let_in_by.is_some()
                && me.and_then(|me| last_admission(&log, &me)) == let_in_by;
            if restored {
                self.write_log(&log)?;
                return Ok(Offered::Restored(existing.name.clone()));
            }
            // Added again: the group waits for the person's yes, and says so on the disk
            // before the log that makes this device a member is there. Stopped between the
            // two, it is an invitation with the log from before, which the next offer brings
            // up to date; never a group that runs at the next start without asking.
            if !existing.pending && !was_member && member(&log) {
                existing.pending = true;
                existing.invited_by = Some(hex::encode(by.as_bytes()));
                let name = existing.name.clone();
                self.write_groups(&held, &list)?;
                self.write_log(&log)?;
                return Ok(Offered::Pending(name));
            }
            self.write_log(&log)?;
            let name = existing.name.clone();
            return Ok(if existing.pending {
                Offered::Pending(name)
            } else {
                Offered::Accepted(name)
            });
        }
        let root = offered.root().map(hex::encode);
        if let Some(at) = list
            .declined
            .iter()
            .position(|d| d.declined_group == id && (d.root.is_none() || d.root == root))
        {
            let declined = &list.declined[at];
            let again = me.is_some_and(|me| {
                admissions(offered, &me)
                    .iter()
                    .any(|a| !declined.admitted.contains(a))
            });
            if !again {
                let leave = declined
                    .leave
                    .as_deref()
                    .and_then(|h| hex::decode(h).ok())
                    .and_then(|b| Entry::decode(&b).ok());
                let admission = me.and_then(|me| latest_admission(offered, &me));
                let leave = leave
                    .zip(admission)
                    .map(|(leave, admission)| Box::new(DeclineLeave { leave, admission }));
                return Ok(Offered::Declined(leave));
            }
            list.declined.remove(at);
        }
        check_offer(&list, &id, by, offered)?;
        // Registered as pending in the same write: a crash never leaves it accepted.
        let name = free_name(&list.groups, &offered.state().name);
        self.write_log(offered)?;
        list.groups.push(StoredGroup {
            name: name.clone(),
            group_id: id,
            pending: true,
            invited_by: Some(hex::encode(by.as_bytes())),
        });
        self.write_groups(&held, &list)?;
        Ok(Offered::Pending(name))
    }

    /// Declines the pending invitation to group `id`: its entry and log are forgotten, and
    /// later offers of the group are ignored until a member adds this device again. While
    /// the log still counts this device as a member, it signs a `Leave` with `identity`,
    /// kept to hand to members: returned with the members to give it to now.
    pub fn decline_group(
        &self,
        id: &str,
        identity: &DeviceIdentity,
        now_ms: u64,
    ) -> Result<(Option<DeclineLeave>, Vec<DevicePublicKey>)> {
        let held = self.lock();
        let mut list = self.read_groups(&held)?;
        let Some(at) = list
            .groups
            .iter()
            .position(|g| g.group_id == id && g.pending)
        else {
            bail!("no such invitation");
        };
        let stored = list.groups.remove(at);
        let mut log = self.load_log(&stored).ok();
        let me = identity.public_key();
        let admitted = log
            .as_ref()
            .map(|log| admissions(log, &me))
            .unwrap_or_default();
        let root = log.as_ref().and_then(GroupLog::root).map(hex::encode);
        let admission = log.as_ref().and_then(|log| latest_admission(log, &me));
        let leave = log.as_mut().and_then(|log| {
            log.state()
                .is_member(&me)
                .then(|| log.append(identity, Body::Leave, now_ms).ok())
                .flatten()
        });
        let members = log
            .map(|log| {
                log.state()
                    .members
                    .into_keys()
                    .filter(|k| *k != me)
                    .collect()
            })
            .unwrap_or_default();
        list.declined.retain(|d| d.declined_group != id);
        list.declined.push(DeclinedGroup {
            declined_group: id.to_string(),
            admitted,
            leave: leave.as_ref().map(|e| hex::encode(e.encode())),
            root,
        });
        let extra = list.declined.len().saturating_sub(MAX_DECLINED);
        list.declined.drain(..extra);
        self.write_groups(&held, &list)?;
        if let Some(group_id) = hex::decode(id)
            .ok()
            .and_then(|b| GroupId::from_bytes(&b).ok())
        {
            let _ = fs::remove_file(self.log_path(&group_id));
            let _ = fs::remove_file(self.judged_path(&group_id));
            let _ = fs::remove_file(self.rules_path(&group_id));
            let _ = fs::remove_file(self.banner_path(&group_id));
            let _ = fs::remove_file(self.picture_path(&group_id));
        }
        let leave = leave
            .zip(admission)
            .map(|(leave, admission)| DeclineLeave { leave, admission });
        Ok((leave, members))
    }

    /// Marks a group as a pending invitation from `invited_by`, or as accepted (`None`).
    pub fn set_pending(&self, id: &str, invited_by: Option<String>) -> Result<()> {
        let held = self.lock();
        let mut list = self.read_groups(&held)?;
        let Some(group) = list.groups.iter_mut().find(|g| g.group_id == id) else {
            bail!("no such group here");
        };
        group.pending = invited_by.is_some();
        group.invited_by = invited_by;
        self.write_groups(&held, &list)
    }

    /// Saves a group's log, registering the group under `name` if it is new here.
    pub fn save_log(&self, name: &str, log: &GroupLog) -> Result<()> {
        let held = self.lock();
        self.save_log_held(&held, log, name, false).map(drop)
    }

    /// Saves a group's log; a group new here is registered under `name`, or `name (2)` and
    /// so on when another group has that name. Returns the group's local name.
    pub fn save_group_log(&self, log: &GroupLog, name: &str) -> Result<String> {
        let held = self.lock();
        self.save_log_held(&held, log, name, true)
    }

    fn save_log_held(
        &self,
        held: &Held,
        log: &GroupLog,
        name: &str,
        any_free_name: bool,
    ) -> Result<String> {
        let id = hex::encode(log.group_id().as_bytes());
        let mut list = self.read_groups(held)?;
        if let Some(known) = list.groups.iter().find(|g| g.group_id == id) {
            self.write_log(log)?;
            return Ok(known.name.clone());
        }
        let name = if any_free_name {
            free_name(&list.groups, name)
        } else if list.groups.iter().any(|g| g.name == name) {
            bail!("a group called \"{name}\" already exists here; choose another name");
        } else {
            name.to_string()
        };
        self.write_log(log)?;
        // Joined here on purpose (with an invite, say): a decline from before no longer holds.
        list.declined.retain(|d| d.declined_group != id);
        list.groups.push(StoredGroup {
            name: name.clone(),
            group_id: id,
            pending: false,
            invited_by: None,
        });
        self.write_groups(held, &list)?;
        Ok(name)
    }

    fn write_log(&self, log: &GroupLog) -> Result<()> {
        // On the disk before what was judged of it (see appended.rs).
        self.store_log(log)?;
        if let Some(key) = self.known_key() {
            let id = log.group_id();
            write_judged(key, &self.judged_path(&id), &judged_context(&id), log);
        }
        // The log holds the room rules now: a file of them from an earlier version goes.
        match fs::remove_file(self.rules_path(&log.group_id())) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                tracing::warn!("cannot remove an old file of room rules: {e}");
            }
            _ => {}
        }
        if let Some(banner) = log.banner_record() {
            let path = self.banner_path(&log.group_id());
            self.write_record(&path, banner.signature, || banner.encode())?;
        }
        if let Some(picture) = log.picture_record() {
            let path = self.picture_path(&log.group_id());
            self.write_record(&path, picture.signature, || picture.encode())?;
        }
        self.note_records(log)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use fogata_crypto::DeviceIdentity;
    use fogata_state::{Body, Entry, EntryHash, GroupLog};

    use super::*;
    use crate::store::{Store, faults};

    #[test]
    fn writes_at_once_keep_every_group() {
        let dir = std::env::temp_dir().join(format!("fogata-store-lock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        let (identity, _) = store.identity().unwrap();
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    let log = GroupLog::create(&identity, "Campfire", 1).unwrap();
                    let id = hex::encode(log.group_id().as_bytes());
                    store.save_group_log(&log, "Campfire").unwrap();
                    store.set_pending(&id, Some("someone".into())).unwrap();
                    store.set_pending(&id, None).unwrap();
                });
            }
        });
        let groups = store.groups().unwrap();
        assert_eq!(groups.len(), 8);
        let names: std::collections::HashSet<_> = groups.iter().map(|g| &g.name).collect();
        assert_eq!(names.len(), 8, "every group has a name of its own");
        assert!(groups.iter().all(|g| !g.pending));
        // No temporary file is left behind.
        let left = fs::read_dir(&dir)
            .unwrap()
            .chain(fs::read_dir(dir.join("logs")).unwrap())
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(left, 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_removal_undone_elsewhere_runs_the_group_again_without_asking() {
        let dir = std::env::temp_dir().join(format!("fogata-store-undone-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let (ana, bea) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        let mut base = GroupLog::create(&ana, "Home", 1).unwrap();
        base.add(&ana, bea.public_key(), "Bea", 2).unwrap();
        base.add(&ana, me.public_key(), "Dani", 3).unwrap();
        store.save_group_log(&base, "Home").unwrap();
        // Ana sends Bea to the end while Bea, not knowing, removes this device: here, the
        // removal is all there is.
        let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
        at_ana
            .append(
                &ana,
                Body::Demote {
                    member: bea.public_key(),
                },
                10,
            )
            .unwrap();
        at_bea.remove(&bea, me.public_key(), 11).unwrap();
        store.save_log("Home", &at_bea).unwrap();
        // Ana, with both, offers the group: this device runs it again, asking nothing.
        at_ana.insert_all(at_bea.entries().cloned());
        assert!(at_ana.state().is_member(&me.public_key()));
        assert_eq!(
            store
                .save_offered_group(&at_ana, &ana.public_key())
                .unwrap(),
            Offered::Restored("Home".to_string())
        );
        let group = store.group("Home").unwrap();
        assert!(!group.pending);
        assert!(
            store
                .load_log(&group)
                .unwrap()
                .state()
                .is_member(&me.public_key())
        );
        // Removed for good, then added again by a new entry: that asks, as before.
        let mut later = at_ana.clone();
        later.remove(&ana, me.public_key(), 20).unwrap();
        store.save_log("Home", &later).unwrap();
        later.add(&ana, me.public_key(), "Dani", 21).unwrap();
        assert_eq!(
            store.save_offered_group(&later, &ana.public_key()).unwrap(),
            Offered::Pending("Home".to_string())
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A device removed from a group and added again, its offer kept with the writes
    /// stopped at each one in turn (a full disk, a computer that stops): the group is never
    /// left to run without the person's yes, and the same offer made again asks.
    #[test]
    fn an_offer_that_adds_again_stopped_at_any_write_never_runs_the_group() {
        for n in 0.. {
            let dir =
                std::env::temp_dir().join(format!("fogata-store-again-{n}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            let store = Store::open(dir.clone()).unwrap();
            let me = store.identity().unwrap().0.public_key();
            let ana = DeviceIdentity::generate().unwrap();
            let mut log = GroupLog::create(&ana, "Home", 1).unwrap();
            log.add(&ana, me, "Dani", 2).unwrap();
            store.save_group_log(&log, "Home").unwrap();
            log.remove(&ana, me, 3).unwrap();
            store.save_log("Home", &log).unwrap();
            log.add(&ana, me, "Dani", 4).unwrap();
            faults::fail_after(Some(n));
            let kept = store.save_offered_group(&log, &ana.public_key());
            let tried = faults::tried();
            faults::fail_after(None);
            // What a start reads: a group that is not pending runs when its log counts this
            // device in.
            let again = Store::open(dir.clone()).unwrap();
            let group = again.group("Home").unwrap();
            let member = again.load_log(&group).unwrap().state().is_member(&me);
            assert!(group.pending || !member, "{n}: runs without asking");
            // Only what was judged of the log is kept on a best effort.
            assert!(kept.is_ok() || tried > n, "{n}: {kept:?}");
            assert_eq!(
                again.save_offered_group(&log, &ana.public_key()).unwrap(),
                Offered::Pending("Home".into()),
                "{n}"
            );
            let group = again.group("Home").unwrap();
            assert!(group.pending, "{n}");
            assert!(
                again.load_log(&group).unwrap().state().is_member(&me),
                "{n}"
            );
            fs::remove_dir_all(&dir).unwrap();
            if tried <= n {
                assert!(n >= 2, "only {n} writes");
                break;
            }
        }
    }

    /// An offer with more entries that waited at the node for parents this copy has than
    /// one may judge here: the rest come with the next offers, and a removal undone is not
    /// run again without asking, since what was left out could have let this device in
    /// another way.
    #[test]
    fn an_offer_taken_in_parts_asks_before_running_the_group_again() {
        let dir = std::env::temp_dir().join(format!("fogata-store-parts-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let (ana, bea) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        let mut base = GroupLog::create(&ana, "Home", 1).unwrap();
        base.add(&ana, bea.public_key(), "Bea", 2).unwrap();
        let old: Vec<EntryHash> = (0..20)
            .map(|i| {
                let body = Body::Rename {
                    name: format!("Home {i}"),
                };
                base.append(&ana, body, 3 + i).unwrap().hash()
            })
            .collect();
        base.add(&ana, me.public_key(), "Dani", 30).unwrap();
        store.save_group_log(&base, "Home").unwrap();
        // As in the test above, Bea removes this device while Ana sends her to the end.
        let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
        let demote = Body::Demote {
            member: bea.public_key(),
        };
        at_ana.append(&ana, demote, 40).unwrap();
        at_bea.remove(&bea, me.public_key(), 41).unwrap();
        // Here, also an entry Ana's copy lacks, and Ana's entries on it, each also signed
        // on an old one: they wait at the node, and need the group worked out here.
        let body = Body::Rename {
            name: "Home x".into(),
        };
        let here_only = Entry::sign(&ana, base.group_id(), at_bea.heads(), 42, body);
        let mut stored = at_bea.clone();
        stored.insert(here_only.clone()).unwrap();
        store.save_log("Home", &stored).unwrap();
        let waiting: Vec<Entry> = (0..20)
            .map(|i| {
                let body = Body::Rename {
                    name: format!("House {i}"),
                };
                let parents = vec![here_only.hash(), old[i]];
                Entry::sign(&ana, base.group_id(), parents, 50 + i as u64, body)
            })
            .collect();
        at_ana.insert_all(at_bea.entries().cloned());
        at_ana.insert_all(waiting.clone());
        assert_eq!(at_ana.waiting().len(), waiting.len());
        assert!(at_ana.state().is_member(&me.public_key()));
        let kept = || {
            let log = store.load_log(&store.group("Home").unwrap()).unwrap();
            waiting
                .iter()
                .filter(|e| log.get(&e.hash()).is_some())
                .count()
        };
        assert_eq!(
            store
                .save_offered_group(&at_ana, &ana.public_key())
                .unwrap(),
            Offered::Pending("Home".to_string())
        );
        assert!(kept() < waiting.len(), "{} taken at once", kept());
        // Offered again, as a friend does when the group changes or it connects again: the
        // rest come.
        for _ in 0..waiting.len() {
            store
                .save_offered_group(&at_ana, &ana.public_key())
                .unwrap();
        }
        assert_eq!(kept(), waiting.len());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_offered_group_waits_for_an_answer() {
        let dir = std::env::temp_dir().join(format!("fogata-store-offer-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let friend = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&friend, "Home", 1).unwrap();
        log.add(&friend, me.public_key(), "Ana", 2).unwrap();
        let by = friend.public_key();

        assert_eq!(
            store.save_offered_group(&log, &by).unwrap(),
            Offered::Pending("Home".to_string())
        );
        let group = store.group("Home").unwrap();
        assert!(group.pending);
        assert_eq!(group.invited_by, Some(hex::encode(by.as_bytes())));
        assert_eq!(store.load_log(&group).unwrap().len(), 2);

        // Offered again, it still waits; once accepted, a newer copy is only merged.
        assert_eq!(
            store.save_offered_group(&log, &by).unwrap(),
            Offered::Pending("Home".to_string())
        );
        store.set_pending(&group.group_id, None).unwrap();
        log.append(&friend, fogata_state::Body::SetIcon { icon: 3 }, 3)
            .unwrap();
        assert_eq!(
            store.save_offered_group(&log, &by).unwrap(),
            Offered::Accepted("Home".to_string())
        );
        assert_eq!(store.load_log(&group).unwrap().len(), 3);

        // Removed, then added again: it asks again, under the name it had.
        log.remove(&friend, me.public_key(), 4).unwrap();
        store.save_log("Home", &log).unwrap();
        log.add(&friend, me.public_key(), "Ana", 5).unwrap();
        assert_eq!(
            store.save_offered_group(&log, &by).unwrap(),
            Offered::Pending("Home".to_string())
        );
        assert!(store.group("Home").unwrap().pending);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_declined_group_stays_declined_until_added_again() {
        let dir = std::env::temp_dir().join(format!("fogata-store-decline-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let friend = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&friend, "Home", 1).unwrap();
        log.add(&friend, me.public_key(), "Ana", 2).unwrap();
        let by = friend.public_key();
        store.save_offered_group(&log, &by).unwrap();
        let id = hex::encode(log.group_id().as_bytes());

        let (leave, members) = store.decline_group(&id, &me, 3).unwrap();
        let leave = leave.expect("a member signs a leave");
        assert_eq!(leave.leave.body, Body::Leave);
        // The invitation it declines: the friend's add.
        let add = log
            .entries()
            .find(|e| matches!(e.body, Body::Add { .. }))
            .unwrap()
            .hash();
        assert_eq!(leave.admission, add);
        assert_eq!(members, vec![by]);
        assert!(store.groups().unwrap().is_empty());
        assert!(!store.log_path(&log.group_id()).exists());
        assert!(store.decline_group(&id, &me, 3).is_err());

        // Offered again as it was, or with later changes: ignored, and the leave comes back
        // to hand to whoever offered it.
        assert_eq!(
            store.save_offered_group(&log, &by).unwrap(),
            Offered::Declined(Some(Box::new(leave.clone())))
        );
        log.append(&friend, Body::SetIcon { icon: 3 }, 4).unwrap();
        assert_eq!(
            store.save_offered_group(&log, &by).unwrap(),
            Offered::Declined(Some(Box::new(leave.clone())))
        );
        assert!(store.groups().unwrap().is_empty());

        // The friend took the leave, then added this device again on purpose: asked again.
        log.insert(leave.leave).unwrap();
        assert!(!log.state().is_member(&me.public_key()));
        log.add(&friend, me.public_key(), "Ana", 5).unwrap();
        assert_eq!(
            store.save_offered_group(&log, &by).unwrap(),
            Offered::Pending("Home".to_string())
        );
        // And the decline from before is gone with it.
        let text = fs::read_to_string(dir.join("groups.json")).unwrap();
        assert!(!text.contains("declined_group"), "{text}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_decline_is_kept_by_versions_that_know_none() {
        let dir = std::env::temp_dir().join(format!("fogata-store-declold-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let friend = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&friend, "Home", 1).unwrap();
        log.add(&friend, me.public_key(), "Ana", 2).unwrap();
        store
            .save_offered_group(&log, &friend.public_key())
            .unwrap();
        let id = hex::encode(log.group_id().as_bytes());
        store.decline_group(&id, &me, 3).unwrap();
        // An earlier version reads groups.json as a list of groups: the decline is not one
        // of them, so it keeps it as an entry it cannot read.
        let values: Vec<serde_json::Value> =
            serde_json::from_slice(&fs::read(dir.join("groups.json")).unwrap()).unwrap();
        assert_eq!(values.len(), 1);
        assert!(StoredGroup::deserialize(&values[0]).is_err());
        // A group joined with an invite afterwards ends the decline.
        store.save_group_log(&log, "Home").unwrap();
        let values: Vec<serde_json::Value> =
            serde_json::from_slice(&fs::read(dir.join("groups.json")).unwrap()).unwrap();
        assert_eq!(values.len(), 1);
        assert!(StoredGroup::deserialize(&values[0]).is_ok());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn declining_a_made_up_group_does_not_keep_the_real_one_out() {
        let dir =
            std::env::temp_dir().join(format!("fogata-store-declfake-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let ana = DeviceIdentity::generate().unwrap();
        let mut real = GroupLog::create(&ana, "Home", 1).unwrap();
        real.add(&ana, me.public_key(), "Bea", 2).unwrap();
        // Leo offers a log of his own under the real group's id first; Bea declines it.
        let leo = DeviceIdentity::generate().unwrap();
        let (start, _) = fogata_state::Rotation::new(&real.group_id(), [leo.public_key()]).unwrap();
        let mut fake = GroupLog::empty(real.group_id());
        fake.insert(Entry::sign(
            &leo,
            real.group_id(),
            vec![],
            0,
            Body::Create {
                name: "Home".into(),
                start,
            },
        ))
        .unwrap();
        fake.add(&leo, me.public_key(), "Bea", 1).unwrap();
        store.save_offered_group(&fake, &leo.public_key()).unwrap();
        let id = hex::encode(real.group_id().as_bytes());
        store.decline_group(&id, &me, 3).unwrap();
        // Leo's log stays declined; Ana's real group is still asked about.
        assert!(matches!(
            store.save_offered_group(&fake, &leo.public_key()).unwrap(),
            Offered::Declined(_)
        ));
        assert_eq!(
            store.save_offered_group(&real, &ana.public_key()).unwrap(),
            Offered::Pending("Home".to_string())
        );
        let group = store.group("Home").unwrap();
        assert_eq!(
            store.load_log(&group).unwrap().state().creator,
            Some(ana.public_key())
        );
        // Kept with its root; versions that know no declines keep it as an unread entry,
        // and those that know no roots read it as before.
        let values: Vec<serde_json::Value> =
            serde_json::from_slice(&fs::read(dir.join("groups.json")).unwrap()).unwrap();
        let record = values
            .iter()
            .find(|v| v.get("declined_group").is_some())
            .unwrap();
        assert!(record.get("root").is_some());
        assert!(StoredGroup::deserialize(record).is_err());
        let mut before_roots = record.clone();
        before_roots.as_object_mut().unwrap().remove("root");
        assert!(DeclinedGroup::deserialize(&before_roots).is_ok());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_group_joined_with_an_invite_replaces_an_invitation_under_its_id() {
        let dir = std::env::temp_dir().join(format!("fogata-store-taken-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        // The real group, and a friend's made-up one under the same id, offered first.
        let ana = DeviceIdentity::generate().unwrap();
        let mut real = GroupLog::create(&ana, "Home", 1).unwrap();
        real.add(&ana, me.public_key(), "Bea", 2).unwrap();
        let leo = DeviceIdentity::generate().unwrap();
        let (start, _) = fogata_state::Rotation::new(&real.group_id(), [leo.public_key()]).unwrap();
        let mut fake = GroupLog::empty(real.group_id());
        fake.insert(fogata_state::Entry::sign(
            &leo,
            real.group_id(),
            vec![],
            0,
            fogata_state::Body::Create {
                name: "Home".into(),
                start,
            },
        ))
        .unwrap();
        fake.add(&leo, me.public_key(), "Bea", 1).unwrap();
        assert!(matches!(
            store.save_offered_group(&fake, &leo.public_key()).unwrap(),
            Offered::Pending(_)
        ));
        // Joined with the invite: what the manager sent is kept whole, and runs.
        let name = store.save_group_log(&real, "Home").unwrap();
        let id = hex::encode(real.group_id().as_bytes());
        store.set_pending(&id, None).unwrap();
        let group = store.group(&name).unwrap();
        assert!(!group.pending);
        let kept = store.load_log(&group).unwrap();
        assert_eq!(kept.state().creator, Some(ana.public_key()));
        assert_eq!(kept.len(), real.len());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_friend_cannot_pile_up_invitations() {
        let dir = std::env::temp_dir().join(format!("fogata-store-offers-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let offer = |by: &DeviceIdentity| {
            let mut log = GroupLog::create(by, "g", 1).unwrap();
            log.add(by, me.public_key(), "Ana", 2).unwrap();
            log
        };
        // A log no real group has is refused, whoever offers it.
        let ivy = DeviceIdentity::generate().unwrap();
        let mut big = offer(&ivy);
        let mut parent = big.heads();
        let entries: Vec<fogata_state::Entry> = (0..90u64)
            .map(|i| {
                let entry = fogata_state::Entry::sign(
                    &ivy,
                    big.group_id(),
                    parent.clone(),
                    10 + i,
                    fogata_state::Body::SetPicture {
                        picture: Some(vec![i as u8; fogata_state::MAX_PICTURE_BYTES]),
                    },
                );
                parent = vec![entry.hash()];
                entry
            })
            .collect();
        big.insert_all(entries);
        assert!(store.save_offered_group(&big, &ivy.public_key()).is_err());
        assert!(store.groups().unwrap().is_empty());
        let leo = DeviceIdentity::generate().unwrap();
        let first = offer(&leo);
        for i in 0..MAX_OFFERS_EACH {
            let log = if i == 0 { first.clone() } else { offer(&leo) };
            assert!(matches!(
                store.save_offered_group(&log, &leo.public_key()).unwrap(),
                Offered::Pending(_)
            ));
        }
        assert!(
            store
                .save_offered_group(&offer(&leo), &leo.public_key())
                .is_err()
        );
        // One already waiting is still brought up to date.
        let mut newer = first.clone();
        newer
            .append(&leo, fogata_state::Body::SetIcon { icon: 2 }, 3)
            .unwrap();
        assert!(store.save_offered_group(&newer, &leo.public_key()).is_ok());
        // Other friends still get theirs in, up to the limit in all.
        let mut friends = 0;
        while store.groups().unwrap().len() < MAX_OFFERS {
            let friend = DeviceIdentity::generate().unwrap();
            assert!(
                store
                    .save_offered_group(&offer(&friend), &friend.public_key())
                    .is_ok()
            );
            friends += 1;
        }
        assert_eq!(friends, MAX_OFFERS - MAX_OFFERS_EACH);
        let bea = DeviceIdentity::generate().unwrap();
        assert!(
            store
                .save_offered_group(&offer(&bea), &bea.public_key())
                .is_err()
        );
        // Once the person answers one, there is room again.
        let answered = hex::encode(first.group_id().as_bytes());
        store.set_pending(&answered, None).unwrap();
        assert!(
            store
                .save_offered_group(&offer(&bea), &bea.public_key())
                .is_ok()
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
