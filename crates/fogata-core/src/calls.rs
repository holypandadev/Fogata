//! Direct calls between friends. The caller makes a group with the two of them for the call,
//! rings the friend over their friend connection with that group's log, and both meet in the
//! group's room, with everything a room has: voice, cameras, screens, chat and files. The
//! group is never saved nor listed, and it goes when the call ends.
//!
//! Anyone in a call can ring their own friends into it, up to [`MAX_IN_CALL`] people: the
//! friend is added to the call's group (every member is a manager, appointed by the one who
//! made it) and invited with its log. A call goes on while anyone is in it; the last one
//! left waits [`ALONE_MS`] for the others to come back, and someone who left can come back
//! while it goes on by calling one who is still in it.
//!
//! Those still in a call tell the friends who left it, now and then, that it goes on
//! ([`CallSignal::Going`]), and that they left it when they do: so a device that left knows
//! which friends it can go back to. When a call is over for a device, a line in its chat
//! with each friend it was in the call with says who started it and how long it lasted.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Weak},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_friends::{CallMessage, CallSignal, FriendBook};
use fogata_state::{Body, Entry, GroupLog};

use crate::{Core, CoreEvent, Inner, Person, State, direct, now_ms};

/// What tests run on a call with a friend just before this device starts entering its
/// room, given the call's group, to end the call meanwhile.
#[cfg(test)]
type Meanwhile = Box<dyn FnOnce(&Inner, GroupId) + Send>;
#[cfg(test)]
static ENTERING: std::sync::Mutex<Option<HashMap<DevicePublicKey, Meanwhile>>> =
    std::sync::Mutex::new(None);

/// What tests run at a point of a call's handling (see [`meanwhile`]): something that comes
/// between this device deciding what to do with a call and doing it.
#[cfg(test)]
type Pause =
    Box<dyn FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send>;
#[cfg(test)]
static PAUSES: std::sync::Mutex<Option<HashMap<(&'static str, GroupId), Pause>>> =
    std::sync::Mutex::new(None);

/// Runs what a test set to happen at `point` of the handling of call `group`.
async fn meanwhile(_point: &'static str, _group: GroupId) {
    #[cfg(test)]
    {
        let pause = PAUSES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_or_insert_default()
            .remove(&(_point, _group));
        if let Some(pause) = pause {
            pause().await;
        }
    }
}

/// Runs what a test set to happen as this device starts entering the room of call `group`
/// with `with`.
fn entering(_inner: &Inner, _with: DevicePublicKey, _group: GroupId) {
    #[cfg(test)]
    {
        let meanwhile = ENTERING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_or_insert_default()
            .remove(&_with);
        if let Some(meanwhile) = meanwhile {
            meanwhile(_inner, _group);
        }
    }
}

/// How long a call rings before it counts as not answered.
pub const RING_MS: u64 = 30_000;

/// The one called gives up waiting a little later, in case the caller's "stop" is lost.
const RING_GRACE_MS: u64 = 10_000;

/// How long the last one left in a call waits for the others to come back, and how long
/// someone who left can come back by calling one who is still in it.
pub const ALONE_MS: u64 = 60_000;

/// Most people in a call at once, those rung into it included.
pub const MAX_IN_CALL: usize = 8;

/// How often those in a call tell the friends who left it that it goes on.
const GOING_EVERY_MS: u64 = 15_000;

/// Someone in a call this device left counts as still in it this long after it last said
/// so: past it, a lost message or a device gone quiet no longer keeps the call going here.
const GONE_AFTER_MS: u64 = 40_000;

/// Steps travel apart and may come out of order: that someone is still in a call this
/// device left counts only this long after it said it left.
const LEFT_SETTLES_MS: u64 = 5_000;

/// How often a call this device left is checked for those gone quiet.
const LEFT_CHECK_MS: u64 = 5_000;

/// The longest a call counts as lasting: someone in it who says it started earlier is
/// taken to mean this long.
const LONGEST_CALL_MS: u64 = 48 * 3600 * 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallPhase {
    /// This device rings the friend.
    Ringing,
    /// The friend rings this device.
    Incoming,
    /// This device is in the call.
    Connected,
}

/// Why a call ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallEnd {
    /// Nobody answered in time.
    NoAnswer,
    /// The friend could not talk.
    Declined,
    /// The friend was in another call.
    Busy,
    /// The ring did not reach the friend.
    Unreachable,
    /// This device stopped ringing, or the friend stopped a ring crossed by this device's
    /// own call to it, which goes on.
    Cancelled,
    /// The friend rang and stopped before this device answered.
    Missed,
    /// The friend rang while this device was in another call, and heard it was busy.
    MissedInCall,
    /// The friend rang while this device appeared offline (it had not heard yet), and the
    /// ring was declined.
    MissedOffline,
    /// Another session of this device (the same identity open on another computer)
    /// answered or declined the friend's ring first.
    Elsewhere,
    /// The friend hung up (an older device, whose calls end when one of the two leaves).
    HungUp,
    /// This device hung up.
    Ended,
    /// The friend's device is too old to join a call of more than two.
    Outdated,
    /// Everyone else left, and nobody came back in [`ALONE_MS`].
    Alone,
}

#[derive(Clone, Debug)]
pub struct CallView {
    pub group: GroupId,
    /// Who rang this device, or the friend this device rang first.
    pub with: Person,
    pub outgoing: bool,
    pub phase: CallPhase,
    /// When it started ringing, or when it was answered.
    pub since_ms: u64,
    /// Everyone else in the call's group, in the order they were added.
    pub members: Vec<Person>,
    /// Friends this device rings into the call now.
    pub adding: Vec<Person>,
    /// Those who left the call's room in the last [`ALONE_MS`], and may come back.
    pub away: Vec<Person>,
    /// Since when this device is the only one left, waiting for the others.
    pub alone_since_ms: Option<u64>,
    /// Why this device cannot ring anyone into the call now; none when it can.
    pub add_blocked: Option<String>,
}

pub(crate) struct CallEntry {
    with: DevicePublicKey,
    outgoing: bool,
    phase: CallPhase,
    since_ms: u64,
    log: GroupLog,
    /// Friends this device rings into the call, since when.
    adding: HashMap<DevicePublicKey, u64>,
    /// Friends in the call that said they take calls of more than two.
    capable: HashSet<DevicePublicKey>,
    /// Friends told that this device does.
    told: HashSet<DevicePublicKey>,
    /// Those who left the call's room, and when.
    left: HashMap<DevicePublicKey, u64>,
    /// Since when this device is the only one left in the call.
    alone_since: Option<u64>,
    /// When the call was answered, by this device's clock: the earliest of when it was
    /// answered here and what those in it said when this device came in.
    started_ms: Option<u64>,
    /// Everyone seen in the call's room.
    seen: HashSet<DevicePublicKey>,
    /// Whether anything came from another device since the call was answered, or one came
    /// into its room: a call answered here whose answer never arrived has had none.
    heard: bool,
    /// Those who left are told now and then that the call goes on.
    telling: bool,
    /// Whether this device started entering the call's room (see [`enter`]).
    entered: bool,
}

impl CallEntry {
    fn new(with: DevicePublicKey, outgoing: bool, phase: CallPhase, log: GroupLog) -> Self {
        let now = now_ms();
        Self {
            with,
            outgoing,
            phase,
            since_ms: now,
            log,
            adding: HashMap::new(),
            capable: HashSet::new(),
            told: HashSet::new(),
            left: HashMap::new(),
            alone_since: None,
            started_ms: (phase == CallPhase::Connected).then_some(now),
            seen: if phase == CallPhase::Connected {
                HashSet::from([with])
            } else {
                HashSet::new()
            },
            heard: false,
            telling: false,
            entered: false,
        }
    }

    /// What tells someone that the call goes on with this device in it.
    fn going(&self, now: u64) -> CallSignal {
        let started = self.started_ms.unwrap_or(self.since_ms);
        CallSignal::Going {
            age_ms: now.saturating_sub(started),
            together_ms: self.alone_since.unwrap_or(now).saturating_sub(started),
        }
    }

    /// Whether `device` takes part in this call: a member of its group, or a friend rung
    /// into it from here.
    fn knows(&self, device: &DevicePublicKey) -> bool {
        *device == self.with
            || self.adding.contains_key(device)
            || self.log.shared_state().is_member(device)
    }

    /// This device rings, is rung, or talks with someone: a ring from anyone else is busy.
    /// The last one left in a call, waiting, is not.
    fn busy(&self) -> bool {
        self.phase != CallPhase::Connected || self.alone_since.is_none()
    }
}

/// A call this device left while others were still in it: calling one of them while they
/// are still in it brings this device back into that call.
pub(crate) struct LeftCall {
    log: GroupLog,
    /// When this device left; it also tells this record from a later one of the same call.
    at_ms: u64,
    capable: HashSet<DevicePublicKey>,
    /// When the call was answered, by this device's clock.
    started_ms: u64,
    /// The last moment two or more people were in the call, as far as this device knows.
    together_until: u64,
    /// Everyone seen in the call.
    seen: HashSet<DevicePublicKey>,
    /// Those still in the call as far as this device knows, with when each last said so (or
    /// was seen in it), and whether someone else was in it with them then.
    inside: HashMap<DevicePublicKey, (u64, bool)>,
    /// Those who said they left it, and when.
    gone: HashMap<DevicePublicKey, u64>,
}

impl LeftCall {
    /// `from`, in the call, says it goes on (see [`CallSignal::Going`]). Returns whether it
    /// was not known to be in it.
    fn going(&mut self, from: DevicePublicKey, age_ms: u64, together_ms: u64, now: u64) -> bool {
        if self
            .gone
            .get(&from)
            .is_some_and(|at| now.saturating_sub(*at) < LEFT_SETTLES_MS)
        {
            return false;
        }
        let their_start = said_start(&self.log, age_ms, now);
        let together_until = their_start.saturating_add(together_ms).min(now);
        self.started_ms = self.started_ms.min(their_start);
        self.together_until = self.together_until.max(together_until);
        self.seen.insert(from);
        // Two or more until this moment, give or take the way here.
        let together = now.saturating_sub(together_until) < LEFT_SETTLES_MS;
        self.inside.insert(from, (now, together)).is_none()
    }

    /// `from` left the call. Returns whether it was known to be in it.
    fn left(&mut self, from: DevicePublicKey, now: u64) -> bool {
        self.gone.insert(from, now);
        match self.inside.remove(&from) {
            Some((_, together)) => {
                // It was not alone: two or more were in the call until now.
                if together {
                    self.together_until = self.together_until.max(now);
                }
                true
            }
            None => false,
        }
    }

    /// Forgets those not heard from in [`GONE_AFTER_MS`]. Returns whether anyone is still
    /// in the call.
    fn prune(&mut self, now: u64) -> bool {
        self.inside
            .retain(|_, (at, _)| now.saturating_sub(*at) < GONE_AFTER_MS);
        !self.inside.is_empty()
    }

    /// Whether calling `friend` now brings this device back into the call: it is still in
    /// it, and its device takes a call of more than two when this one was of two.
    fn returnable(&self, friend: &DevicePublicKey, now: u64) -> bool {
        let state = self.log.shared_state();
        let members = &state.members;
        self.inside
            .get(friend)
            .is_some_and(|(at, _)| now.saturating_sub(*at) < GONE_AFTER_MS)
            && members.contains_key(friend)
            && (members.len() > 2 || self.capable.contains(friend))
    }

    /// How long the call lasted, as far as this device knows.
    fn lasted_ms(&self) -> u64 {
        self.together_until.saturating_sub(self.started_ms)
    }
}

/// When someone in the call `log` says it started, `age_ms` before `now`: never before its
/// group was made (by the clock of the one who made it), nor more than [`LONGEST_CALL_MS`]
/// before now. Anyone in a call says so, and the start shows in every chat the call is
/// noted in.
fn said_start(log: &GroupLog, age_ms: u64, now: u64) -> u64 {
    let made = log
        .root()
        .and_then(|root| log.get(&root))
        .map_or(0, |e| e.time_ms);
    now.saturating_sub(age_ms.min(LONGEST_CALL_MS))
        .max(made.min(now))
}

/// The call `group` is answered here, or this device is back in it: a record of having left
/// it goes, and the call keeps that record's start and those seen in it.
fn answered(state: &mut State, group: GroupId) {
    let left = state.left_calls.remove(&group);
    let Some(call) = state.calls.get_mut(&group) else {
        return;
    };
    let now = now_ms();
    call.phase = CallPhase::Connected;
    call.since_ms = now;
    let earlier = left.as_ref().map(|l| l.started_ms);
    call.started_ms = Some(
        [call.started_ms, earlier]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(now),
    );
    call.seen.insert(call.with);
    if let Some(left) = left {
        call.seen.extend(left.seen);
        call.capable.extend(left.capable);
    }
}

impl Core {
    /// Rings a friend who is online: makes the call's group, enters its room (leaving any
    /// other) and rings for [`RING_MS`]. Returns the call's group. A call ringing here from
    /// another friend is declined first: one call at a time. A call ringing here from this
    /// friend is answered instead, and its group returned. A friend still in a call this
    /// device left, or one who left the call this device waits in, is rung into that call.
    pub async fn start_call(&self, friend: DevicePublicKey) -> Result<GroupId> {
        let book = self
            .inner
            .node
            .friends()
            .await?
            .context("this device has no profile")?;
        let name = book
            .friends
            .get(&friend)
            .map(|f| f.profile.name.clone())
            .context("only friends can be called")?;
        let now = now_ms();
        let (waiting_here, back) = {
            let state = self.inner.lock();
            if !state.friends_online.contains_key(&friend) || state.hidden.contains(&friend) {
                bail!("{name} is offline");
            }
            let waiting_here = state
                .calls
                .iter()
                .find(|(_, c)| {
                    c.phase == CallPhase::Connected
                        && c.alone_since.is_some()
                        && c.log.shared_state().is_member(&friend)
                })
                .map(|(group, _)| *group);
            let back = state
                .left_calls
                .iter()
                .find(|(_, l)| l.returnable(&friend, now))
                .map(|(group, l)| {
                    (
                        *group,
                        l.log.clone(),
                        l.capable.clone(),
                        l.started_ms,
                        l.seen.clone(),
                    )
                });
            (waiting_here, back)
        };
        // The one this device waits for, alone in the call: rung back into it.
        if let Some(group) = waiting_here {
            ring_into(&self.inner, group, friend, &name).await?;
            return Ok(group);
        }
        let identity = &self.inner.identity;
        let (group, log, signal, capable, earlier) = match back {
            Some((group, log, capable, started, seen)) => {
                let entries = encode(&log);
                (
                    group,
                    log,
                    CallSignal::Invite { log: entries },
                    capable,
                    Some((started, seen)),
                )
            }
            None => {
                let mut log = GroupLog::create(identity, "Call", now)?;
                // Every member manages it, so anyone in the call can ring friends in.
                log.add(identity, friend, &name, now)?;
                let entries = encode(&log);
                (
                    log.group_id(),
                    log,
                    CallSignal::Ring { log: entries },
                    HashSet::new(),
                    None,
                )
            }
        };
        // Looked at and noted at once: a ring from the friend that comes in meanwhile is
        // either found here, or finds this call and crosses it.
        let (theirs, waiting, alone) = {
            let mut state = self.inner.lock();
            if state
                .calls
                .values()
                .any(|c| c.phase != CallPhase::Incoming && c.busy())
            {
                bail!("you are already in a call");
            }
            let theirs = state
                .calls
                .iter()
                .find(|(_, c)| c.with == friend && c.phase == CallPhase::Incoming)
                .map(|(group, _)| *group);
            let waiting: Vec<GroupId> = state
                .calls
                .iter()
                .filter(|(_, c)| c.with != friend && c.phase == CallPhase::Incoming)
                .map(|(group, _)| *group)
                .collect();
            // A call this device waits in alone ends for the new one.
            let alone: Vec<GroupId> = state
                .calls
                .iter()
                .filter(|(_, c)| c.phase == CallPhase::Connected)
                .map(|(group, _)| *group)
                .collect();
            // Going back into a call left keeps its record until the call is answered: a
            // ring that does not get through leaves it as it was.
            if theirs.is_none() {
                let mut entry = CallEntry::new(friend, true, CallPhase::Ringing, log.clone());
                entry.capable = capable;
                if let Some((started, seen)) = earlier {
                    entry.started_ms = Some(started);
                    entry.seen = seen;
                }
                state.calls.insert(group, entry);
            }
            (theirs, waiting, alone)
        };
        for other in waiting {
            let _ = self.decline_call(other).await;
        }
        // The friend rang first: both want to talk, so this device answers that call.
        if let Some(theirs) = theirs {
            self.accept_call(theirs).await?;
            return Ok(theirs);
        }
        for other in alone {
            let _ = self.end_call(other).await;
        }
        entering(&self.inner, friend, group);
        // The friend is rung only once this device is in the call's room.
        let rung = enter(&self.inner, group, log, self.join_room(group)).await;
        let rung = match rung {
            Ok(()) => send(&self.inner, friend, group, signal).await,
            Err(e) => Err(e),
        };
        if let Err(e) = rung {
            finish(&self.inner, group, CallEnd::Unreachable).await;
            let_go(&self.inner, group).await;
            return Err(e);
        }
        if !self.inner.lock().calls.contains_key(&group) {
            // Hung up, or crossed by the friend's call, before its room was entered here:
            // the friend hears it ended, after the ring.
            let _ = send(&self.inner, friend, group, CallSignal::Cancel).await;
            let_go(&self.inner, group).await;
            self.inner.emit(CoreEvent::CallsChanged);
            return Ok(group);
        }
        expire(
            Arc::downgrade(&self.inner),
            group,
            RING_MS,
            CallPhase::Ringing,
            CallEnd::NoAnswer,
        );
        self.inner.emit(CoreEvent::CallsChanged);
        Ok(group)
    }

    /// Answers a call that rings here: enters its room (leaving any other). A call this
    /// device rings or is in ends first: one call at a time.
    pub async fn accept_call(&self, group: GroupId) -> Result<()> {
        let others: Vec<GroupId> = {
            let state = self.inner.lock();
            state
                .calls
                .get(&group)
                .filter(|c| c.phase == CallPhase::Incoming)
                .context("that call no longer rings")?;
            state
                .calls
                .iter()
                .filter(|(g, c)| **g != group && c.phase != CallPhase::Incoming)
                .map(|(g, _)| *g)
                .collect()
        };
        for other in others {
            let _ = self.end_call(other).await;
        }
        let (with, log) = {
            let state = self.inner.lock();
            let call = state
                .calls
                .get(&group)
                .filter(|c| c.phase == CallPhase::Incoming)
                .context("that call no longer rings")?;
            (call.with, call.log.clone())
        };
        entering(&self.inner, with, group);
        // Answered here only once in its room and the friend told: a call whose room cannot
        // be entered ends as not answered (the friend hears it cannot be now), and keeps a
        // record of having left it before.
        let joined = enter(&self.inner, group, log, self.join_room(group)).await;
        if let Err(e) = joined {
            let _ = send(&self.inner, with, group, CallSignal::Decline).await;
            finish(&self.inner, group, CallEnd::Unreachable).await;
            let_go(&self.inner, group).await;
            return Err(e);
        }
        if let Err(e) = send(&self.inner, with, group, CallSignal::Accept).await {
            finish(&self.inner, group, CallEnd::Unreachable).await;
            let_go(&self.inner, group).await;
            return Err(e);
        }
        let ended = {
            let mut state = self.inner.lock();
            match state.calls.get(&group).map(|c| c.phase) {
                Some(CallPhase::Incoming) => {
                    answered(&mut state, group);
                    false
                }
                // Answered meanwhile, by a click before this one.
                Some(_) => return Ok(()),
                // The ring stopped meanwhile: the call ended, perhaps before its room was
                // entered here.
                None => true,
            }
        };
        if ended {
            let_go(&self.inner, group).await;
            bail!("that call no longer rings");
        }
        tell_capable(&self.inner, group, with).await;
        self.inner.emit(CoreEvent::CallsChanged);
        Ok(())
    }

    /// Does not answer a call that rings here; the caller hears that it cannot be now.
    pub async fn decline_call(&self, group: GroupId) -> Result<()> {
        let call = {
            let mut state = self.inner.lock();
            match state.calls.get(&group) {
                Some(c) if c.phase == CallPhase::Incoming => state.calls.remove(&group),
                _ => None,
            }
        };
        let call = call.context("that call no longer rings")?;
        let _ = send(&self.inner, call.with, group, CallSignal::Decline).await;
        self.inner.emit(CoreEvent::CallsChanged);
        Ok(())
    }

    /// Stops ringing, or hangs up. The others in the call go on without this device, which
    /// can come back by calling one of them while they are still in it.
    pub async fn end_call(&self, group: GroupId) -> Result<()> {
        let (with, phase) = {
            let state = self.inner.lock();
            let call = state.calls.get(&group).context("no such call")?;
            (call.with, call.phase)
        };
        match phase {
            CallPhase::Incoming => return self.decline_call(group).await,
            CallPhase::Ringing => {
                let _ = send(&self.inner, with, group, CallSignal::Cancel).await;
                finish(&self.inner, group, CallEnd::Cancelled).await;
            }
            CallPhase::Connected => {
                // An older friend's call ends with this; a newer one waits for this device.
                let _ = send(&self.inner, with, group, CallSignal::End).await;
                let others = people_in_room(&self.inner, group).await;
                let now = now_ms();
                {
                    let mut state = self.inner.lock();
                    // Only friends say that the call goes on: with none of them left in it, it
                    // is over here.
                    let friends: Vec<DevicePublicKey> = others
                        .iter()
                        .filter(|d| state.friends_online.contains_key(*d))
                        .copied()
                        .collect();
                    if !friends.is_empty() {
                        let left = state.calls.get(&group).map(|c| {
                            let mut seen = c.seen.clone();
                            seen.extend(others.iter().copied());
                            LeftCall {
                                log: c.log.clone(),
                                at_ms: now,
                                capable: c.capable.clone(),
                                started_ms: c.started_ms.unwrap_or(c.since_ms),
                                together_until: now,
                                seen,
                                inside: friends
                                    .iter()
                                    .map(|d| (*d, (now, others.len() > 1)))
                                    .collect(),
                                gone: HashMap::new(),
                            }
                        });
                        if let Some(left) = left {
                            state.left_calls.insert(group, left);
                            watch_left(Arc::downgrade(&self.inner), group, now);
                        }
                    }
                }
                finish(&self.inner, group, CallEnd::Ended).await;
            }
        }
        Ok(())
    }

    /// Rings a friend into the call this device is in (see [`CallView::add_blocked`]).
    pub async fn add_to_call(&self, group: GroupId, friend: DevicePublicKey) -> Result<()> {
        let book = self
            .inner
            .node
            .friends()
            .await?
            .context("this device has no profile")?;
        let name = book
            .friends
            .get(&friend)
            .map(|f| f.profile.name.clone())
            .context("only friends can be rung into a call")?;
        {
            let state = self.inner.lock();
            if !state.friends_online.contains_key(&friend) || state.hidden.contains(&friend) {
                bail!("{name} is offline");
            }
            let call = state.calls.get(&group).context("no such call")?;
            if call.adding.contains_key(&friend) {
                bail!("{name}’s call is ringing already");
            }
        }
        if let Some(blocked) = add_blocked(&self.inner, group, Some(&book)).await {
            bail!(blocked);
        }
        let room = self
            .inner
            .node
            .room(group, fogata_state::FIRST_ROOM)
            .await?;
        if room.is_some_and(|r| r.participants.iter().any(|(d, _, _)| *d == friend)) {
            bail!("{name} is in the call already");
        }
        ring_into(&self.inner, group, friend, &name).await
    }

    /// Stops ringing a friend into the call.
    pub async fn cancel_add(&self, group: GroupId, friend: DevicePublicKey) -> Result<()> {
        let rang = self
            .inner
            .lock()
            .calls
            .get_mut(&group)
            .and_then(|c| c.adding.remove(&friend));
        rang.context("that friend is not being rung")?;
        let _ = send(&self.inner, friend, group, CallSignal::Cancel).await;
        self.inner.emit(CoreEvent::CallsChanged);
        Ok(())
    }

    /// The calls ringing or going on. The friend shows as known now: a name or picture
    /// changed during the call, or the friend removed meanwhile, still shows rightly.
    pub async fn calls(&self) -> Result<Vec<CallView>> {
        let book = self.inner.node.friends().await?;
        let me = self.me();
        let now = now_ms();
        struct Seen {
            group: GroupId,
            with: DevicePublicKey,
            outgoing: bool,
            phase: CallPhase,
            since_ms: u64,
            log: GroupLog,
            adding: Vec<(DevicePublicKey, u64)>,
            away: Vec<DevicePublicKey>,
            alone_since_ms: Option<u64>,
        }
        let calls: Vec<Seen> = self
            .inner
            .lock()
            .calls
            .iter()
            .map(|(group, call)| {
                let mut adding: Vec<(DevicePublicKey, u64)> =
                    call.adding.iter().map(|(d, at)| (*d, *at)).collect();
                adding.sort_by_key(|(_, at)| *at);
                Seen {
                    group: *group,
                    with: call.with,
                    outgoing: call.outgoing,
                    phase: call.phase,
                    since_ms: call.since_ms,
                    log: call.log.clone(),
                    adding,
                    away: call
                        .left
                        .iter()
                        .filter(|(_, at)| now.saturating_sub(**at) < ALONE_MS)
                        .map(|(d, _)| *d)
                        .collect(),
                    alone_since_ms: call.alone_since,
                }
            })
            .collect();
        let mut out = Vec::with_capacity(calls.len());
        for seen in calls {
            let shared = seen.log.shared_state();
            let shown = |device| self.person_with(device, None, book.as_ref(), &shared);
            // Someone this device knows neither as a friend nor by a profile of its own shows
            // under the name whoever added it gave it: said so.
            let known = |device: &DevicePublicKey| {
                book.as_ref()
                    .is_some_and(|b| b.friends.contains_key(device))
                    || self.inner.lock().profiles.contains_key(device)
            };
            let person = |device: DevicePublicKey| {
                let person = shown(device);
                match named_by_another(&seen.log, device, me) {
                    Some(namer) if !known(&device) => Person {
                        name: format!("{} (named by {})", person.name, shown(namer).name),
                        ..person
                    },
                    _ => person,
                }
            };
            let members = seen
                .log
                .state()
                .members
                .keys()
                .filter(|d| **d != me)
                .map(|d| person(*d))
                .collect();
            let add_blocked = add_blocked(&self.inner, seen.group, book.as_ref()).await;
            out.push(CallView {
                group: seen.group,
                with: person(seen.with),
                outgoing: seen.outgoing,
                phase: seen.phase,
                since_ms: seen.since_ms,
                members,
                adding: seen.adding.iter().map(|(d, _)| person(*d)).collect(),
                away: seen.away.iter().map(|d| person(*d)).collect(),
                alone_since_ms: seen.alone_since_ms,
                add_blocked,
            });
        }
        out.sort_by_key(|c| c.since_ms);
        Ok(out)
    }

    /// Friends still in a call this device left: calling one of them brings this device
    /// back into that call.
    pub fn call_returns(&self) -> Vec<DevicePublicKey> {
        let now = now_ms();
        let state = self.inner.lock();
        let mut out: Vec<DevicePublicKey> = state
            .left_calls
            .values()
            .flat_map(|l| l.inside.keys().filter(move |d| l.returnable(d, now)))
            .copied()
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Stops ringing or hangs up every call with `device`.
    pub(crate) async fn end_calls_with(&self, device: DevicePublicKey) {
        let calls: Vec<GroupId> = self
            .inner
            .lock()
            .calls
            .iter()
            .filter(|(_, c)| c.with == device)
            .map(|(g, _)| *g)
            .collect();
        for group in calls {
            let _ = self.end_call(group).await;
        }
    }

    /// Whether `group` is a call's group, which is neither saved nor listed.
    pub fn is_call(&self, group: GroupId) -> bool {
        self.inner.is_call(group)
    }
}

impl Inner {
    pub(crate) fn is_call(&self, group: GroupId) -> bool {
        self.lock().calls.contains_key(&group)
    }
}

fn encode(log: &GroupLog) -> Vec<Vec<u8>> {
    log.entries_in_order()
        .into_iter()
        .map(Entry::encode)
        .collect()
}

/// Why this device cannot ring anyone into the call `group` now; none when it can.
async fn add_blocked(inner: &Inner, group: GroupId, book: Option<&FriendBook>) -> Option<String> {
    let me = inner.identity.public_key();
    let (phase, log, adding, capable, with) = {
        let state = inner.lock();
        let call = state.calls.get(&group)?;
        (
            call.phase,
            call.log.clone(),
            call.adding.len(),
            call.capable.clone(),
            call.with,
        )
    };
    if phase != CallPhase::Connected {
        return Some("People can be added once the call is answered".into());
    }
    let state = log.state();
    if state.members.len() == 2 && !capable.contains(&with) {
        let name = book
            .and_then(|b| b.friends.get(&with))
            .map_or_else(|| "The other person".into(), |f| f.profile.name.clone());
        return Some(format!(
            "{name}’s Fogata is too old for calls of more than two"
        ));
    }
    if !state.is_manager(&me) {
        return Some("Only someone in the call before you can add people now".into());
    }
    let room = inner
        .node
        .room(group, fogata_state::FIRST_ROOM)
        .await
        .ok()
        .flatten();
    let inside = room.map_or(1, |r| r.participants.len().max(1));
    if inside + adding >= MAX_IN_CALL {
        return Some(format!("A call takes {MAX_IN_CALL} people at most"));
    }
    None
}

/// Rings `friend` into the call `group` this device is in: adds it to the call's group if
/// it is not a member yet, and invites it with the group's log.
async fn ring_into(
    inner: &Arc<Inner>,
    group: GroupId,
    friend: DevicePublicKey,
    name: &str,
) -> Result<()> {
    let identity = &inner.identity;
    let mut log = inner
        .node
        .group(group)
        .await?
        .context("that call is over")?;
    let now = now_ms();
    let state = log.state();
    let mut changed = false;
    if !state.is_member(&friend) {
        // Someone new gets the call's group with every member it ever had: no more than a
        // call takes, or the friend would refuse it (see `ring_fits`).
        if state.members.len() >= MAX_IN_CALL {
            bail!("No one new can be added to this call");
        }
        log.add(identity, friend, name, now)?;
        // A group takes so many members, those who left the call included, and an add
        // past them is ignored: the friend could not read the invitation.
        if !log.state().is_member(&friend) {
            bail!("No one else can be added to this call");
        }
        changed = true;
    }
    if changed {
        inner.node.merge_group(log.clone()).await?;
    }
    {
        let mut state = inner.lock();
        let call = state.calls.get_mut(&group).context("that call is over")?;
        call.log = log.clone();
        call.adding.insert(friend, now);
    }
    let invite = CallSignal::Invite { log: encode(&log) };
    if let Err(e) = send(inner, friend, group, invite).await {
        if let Some(call) = inner.lock().calls.get_mut(&group) {
            call.adding.remove(&friend);
        }
        return Err(e);
    }
    let weak = Arc::downgrade(inner);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(RING_MS)).await;
        let Some(inner) = weak.upgrade() else {
            return;
        };
        let gone = inner.lock().calls.get_mut(&group).is_some_and(|c| {
            c.adding.get(&friend) == Some(&now) && c.adding.remove(&friend).is_some()
        });
        if gone {
            let _ = send(&inner, friend, group, CallSignal::Cancel).await;
            added_ended(&inner, group, friend, CallEnd::NoAnswer);
        }
    });
    inner.emit(CoreEvent::CallsChanged);
    Ok(())
}

/// A friend rung into a call did not come in.
fn added_ended(inner: &Inner, group: GroupId, friend: DevicePublicKey, reason: CallEnd) {
    inner.emit(CoreEvent::CallAddEnded {
        group,
        friend,
        reason,
    });
    inner.emit(CoreEvent::CallsChanged);
}

/// Tells a friend in the call that this device takes calls of more than two, once.
async fn tell_capable(inner: &Inner, group: GroupId, friend: DevicePublicKey) {
    let first = inner
        .lock()
        .calls
        .get_mut(&group)
        .is_some_and(|c| c.told.insert(friend));
    if first {
        // An older device refuses it, and nobody waits for it.
        let _ = send(inner, friend, group, CallSignal::GroupCalls).await;
    }
}

/// A call step from a friend.
pub(crate) async fn on_call(inner: &Arc<Inner>, from: DevicePublicKey, message: CallMessage) {
    let Ok(group) = GroupId::from_bytes(&message.call) else {
        return;
    };
    let signal = match message.signal {
        CallSignal::Ring { log } => return on_ring(inner, from, group, &log, true).await,
        CallSignal::Invite { log } => return on_ring(inner, from, group, &log, false).await,
        signal => signal,
    };
    on_left_signal(inner, from, group, &signal).await;
    let (phase, with, adding) = {
        let state = inner.lock();
        match state.calls.get(&group) {
            Some(call) if call.knows(&from) => {
                (call.phase, call.with, call.adding.contains_key(&from))
            }
            _ => return,
        }
    };
    if let Some(call) = inner.lock().calls.get_mut(&group) {
        call.heard = true;
    }
    match (signal, phase) {
        (CallSignal::GroupCalls, _) => {
            if let Some(call) = inner.lock().calls.get_mut(&group) {
                call.capable.insert(from);
            }
            tell_capable(inner, group, from).await;
            inner.emit(CoreEvent::CallsChanged);
        }
        // Someone in the call says since when it goes: this device came in later.
        (CallSignal::Going { age_ms, .. }, _) => {
            if let Some(call) = inner.lock().calls.get_mut(&group) {
                let start = said_start(&call.log, age_ms, now_ms());
                call.started_ms = Some(call.started_ms.map_or(start, |s| s.min(start)));
            }
        }
        (CallSignal::Accept, CallPhase::Ringing) if from == with => {
            answered(&mut inner.lock(), group);
            tell_capable(inner, group, from).await;
            inner.emit(CoreEvent::CallAccepted { group });
            inner.emit(CoreEvent::CallsChanged);
        }
        // A friend rung into the call answers; it shows once it is in the room, and hears
        // since when the call goes.
        (CallSignal::Accept, CallPhase::Connected) if adding => {
            if let Some(call) = inner.lock().calls.get_mut(&group) {
                call.adding.remove(&from);
            }
            tell_capable(inner, group, from).await;
            tell_going(inner, group, &[from]).await;
            inner.emit(CoreEvent::CallsChanged);
        }
        // A friend rung into the call cannot come.
        (signal @ (CallSignal::Decline | CallSignal::Busy), CallPhase::Connected) if adding => {
            if let Some(call) = inner.lock().calls.get_mut(&group) {
                call.adding.remove(&from);
            }
            let reason = if signal == CallSignal::Busy {
                CallEnd::Busy
            } else {
                CallEnd::Declined
            };
            added_ended(inner, group, from, reason);
        }
        (CallSignal::Decline, CallPhase::Ringing) if from == with => {
            finish(inner, group, CallEnd::Declined).await
        }
        (CallSignal::Busy, CallPhase::Ringing) if from == with => {
            finish(inner, group, CallEnd::Busy).await
        }
        (CallSignal::Cancel, CallPhase::Incoming) if from == with => {
            // A ring stopped because the friend took this device's own call to it instead,
            // which goes on: nothing was missed.
            let crossed = inner
                .lock()
                .calls
                .iter()
                .any(|(g, c)| *g != group && c.with == from && c.phase != CallPhase::Incoming);
            let reason = if crossed {
                CallEnd::Cancelled
            } else {
                CallEnd::Missed
            };
            finish(inner, group, reason).await
        }
        // The friend left; the others go on, and the last one waits for them a while.
        (CallSignal::Cancel | CallSignal::End, CallPhase::Connected) => {
            someone_left(inner, group, from).await
        }
        // Only the node of the friend who rang says another session took its ring.
        (CallSignal::Elsewhere, CallPhase::Incoming | CallPhase::Connected) if from == with => {
            finish(inner, group, CallEnd::Elsewhere).await
        }
        _ => {}
    }
}

/// A ring (`new_call`) or an invitation into a call that goes on.
async fn on_ring(
    inner: &Arc<Inner>,
    from: DevicePublicKey,
    group: GroupId,
    entries: &[Vec<u8>],
    new_call: bool,
) {
    // A call this device is in, or left a short while ago, is rung or invited into only by
    // someone in it: a friend that learnt its id could otherwise make up a group of its own
    // under it, and be answered as one coming back (told the call goes on, and since when),
    // or take the place of the call left here.
    let stranger = {
        let state = inner.lock();
        match (state.calls.get(&group), state.left_calls.get(&group)) {
            (Some(call), _) => !call.knows(&from),
            (None, Some(left)) => !left.log.shared_state().is_member(&from),
            (None, None) => false,
        }
    };
    if stranger {
        tracing::debug!(
            "a ring from {} into a call it is not in",
            from.fingerprint()
        );
        return;
    }
    let me = inner.identity.public_key();
    let Some(log) = ring_log(inner, group, from, me, entries, new_call) else {
        tracing::debug!(
            "a ring from {} with a group that does not fit",
            from.fingerprint()
        );
        return;
    };
    // Someone who left comes back, or the friend rung back into this call rings this device
    // into it at the same moment: answered at once.
    let back = {
        let mut state = inner.lock();
        match state.calls.get_mut(&group) {
            Some(call) => {
                let back = match call.phase {
                    CallPhase::Connected => true,
                    CallPhase::Ringing => call.with == from,
                    CallPhase::Incoming => false,
                };
                if !back {
                    return;
                }
                let ringing = call.phase == CallPhase::Ringing;
                call.left.remove(&from);
                call.adding.remove(&from);
                if ringing {
                    answered(&mut state, group);
                }
                Some(ringing)
            }
            None => None,
        }
    };
    if let Some(ringing) = back {
        let _ = send(inner, from, group, CallSignal::Accept).await;
        tell_capable(inner, group, from).await;
        tell_going(inner, group, &[from]).await;
        if ringing {
            inner.emit(CoreEvent::CallAccepted { group });
        }
        inner.emit(CoreEvent::CallsChanged);
        return;
    }
    // Looked at and noted at once, as start_call does: this device's own call to the friend
    // is either found here, or finds this ring and answers it.
    let (crossed, with_them) = {
        let mut state = inner.lock();
        if state.calls.contains_key(&group) {
            return;
        }
        let with_them = state.calls.values().any(|c| c.with == from);
        let crossed = if state.appear_offline {
            Err(CallSignal::Decline)
        } else if let Some(mine) = crossing(&state.calls, from) {
            Ok(Some(mine))
        } else if state.calls.values().any(CallEntry::busy) {
            Err(CallSignal::Busy)
        } else {
            state.calls.insert(
                group,
                CallEntry::new(from, false, CallPhase::Incoming, log.clone()),
            );
            Ok(None)
        };
        (crossed, with_them)
    };
    match crossed {
        Err(answer) => {
            let reason = if answer == CallSignal::Busy {
                CallEnd::MissedInCall
            } else {
                CallEnd::MissedOffline
            };
            let _ = send(inner, from, group, answer).await;
            // The friend hears this device is in another call, or is declined while this
            // device appears offline; here it shows as a missed call, unless the friend is
            // the one this device is calling or talking to.
            if !with_them {
                inner.emit(CoreEvent::CallEnded {
                    group,
                    with: from,
                    reason,
                });
            }
            return;
        }
        // Both rang each other at once: the call with the lower id goes on, on both sides,
        // instead of each hearing that the other is busy. The friend answers this device's
        // own call when that is the lower one.
        Ok(Some(mine)) if mine < group => return,
        Ok(Some(mine)) => return take_crossed(inner, from, group, mine, log).await,
        Ok(None) => {}
    }
    expire(
        Arc::downgrade(inner),
        group,
        RING_MS + RING_GRACE_MS,
        CallPhase::Incoming,
        CallEnd::Missed,
    );
    inner.emit(CoreEvent::CallIncoming { group, from });
    inner.emit(CoreEvent::CallsChanged);
}

/// This device's own call, when its only call rings `from`: a ring from that friend then
/// crossed it on the way.
fn crossing(calls: &HashMap<GroupId, CallEntry>, from: DevicePublicKey) -> Option<GroupId> {
    let mut calls = calls.iter();
    match (calls.next(), calls.next()) {
        (Some((group, call)), None)
            if call.with == from && call.outgoing && call.phase == CallPhase::Ringing =>
        {
            Some(*group)
        }
        _ => None,
    }
}

/// Lets this device's own call `mine` go for the friend's `group`, which rang at the same
/// time and has the lower id, and answers it: both pressed Call, so both are in it at once.
async fn take_crossed(
    inner: &Arc<Inner>,
    from: DevicePublicKey,
    group: GroupId,
    mine: GroupId,
    log: GroupLog,
) {
    let _ = send(inner, from, mine, CallSignal::Cancel).await;
    if inner.lock().calls.remove(&mine).is_none() {
        return;
    }
    let_go(inner, mine).await;
    inner.lock().calls.insert(
        group,
        CallEntry::new(from, false, CallPhase::Connected, log.clone()),
    );
    entering(inner, from, group);
    let joined = enter(inner, group, log, join_call_room(inner, group)).await;
    // The friend hears it is answered only once this device is in the call's room, and
    // that it cannot be now otherwise.
    let answered = match joined {
        Ok(()) => send(inner, from, group, CallSignal::Accept).await,
        Err(e) => {
            let _ = send(inner, from, group, CallSignal::Decline).await;
            Err(e)
        }
    };
    if let Err(e) = answered {
        tracing::debug!("a call that crossed this device's own did not go on: {e:#}");
        // It never went on: it ends as one not answered, which no chat notes.
        if let Some(call) = inner.lock().calls.get_mut(&group) {
            call.phase = CallPhase::Incoming;
        }
        finish(inner, group, CallEnd::Unreachable).await;
        let_go(inner, group).await;
        return;
    }
    if !inner.lock().calls.contains_key(&group) {
        // It ended meanwhile, perhaps before its room was entered here.
        let_go(inner, group).await;
        return;
    }
    tell_capable(inner, group, from).await;
    inner.emit(CoreEvent::CallAccepted { group });
    inner.emit(CoreEvent::CallsChanged);
}

/// Enters a call's room under the profile's name.
async fn join_call_room(inner: &Inner, group: GroupId) -> Result<()> {
    let nick = inner
        .node
        .friends()
        .await?
        .map_or_else(|| "anon".into(), |b| b.profile.name);
    Ok(inner
        .node
        .join_room(group, fogata_state::FIRST_ROOM, nick)
        .await?)
}

/// The call's group from a ring (`new_call`) or an invitation: one that fits
/// ([`ring_fits`]), and readable here. Never one of this device's own groups: hanging up
/// would stop running it.
fn ring_log(
    inner: &Inner,
    group: GroupId,
    from: DevicePublicKey,
    me: DevicePublicKey,
    entries: &[Vec<u8>],
    new_call: bool,
) -> Option<GroupLog> {
    if inner.store.group_name(&group).is_some() || inner.direct.is_chat_group(&group) {
        return None;
    }
    let decoded: Vec<Entry> = entries
        .iter()
        .map(|bytes| Entry::decode(bytes).ok())
        .collect::<Option<_>>()?;
    let count = decoded.len();
    let mut log = GroupLog::empty(group);
    if log.insert_all(decoded).len() != count {
        return None;
    }
    let rung = if new_call {
        Rung::New
    } else {
        let state = inner.lock();
        if state.calls.contains_key(&group) || state.left_calls.contains_key(&group) {
            Rung::Known
        } else {
            Rung::Invited
        }
    };
    let fits = ring_fits(&log, from, me, rung) && log.current_keys(&inner.identity).is_some();
    fits.then_some(log)
}

/// How a call's group came to this device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rung {
    /// A ring: a new call.
    New,
    /// An invitation into a call this device is in, or left a short while ago.
    Known,
    /// An invitation into a call this device was never in.
    Invited,
}

/// Whether a call's group fits what came with it: the one ringing and this device among
/// its members; a new call made by the one ringing, with exactly the two of them; a call
/// this device is in or left, with at most as many members as a group takes (those who
/// left it stay members, and [`MAX_IN_CALL`] counts only those in it now, see
/// `add_blocked`); a call this device was never in, only when the one inviting is the one
/// who put this device in its group, and with no more members than a call takes (as
/// `ring_into` adds no one past them). Anything else would put this device, with its name
/// and picture, in a room with people it never chose to call, or that a friend took from
/// someone else's call.
fn ring_fits(log: &GroupLog, from: DevicePublicKey, me: DevicePublicKey, rung: Rung) -> bool {
    let state = log.state();
    let fits = state.creator.is_some()
        && state.members.contains_key(&from)
        && state.members.contains_key(&me);
    match rung {
        Rung::New => fits && state.members.len() == 2 && state.creator == Some(from),
        Rung::Known => fits && state.members.len() <= usize::from(fogata_state::MAX_MEMBERS),
        Rung::Invited => {
            fits && state.members.len() <= MAX_IN_CALL && added_by(log, &me) == Some(from)
        }
    }
}

/// Who gave `device` the name it has in the call's group, when that is someone else than it
/// and this device: the one who added it.
fn named_by_another(
    log: &GroupLog,
    device: DevicePublicKey,
    me: DevicePublicKey,
) -> Option<DevicePublicKey> {
    if device == me {
        return None;
    }
    added_by(log, &device).filter(|namer| *namer != me && *namer != device)
}

/// Who added `device` to the call's group, by the last entry that made it a member there;
/// `None` when it made the group or joined by itself.
fn added_by(log: &GroupLog, device: &DevicePublicKey) -> Option<DevicePublicKey> {
    let state = log.state();
    state.applied.iter().rev().find_map(|hash| {
        let entry = log.get(hash)?;
        match &entry.body {
            Body::Add { member, .. } if member == device => Some(Some(entry.author)),
            Body::Create { .. } | Body::Join { .. } if entry.author == *device => Some(None),
            _ => None,
        }
    })?
}

/// A call step that did not reach the friend; `refused` when the friend's device could not
/// read it, as an older one cannot read an invitation into a call of more than two.
pub(crate) async fn on_undelivered(
    inner: &Arc<Inner>,
    to: DevicePublicKey,
    call: [u8; 16],
    refused: bool,
) {
    let Ok(group) = GroupId::from_bytes(&call) else {
        return;
    };
    let (adding, ringing, accepted) = {
        let mut state = inner.lock();
        let Some(c) = state.calls.get_mut(&group) else {
            return;
        };
        (
            c.adding.remove(&to).is_some(),
            c.with == to && c.phase == CallPhase::Ringing,
            c.with == to && c.phase == CallPhase::Connected,
        )
    };
    if adding {
        let reason = if refused {
            CallEnd::Outdated
        } else {
            CallEnd::Unreachable
        };
        added_ended(inner, group, to, reason);
    } else if ringing || (accepted && others_in_room(inner, group).await == 0) {
        // Rung, or answered, and nobody else is there: unless it was answered, or it is no
        // longer the same call, meanwhile.
        let phase = if ringing {
            CallPhase::Ringing
        } else {
            CallPhase::Connected
        };
        finish_if(inner, group, CallEnd::Unreachable, |c| {
            c.with == to && c.phase == phase
        })
        .await;
    }
}

/// How many others are in the call's room now.
async fn others_in_room(inner: &Inner, group: GroupId) -> usize {
    people_in_room(inner, group).await.len()
}

/// The others in the call's room now.
async fn people_in_room(inner: &Inner, group: GroupId) -> Vec<DevicePublicKey> {
    let me = inner.identity.public_key();
    inner
        .node
        .room(group, fogata_state::FIRST_ROOM)
        .await
        .ok()
        .flatten()
        .map_or_else(Vec::new, |r| {
            r.participants
                .iter()
                .map(|(d, _, _)| *d)
                .filter(|d| *d != me)
                .collect()
        })
}

/// Someone came into the call's room: no longer away, nor waited for, nor rung. Those who
/// left hear that the call goes on.
pub(crate) async fn on_joined(inner: &Arc<Inner>, group: GroupId, device: DevicePublicKey) {
    let changed = inner.lock().calls.get_mut(&group).is_some_and(|c| {
        c.seen.insert(device);
        c.heard = true;
        let away = c.left.remove(&device).is_some();
        let rung = c.adding.remove(&device).is_some();
        away || rung || c.alone_since.is_some()
    });
    if changed {
        check_alone(inner, group).await;
        tell_going(inner, group, &[]).await;
    }
}

/// Events from the node were dropped (see [`crate::NodeEvent::Resync`]): who is in each
/// call's room is read anew, as those who came and went would have told.
pub(crate) async fn resync(inner: &Arc<Inner>) {
    let groups: Vec<GroupId> = inner.lock().calls.keys().copied().collect();
    for group in groups {
        let here: HashSet<DevicePublicKey> =
            people_in_room(inner, group).await.into_iter().collect();
        for device in &here {
            on_joined(inner, group, *device).await;
        }
        let gone: Vec<DevicePublicKey> =
            inner.lock().calls.get(&group).map_or_else(Vec::new, |c| {
                c.seen
                    .iter()
                    .filter(|d| !here.contains(d) && !c.left.contains_key(d))
                    .copied()
                    .collect()
            });
        for device in gone {
            on_left(inner, group, device).await;
        }
        check_alone(inner, group).await;
    }
    inner.emit(CoreEvent::CallsChanged);
}

/// Someone left the call's room: the others go on, and the last one waits a while.
pub(crate) async fn on_left(inner: &Arc<Inner>, group: GroupId, device: DevicePublicKey) {
    someone_left(inner, group, device).await;
}

async fn someone_left(inner: &Arc<Inner>, group: GroupId, device: DevicePublicKey) {
    let first = {
        let mut state = inner.lock();
        let Some(call) = state
            .calls
            .get_mut(&group)
            .filter(|c| c.phase == CallPhase::Connected)
        else {
            return;
        };
        call.left.insert(device, now_ms());
        !std::mem::replace(&mut call.telling, true)
    };
    check_alone(inner, group).await;
    tell_going(inner, group, &[]).await;
    if first {
        keep_telling(Arc::downgrade(inner), group);
    }
}

/// Tells `to`, or else the friends online who left the call `group`, that this device is
/// still in it, since when, and how long two or more people were.
async fn tell_going(inner: &Inner, group: GroupId, to: &[DevicePublicKey]) {
    let (to, signal) = {
        let state = inner.lock();
        let Some(call) = state
            .calls
            .get(&group)
            .filter(|c| c.phase == CallPhase::Connected)
        else {
            return;
        };
        let to: Vec<DevicePublicKey> = if to.is_empty() {
            call.left
                .keys()
                .filter(|d| state.friends_online.contains_key(*d))
                .copied()
                .collect()
        } else {
            to.to_vec()
        };
        (to, call.going(now_ms()))
    };
    for friend in to {
        let _ = send(inner, friend, group, signal.clone()).await;
    }
}

/// Tells those who left the call `group`, every [`GOING_EVERY_MS`] while it goes on here,
/// that it does.
fn keep_telling(inner: Weak<Inner>, group: GroupId) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(GOING_EVERY_MS)).await;
            let Some(inner) = inner.upgrade() else {
                return;
            };
            if !inner.lock().calls.contains_key(&group) {
                return;
            }
            tell_going(&inner, group, &[]).await;
        }
    });
}

/// A call step about a call this device left: someone still in it says it goes on, or left
/// it too. Once nobody is known to be in it, it is over here.
async fn on_left_signal(
    inner: &Arc<Inner>,
    from: DevicePublicKey,
    group: GroupId,
    signal: &CallSignal,
) {
    let now = now_ms();
    let (changed, over) = {
        let mut state = inner.lock();
        let Some(left) = state.left_calls.get_mut(&group) else {
            return;
        };
        if !left.log.shared_state().is_member(&from) {
            return;
        }
        let changed = match signal {
            CallSignal::Going {
                age_ms,
                together_ms,
            } => left.going(from, *age_ms, *together_ms, now),
            CallSignal::End => left.left(from, now),
            _ => return,
        };
        let over = if left.prune(now) {
            None
        } else {
            state.left_calls.remove(&group)
        };
        (changed, over)
    };
    if let Some(left) = over {
        left_over(inner, left).await;
    } else if changed {
        inner.emit(CoreEvent::CallsChanged);
    }
}

/// Checks a call this device left, every [`LEFT_CHECK_MS`] while it keeps its record from
/// `at_ms`, for those gone quiet; once nobody is known to be in it, it is over here.
fn watch_left(inner: Weak<Inner>, group: GroupId, at_ms: u64) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(LEFT_CHECK_MS)).await;
            let Some(inner) = inner.upgrade() else {
                return;
            };
            let (changed, over) = {
                let mut state = inner.lock();
                let Some(left) = state
                    .left_calls
                    .get_mut(&group)
                    .filter(|l| l.at_ms == at_ms)
                else {
                    return;
                };
                let before = left.inside.len();
                if left.prune(now_ms()) {
                    (left.inside.len() != before, None)
                } else {
                    (true, state.left_calls.remove(&group))
                }
            };
            if let Some(left) = over {
                left_over(&inner, left).await;
                return;
            }
            if changed {
                inner.emit(CoreEvent::CallsChanged);
            }
        }
    });
}

/// A call this device left is over as far as it knows: noted in the chats.
async fn left_over(inner: &Inner, left: LeftCall) {
    note(inner, &left.log, &left.seen, left.lasted_ms()).await;
    inner.emit(CoreEvent::CallsChanged);
}

/// As the core stops: the calls this device left are over for it, as far as it knows.
pub(crate) async fn on_shutdown(inner: &Inner) {
    let left: Vec<LeftCall> = inner
        .lock()
        .left_calls
        .drain()
        .map(|(_, left)| left)
        .collect();
    for left in left {
        note(inner, &left.log, &left.seen, left.lasted_ms()).await;
    }
}

/// Notes a call that is over in this device's chats with those it was in it with (see
/// [`note_with`]): who started it and how long it lasted.
async fn note(inner: &Inner, log: &GroupLog, seen: &HashSet<DevicePublicKey>, lasted_ms: u64) {
    let Ok(Some(book)) = inner.node.friends().await else {
        return;
    };
    let me = inner.identity.public_key();
    let Some(starter) = log.state().creator else {
        return;
    };
    for friend in note_with(log, me, seen, |d| book.friends.contains_key(d)) {
        let (mine, name) = if starter == me {
            (true, None)
        } else if starter == friend {
            (false, None)
        } else {
            (false, Some(name_of(inner, &book, starter, log)))
        };
        direct::note_call(inner, &book, friend, mine, name, lasted_ms);
    }
}

/// Those a call is noted with, each in its chat with this device: of the friends seen in the
/// call, the one who started it, or all of them when this device did; and those this device
/// rang into it or that rang it in. Each of them notes it with this device by the same rule.
fn note_with(
    log: &GroupLog,
    me: DevicePublicKey,
    seen: &HashSet<DevicePublicKey>,
    is_friend: impl Fn(&DevicePublicKey) -> bool,
) -> Vec<DevicePublicKey> {
    let starter = log.state().creator;
    let added_by: HashMap<DevicePublicKey, DevicePublicKey> = log
        .entries_in_order()
        .into_iter()
        .filter_map(|e| match &e.body {
            Body::Add { member, .. } => Some((*member, e.author)),
            _ => None,
        })
        .collect();
    let mut out: Vec<DevicePublicKey> = seen
        .iter()
        .filter(|d| **d != me && is_friend(d))
        .filter(|d| {
            starter == Some(me)
                || starter == Some(**d)
                || added_by.get(*d) == Some(&me)
                || added_by.get(&me) == Some(*d)
        })
        .copied()
        .collect();
    out.sort();
    out
}

/// Someone's name as known here: a friend's, a member's profile, or the one in the log.
fn name_of(inner: &Inner, book: &FriendBook, device: DevicePublicKey, log: &GroupLog) -> String {
    if let Some(friend) = book.friends.get(&device) {
        return friend.profile.name.clone();
    }
    if let Some(profile) = inner.lock().profiles.get(&device) {
        return profile.name.clone();
    }
    log.state()
        .members
        .get(&device)
        .map(|m| m.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| device.short_fingerprint())
}

/// Notes whether this device is the only one left in the call, and ends it when nobody
/// came back in [`ALONE_MS`].
async fn check_alone(inner: &Arc<Inner>, group: GroupId) {
    let others = others_in_room(inner, group).await;
    let started = {
        let mut state = inner.lock();
        let Some(call) = state
            .calls
            .get_mut(&group)
            .filter(|c| c.phase == CallPhase::Connected)
        else {
            return;
        };
        if others > 0 {
            call.alone_since = None;
            call.heard = true;
            None
        } else if call.alone_since.is_none() {
            let now = now_ms();
            call.alone_since = Some(now);
            Some(now)
        } else {
            None
        }
    };
    inner.emit(CoreEvent::CallsChanged);
    let Some(since) = started else {
        return;
    };
    let weak = Arc::downgrade(inner);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(ALONE_MS)).await;
        let Some(inner) = weak.upgrade() else {
            return;
        };
        alone_over(&inner, group, since).await;
    });
}

/// Ends call `group` when this device is still the only one in it, as it has been since
/// `since`.
async fn alone_over(inner: &Arc<Inner>, group: GroupId, since: u64) {
    let still = inner
        .lock()
        .calls
        .get(&group)
        .is_some_and(|c| c.alone_since == Some(since));
    if still && others_in_room(inner, group).await == 0 {
        meanwhile("alone", group).await;
        // Someone may have come back while the room was looked at.
        finish_if(inner, group, CallEnd::Alone, |c| {
            c.alone_since == Some(since)
        })
        .await;
    }
}

/// The call's group changed: kept. Every member manages it, so anyone in the call can ring
/// friends in.
pub(crate) async fn on_group_changed(inner: &Arc<Inner>, group: GroupId) {
    let Ok(Some(log)) = inner.node.group(group).await else {
        return;
    };
    if let Some(call) = inner.lock().calls.get_mut(&group) {
        call.log = log;
    }
}

/// The friends changed: a call of two with someone who is no longer a friend ends,
/// whichever of the two removed the other; a larger one goes on. Group voice rooms are not
/// calls and go on.
pub(crate) async fn on_friends_changed(inner: &Arc<Inner>, book: &FriendBook) {
    let gone: Vec<(GroupId, DevicePublicKey, CallPhase)> = inner
        .lock()
        .calls
        .iter()
        .filter(|(_, c)| {
            !book.friends.contains_key(&c.with)
                && (c.phase != CallPhase::Connected || c.log.shared_state().members.len() <= 2)
        })
        .map(|(g, c)| (*g, c.with, c.phase))
        .collect();
    for (group, with, phase) in gone {
        let (signal, reason) = match phase {
            CallPhase::Connected => (CallSignal::End, CallEnd::HungUp),
            CallPhase::Ringing => (CallSignal::Cancel, CallEnd::Cancelled),
            CallPhase::Incoming => (CallSignal::Decline, CallEnd::Missed),
        };
        // Most likely refused, the other no longer being a friend; it ends its side when
        // the removal reaches it.
        let _ = send(inner, with, group, signal).await;
        finish(inner, group, reason).await;
    }
}

/// A friend went offline: a call still ringing with it stops, and so does its ring into a
/// call; it is no longer in a call this device left.
pub(crate) async fn on_offline(inner: &Arc<Inner>, device: DevicePublicKey) {
    let (changed, over) = {
        let mut state = inner.lock();
        let mut changed = false;
        for left in state.left_calls.values_mut() {
            changed |= left.inside.remove(&device).is_some();
        }
        let empty: Vec<GroupId> = state
            .left_calls
            .iter()
            .filter(|(_, l)| l.inside.is_empty())
            .map(|(g, _)| *g)
            .collect();
        let over: Vec<LeftCall> = empty
            .iter()
            .filter_map(|g| state.left_calls.remove(g))
            .collect();
        (changed, over)
    };
    for left in over {
        left_over(inner, left).await;
    }
    if changed {
        inner.emit(CoreEvent::CallsChanged);
    }
    let (ringing, adding): (Vec<(GroupId, CallPhase)>, Vec<GroupId>) = {
        let mut state = inner.lock();
        let ringing = state
            .calls
            .iter()
            .filter(|(_, c)| c.with == device && c.phase != CallPhase::Connected)
            .map(|(g, c)| (*g, c.phase))
            .collect();
        let adding = state
            .calls
            .iter_mut()
            .filter_map(|(g, c)| c.adding.remove(&device).map(|_| *g))
            .collect();
        (ringing, adding)
    };
    for group in adding {
        added_ended(inner, group, device, CallEnd::Unreachable);
    }
    for (group, phase) in ringing {
        let reason = if phase == CallPhase::Incoming {
            CallEnd::Missed
        } else {
            CallEnd::Unreachable
        };
        // Still ringing: one answered meanwhile goes on in its room.
        finish_if(inner, group, reason, |c| {
            c.with == device && c.phase == phase
        })
        .await;
    }
}

async fn send(
    inner: &Inner,
    to: DevicePublicKey,
    group: GroupId,
    signal: CallSignal,
) -> Result<()> {
    let message = CallMessage {
        call: *group.as_bytes(),
        signal,
    };
    inner
        .node
        .send_call(to, message)
        .await?
        .map_err(anyhow::Error::msg)
}

/// Enters the room of call `group`, running its group from `log`, with `join`: noted on
/// the call, while it is still here, as entered. With the call rooms held (see
/// [`Inner::call_rooms`]): a call of the same group let go of meanwhile lets go of its room
/// and group before or after this, never in the middle.
async fn enter(
    inner: &Inner,
    group: GroupId,
    log: GroupLog,
    join: impl std::future::Future<Output = Result<()>>,
) -> Result<()> {
    let _rooms = inner.call_rooms.lock().await;
    if let Some(call) = inner.lock().calls.get_mut(&group) {
        call.entered = true;
    }
    inner.node.add_call(log).await?;
    join.await
}

/// Leaves the room of call `group` and lets its group go, unless a call of that group
/// entered its room here since this was decided: one that came back (someone in it rang
/// this device back in, and it answered). Looked at and done with the call rooms held (see
/// [`Inner::call_rooms`]), as [`enter`] does: a call that comes back after it was looked
/// at enters its room after this, and runs its group again.
async fn let_go(inner: &Inner, group: GroupId) {
    let _rooms = inner.call_rooms.lock().await;
    if inner.lock().calls.get(&group).is_some_and(|c| c.entered) {
        return;
    }
    meanwhile("letting go", group).await;
    // Serving this device's files in it stops before and after (see `shares::leaving`).
    crate::shares::leaving(inner, group, async {
        if let Ok(Some(room)) = inner.node.room(group, fogata_state::FIRST_ROOM).await
            && room.joined
        {
            let _ = inner.node.leave_room(group).await;
        }
        let _ = inner.node.drop_group(group).await;
    })
    .await;
}

/// Ends a call here: stops ringing friends into it, leaves its room and lets its group go.
/// The friends who left it before hear that this device left too. A call that went on and
/// that this device did not leave while others were in it is over: noted in the chats.
async fn finish(inner: &Inner, group: GroupId, reason: CallEnd) {
    finish_if(inner, group, reason, |_| true).await;
}

/// Ends call `group` as [`finish`] does, if `still` holds of it as it is taken: what
/// decided to end it and waited meanwhile (for a timer, for the room) leaves it be when it
/// changed since (answered, someone came back into it).
async fn finish_if(
    inner: &Inner,
    group: GroupId,
    reason: CallEnd,
    still: impl FnOnce(&CallEntry) -> bool,
) {
    if let Some(ending) = take_call(inner, group, still) {
        ended(inner, group, ending, reason).await;
    }
}

/// A call taken off this device's calls to be ended ([`take_call`], [`ended`]).
struct Ending {
    call: CallEntry,
    /// The friends online who left it.
    away: Vec<DevicePublicKey>,
    /// Whether this device left it while others were in it.
    left_it: bool,
}

/// Takes call `group` off this device's calls if `still` holds of it, looked at and taken
/// at once.
fn take_call(
    inner: &Inner,
    group: GroupId,
    still: impl FnOnce(&CallEntry) -> bool,
) -> Option<Ending> {
    let mut state = inner.lock();
    if !state.calls.get(&group).is_some_and(still) {
        return None;
    }
    let call = state.calls.remove(&group)?;
    let away: Vec<DevicePublicKey> = call
        .left
        .keys()
        .filter(|d| state.friends_online.contains_key(*d))
        .copied()
        .collect();
    let left_it = state.left_calls.contains_key(&group);
    Some(Ending {
        call,
        away,
        left_it,
    })
}

/// Ends call `group`, taken off this device's calls (see [`finish`]).
async fn ended(inner: &Inner, group: GroupId, ending: Ending, reason: CallEnd) {
    let now = now_ms();
    let Ending {
        call,
        away,
        left_it,
    } = ending;
    for friend in call.adding.keys() {
        let _ = send(inner, *friend, group, CallSignal::Cancel).await;
    }
    let mut seen = call.seen.clone();
    if let Ok(Some(room)) = inner.node.room(group, fogata_state::FIRST_ROOM).await {
        seen.extend(room.participants.iter().map(|(d, _, _)| *d));
    }
    let_go(inner, group).await;
    if call.phase == CallPhase::Connected {
        for friend in away {
            let _ = send(inner, friend, group, CallSignal::End).await;
        }
        // One that was never reached took no time: nobody was in it.
        let unheard = reason == CallEnd::Unreachable && !call.heard;
        if !left_it && reason != CallEnd::Elsewhere && !unheard {
            let started = call.started_ms.unwrap_or(call.since_ms);
            let lasted = call.alone_since.unwrap_or(now).saturating_sub(started);
            note(inner, &call.log, &seen, lasted).await;
        }
    }
    inner.emit(CoreEvent::CallEnded {
        group,
        with: call.with,
        reason,
    });
    inner.emit(CoreEvent::CallsChanged);
}

/// Ends the call with `reason` after `after_ms` if it is still in `phase`; a caller that
/// gave up tells the friend.
fn expire(inner: Weak<Inner>, group: GroupId, after_ms: u64, phase: CallPhase, reason: CallEnd) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(after_ms)).await;
        let Some(inner) = inner.upgrade() else {
            return;
        };
        meanwhile("expiring", group).await;
        // Taken at once: an answer that comes as the friend is told leaves it ended.
        let Some(ending) = take_call(&inner, group, |c| c.phase == phase) else {
            return;
        };
        if phase == CallPhase::Ringing {
            let _ = send(&inner, ending.call.with, group, CallSignal::Cancel).await;
        }
        ended(&inner, group, ending, reason).await;
    });
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use fogata_friends::Friend;
    use tokio::time::timeout;

    use super::*;
    use crate::{NetOptions, Store};

    async fn wait_for(
        events: &mut crate::CoreEvents,
        what: &str,
        mut done: impl FnMut(&CoreEvent) -> bool,
    ) {
        let found = timeout(Duration::from_secs(60), async {
            while let Some(event) = events.recv().await {
                if done(&event) {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap_or(false);
        assert!(found, "timed out waiting for {what}");
    }

    /// Options for two cores on this machine that dial each other at ports of their own, rather
    /// than look for each other on the local network, which a busy or locked-down computer
    /// may not carry.
    fn dialling() -> [NetOptions; 2] {
        // Both bound at once, so the two get different ports.
        let sockets = [0, 1].map(|_| std::net::UdpSocket::bind("127.0.0.1:0").unwrap());
        let ports = [&sockets[0], &sockets[1]].map(|s| s.local_addr().unwrap().port());
        drop(sockets);
        let to = |port: u16| {
            format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
                .parse()
                .unwrap()
        };
        [(ports[0], ports[1]), (ports[1], ports[0])].map(|(mine, other)| NetOptions {
            listen_port: mine,
            use_dht: false,
            use_mdns: false,
            peers: vec![to(other)],
            ..Default::default()
        })
    }

    /// Two data folders whose profiles are already friends.
    fn friends(base: &Path) -> [PathBuf; 2] {
        let dirs = [base.join("ana"), base.join("bea")];
        for (dir, name) in dirs.iter().zip(["Ana", "Bea"]) {
            Core::create_profile(dir, name, None).unwrap();
        }
        let stores = dirs.clone().map(|dir| Store::open(dir).unwrap());
        let books = [&stores[0], &stores[1]].map(|s| s.friends().unwrap().unwrap());
        for (i, store) in stores.iter().enumerate() {
            let other = &books[1 - i];
            let mut book = books[i].clone();
            book.friends.insert(
                other.profile.device,
                Friend {
                    profile: other.profile.clone(),
                    secret: other.secret.clone(),
                    since_ms: 2,
                    delivered: true,
                    banner: None,
                    bio: None,
                },
            );
            store.save_friends(&book).unwrap();
        }
        dirs
    }

    fn identity() -> fogata_crypto::DeviceIdentity {
        fogata_crypto::DeviceIdentity::generate().unwrap()
    }

    #[test]
    fn a_call_left_goes_on_while_those_in_it_say_so() {
        let ana = identity();
        let (bea, carla) = (identity().public_key(), identity().public_key());
        let mut log = GroupLog::create(&ana, "Call", 1).unwrap();
        log.add(&ana, bea, "Bea", 1).unwrap();
        log.add(&ana, carla, "Carla", 1).unwrap();
        // Ana left at 100 s, the call having started at 40 s; Bea and Carla were in it.
        let mut left = LeftCall {
            log,
            at_ms: 100_000,
            capable: HashSet::new(),
            started_ms: 40_000,
            together_until: 100_000,
            seen: HashSet::from([bea, carla]),
            inside: HashMap::from([(bea, (100_000, true)), (carla, (100_000, true))]),
            gone: HashMap::new(),
        };
        // A call of three: anyone in it takes Ana back.
        assert!(left.returnable(&bea, 100_000) && left.returnable(&carla, 100_000));
        // Bea says it goes on, with two or more in it: the start by her clock counts if
        // earlier.
        assert!(!left.going(bea, 91_000, 91_000, 130_000));
        assert_eq!((left.started_ms, left.together_until), (39_000, 130_000));
        // Carla said nothing for too long: no longer counted in it.
        assert!(left.prune(145_000));
        assert!(!left.returnable(&carla, 145_000) && left.returnable(&bea, 145_000));
        // Bea is alone since 130 s: what she says no longer moves the end.
        assert!(!left.going(bea, 106_000, 91_000, 145_000));
        assert_eq!(left.together_until, 130_000);
        // She leaves too: nobody is in it, and it lasted from 39 s to 130 s.
        assert!(left.left(bea, 150_000));
        assert!(!left.prune(150_000));
        assert_eq!(left.lasted_ms(), 91_000);
        // A word of hers that comes after she left, out of order, does not bring her back.
        assert!(!left.going(bea, 112_000, 91_000, 151_000));
        assert!(left.inside.is_empty() && !left.returnable(&bea, 151_000));
        // Later on, she may well be back in it.
        assert!(left.going(bea, 160_000, 160_000, 200_000));
        assert_eq!(left.together_until, 200_000);
        // Leaving while with others moves the end to then.
        assert!(left.left(bea, 210_000));
        assert_eq!(left.together_until, 210_000);
    }

    #[test]
    fn a_ring_takes_only_a_call_of_the_two_made_by_the_one_ringing() {
        let (ana, bea, carla) = (identity(), identity(), identity());
        let me = bea.public_key();
        let mut log = GroupLog::create(&ana, "Call", 1).unwrap();
        log.add(&ana, me, "Bea", 1).unwrap();
        let from = ana.public_key();
        for rung in [Rung::New, Rung::Known, Rung::Invited] {
            assert!(ring_fits(&log, from, me, rung), "{rung:?}");
        }
        // Made by someone else, or with a stranger in it: refused as a new call.
        let mut theirs = GroupLog::create(&carla, "Call", 1).unwrap();
        theirs.add(&carla, from, "Ana", 1).unwrap();
        theirs.add(&carla, me, "Bea", 1).unwrap();
        assert!(!ring_fits(&theirs, from, me, Rung::New));
        log.add(&ana, carla.public_key(), "Carla", 1).unwrap();
        assert!(!ring_fits(&log, from, me, Rung::New));
        // An invitation into a call this device is in, or left, takes it with more members
        // than a call takes at once: those who left stay members, and only those in it now
        // count.
        assert!(ring_fits(&log, from, me, Rung::Known));
        while log.state().members.len() < usize::from(fogata_state::MAX_MEMBERS) {
            log.add(&ana, identity().public_key(), "Ana's friend", 1)
                .unwrap();
        }
        assert!(log.state().members.len() > MAX_IN_CALL);
        assert!(ring_fits(&log, from, me, Rung::Known));
        assert!(!ring_fits(&log, from, me, Rung::New));
        // Without the one ringing, or without this device: never.
        let mut without = GroupLog::create(&ana, "Call", 1).unwrap();
        without.add(&ana, carla.public_key(), "Carla", 1).unwrap();
        for rung in [Rung::New, Rung::Known, Rung::Invited] {
            assert!(!ring_fits(&without, from, me, rung));
            assert!(!ring_fits(&without, carla.public_key(), me, rung));
        }
    }

    #[test]
    fn an_invitation_does_not_open_a_call_made_by_someone_else() {
        let (ana, bea, carla) = (identity(), identity(), identity());
        let me = bea.public_key();
        let from = ana.public_key();
        // Carla's call, which Carla put this device and Ana in: Ana cannot ring this device
        // into it as a new call, though it is fine once this device is in it.
        let mut theirs = GroupLog::create(&carla, "Call", 1).unwrap();
        theirs.add(&carla, from, "Ana", 1).unwrap();
        theirs.add(&carla, me, "Bea", 1).unwrap();
        assert!(!ring_fits(&theirs, from, me, Rung::Invited));
        assert!(ring_fits(&theirs, carla.public_key(), me, Rung::Invited));
        assert!(ring_fits(&theirs, from, me, Rung::Known));
        assert_eq!(added_by(&theirs, &me), Some(carla.public_key()));
        assert_eq!(added_by(&theirs, &carla.public_key()), None);
        // Ana's name there is Carla's word for her; Carla's, her own; this device's, its own.
        assert_eq!(
            named_by_another(&theirs, from, me),
            Some(carla.public_key())
        );
        assert_eq!(named_by_another(&theirs, carla.public_key(), me), None);
        assert_eq!(named_by_another(&theirs, me, me), None);
        assert_eq!(named_by_another(&theirs, from, carla.public_key()), None);

        // Ana rings this device into Carla's call herself: as a call goes on, with no more
        // people than a call takes.
        let mut call = GroupLog::create(&carla, "Call", 1).unwrap();
        call.add(&carla, from, "Ana", 1).unwrap();
        call.add(&ana, me, "Bea", 1).unwrap();
        assert!(ring_fits(&call, from, me, Rung::Invited));
        while call.state().members.len() < MAX_IN_CALL {
            call.add(&ana, identity().public_key(), "Ana's friend", 1)
                .unwrap();
        }
        assert!(ring_fits(&call, from, me, Rung::Invited));
        call.add(&ana, identity().public_key(), "One more", 1)
            .unwrap();
        assert!(!ring_fits(&call, from, me, Rung::Invited));
        assert!(ring_fits(&call, from, me, Rung::Known));
    }

    #[test]
    fn a_call_of_two_takes_back_only_a_device_that_takes_group_calls() {
        let ana = identity();
        let bea = identity().public_key();
        let mut log = GroupLog::create(&ana, "Call", 1).unwrap();
        log.add(&ana, bea, "Bea", 1).unwrap();
        let mut left = LeftCall {
            log,
            at_ms: 10,
            capable: HashSet::new(),
            started_ms: 0,
            together_until: 10,
            seen: HashSet::from([bea]),
            inside: HashMap::from([(bea, (10, false))]),
            gone: HashMap::new(),
        };
        assert!(!left.returnable(&bea, 20));
        left.capable.insert(bea);
        assert!(left.returnable(&bea, 20));
        assert!(!left.returnable(&bea, 10 + GONE_AFTER_MS));
        // Someone not in the call's group is never counted.
        assert!(!left.returnable(&identity().public_key(), 20));
    }

    #[test]
    fn a_call_is_noted_with_its_starter_and_with_who_rang_whom_in() {
        let (ana, bea) = (identity(), identity());
        let (carla, dani, eva) = (
            identity().public_key(),
            identity().public_key(),
            identity().public_key(),
        );
        let (a, b) = (ana.public_key(), bea.public_key());
        // Ana starts it with Bea and rings Carla in; Bea rings Dani in.
        let mut log = GroupLog::create(&ana, "Call", 1).unwrap();
        log.add(&ana, b, "Bea", 1).unwrap();
        log.add(&ana, carla, "Carla", 2).unwrap();
        log.add(&bea, dani, "Dani", 3).unwrap();
        log.add(&bea, eva, "Eva", 4).unwrap();
        let all = HashSet::from([a, b, carla, dani, eva]);
        let friends = |of: &[DevicePublicKey]| {
            let of: HashSet<DevicePublicKey> = of.iter().copied().collect();
            move |d: &DevicePublicKey| of.contains(d)
        };
        let sorted = |mut v: Vec<DevicePublicKey>| {
            v.sort();
            v
        };
        // Ana started it: with every friend she saw in it.
        assert_eq!(
            note_with(&log, a, &all, friends(&[b, carla])),
            sorted(vec![b, carla])
        );
        // Bea: with Ana, who started it, and Dani, whom she rang in; not Carla, a friend
        // too, whom someone else rang in.
        assert_eq!(
            note_with(&log, b, &all, friends(&[a, carla, dani])),
            sorted(vec![a, dani])
        );
        // Carla: with Ana only.
        assert_eq!(note_with(&log, carla, &all, friends(&[a, dani])), vec![a]);
        // Dani, no friend of Ana's: with Bea, who rang her in.
        assert_eq!(note_with(&log, dani, &all, friends(&[b, eva])), vec![b]);
        // Eva, a friend of Ana's who never saw her in the call: with Bea only.
        let without_ana: HashSet<DevicePublicKey> =
            all.iter().filter(|d| **d != a).copied().collect();
        assert_eq!(
            note_with(&log, eva, &without_ana, friends(&[a, b])),
            vec![b]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_ring_while_appearing_offline_is_declined_and_shows_as_missed() {
        let base = std::env::temp_dir().join(format!("fogata-ring-offline-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let [dir_a, dir_b] = friends(&base);
        let [options_a, options_b] = dialling();
        let (ana, mut events_a) = Core::start(dir_a, options_a).unwrap();
        let (bea, mut events_b) = Core::start(dir_b, options_b).unwrap();
        let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
        wait_for(&mut events_a, "Bea online", online).await;
        wait_for(&mut events_b, "Ana online", online).await;

        // Ana appears offline, and Bea has not heard yet (as a copy from before it would
        // not): her ring is declined, and Ana sees a missed call.
        ana.inner.lock().appear_offline = true;
        let call = bea.start_call(ana.me()).await.unwrap();
        wait_for(&mut events_b, "the decline", |e| {
            matches!(e, CoreEvent::CallEnded { group, reason: CallEnd::Declined, .. } if *group == call)
        })
        .await;
        wait_for(&mut events_a, "the missed call", |e| {
            matches!(
                e,
                CoreEvent::CallEnded { group, with, reason: CallEnd::MissedOffline }
                    if *group == call && *with == bea.me()
            )
        })
        .await;
        // It never rang here.
        assert!(ana.calls().await.unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Ana's core alone, with Bea a friend of hers who is not online; and Bea's identity.
    fn ana_alone(base: &Path) -> (Core, fogata_crypto::DeviceIdentity) {
        let _ = std::fs::remove_dir_all(base);
        let [dir_a, dir_b] = friends(base);
        let (bea, _) = Store::open(dir_b).unwrap().identity().unwrap();
        let options = NetOptions {
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        };
        let (ana, _events) = Core::start(dir_a, options).unwrap();
        (ana, bea)
    }

    fn step(group: GroupId, signal: CallSignal) -> CallMessage {
        CallMessage {
            call: *group.as_bytes(),
            signal,
        }
    }

    #[test]
    fn what_those_in_a_call_say_of_its_start_stays_within_the_call() {
        let ana = identity();
        let bea = identity().public_key();
        let now = 2_000_000_000_000;
        for (made, earliest) in [
            // Made a minute ago: it cannot have started before.
            (now - 60_000, now - 60_000),
            // Its maker dated it long ago: no call is taken to last more than two days.
            (1, now - LONGEST_CALL_MS),
        ] {
            let mut log = GroupLog::create(&ana, "Call", made).unwrap();
            log.add(&ana, bea, "Bea", made).unwrap();
            let mut left = LeftCall {
                log,
                at_ms: now - 1000,
                capable: HashSet::new(),
                started_ms: now - 30_000,
                together_until: now - 1000,
                seen: HashSet::from([bea]),
                inside: HashMap::from([(bea, (now - 1000, true))]),
                gone: HashMap::new(),
            };
            left.going(bea, u64::MAX, u64::MAX, now);
            assert_eq!(left.started_ms, earliest);
            assert!(left.lasted_ms() <= now - earliest + LEFT_SETTLES_MS);
            // What fits stays as it was said.
            left.started_ms = now - 30_000;
            left.going(bea, 40_000, 40_000, now);
            assert_eq!(left.started_ms, now - 40_000);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_start_a_member_says_of_a_call_this_device_is_in_stays_within_it() {
        let base = std::env::temp_dir().join(format!("fogata-call-start-{}", std::process::id()));
        let (ana, bea) = ana_alone(&base);
        let made = now_ms() - 60_000;
        let mut log = GroupLog::create(&bea, "Call", made).unwrap();
        log.add(&bea, ana.me(), "Ana", made).unwrap();
        let group = log.group_id();
        ana.inner.lock().calls.insert(
            group,
            CallEntry::new(bea.public_key(), false, CallPhase::Connected, log),
        );
        let going = CallSignal::Going {
            age_ms: u64::MAX,
            together_ms: u64::MAX,
        };
        on_call(&ana.inner, bea.public_key(), step(group, going)).await;
        let started = ana.inner.lock().calls[&group].started_ms.unwrap();
        assert!(started >= made, "{started} is before the call was made");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A group made up by Dan under the id of a call he is not in, with him and Ana in it.
    fn made_up_under(
        group: GroupId,
        dan: &fogata_crypto::DeviceIdentity,
        ana: DevicePublicKey,
    ) -> Vec<Vec<u8>> {
        let (start, _) = fogata_state::Rotation::new(&group, [dan.public_key()]).unwrap();
        let create = fogata_state::Entry::sign(
            dan,
            group,
            Vec::new(),
            now_ms(),
            fogata_state::Body::Create {
                name: "Call".into(),
                start,
            },
        );
        let mut log = GroupLog::empty(group);
        log.insert(create).unwrap();
        log.add(dan, ana, "Ana", now_ms()).unwrap();
        log.entries().map(|e| e.encode()).collect()
    }

    /// A friend that learnt the id of a call it is not in rings or invites this device into
    /// it with a group it made up under that id: it is not answered as one coming back, and
    /// does not take the place of a call this device left.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_friend_not_in_a_call_cannot_ring_into_it_under_its_id() {
        let base = std::env::temp_dir().join(format!("fogata-call-id-{}", std::process::id()));
        let (ana, bea) = ana_alone(&base);
        let dan = identity();
        let mut log = GroupLog::create(&bea, "Call", now_ms()).unwrap();
        log.add(&bea, ana.me(), "Ana", now_ms()).unwrap();
        let group = log.group_id();
        ana.inner.lock().calls.insert(
            group,
            CallEntry::new(bea.public_key(), false, CallPhase::Connected, log.clone()),
        );
        let made_up = made_up_under(group, &dan, ana.me());
        for signal in [
            CallSignal::Ring {
                log: made_up.clone(),
            },
            CallSignal::Invite {
                log: made_up.clone(),
            },
        ] {
            on_call(&ana.inner, dan.public_key(), step(group, signal)).await;
        }
        assert!(
            !ana.inner.lock().calls[&group]
                .told
                .contains(&dan.public_key())
        );
        // Left here: Dan's ring does not open a call under its id.
        let now = now_ms();
        let left = LeftCall {
            log,
            at_ms: now,
            capable: HashSet::new(),
            started_ms: now - 30_000,
            together_until: now,
            seen: HashSet::new(),
            inside: HashMap::new(),
            gone: HashMap::new(),
        };
        {
            let mut state = ana.inner.lock();
            state.calls.remove(&group);
            state.left_calls.insert(group, left);
        }
        let ring = CallSignal::Ring { log: made_up };
        on_call(&ana.inner, dan.public_key(), step(group, ring)).await;
        assert!(!ana.is_call(group));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn only_the_one_who_rang_says_the_call_was_taken_elsewhere() {
        let base =
            std::env::temp_dir().join(format!("fogata-call-elsewhere-{}", std::process::id()));
        let (ana, bea) = ana_alone(&base);
        let cleo = identity().public_key();
        let mut log = GroupLog::create(&bea, "Call", now_ms()).unwrap();
        log.add(&bea, ana.me(), "Ana", now_ms()).unwrap();
        log.add(&bea, cleo, "Cleo", now_ms()).unwrap();
        let group = log.group_id();
        ana.inner.lock().calls.insert(
            group,
            CallEntry::new(bea.public_key(), false, CallPhase::Connected, log),
        );
        // Cleo, in the call too, cannot end it here.
        on_call(&ana.inner, cleo, step(group, CallSignal::Elsewhere)).await;
        assert!(ana.is_call(group));
        on_call(
            &ana.inner,
            bea.public_key(),
            step(group, CallSignal::Elsewhere),
        )
        .await;
        assert!(!ana.is_call(group));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A call answered here whose room cannot be entered (here kept out of it) does not stay
    /// as a call this device is in: every other ring would hear it is busy. A record of
    /// having left it before stays, and no line in the chat says a call took place.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_call_whose_room_cannot_be_entered_is_not_answered() {
        let base =
            std::env::temp_dir().join(format!("fogata-call-kept-out-{}", std::process::id()));
        let (ana, bea) = ana_alone(&base);
        let now = now_ms();
        let mut log = GroupLog::create(&bea, "Call", now - 60_000).unwrap();
        log.add(&bea, ana.me(), "Ana", now - 60_000).unwrap();
        log.keep_out(&bea, fogata_state::FIRST_ROOM, ana.me(), None, now)
            .unwrap();
        let group = log.group_id();
        {
            let mut state = ana.inner.lock();
            state.left_calls.insert(
                group,
                LeftCall {
                    log: log.clone(),
                    at_ms: now - 10_000,
                    capable: HashSet::new(),
                    started_ms: now - 50_000,
                    together_until: now - 10_000,
                    seen: HashSet::from([bea.public_key()]),
                    inside: HashMap::from([(bea.public_key(), (now, true))]),
                    gone: HashMap::new(),
                },
            );
            state.calls.insert(
                group,
                CallEntry::new(bea.public_key(), false, CallPhase::Incoming, log),
            );
        }
        assert!(ana.accept_call(group).await.is_err());
        {
            let state = ana.inner.lock();
            assert!(!state.calls.values().any(CallEntry::busy));
            assert!(state.calls.is_empty());
            assert!(state.left_calls.contains_key(&group));
        }
        let chat = ana.direct_chat(bea.public_key(), 50).await.unwrap();
        assert!(chat.items.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Whether this device is in the room of call `group`.
    async fn in_room(core: &Core, group: GroupId) -> bool {
        matches!(
            core.inner.node.room(group, fogata_state::FIRST_ROOM).await,
            Ok(Some(room)) if room.joined
        )
    }

    /// A call that ends while this device enters its room (the friend cancels it, the ring
    /// times out, the person hangs up, before the room is entered) leaves no room entered
    /// with nothing tracking the call: one answered, one that crossed this device's own, and
    /// one this device rings.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_call_that_ends_while_its_room_is_entered_leaves_it() {
        let base = std::env::temp_dir().join(format!("fogata-call-gone-{}", std::process::id()));
        let (ana, bea) = ana_alone(&base);
        let now = now_ms();
        let ends_meanwhile = || {
            ENTERING.lock().unwrap().get_or_insert_default().insert(
                bea.public_key(),
                Box::new(|inner: &Inner, group: GroupId| {
                    inner.lock().calls.remove(&group);
                }),
            );
        };
        let mut log = GroupLog::create(&bea, "Call", now - 60_000).unwrap();
        log.add(&bea, ana.me(), "Ana", now - 60_000).unwrap();
        let group = log.group_id();
        ana.inner.lock().calls.insert(
            group,
            CallEntry::new(bea.public_key(), false, CallPhase::Incoming, log),
        );
        ends_meanwhile();
        assert!(ana.accept_call(group).await.is_err());
        assert!(ana.inner.lock().calls.is_empty());
        assert!(
            !in_room(&ana, group).await,
            "still in the room of a call gone"
        );

        let mut theirs = GroupLog::create(&bea, "Call", now).unwrap();
        theirs.add(&bea, ana.me(), "Ana", now).unwrap();
        let mine = GroupLog::create(&ana.inner.identity, "Call", now).unwrap();
        let mine_id = mine.group_id();
        ana.inner.lock().calls.insert(
            mine_id,
            CallEntry::new(bea.public_key(), true, CallPhase::Ringing, mine),
        );
        let group = theirs.group_id();
        ends_meanwhile();
        take_crossed(&ana.inner, bea.public_key(), group, mine_id, theirs).await;
        assert!(ana.inner.lock().calls.is_empty());
        assert!(
            !in_room(&ana, group).await,
            "still in the room of a crossed call gone"
        );

        // Ana's own call, hung up as its room is entered.
        ana.inner
            .lock()
            .friends_online
            .insert(bea.public_key(), fogata_net::Path::Direct);
        let started = Arc::new(std::sync::Mutex::new(None));
        let noted = started.clone();
        ENTERING.lock().unwrap().get_or_insert_default().insert(
            bea.public_key(),
            Box::new(move |inner: &Inner, group: GroupId| {
                inner.lock().calls.remove(&group);
                *noted.lock().unwrap() = Some(group);
            }),
        );
        let _ = ana.start_call(bea.public_key()).await;
        let group = started.lock().unwrap().expect("the call was started");
        assert!(ana.inner.lock().calls.is_empty());
        assert!(
            !in_room(&ana, group).await,
            "still in the room of a call hung up"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A call that crossed this device's own and whose room cannot be entered never took
    /// place: no line in the chat says it did.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_crossed_call_that_does_not_go_on_is_not_noted() {
        let base = std::env::temp_dir().join(format!("fogata-call-crossed-{}", std::process::id()));
        let (ana, bea) = ana_alone(&base);
        let now = now_ms();
        let mut theirs = GroupLog::create(&bea, "Call", now).unwrap();
        theirs.add(&bea, ana.me(), "Ana", now).unwrap();
        theirs
            .keep_out(&bea, fogata_state::FIRST_ROOM, ana.me(), None, now)
            .unwrap();
        let mine = GroupLog::create(&ana.inner.identity, "Call", now).unwrap();
        let mine_id = mine.group_id();
        ana.inner.lock().calls.insert(
            mine_id,
            CallEntry::new(bea.public_key(), true, CallPhase::Ringing, mine),
        );
        take_crossed(
            &ana.inner,
            bea.public_key(),
            theirs.group_id(),
            mine_id,
            theirs,
        )
        .await;
        assert!(ana.inner.lock().calls.is_empty());
        let chat = ana.direct_chat(bea.public_key(), 50).await.unwrap();
        assert!(chat.items.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// This device answers, its Accept does not reach the friend and nobody is in the
    /// room: the call never took place, and no line in the chat says it did.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_answer_that_is_not_delivered_is_not_noted() {
        let base = std::env::temp_dir().join(format!("fogata-call-unheard-{}", std::process::id()));
        let (ana, bea) = ana_alone(&base);
        let now = now_ms();
        let mut log = GroupLog::create(&bea, "Call", now).unwrap();
        log.add(&bea, ana.me(), "Ana", now).unwrap();
        let group = log.group_id();
        {
            let mut state = ana.inner.lock();
            state.calls.insert(
                group,
                CallEntry::new(bea.public_key(), false, CallPhase::Incoming, log),
            );
            answered(&mut state, group);
        }
        on_undelivered(&ana.inner, bea.public_key(), *group.as_bytes(), false).await;
        assert!(ana.inner.lock().calls.is_empty());
        let chat = ana.direct_chat(bea.public_key(), 50).await.unwrap();
        assert!(chat.items.is_empty(), "{:?}", chat.items.len());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Sets the call `group` to come back at `point` of its handling on `core`: `from`, in
    /// it, rings this device back into it, which answers. What the answer gave, once it did.
    fn rung_back_meanwhile(
        core: &Core,
        from: DevicePublicKey,
        group: GroupId,
        point: &'static str,
    ) -> Arc<std::sync::Mutex<Option<Result<()>>>> {
        let answered = Arc::new(std::sync::Mutex::new(None));
        let (core, kept) = (core.clone(), answered.clone());
        let pause: Pause = Box::new(move || {
            Box::pin(async move {
                let log = core.inner.node.group(group).await.unwrap().unwrap();
                let invite = CallSignal::Invite { log: encode(&log) };
                on_call(&core.inner, from, step(group, invite)).await;
                let answering = core.clone();
                let answer = kept.clone();
                tokio::spawn(async move {
                    let result = answering.accept_call(group).await;
                    *answer.lock().unwrap() = Some(result);
                });
                // Answered at once, unless what lets the call go holds its room meanwhile.
                for _ in 0..30 {
                    if kept.lock().unwrap().is_some() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            })
        });
        PAUSES
            .lock()
            .unwrap()
            .get_or_insert_default()
            .insert((point, group), pause);
        answered
    }

    /// Waits for the answer [`rung_back_meanwhile`] set up, and checks the call that came
    /// back is in its room, with its group running.
    async fn back_in(core: &Core, group: GroupId, answered: &std::sync::Mutex<Option<Result<()>>>) {
        let result = timeout(Duration::from_secs(30), async {
            loop {
                if let Some(result) = answered.lock().unwrap().take() {
                    return result;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("the call that came back was not answered");
        result.unwrap();
        assert_eq!(
            core.inner.lock().calls.get(&group).map(|c| c.phase),
            Some(CallPhase::Connected)
        );
        assert!(
            in_room(core, group).await,
            "the call that came back is not in its room"
        );
        assert!(
            core.inner.node.group(group).await.unwrap().is_some(),
            "the group of the call that came back was let go of"
        );
    }

    /// The room and group of a call this device lets go of, while the same call comes back
    /// (someone in it rings this device back in, and it answers), stay for the call that
    /// came back: as a call ended while its room was entered is let go of, and as this
    /// device hangs up.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_call_let_go_of_as_it_comes_back_keeps_its_room() {
        let base = std::env::temp_dir().join(format!("fogata-call-back-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let [dir_a, dir_b] = friends(&base);
        let [options_a, options_b] = dialling();
        let (ana, mut events_a) = Core::start(dir_a, options_a).unwrap();
        let (bea, mut events_b) = Core::start(dir_b, options_b).unwrap();
        let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
        wait_for(&mut events_a, "Bea online", online).await;
        wait_for(&mut events_b, "Ana online", online).await;

        // Bea calls; the ring stops as Ana enters the room, and Bea rings her back in just
        // as Ana lets the room go.
        let group = bea.start_call(ana.me()).await.unwrap();
        wait_for(
            &mut events_a,
            "the ring",
            |e| matches!(e, CoreEvent::CallIncoming { group: g, .. } if *g == group),
        )
        .await;
        ENTERING.lock().unwrap().get_or_insert_default().insert(
            bea.me(),
            Box::new(|inner: &Inner, group: GroupId| {
                inner.lock().calls.remove(&group);
            }),
        );
        let answered = rung_back_meanwhile(&ana, bea.me(), group, "letting go");
        assert!(ana.accept_call(group).await.is_err());
        back_in(&ana, group, &answered).await;

        // Ana hangs up, and Bea rings her back in as she leaves.
        let answered = rung_back_meanwhile(&ana, bea.me(), group, "letting go");
        ana.end_call(group).await.unwrap();
        back_in(&ana, group, &answered).await;
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A ring that times out as the friend answers is not ended: the call goes on.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_ring_answered_as_it_times_out_goes_on() {
        let base = std::env::temp_dir().join(format!("fogata-call-late-{}", std::process::id()));
        let (ana, bea) = ana_alone(&base);
        let mut log = GroupLog::create(&ana.inner.identity, "Call", now_ms()).unwrap();
        log.add(&ana.inner.identity, bea.public_key(), "Bea", now_ms())
            .unwrap();
        let group = log.group_id();
        ana.inner.lock().calls.insert(
            group,
            CallEntry::new(bea.public_key(), true, CallPhase::Ringing, log),
        );
        let core = ana.clone();
        let from = bea.public_key();
        let pause: Pause = Box::new(move || {
            Box::pin(async move {
                on_call(&core.inner, from, step(group, CallSignal::Accept)).await;
            })
        });
        PAUSES
            .lock()
            .unwrap()
            .get_or_insert_default()
            .insert(("expiring", group), pause);
        expire(
            Arc::downgrade(&ana.inner),
            group,
            0,
            CallPhase::Ringing,
            CallEnd::NoAnswer,
        );
        // The ring timed out, and was answered meanwhile.
        for _ in 0..100 {
            let taken = !PAUSES
                .lock()
                .unwrap()
                .get_or_insert_default()
                .contains_key(&("expiring", group));
            if taken
                && ana.inner.lock().calls.get(&group).map(|c| c.phase) != Some(CallPhase::Ringing)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(
            ana.inner.lock().calls.get(&group).map(|c| c.phase),
            Some(CallPhase::Connected),
            "a call answered as its ring timed out was ended"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The last one in a call, waiting, is not left alone in it when someone came back just
    /// as the wait ran out.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_call_someone_came_back_to_as_the_wait_ran_out_goes_on() {
        let base = std::env::temp_dir().join(format!("fogata-call-alone-{}", std::process::id()));
        let (ana, bea) = ana_alone(&base);
        let mut log = GroupLog::create(&bea, "Call", now_ms()).unwrap();
        log.add(&bea, ana.me(), "Ana", now_ms()).unwrap();
        let group = log.group_id();
        let since = now_ms() - ALONE_MS;
        let mut call = CallEntry::new(bea.public_key(), false, CallPhase::Connected, log);
        call.alone_since = Some(since);
        ana.inner.lock().calls.insert(group, call);
        let core = ana.clone();
        let pause: Pause = Box::new(move || {
            Box::pin(async move {
                // Bea is back in the room: this device is no longer alone there.
                if let Some(call) = core.inner.lock().calls.get_mut(&group) {
                    call.alone_since = None;
                }
            })
        });
        PAUSES
            .lock()
            .unwrap()
            .get_or_insert_default()
            .insert(("alone", group), pause);
        alone_over(&ana.inner, group, since).await;
        assert!(
            ana.inner.lock().calls.contains_key(&group),
            "a call someone came back to was ended as alone"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
