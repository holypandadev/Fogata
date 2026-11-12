//! Friends on the node: announcing this device to its friends, finding them, recognising them
//! on new connections, and friend requests made with links and codes.
//!
//! This file holds what is kept of friends and the friend protocol's requests and answers;
//! finding friends on the DHT is in friend_lookups.rs, reaching them in friend_contact.rs,
//! what they are told of this device in friend_profile.rs, and groups offered to them in
//! friend_offers.rs.
//!
//! A device announces itself under its own friend secret, so only its friends can find it,
//! with a proof that the announcement is its own (see lan.rs): its other friends know that
//! secret too. The device that connects proves it knows that secret and says, sealed to the
//! friend, who it is; only then does the friend answer. On the local network a device asks
//! each peer for a probe tag, which only that peer's friends recognise.
//!
//! A device that removes a friend tells it, so it forgets that device as well: right away if
//! it is connected, or by looking it up until it is.
//!
//! What concerns two devices alone (a new friend secret or an acceptance to deliver, a
//! removal to tell) meets under the secret those two share (`FriendSecret::pair`), never
//! under a friend's own secret, which its other friends know. Getting through two NATs needs
//! both ends to dial each other at the same moments, so both ends must find each other: the
//! device that made a link or shows a code looks up the peers announcing under it, and a
//! device that accepted a request announces under the pair secret, which the requester looks
//! up while it waits for answers.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::IpAddr,
    time::{Duration, Instant},
};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
use fogata_friends::{
    Animated, Animation, CODE_LIFETIME_MS, CodeInviter, CodeReply, CodeRequester, CodeSecrets,
    CodeStart, FriendBook, FriendError, FriendLink, FriendSecret, Introduction, LINK_LIFETIME_MS,
    MAX_FAILED_RUNS, MAX_LINK_ADDRS, MutualAsk, RequestOutcome,
};
use fogata_proto::{FriendRequest, FriendResponse};
use fogata_state::Entry;
use libp2p::{
    Multiaddr, PeerId,
    kad::RecordKey,
    request_response::{self, OutboundRequestId},
    swarm::dial_opts::{DialOpts, PeerCondition},
};

use super::friend_contact::Contact;
use super::friend_offers::{OfferIn, OfferPart, Offering};
use super::friend_profile::MutualReply;
use super::{Node, is_loopback, limits::FriendUpdate, now_ms};
use crate::{
    Command, NodeEvent, Path, addr,
    addr::{OwnNetworks, Reach},
};

/// After starting, look for friends often for a while: friends that started at about the
/// same time cannot find each other before both have announced themselves.
const EAGER: Duration = Duration::from_secs(150);
/// How long a friend or former friend that stays away waits between lookups, one step after
/// another (and a little more for some, so that they spread out): each lookup is four DHT
/// searches (its secret and the one the two share, each in two windows) of some hundreds
/// of requests, and a device with dozens of friends away would otherwise make them every
/// couple of minutes for as long as it runs. One that just went is looked up at every slot
/// for a while (see `SEARCH_HARD`), and a change here or of the network starts over.
const AWAY_STEPS: [Duration; 4] = [
    Duration::from_secs(2 * 60),
    Duration::from_secs(5 * 60),
    Duration::from_secs(15 * 60),
    Duration::from_secs(60 * 60),
];
/// Also look often for a while after a request, an answer or a new friend secret.
const AFTER_CHANGE: Duration = Duration::from_secs(10 * 60);
/// A friend gets at most one answer about friends in common this often, and for at most
/// [`MUTUAL_POINTS_A_DAY`] keys asked about in a day: each question can test up to a
/// thousand device keys of its choice for "is this one your friend?", so a friend learns it
/// of at most 4,096 keys a day (one an hour alone let 24,000). A device with up to 112
/// friends asks with 128 keys, so it still gets an answer every hour; one with a thousand,
/// four a day. Friends in common change seldom; the application shows the last answer
/// meanwhile.
const MUTUAL_EVERY: Duration = Duration::from_secs(60 * 60);
const MUTUAL_POINTS_A_DAY: usize = 4096;
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// Whether a friend whose answers about friends in common in the last day are `answers`
/// gets one at `now`, for `points` keys once they are known (then counted when it does).
fn may_answer(answers: &mut Vec<(Instant, usize)>, now: Instant, points: Option<usize>) -> bool {
    answers.retain(|(t, _)| now.duration_since(*t) < DAY);
    if answers
        .iter()
        .any(|(t, _)| now.duration_since(*t) < MUTUAL_EVERY)
    {
        return false;
    }
    let Some(points) = points else {
        return true;
    };
    let asked: usize = answers.iter().map(|(_, n)| n).sum();
    if asked + points > MUTUAL_POINTS_A_DAY {
        return false;
    }
    answers.push((now, points));
    true
}
/// How long a run of the code shown here may wait for its last message before it counts as
/// failed.
const CODE_RUN: Duration = Duration::from_secs(2 * 60);

/// What a peer found on the DHT or on the local network may be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Friend(DevicePublicKey),
    /// The device that made the link used here.
    LinkOwner,
    /// The device showing the code typed here.
    CodeOwner,
    /// A device using the link or code shown here. It sends the request.
    Requester,
    /// The device a request made here went to, announcing under the secret the two share:
    /// it accepted, and sends the acceptance itself.
    Accepting,
    /// A former friend that has not heard yet that it was removed.
    Departing(DevicePublicKey),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum FriendPending {
    Probe([u8; 16]),
    Hello(DevicePublicKey),
    LinkRequest,
    CodeStart,
    CodeFinish,
    Accept(DevicePublicKey),
    Heartbeat,
    Profile,
    Group,
    /// A part of a long log offered (see `share_group`).
    GroupPart(GroupId),
    /// A status, and whether it was sent again after a refusal.
    Status(bool),
    /// A direct call's step, by its number (see call_sessions.rs).
    Call(u64),
    Banner,
    Bio,
    Animation,
    /// A removal told to a former friend, and the SHA-256 of what was sent, which its
    /// acknowledgment signs.
    Unfriend(DevicePublicKey, [u8; 32]),
    Mutual(DevicePublicKey),
    Direct(GroupId),
}

pub(super) struct ShownCode {
    pub(super) secrets: CodeSecrets,
    pub(super) expires_ms: u64,
    failed: u32,
}

pub(super) struct SoughtCode {
    pub(super) secrets: CodeSecrets,
    pub(super) expires_ms: u64,
    /// Request secret for the request this run ends with.
    request_secret: FriendSecret,
}

pub(super) struct FriendContext {
    pub(super) book: FriendBook,
    /// This device's animations of its picture and banner, set since it started.
    pub(super) animations: BTreeMap<Animated, Animation>,
    /// Link made here, while it is valid and unused.
    pub(super) link: Option<FriendLink>,
    /// Code shown here, while it is valid and unused.
    pub(super) code: Option<ShownCode>,
    /// Link used here, until its owner has our request.
    pub(super) sought_link: Option<FriendLink>,
    /// Code typed here, until the device showing it has our request.
    pub(super) sought_code: Option<SoughtCode>,
    /// Device reached with the code typed here, until it has our request.
    code_target: Option<(DevicePublicKey, String)>,
    pub(super) requesting: HashMap<PeerId, CodeRequester>,
    /// Runs of the code shown here waiting for their last message, and when each began.
    pub(super) inviting: HashMap<PeerId, (CodeInviter, Instant)>,
    /// Friends recognised on a connection, by transport identity.
    pub(super) online: HashMap<PeerId, DevicePublicKey>,
    pub(super) targets: HashMap<PeerId, Target>,
    pub(super) contacted: HashSet<(PeerId, Contact)>,
    /// Peers on the local network, or dialled at an address given or remembered: asked
    /// whether they are friends, and tried with a code.
    pub(super) lan: HashSet<PeerId>,
    /// The connection each unanswered friend request came on.
    request_peers: HashMap<DevicePublicKey, PeerId>,
    /// Friends that are members of our group and were sent its log on this connection.
    pub(super) group_shared: HashSet<(PeerId, GroupId)>,
    /// Long logs on their way to friends, part after part.
    pub(super) offering: HashMap<(PeerId, GroupId), Offering>,
    /// Parts of a long log a friend is sending, one log per friend device at a time,
    /// whichever of its sessions sends them.
    pub(super) offers_in: HashMap<DevicePublicKey, OfferIn>,
    /// When each friend session came online.
    pub(super) online_since: HashMap<PeerId, Instant>,
    /// DHT keys of the last friend lookup: what a peer found under each is, and, for a key of
    /// a friend's own secret, that secret, under which the friend's proof is sealed.
    pub(super) keys: HashMap<RecordKey, (Target, Option<[u8; 32]>)>,
    pub(super) last_lookup: Option<Instant>,
    /// Until when friends are looked up at every slot, after one went (see `SEARCH_HARD`).
    pub(super) searching_until: Option<Instant>,
    started: Instant,
    /// Last request, answer or new friend secret made here.
    pub(super) recent: Option<Instant>,
    /// Friends and former friends away, each with when it is looked up next and the step of
    /// [`AWAY_STEPS`] it is at; when that was last started over; and when each friend that
    /// went lately went (see `SEARCH_HARD`).
    away: HashMap<DevicePublicKey, (Instant, usize)>,
    away_reset: Instant,
    pub(super) gone_at: HashMap<DevicePublicKey, Instant>,
    pub(super) pending: HashMap<OutboundRequestId, (PeerId, FriendPending)>,
    /// Questions about friends in common waiting for their answer, and who waits for it.
    pub(super) asking: HashMap<DevicePublicKey, (MutualAsk, Vec<MutualReply>)>,
    /// When each friend got answers about friends in common in the last day, and for how
    /// many keys each.
    answered: HashMap<DevicePublicKey, Vec<(Instant, usize)>>,
    /// Friends this device's profile is on its way to, and whether a newer one waits to go
    /// once it has arrived: one at a time, so that a burst of changes cannot overflow the
    /// connection's streams and lose the newest.
    pub(super) profile_sends: HashMap<PeerId, bool>,
    /// Friends a profile did not reach: sent again at the next heartbeat.
    pub(super) profile_retry: HashSet<PeerId>,
    /// Friends that refused a status, as one does when it comes before the answer to the
    /// hello: sent once more at the next heartbeat.
    pub(super) status_retry: HashSet<PeerId>,
    /// Peers that acknowledged an acceptance for a friend before proving they are that
    /// friend: it counts as delivered once they do.
    pub(super) acked: HashSet<(PeerId, DevicePublicKey)>,
    /// The secret shared with each friend, former friend and device asked (see
    /// `FriendSecret::pair`), worked out once.
    pairs: HashMap<DevicePublicKey, FriendSecret>,
    /// Members whose friend request made from a group came lately, and when (see
    /// member_friends.rs).
    pub(super) member_asks: HashMap<DevicePublicKey, Instant>,
}

impl FriendContext {
    pub(super) fn new(book: FriendBook) -> Self {
        Self {
            book,
            animations: BTreeMap::new(),
            link: None,
            code: None,
            sought_link: None,
            sought_code: None,
            code_target: None,
            requesting: HashMap::new(),
            inviting: HashMap::new(),
            online: HashMap::new(),
            targets: HashMap::new(),
            contacted: HashSet::new(),
            lan: HashSet::new(),
            request_peers: HashMap::new(),
            group_shared: HashSet::new(),
            offering: HashMap::new(),
            offers_in: HashMap::new(),
            online_since: HashMap::new(),
            keys: HashMap::new(),
            last_lookup: None,
            searching_until: None,
            started: Instant::now(),
            recent: None,
            pending: HashMap::new(),
            asking: HashMap::new(),
            answered: HashMap::new(),
            profile_sends: HashMap::new(),
            profile_retry: HashSet::new(),
            status_retry: HashSet::new(),
            acked: HashSet::new(),
            pairs: HashMap::new(),
            member_asks: HashMap::new(),
            away: HashMap::new(),
            away_reset: Instant::now(),
            gone_at: HashMap::new(),
        }
    }

    /// Whether a removal from `device` gets this device's signed answer: only a device it
    /// knows does (a friend, a request either way, a former friend being told, one that
    /// removed this device lately, kept in the book across starts). Anyone else gets the
    /// plain refusal any device gives a removal that is not for it, so that a removal
    /// sealed to a device key, sent around, does not tell which peer that device is.
    fn answers_removal(&self, device: &DevicePublicKey, now_ms: u64) -> bool {
        self.book.friends.contains_key(device)
            || self.book.incoming.contains_key(device)
            || self.book.outgoing.contains_key(device)
            || self.book.departing.contains_key(device)
            || self.book.removed_us_lately(device, now_ms)
    }

    /// The secret this device shares with `device` alone.
    pub(super) fn pair(
        &mut self,
        identity: &DeviceIdentity,
        device: &DevicePublicKey,
    ) -> Option<FriendSecret> {
        if let Some(secret) = self.pairs.get(device) {
            return Some(secret.clone());
        }
        let secret = FriendSecret::pair(identity, device).ok()?;
        self.pairs.insert(*device, secret.clone());
        Some(secret)
    }

    /// The friend recognised on `peer`'s connection.
    pub(super) fn online_device(&self, peer: &PeerId) -> Option<&DevicePublicKey> {
        self.online.get(peer)
    }

    pub(super) fn is_online(&self, device: &DevicePublicKey) -> bool {
        self.online.values().any(|d| d == device)
    }

    pub(super) fn anyone_offline(&self) -> bool {
        self.book.friends.keys().any(|d| !self.is_online(d)) || !self.book.departing.is_empty()
    }

    /// Secrets to announce under besides our own: the link or code shown here or used here;
    /// and the secrets shared with each friend that does not have our current friend secret
    /// yet (an acceptance or a new secret to deliver) and each device a request made here
    /// went to (its acceptance to receive), which they look up. Never a friend's own secret:
    /// its other friends know it, and would find this device there.
    pub(super) fn extra_announcements(&mut self, identity: &DeviceIdentity) -> Vec<[u8; 32]> {
        let mut out = Vec::new();
        out.extend(self.link.iter().map(FriendLink::rendezvous));
        out.extend(self.code.iter().map(|c| c.secrets.rendezvous()));
        out.extend(self.sought_link.iter().map(FriendLink::rendezvous));
        out.extend(self.sought_code.iter().map(|c| c.secrets.rendezvous()));
        let waiting: Vec<DevicePublicKey> = self
            .book
            .friends
            .iter()
            .filter(|(_, f)| !f.delivered)
            .map(|(device, _)| *device)
            .chain(self.book.outgoing.keys().copied())
            .collect();
        for device in waiting {
            out.extend(self.pair(identity, &device).map(|p| p.rendezvous()));
        }
        out
    }

    /// Secrets to look up, what a peer found under each is, and whether it must prove it is
    /// that friend (see lan.rs): a friend's own secret is known to all its friends, who can
    /// announce under it too. A friend or former friend away is also looked for under the
    /// secret the two share, where it announces when it has something to deliver here, and
    /// so is a device a request made here went to, which announces there once it accepts.
    /// Those away are looked up less and less often while they stay away (see
    /// [`AWAY_STEPS`]).
    pub(super) fn lookups(
        &mut self,
        identity: &DeviceIdentity,
    ) -> Vec<([u8; 32], Target, Option<[u8; 32]>)> {
        self.lookups_at(identity, Instant::now())
    }

    /// Starts the lookups of friends away over: they are looked up at the next slot, then
    /// less and less often again.
    pub(super) fn look_for_the_away_again(&mut self) {
        self.away.clear();
        self.away_reset = Instant::now();
    }

    /// The friends and former friends away whose lookup is due at `now`, their next one
    /// set: one that just went at every slot, any other at its step of [`AWAY_STEPS`].
    fn away_due(&mut self, now: Instant) -> HashSet<DevicePublicKey> {
        if self.recent.is_some_and(|t| t > self.away_reset) {
            self.away.clear();
            self.away_reset = now;
        }
        let away: Vec<DevicePublicKey> = self
            .book
            .friends
            .keys()
            .filter(|device| !self.is_online(device))
            .chain(self.book.departing.keys())
            .copied()
            .collect();
        self.away.retain(|device, _| away.contains(device));
        self.gone_at.retain(|device, at| {
            away.contains(device) && now.saturating_duration_since(*at) < super::SEARCH_HARD
        });
        let mut due = HashSet::new();
        for device in away {
            let just_went = self.gone_at.contains_key(&device);
            let step = match self.away.get(&device) {
                Some((next, _)) if *next > now && !just_went => continue,
                Some((_, step)) => (*step + 1).min(AWAY_STEPS.len() - 1),
                None => 0,
            };
            // Up to a quarter more, by device, so that those away are not all looked up at
            // the same slot.
            let wait = AWAY_STEPS[step];
            let spread = wait.mul_f64(f64::from(device.as_bytes()[0]) / 255.0 / 4.0);
            let step = if just_went { 0 } else { step };
            self.away.insert(device, (now + wait + spread, step));
            due.insert(device);
        }
        due
    }

    /// [`Self::lookups`] at `now`.
    fn lookups_at(
        &mut self,
        identity: &DeviceIdentity,
        now: Instant,
    ) -> Vec<([u8; 32], Target, Option<[u8; 32]>)> {
        let due = self.away_due(now);
        let own = |secret: &FriendSecret, target| {
            let rendezvous = secret.rendezvous();
            (rendezvous, target, Some(rendezvous))
        };
        let mut out: Vec<([u8; 32], Target, Option<[u8; 32]>)> = self
            .book
            .friends
            .iter()
            .filter(|(device, _)| due.contains(*device))
            .map(|(device, friend)| own(&friend.secret, Target::Friend(*device)))
            .collect();
        out.extend(
            self.book
                .departing
                .iter()
                .filter(|(device, _)| due.contains(*device))
                .map(|(device, d)| own(&d.secret, Target::Departing(*device))),
        );
        if let Some(link) = &self.sought_link {
            out.push((link.rendezvous(), Target::LinkOwner, None));
        }
        if let Some(code) = &self.sought_code {
            out.push((code.secrets.rendezvous(), Target::CodeOwner, None));
        }
        if let Some(link) = &self.link {
            out.push((link.rendezvous(), Target::Requester, None));
        }
        if let Some(code) = &self.code {
            out.push((code.secrets.rendezvous(), Target::Requester, None));
        }
        let shared: Vec<(DevicePublicKey, Target)> = self
            .book
            .friends
            .keys()
            .filter(|device| due.contains(*device))
            .map(|device| (*device, Target::Friend(*device)))
            .chain(
                self.book
                    .departing
                    .keys()
                    .filter(|device| due.contains(*device))
                    .map(|device| (*device, Target::Departing(*device))),
            )
            .chain(
                self.book
                    .outgoing
                    .keys()
                    .map(|device| (*device, Target::Accepting)),
            )
            .collect();
        for (device, target) in shared {
            if let Some(pair) = self.pair(identity, &device) {
                out.push((pair.rendezvous(), target, None));
            }
        }
        out
    }

    /// Forgets the shared secrets of devices that are no longer friends, former friends
    /// being told, or asked.
    pub(super) fn forget_pairs(&mut self) {
        let book = &self.book;
        self.pairs.retain(|device, _| {
            book.friends.contains_key(device)
                || book.departing.contains_key(device)
                || book.outgoing.contains_key(device)
        });
    }

    /// Whether a link or code is shown or used here: the other device can only be found
    /// through the DHT, and only while it lasts.
    pub(super) fn inviting(&self) -> bool {
        self.link.is_some()
            || self.code.is_some()
            || self.sought_link.is_some()
            || self.sought_code.is_some()
    }

    /// Whether there is nobody to look for: every friend is online, no former friend is
    /// told it is one no more, no request waits for its answer and no invite is in use.
    /// Nothing is looked up then.
    pub(super) fn nobody_missing(&self) -> bool {
        !self.anyone_offline() && self.book.outgoing.is_empty() && !self.inviting()
    }

    /// Whether lookups should be frequent: just started, an invite is in use, or something
    /// changed a moment ago.
    pub(super) fn in_a_hurry(&self) -> bool {
        self.started.elapsed() < EAGER
            || self.recent.is_some_and(|t| t.elapsed() < AFTER_CHANGE)
            || self.link.is_some()
            || self.code.is_some()
            || self.sought_link.is_some()
            || self.sought_code.is_some()
    }
}

/// The addresses in a friend link that this device dials. Plain addresses only: a name in
/// a link would have this device ask DNS for a host whoever made the link chose. And only
/// those `own` may dial: the maker chose them too, and may have put any local address in
/// (see [`OwnNetworks::may_dial`]); its own local ones serve a friend at the same place,
/// behind another router too.
fn link_dial_addresses(addrs: &[Vec<u8>], own: &OwnNetworks) -> Vec<Multiaddr> {
    addrs
        .iter()
        .filter_map(|bytes| Multiaddr::try_from(bytes.clone()).ok())
        .filter(|a| addr::is_dialable(a) && !addr::is_named(a) && own.may_dial(a, Reach::SamePlace))
        .map(|a| addr::without_peer(&a))
        .collect()
}

impl Node {
    pub(super) fn changed(&self) {
        self.emit(NodeEvent::FriendsChanged);
    }

    /// Keeps a friend's newer profile, banner or bio, at most one of each kind per friend
    /// every few seconds (see limits.rs).
    fn friend_update(&mut self, update: FriendUpdate) {
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        if update.is_news(&f.book)
            && let Some(update) = self.limits.friend_update(update, Instant::now())
            && update.apply(&mut f.book)
        {
            self.changed();
        }
    }

    /// Takes the friends' updates and members' profiles that waited for their turn.
    pub(super) fn take_held_updates(&mut self) {
        let (updates, profiles, animations) = self.limits.due_updates(Instant::now());
        if let Some(f) = self.friends.as_mut()
            && !updates.is_empty()
        {
            let mut changed = false;
            for update in updates {
                changed |= update.apply(&mut f.book);
            }
            if changed {
                self.changed();
            }
        }
        for profile in profiles {
            self.emit(NodeEvent::MemberProfile { profile });
        }
        for animation in animations {
            self.emit(NodeEvent::Animation { animation });
        }
    }

    pub(super) fn friend_request(
        &mut self,
        peer: PeerId,
        request: FriendRequest,
        kind: FriendPending,
    ) {
        let id = self
            .swarm
            .behaviour_mut()
            .friend
            .send_request(&peer, request);
        if let Some(f) = self.friends.as_mut() {
            f.pending.insert(id, (peer, kind));
        }
    }

    /// The connections of these friends, while they are online.
    pub(super) fn friend_peers(&self, devices: &[DevicePublicKey]) -> Vec<PeerId> {
        self.friends
            .as_ref()
            .map(|f| {
                f.online
                    .iter()
                    .filter(|(_, device)| devices.contains(device))
                    .map(|(peer, _)| *peer)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The friend recognised on `peer`'s connection.
    pub(super) fn friend_device(&self, peer: &PeerId) -> Option<DevicePublicKey> {
        self.friends.as_ref()?.online.get(peer).copied()
    }

    /// Friends recognised on their connections.
    pub(super) fn online_friend_peers(&self) -> impl Iterator<Item = &PeerId> {
        self.friends.iter().flat_map(|f| f.online.keys())
    }

    /// Whether `peer` is a friend recognised on its connection.
    pub(super) fn is_friend_peer(&self, peer: &PeerId) -> bool {
        self.friends
            .as_ref()
            .is_some_and(|f| f.online.contains_key(peer))
    }

    // ----- Commands -----

    pub(super) fn on_friend_command(&mut self, command: Command) {
        let now = now_ms();
        if self.friends.is_none() {
            match command {
                Command::Friends(reply) => {
                    let _ = reply.send(None);
                }
                Command::FriendsOnline(reply) => {
                    let _ = reply.send(Vec::new());
                }
                Command::CreateLink(reply) => {
                    let _ = reply.send(Err("this node runs without friends".into()));
                }
                Command::SetProfile { reply, .. } => {
                    let _ = reply.send(Err("this node runs without a profile".into()));
                }
                Command::SendCall { reply, .. } => {
                    let _ = reply.send(Err("this node runs without friends".into()));
                }
                Command::SetBanner { reply, .. }
                | Command::SetBio { reply, .. }
                | Command::SetAnimation { reply, .. } => {
                    let _ = reply.send(Err("this node runs without a profile".into()));
                }
                Command::MutualFriends { reply, .. } => {
                    let _ = reply.send(Err("this node runs without friends".into()));
                }
                _ => {}
            }
            return;
        }
        match command {
            Command::Friends(reply) => {
                let _ = reply.send(self.friends.as_ref().map(|f| f.book.clone()));
            }
            Command::SetProfile {
                name,
                avatar,
                reply,
            } => {
                let f = self.friends.as_mut().expect("checked above");
                let before = f.book.profile.name.clone();
                let result = f
                    .book
                    .set_profile(&self.identity, name.as_deref(), avatar, now)
                    .map_err(|e| e.to_string());
                if result.is_ok() {
                    let after = f.book.profile.name.clone();
                    self.changed();
                    self.send_profile_to_friends();
                    self.send_profile_to_members();
                    if after != before {
                        self.renamed_in_rooms(&before, &after);
                    }
                }
                let _ = reply.send(result);
            }
            Command::SendCall { to, message, reply } => {
                let _ = reply.send(self.send_call(to, message));
            }
            Command::SetBanner {
                color,
                picture,
                reply,
            } => {
                let f = self.friends.as_mut().expect("checked above");
                let result = f
                    .book
                    .set_banner(&self.identity, color, picture, now)
                    .map_err(|e| e.to_string());
                if result == Ok(true) {
                    self.changed();
                    self.send_banner_to_friends();
                }
                let _ = reply.send(result.map(|_| ()));
            }
            Command::SetBio { text, reply } => {
                let f = self.friends.as_mut().expect("checked above");
                let result = f
                    .book
                    .set_bio(&self.identity, &text, now)
                    .map_err(|e| e.to_string());
                if result == Ok(true) {
                    self.changed();
                    self.send_bio_to_friends();
                }
                let _ = reply.send(result.map(|_| ()));
            }
            Command::SetAnimation { of, picture, reply } => {
                let f = self.friends.as_mut().expect("checked above");
                // The still picture the animation moves in place of: the one every version
                // shows.
                let still = match of {
                    Animated::Photo => match &f.book.profile.avatar {
                        fogata_friends::Avatar::Photo(bytes) => Some(bytes.clone()),
                        fogata_friends::Avatar::Preset(_) => None,
                    },
                    Animated::Banner => f.book.banner.as_ref().and_then(|b| b.picture.clone()),
                };
                let result = match still {
                    None if !picture.is_empty() => {
                        Err("there is no still picture for the animation".to_string())
                    }
                    still => {
                        // Newer than the one it replaces, whatever the clock says.
                        let version = f
                            .animations
                            .get(&of)
                            .map_or(now, |a| now.max(a.version_ms + 1));
                        Animation::sign(
                            &self.identity,
                            of,
                            still.as_deref().unwrap_or_default(),
                            picture,
                            version,
                        )
                        .map_err(|e| e.to_string())
                    }
                };
                let result = result.map(|animation| {
                    f.animations.insert(of, animation);
                });
                if result.is_ok() {
                    self.send_animation_to_friends(of);
                    if of == Animated::Photo {
                        self.send_profile_to_members();
                    }
                }
                let _ = reply.send(result);
            }
            Command::MutualFriends { device, reply } => self.ask_mutual(device, reply),
            Command::FriendsOnline(reply) => {
                let list = self
                    .friends
                    .as_ref()
                    .map(|f| {
                        f.online
                            .iter()
                            .map(|(peer, device)| {
                                let path = self
                                    .best_connection(peer)
                                    .map(|c| c.path)
                                    .unwrap_or(Path::Direct);
                                (*device, path)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let _ = reply.send(list);
            }
            Command::CreateLink(reply) => {
                let addrs: Vec<Vec<u8>> =
                    self.link_addresses().iter().map(|a| a.to_vec()).collect();
                let peer = self.local_peer().to_bytes();
                let f = self.friends.as_mut().expect("checked above");
                let profile = &f.book.profile;
                let result = FriendLink::new(profile.device, &profile.name, peer, addrs, now)
                    .map_err(|e| e.to_string());
                if let Ok(link) = &result {
                    f.link = Some(link.clone());
                    // Announce at the next slot rather than after the usual interval.
                    self.provided.1 = None;
                }
                let _ = reply.send(result);
            }
            Command::ShowCode(secrets) => {
                let f = self.friends.as_mut().expect("checked above");
                f.code = Some(ShownCode {
                    secrets,
                    expires_ms: now + CODE_LIFETIME_MS,
                    failed: 0,
                });
                self.provided.1 = None;
            }
            Command::UseLink(link) => self.use_link(link, now),
            Command::AskMember { device, name } => self.ask_member(device, name, now),
            Command::UseCode(secrets) => {
                let Ok(request_secret) = FriendSecret::generate() else {
                    return;
                };
                let f = self.friends.as_mut().expect("checked above");
                // This device's own code would only be looked for until it ran out.
                if f.code.as_ref().is_some_and(|shown| {
                    shown.secrets.code() == secrets.code() && shown.expires_ms > now
                }) {
                    self.emit(NodeEvent::FriendRequestFailed(
                        "this code was made on this device".to_string(),
                    ));
                    return;
                }
                f.sought_code = Some(SoughtCode {
                    secrets,
                    expires_ms: now + CODE_LIFETIME_MS,
                    request_secret,
                });
                f.contacted.retain(|(_, c)| *c != Contact::Code);
                f.last_lookup = None;
                let lan: Vec<PeerId> = f.lan.iter().copied().collect();
                for peer in lan {
                    self.contact(peer);
                }
            }
            Command::AnswerFriend { device, accept } => {
                let f = self.friends.as_mut().expect("checked above");
                let added = f
                    .book
                    .answer(&device, accept, now)
                    .map(|friend| friend.profile.clone());
                let peer = f.request_peers.remove(&device);
                f.recent = Some(Instant::now());
                self.changed();
                let Some(profile) = added else {
                    return;
                };
                self.emit(NodeEvent::FriendAdded { profile });
                let f = self.friends.as_mut().expect("checked above");
                f.last_lookup = None;
                if let Some(peer) = peer {
                    f.targets.insert(peer, Target::Friend(device));
                    f.contacted.remove(&(peer, Contact::Friend));
                    self.contact(peer);
                }
            }
            Command::RemoveFriend(device) => {
                let f = self.friends.as_mut().expect("checked above");
                match f.book.remove(&device, now) {
                    Ok(true) => {}
                    Ok(false) => return,
                    Err(e) => {
                        self.info(format!("cannot forget the friend: {e}"));
                        return;
                    }
                }
                f.recent = Some(Instant::now());
                f.last_lookup = None;
                f.contacted.retain(|(_, c)| *c != Contact::Update);
                // Announce under the new secret at the next slot.
                self.provided.1 = None;
                let peers: Vec<PeerId> = f
                    .online
                    .iter()
                    .filter(|(_, d)| **d == device)
                    .map(|(p, _)| *p)
                    .collect();
                for peer in &peers {
                    f.online.remove(peer);
                }
                f.targets.retain(|_, t| *t != Target::Friend(device));
                // Tell the former friend on the connections it has now.
                for peer in &peers {
                    f.targets.insert(*peer, Target::Departing(device));
                }
                if !peers.is_empty() {
                    self.emit(NodeEvent::FriendOffline { device });
                }
                self.changed();
                for peer in peers {
                    self.contact(peer);
                }
                self.update_online_friends();
            }
            _ => {}
        }
    }

    /// Addresses for a link: those others confirmed, then local network ones, QUIC and IPv4
    /// first.
    /// This device's addresses on its local networks, QUIC and IPv4 first.
    pub(super) fn lan_addresses(&self) -> Vec<Multiaddr> {
        let mut lan: Vec<Multiaddr> = self
            .listen_addrs
            .iter()
            .filter(|a| {
                !is_loopback(a)
                    && !matches!(addr::ip(a), Some(IpAddr::V6(ip)) if (ip.segments()[0] & 0xffc0) == 0xfe80)
            })
            .map(addr::without_peer)
            .collect();
        lan.sort_by_key(|a| {
            (
                !addr::is_quic(a),
                matches!(addr::ip(a), Some(IpAddr::V6(_))),
            )
        });
        let mut seen = HashSet::new();
        lan.retain(|a| seen.insert(a.clone()));
        lan.truncate(MAX_LINK_ADDRS);
        lan
    }

    fn link_addresses(&self) -> Vec<Multiaddr> {
        let lan = self.lan_addresses();
        let mut out: Vec<Multiaddr> = Vec::new();
        for address in self
            .confirmed
            .iter()
            .filter(|a| !addr::is_relayed(a))
            .chain(lan.iter())
        {
            let address = addr::without_peer(address);
            if !out.contains(&address) {
                out.push(address);
            }
        }
        out.truncate(MAX_LINK_ADDRS);
        out
    }

    fn use_link(&mut self, mut link: FriendLink, now: u64) {
        let own = self.own_networks();
        let f = self.friends.as_mut().expect("checked by the caller");
        let problem = if link.device == f.book.profile.device {
            Some("this link was made on this device")
        } else if link.is_surely_expired(now) {
            Some("this link has expired; ask for a new one")
        } else if f.book.friends.contains_key(&link.device) {
            Some("you are already friends")
        } else {
            None
        };
        if let Some(problem) = problem {
            self.emit(NodeEvent::FriendRequestFailed(problem.to_string()));
            return;
        }
        let addrs = link_dial_addresses(&link.addrs, &own);
        let peer = PeerId::from_bytes(&link.peer).ok();
        // Sought for a link's lifetime by this device's clock, which may differ from the
        // maker's; the maker refuses it once it expired there.
        link.expires_ms = now.saturating_add(LINK_LIFETIME_MS);
        f.sought_link = Some(link);
        f.contacted.retain(|(_, c)| *c != Contact::Link);
        f.last_lookup = None;
        // Without the session's transport identity, the DHT lookup finds the device.
        let Some(peer) = peer else {
            return;
        };
        f.targets.insert(peer, Target::LinkOwner);
        self.wanted.want(peer);
        if self.swarm.is_connected(&peer) {
            self.contact(peer);
        } else if !addrs.is_empty() {
            let opts = DialOpts::peer_id(peer)
                .addresses(addrs)
                .condition(PeerCondition::DisconnectedAndNotDialing)
                .build();
            if let Err(e) = self.swarm.dial(opts) {
                tracing::debug!("cannot dial the link's device: {e}");
            }
        }
    }

    // ----- Protocol -----

    pub(super) fn on_friend_event(
        &mut self,
        event: request_response::Event<FriendRequest, FriendResponse>,
    ) {
        match event {
            request_response::Event::Message {
                peer,
                message:
                    request_response::Message::Request {
                        request, channel, ..
                    },
                ..
            } => {
                let response = self.on_friend_request(peer, request);
                let _ = self
                    .swarm
                    .behaviour_mut()
                    .friend
                    .send_response(channel, response);
            }
            request_response::Event::Message {
                peer,
                message:
                    request_response::Message::Response {
                        request_id,
                        response,
                    },
                ..
            } => {
                let kind = self
                    .friends
                    .as_mut()
                    .and_then(|f| f.pending.remove(&request_id));
                if let Some((_, kind)) = kind {
                    self.on_friend_response(peer, kind, response);
                }
            }
            request_response::Event::OutboundFailure {
                peer, request_id, ..
            } => {
                let Some(f) = self.friends.as_mut() else {
                    return;
                };
                let kind = f.pending.remove(&request_id).map(|(_, kind)| kind);
                // Let the next heartbeat try again.
                let contact = match kind {
                    Some(FriendPending::Accept(_)) => {
                        f.contacted.remove(&(peer, Contact::Update));
                        Contact::Friend
                    }
                    Some(FriendPending::Hello(_)) => Contact::Friend,
                    Some(FriendPending::LinkRequest) => Contact::Link,
                    Some(FriendPending::CodeStart) => {
                        f.requesting.remove(&peer);
                        Contact::Code
                    }
                    Some(FriendPending::Call(id)) => {
                        self.call_step_failed(id, peer, false);
                        return;
                    }
                    Some(FriendPending::Unfriend(..)) => Contact::Unfriend,
                    Some(FriendPending::Banner) => Contact::Banner,
                    // A device too old to read a bio fails it every time: it goes again on
                    // the next connection, or when it changes.
                    Some(FriendPending::Bio) => return,
                    // The same for an animation: the friend shows the still picture.
                    Some(FriendPending::Animation) => return,
                    Some(FriendPending::Profile) => {
                        self.profile_went(peer, false);
                        return;
                    }
                    Some(FriendPending::Mutual(device)) => {
                        self.mutual_done(device, Err("the friend did not answer".into()));
                        return;
                    }
                    Some(FriendPending::GroupPart(group)) => {
                        self.offered_part_answered(peer, group, None);
                        return;
                    }
                    // Offered again on the next connection, or when the chat is run again.
                    Some(FriendPending::Direct(group)) => {
                        f.group_shared.remove(&(peer, group));
                        return;
                    }
                    _ => return,
                };
                f.contacted.remove(&(peer, contact));
            }
            _ => {}
        }
    }

    pub(super) fn on_friend_request(
        &mut self,
        peer: PeerId,
        request: FriendRequest,
    ) -> FriendResponse {
        // A stranger past its address's budget is refused (see limits.rs).
        if self.stranger_over_budget(&peer) {
            return FriendResponse::Refused;
        }
        // The application is far behind (see events.rs): a call step or an offered group,
        // which would only add to its events, is refused; the friend tells its own person
        // the step did not go, or offers the group again later.
        if matches!(
            request,
            FriendRequest::Call { .. } | FriendRequest::Group { .. } | FriendRequest::Direct { .. }
        ) && self.events.full()
        {
            return FriendResponse::Refused;
        }
        let local = self.local_peer().to_bytes();
        let remote = peer.to_bytes();
        let now = now_ms();
        let hidden = self.hidden;
        if self.friends.is_none() {
            return FriendResponse::Refused;
        }
        match request {
            FriendRequest::Probe { nonce } => self.on_friend_probe_request(local, remote, nonce),
            FriendRequest::Hello { mac, sealed } => {
                self.on_friend_hello_request(peer, local, remote, hidden, mac, sealed)
            }
            FriendRequest::LinkRequest { mac, sealed } => {
                self.on_friend_link_request(peer, local, remote, now, mac, sealed)
            }
            FriendRequest::CodeStart { gate, spake } => {
                self.on_friend_code_start_request(peer, local, remote, now, gate, spake)
            }
            FriendRequest::CodeFinish { sealed } => {
                // Kept, as checked above.
                let Some(f) = self.friends.as_mut() else {
                    return FriendResponse::Refused;
                };
                let Some((inviter, _)) = f.inviting.remove(&peer) else {
                    return FriendResponse::Refused;
                };
                match inviter.finish(&sealed) {
                    Ok(intro) => {
                        f.code = None;
                        f.inviting.clear();
                        self.received_request(peer, intro);
                        FriendResponse::Pending
                    }
                    Err(e) => {
                        tracing::debug!("a code run from {peer} failed: {e}");
                        if let Some(code) = f.code.as_mut() {
                            code.failed += 1;
                        }
                        FriendResponse::Refused
                    }
                }
            }
            FriendRequest::Accept { mac, sealed } => {
                // Kept, as checked above.
                let Some(f) = self.friends.as_mut() else {
                    return FriendResponse::Refused;
                };
                // Keyed by the secret shared with a friend (sending its new secret) or with a
                // device a request went to.
                let devices: Vec<DevicePublicKey> = f
                    .book
                    .friends
                    .keys()
                    .chain(f.book.outgoing.keys())
                    .copied()
                    .collect();
                let intro = devices.into_iter().find_map(|device| {
                    let pair = f.pair(&self.identity, &device)?;
                    fogata_friends::open_accept(
                        &self.identity,
                        &pair,
                        &local,
                        &remote,
                        &mac,
                        &sealed,
                    )
                    .ok()
                    .filter(|intro| intro.profile.device == device)
                });
                let Some(intro) = intro else {
                    tracing::debug!("refused an acceptance from {peer}");
                    return FriendResponse::Refused;
                };
                let profile = intro.profile.clone();
                let device = profile.device;
                let new = !f.book.friends.contains_key(&device);
                if !f.book.accept_received(intro, now) {
                    return FriendResponse::Refused;
                }
                f.targets.insert(peer, Target::Friend(device));
                self.changed();
                if new {
                    self.emit(NodeEvent::FriendAdded { profile });
                }
                // A new friend has only our request secret: send our friend secret back.
                self.contact(peer);
                FriendResponse::Ack
            }
            FriendRequest::Profile { sealed } => {
                // Kept, as checked above.
                let Some(f) = self.friends.as_mut() else {
                    return FriendResponse::Refused;
                };
                let Some(device) = f.online.get(&peer).copied() else {
                    return FriendResponse::Refused;
                };
                match fogata_friends::open_profile_update(
                    &self.identity,
                    &device,
                    &local,
                    &remote,
                    &sealed,
                ) {
                    Ok(profile) => {
                        self.friend_update(FriendUpdate::Profile(profile));
                        FriendResponse::Ack
                    }
                    Err(_) => FriendResponse::Refused,
                }
            }
            FriendRequest::Group {
                entries,
                declined,
                part,
            } => {
                // Kept, as checked above.
                let Some(f) = self.friends.as_mut() else {
                    return FriendResponse::Refused;
                };
                let Some(by) = f.online.get(&peer).copied() else {
                    return FriendResponse::Refused;
                };
                if !self.limits.take_friend_offer(by, Instant::now()) {
                    return FriendResponse::Refused;
                }
                let entries: Vec<Vec<u8>> = entries.into_iter().map(|e| e.into_vec()).collect();
                let Some((index, count)) = part else {
                    return self.on_group_offer(peer, by, entries, declined);
                };
                // A long log, in parts: kept until they are all here, up to the bounds of
                // one offer.
                let Some(group) = entries
                    .first()
                    .and_then(|e| Entry::decode(e).ok())
                    .map(|e| e.group_id)
                else {
                    return FriendResponse::Refused;
                };
                match f.take_offered_part(by, group, index, count, entries, Instant::now()) {
                    OfferPart::Kept => FriendResponse::Ack,
                    OfferPart::Refused => FriendResponse::Refused,
                    OfferPart::Whole(entries) => self.on_group_offer(peer, by, entries, declined),
                }
            }
            FriendRequest::Call { sealed } => {
                // Kept, as checked above.
                let Some(f) = self.friends.as_mut() else {
                    return FriendResponse::Refused;
                };
                let Some(device) = f.online.get(&peer).copied() else {
                    return FriendResponse::Refused;
                };
                match fogata_friends::open_call_message(&self.identity, &local, &remote, &sealed) {
                    Ok(message) => {
                        self.on_call_step(peer, device, message);
                        FriendResponse::Ack
                    }
                    Err(_) => FriendResponse::Refused,
                }
            }
            FriendRequest::Status { sealed } => {
                // Kept, as checked above.
                let Some(f) = self.friends.as_mut() else {
                    return FriendResponse::Refused;
                };
                let Some(device) = f.online.get(&peer).copied() else {
                    return FriendResponse::Refused;
                };
                match fogata_friends::open_status(&self.identity, &local, &remote, &sealed) {
                    Ok(status) => {
                        let group = status
                            .group
                            .and_then(|g| fogata_crypto::GroupId::from_bytes(&g).ok());
                        self.emit(NodeEvent::FriendStatus {
                            device,
                            in_room: status.in_room,
                            group,
                            hidden: status.hidden,
                            away: status.away && !status.hidden && !status.in_room,
                        });
                        FriendResponse::Ack
                    }
                    Err(_) => FriendResponse::Refused,
                }
            }
            FriendRequest::Banner { sealed } => {
                // Kept, as checked above.
                let Some(f) = self.friends.as_mut() else {
                    return FriendResponse::Refused;
                };
                let Some(device) = f.online.get(&peer).copied() else {
                    return FriendResponse::Refused;
                };
                match fogata_friends::open_banner_update(
                    &self.identity,
                    &device,
                    &local,
                    &remote,
                    &sealed,
                ) {
                    Ok(banner) => {
                        self.friend_update(FriendUpdate::Banner(banner));
                        FriendResponse::Ack
                    }
                    Err(_) => FriendResponse::Refused,
                }
            }
            FriendRequest::Bio { sealed } => {
                // Kept, as checked above.
                let Some(f) = self.friends.as_mut() else {
                    return FriendResponse::Refused;
                };
                let Some(device) = f.online.get(&peer).copied() else {
                    return FriendResponse::Refused;
                };
                match fogata_friends::open_bio_update(
                    &self.identity,
                    &device,
                    &local,
                    &remote,
                    &sealed,
                ) {
                    Ok(bio) => {
                        self.friend_update(FriendUpdate::Bio(bio));
                        FriendResponse::Ack
                    }
                    Err(e) => {
                        tracing::debug!("refused a bio from {peer}: {e}");
                        FriendResponse::Refused
                    }
                }
            }
            FriendRequest::Animation { sealed } => {
                // Kept, as checked above.
                let Some(f) = self.friends.as_mut() else {
                    return FriendResponse::Refused;
                };
                let Some(device) = f.online.get(&peer).copied() else {
                    return FriendResponse::Refused;
                };
                match fogata_friends::open_animation_update(
                    &self.identity,
                    &device,
                    &local,
                    &remote,
                    &sealed,
                ) {
                    Ok(animation) => {
                        // At most a couple every few seconds from each (see limits.rs).
                        if let Some(animation) = self.limits.animation(animation, Instant::now()) {
                            self.emit(NodeEvent::Animation { animation });
                        }
                        FriendResponse::Ack
                    }
                    Err(e) => {
                        tracing::debug!("refused an animation from {peer}: {e}");
                        FriendResponse::Refused
                    }
                }
            }
            FriendRequest::Unfriend { sealed } => {
                // Kept, as checked above.
                let Some(f) = self.friends.as_mut() else {
                    return FriendResponse::Refused;
                };
                let device =
                    match fogata_friends::open_unfriend(&self.identity, &local, &remote, &sealed) {
                        Ok(device) => device,
                        Err(e) => {
                            tracing::debug!("refused a removal from {peer}: {e}");
                            return FriendResponse::Refused;
                        }
                    };
                if !f.answers_removal(&device, now) {
                    tracing::debug!("refused a removal from a device this one does not know");
                    return FriendResponse::Refused;
                }
                // Signed, so that only this device's answer ends the removal over there.
                let digest = fogata_friends::removal_digest(&sealed);
                let Ok(ack) =
                    fogata_friends::unfriend_ack(&self.identity, &device, &local, &remote, &digest)
                else {
                    return FriendResponse::Refused;
                };
                // Both may have removed the other at once.
                let mut changed = f.book.departed(&device);
                match f.book.removed_by(&device, now) {
                    Ok(true) => {
                        changed = true;
                        f.recent = Some(Instant::now());
                        f.last_lookup = None;
                        f.contacted.retain(|(_, c)| *c != Contact::Update);
                        let peers: Vec<PeerId> = f
                            .online
                            .iter()
                            .filter(|(_, d)| **d == device)
                            .map(|(p, _)| *p)
                            .collect();
                        for peer in &peers {
                            f.online.remove(peer);
                        }
                        f.targets.retain(|_, t| *t != Target::Friend(device));
                        // Announce under the new secret at the next slot.
                        self.provided.1 = None;
                        if !peers.is_empty() {
                            self.emit(NodeEvent::FriendOffline { device });
                        }
                        self.update_online_friends();
                    }
                    Ok(false) => {}
                    Err(e) => self.info(format!("cannot forget the friend: {e}")),
                }
                if changed {
                    self.changed();
                }
                FriendResponse::Unfriended { sealed: ack }
            }
            FriendRequest::Mutual { sealed } => {
                // Kept, as checked above.
                let Some(f) = self.friends.as_mut() else {
                    return FriendResponse::Refused;
                };
                let Some(device) = f.online.get(&peer).copied() else {
                    return FriendResponse::Refused;
                };
                f.answered.retain(|_, answers| {
                    answers.retain(|(t, _)| t.elapsed() < DAY);
                    !answers.is_empty()
                });
                let answers = f.answered.entry(device).or_default();
                if !may_answer(answers, Instant::now(), None) {
                    return FriendResponse::Refused;
                }
                let points =
                    match fogata_friends::open_mutual_ask(&self.identity, &local, &remote, &sealed)
                    {
                        Ok(points) => points,
                        Err(e) => {
                            tracing::debug!("cannot answer about friends in common: {e}");
                            return FriendResponse::Refused;
                        }
                    };
                // Counted before the answer is worked out: a question that fails at its last
                // point costs as much as one that does not.
                if !may_answer(answers, Instant::now(), Some(points.len())) {
                    return FriendResponse::Refused;
                }
                let friends: Vec<DevicePublicKey> = f
                    .book
                    .friends
                    .keys()
                    .filter(|d| **d != device)
                    .copied()
                    .collect();
                let reply = fogata_friends::mutual_answer(&points, &friends).and_then(|answer| {
                    fogata_friends::mutual_reply(&device, &answer, &local, &remote)
                });
                match reply {
                    Ok(sealed) => FriendResponse::Mutual { sealed },
                    Err(e) => {
                        tracing::debug!("cannot answer about friends in common: {e}");
                        FriendResponse::Refused
                    }
                }
            }
            FriendRequest::Direct { entries } => {
                // Kept, as checked above.
                let Some(f) = self.friends.as_mut() else {
                    return FriendResponse::Refused;
                };
                let Some(by) = f.online.get(&peer).copied() else {
                    return FriendResponse::Refused;
                };
                if !self.limits.take_friend_offer(by, Instant::now()) {
                    return FriendResponse::Refused;
                }
                self.on_direct_offer(peer, by, entries)
            }
            FriendRequest::Heartbeat => {
                // Kept, as checked above.
                let Some(f) = self.friends.as_mut() else {
                    return FriendResponse::Refused;
                };
                if f.online.contains_key(&peer) {
                    FriendResponse::Ack
                } else {
                    FriendResponse::Refused
                }
            }
        }
    }

    /// Answers a probe for this device's friend tag, whoever asks.
    fn on_friend_probe_request(
        &mut self,
        local: Vec<u8>,
        remote: Vec<u8>,
        nonce: Vec<u8>,
    ) -> FriendResponse {
        // Kept, as checked above.
        let Some(f) = self.friends.as_mut() else {
            return FriendResponse::Refused;
        };
        if !(16..=32).contains(&nonce.len()) {
            return FriendResponse::Refused;
        }
        // Under the current secret only: a former friend knows the earlier ones and
        // could tell this device by them.
        FriendResponse::ProbeReply {
            tag: f.book.secret.probe_tag(&nonce, &local, &remote).to_vec(),
        }
    }

    /// Answers a friend's hello.
    fn on_friend_hello_request(
        &mut self,
        peer: PeerId,
        local: Vec<u8>,
        remote: Vec<u8>,
        hidden: bool,
        mac: Vec<u8>,
        sealed: Vec<u8>,
    ) -> FriendResponse {
        // Kept, as checked above.
        let Some(f) = self.friends.as_mut() else {
            return FriendResponse::Refused;
        };
        // A friend online already says hello once per connection: one saying it
        // again and again is answered only so often (see limits.rs).
        if let Some(device) = f.online.get(&peer).copied()
            && !self.limits.take_friend_hello(device, Instant::now())
        {
            tracing::debug!("{} said hello too often", device.fingerprint());
            return FriendResponse::Refused;
        }
        // Keyed by the secret shared with the friend that sent it: tried with each
        // friend's.
        let own: Vec<FriendSecret> = f.book.own_secrets().cloned().collect();
        let own: Vec<&FriendSecret> = own.iter().collect();
        let devices: Vec<DevicePublicKey> = f.book.friends.keys().copied().collect();
        let opened = devices.into_iter().find_map(|device| {
            let pair = f.pair(&self.identity, &device)?;
            fogata_friends::open_hello(&self.identity, &pair, &own, &local, &remote, &mac, &sealed)
                .ok()
                .filter(|from| from.device == device)
        });
        // A hello that proves the friend has our current friend secret says it was
        // delivered.
        let current = opened.as_ref().is_some_and(|from| from.knows == Some(0));
        let Some(from) = opened else {
            tracing::debug!("refused a hello from {peer}");
            return FriendResponse::Refused;
        };
        let Some(friend) = f.book.friends.get(&from.device) else {
            tracing::debug!("hello from {}, not a friend", from.device.fingerprint());
            return FriendResponse::Refused;
        };
        // Not again with every hello of a friend that says it has an older one.
        let newer = (from.knows_version < f.book.profile.version_ms
            && self.limits.hello_profile(from.device, Instant::now()))
        .then_some(&f.book.profile);
        let reply = match fogata_friends::hello_reply(
            &self.identity,
            &from.device,
            &local,
            &remote,
            friend.profile.version_ms,
            newer,
            hidden,
        ) {
            Ok(reply) => reply,
            Err(_) => return FriendResponse::Refused,
        };
        // A hello keyed by our current friend secret says the friend has it.
        if current && !friend.delivered {
            f.book.set_delivered(&from.device);
            self.changed();
        }
        self.mark_friend_online(peer, from.device, from.hidden);
        FriendResponse::HelloReply { sealed: reply }
    }

    /// Takes a friend request made with this device's link.
    fn on_friend_link_request(
        &mut self,
        peer: PeerId,
        local: Vec<u8>,
        remote: Vec<u8>,
        now: u64,
        mac: Vec<u8>,
        sealed: Vec<u8>,
    ) -> FriendResponse {
        // Kept, as checked above.
        let Some(f) = self.friends.as_mut() else {
            return FriendResponse::Refused;
        };
        let Some(link) = f.link.as_ref().filter(|l| !l.is_expired(now)) else {
            return FriendResponse::Refused;
        };
        match fogata_friends::open_link_request(
            &self.identity,
            link,
            &local,
            &remote,
            &mac,
            &sealed,
        ) {
            Ok(intro) => {
                f.link = None;
                self.received_request(peer, intro);
                FriendResponse::Pending
            }
            Err(e) => {
                tracing::debug!("refused a link request from {peer}: {e}");
                FriendResponse::Refused
            }
        }
    }

    /// Answers the start of a run of this device's friend code.
    fn on_friend_code_start_request(
        &mut self,
        peer: PeerId,
        local: Vec<u8>,
        remote: Vec<u8>,
        now: u64,
        gate: Vec<u8>,
        spake: Vec<u8>,
    ) -> FriendResponse {
        // Kept, as checked above.
        let Some(f) = self.friends.as_mut() else {
            return FriendResponse::Refused;
        };
        let identity = self.identity.clone();
        let profile = f.book.profile.clone();
        let Some(code) = f.code.as_mut().filter(|c| now <= c.expires_ms) else {
            return FriendResponse::Refused;
        };
        if !fogata_friends::code_gate_ok(&code.secrets, &local, &remote, &gate) {
            return FriendResponse::Refused;
        }
        // Only runs that end badly count: a last message that does not open, or none
        // in time. A device that asks again with its run still open (the answer was
        // lost, the connection dropped) starts it again for free.
        let expired = f
            .inviting
            .iter()
            .filter(|(p, (_, at))| **p != peer && at.elapsed() >= CODE_RUN)
            .map(|(p, _)| *p)
            .collect::<Vec<_>>();
        for gone in expired {
            f.inviting.remove(&gone);
            code.failed += 1;
        }
        if code.failed >= MAX_FAILED_RUNS {
            f.code = None;
            f.inviting.clear();
            self.info("the friend code failed too many times; it no longer works");
            return FriendResponse::Refused;
        }
        let Ok(gate) = <[u8; 32]>::try_from(gate.as_slice()) else {
            return FriendResponse::Refused;
        };
        let start = CodeStart { gate, spake };
        match fogata_friends::code_reply(
            &identity,
            &profile,
            &code.secrets,
            &local,
            &remote,
            &start,
        ) {
            Ok((inviter, reply)) => {
                f.inviting.insert(peer, (inviter, Instant::now()));
                FriendResponse::CodeReply {
                    spake: reply.spake,
                    sealed: reply.sealed,
                }
            }
            Err(_) => FriendResponse::Refused,
        }
    }

    /// Records a checked friend request that came from `peer`.
    pub(super) fn received_request(&mut self, peer: PeerId, intro: Introduction) {
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        let profile = intro.profile.clone();
        let device = profile.device;
        match f.book.request_received(intro, now_ms()) {
            RequestOutcome::New => {
                f.request_peers.insert(device, peer);
                self.changed();
                self.emit(NodeEvent::FriendRequest { profile });
            }
            RequestOutcome::AlreadyFriend => {
                // The friend lost our acceptance; send it again.
                f.targets.insert(peer, Target::Friend(device));
                f.contacted.remove(&(peer, Contact::Friend));
                self.changed();
                self.contact(peer);
            }
            RequestOutcome::Ignored => {}
        }
    }

    fn on_friend_response(&mut self, peer: PeerId, kind: FriendPending, response: FriendResponse) {
        let local = self.local_peer().to_bytes();
        let remote = peer.to_bytes();
        let now = now_ms();
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        match (kind, response) {
            (FriendPending::Probe(nonce), FriendResponse::ProbeReply { tag }) => {
                let matches =
                    |secret: &FriendSecret| secret.probe_matches(&nonce, &remote, &local, &tag);
                let found = f
                    .book
                    .friends
                    .iter()
                    .find(|(_, friend)| matches(&friend.secret))
                    .map(|(device, _)| *device);
                let departing = || {
                    f.book
                        .departing
                        .iter()
                        .find(|(_, d)| matches(&d.secret))
                        .map(|(device, _)| *device)
                };
                let target = match found {
                    Some(device) => Some(Target::Friend(device)),
                    None => departing().map(Target::Departing),
                };
                if let Some(target) = target {
                    f.targets.insert(peer, target);
                    self.contact(peer);
                } else {
                    // Maybe a friend that changed its secret while this device had changed
                    // its own too: the hellos tell, and only that friend can open its own.
                    self.greet_waiting_friends(peer);
                }
            }
            (FriendPending::Hello(device), FriendResponse::HelloReply { sealed }) => {
                let reply = match fogata_friends::open_hello_reply(
                    &self.identity,
                    &device,
                    &local,
                    &remote,
                    &sealed,
                ) {
                    Ok(reply) => reply,
                    Err(e) => {
                        tracing::debug!("bad hello reply from {peer}: {e}");
                        return;
                    }
                };
                let mut changed = false;
                if let Some(profile) = reply.profile {
                    changed |= f.book.update_profile(profile);
                }
                let update = reply.knows_version < f.book.profile.version_ms;
                if changed {
                    self.changed();
                }
                self.mark_friend_online(peer, device, reply.hidden);
                if update {
                    self.send_profile(peer, device);
                }
            }
            (FriendPending::Profile, _) => self.profile_went(peer, true),
            // A version before `away` refuses a status that is away every time: once more
            // is all it gets.
            (FriendPending::Status(false), FriendResponse::Refused) => {
                f.status_retry.insert(peer);
            }
            (FriendPending::GroupPart(group), response) => {
                let taken = matches!(response, FriendResponse::Ack);
                self.offered_part_answered(peer, group, Some(taken));
            }
            (FriendPending::Call(id), FriendResponse::Refused) => {
                self.call_step_failed(id, peer, true);
            }
            (FriendPending::Call(id), _) => self.call_step_delivered(id),
            (FriendPending::Hello(device), FriendResponse::Refused) => {
                tracing::debug!(
                    "{} did not recognise this device as a friend",
                    device.fingerprint()
                );
            }
            (FriendPending::LinkRequest, FriendResponse::Pending) => {
                if let Some(link) = f.sought_link.take() {
                    self.emit(NodeEvent::FriendRequestSent {
                        device: link.device,
                        name: link.name,
                    });
                }
            }
            (FriendPending::LinkRequest, FriendResponse::Refused) => {
                if let Some(link) = f.sought_link.take() {
                    f.book.outgoing.remove(&link.device);
                    self.changed();
                    self.emit(NodeEvent::FriendRequestFailed(
                        "the link was already used or has expired; ask for a new one".into(),
                    ));
                }
            }
            (FriendPending::CodeStart, FriendResponse::CodeReply { spake, sealed }) => {
                let Some(requester) = f.requesting.remove(&peer) else {
                    return;
                };
                let Some(request_secret) = f.sought_code.as_ref().map(|c| c.request_secret.clone())
                else {
                    return;
                };
                let reply = CodeReply { spake, sealed };
                match requester.finish(&self.identity, &f.book.profile, &request_secret, &reply) {
                    Ok((inviter, finish)) => {
                        f.book.request_sent_with(
                            inviter.device,
                            &inviter.name,
                            request_secret,
                            now,
                        );
                        f.recent = Some(Instant::now());
                        f.code_target = Some((inviter.device, inviter.name));
                        self.changed();
                        self.friend_request(
                            peer,
                            FriendRequest::CodeFinish { sealed: finish },
                            FriendPending::CodeFinish,
                        );
                    }
                    Err(FriendError::WrongCode) => {
                        // Any device reached may answer with nothing a code could give: it is
                        // not the one showing the code, which another may still be.
                        tracing::debug!("{peer} answered the code with nonsense");
                        if f.targets.get(&peer) == Some(&Target::CodeOwner) {
                            f.targets.remove(&peer);
                        }
                    }
                    Err(e) => tracing::debug!("code run with {peer} failed: {e}"),
                }
            }
            (FriendPending::CodeStart, FriendResponse::Refused) => {
                // Not the device showing the code, or the code no longer works there.
                f.requesting.remove(&peer);
            }
            (FriendPending::CodeFinish, FriendResponse::Pending) => {
                f.sought_code = None;
                if let Some((device, name)) = f.code_target.take() {
                    self.emit(NodeEvent::FriendRequestSent { device, name });
                }
            }
            (FriendPending::CodeFinish, FriendResponse::Refused) => {
                f.sought_code = None;
                if let Some((device, _)) = f.code_target.take() {
                    f.book.outgoing.remove(&device);
                    self.changed();
                }
                self.emit(NodeEvent::FriendRequestFailed(
                    "the other device refused the request; the code may have been used already"
                        .into(),
                ));
            }
            (FriendPending::Accept(device), FriendResponse::Ack) => {
                // An acknowledgment proves nothing: whoever knows the friend's secret can
                // announce under it or answer probes with its tag, and acknowledge to stop
                // the delivery for good. It counts from a peer that proved it is that friend,
                // now or in its answer to the hello below; until then, delivery goes on.
                let proved = f.online.get(&peer) == Some(&device);
                if proved {
                    f.book.set_delivered(&device);
                } else {
                    f.acked.insert((peer, device));
                }
                f.contacted.remove(&(peer, Contact::Friend));
                let online = f.online.contains_key(&peer);
                if proved {
                    self.changed();
                }
                // Now both know each other: greet as friends.
                if !online {
                    self.send_hello(peer, device);
                }
            }
            (FriendPending::Accept(device), FriendResponse::Refused) => {
                // Keyed by a secret of the friend's it had just stopped taking, as when both
                // send their new ones at once: the next heartbeat tries with the one it
                // sent since.
                tracing::debug!("{} refused our acceptance", device.fingerprint());
                f.contacted.remove(&(peer, Contact::Update));
            }
            (FriendPending::Unfriend(device, digest), FriendResponse::Unfriended { sealed })
                if fogata_friends::open_unfriend_ack(
                    &self.identity,
                    &device,
                    &local,
                    &remote,
                    &digest,
                    &sealed,
                )
                .is_ok() =>
            {
                f.targets.retain(|_, t| *t != Target::Departing(device));
                if f.book.departed(&device) {
                    self.changed();
                }
            }
            (FriendPending::Unfriend(device, _), _) => {
                // Not the former friend, which acknowledges a removal it can open even after
                // forgetting this device, signed: another of its friends, which announces
                // under its secret or answers probes with a tag of it. It is still told when
                // found.
                tracing::debug!("{} did not acknowledge its removal", device.fingerprint());
                if f.targets.get(&peer) == Some(&Target::Departing(device)) {
                    f.targets.remove(&peer);
                }
            }
            (FriendPending::Mutual(device), FriendResponse::Mutual { sealed }) => {
                let outcome =
                    fogata_friends::open_mutual_reply(&self.identity, &local, &remote, &sealed)
                        .and_then(|answer| match f.asking.get(&device) {
                            Some((ask, _)) => ask.finish(&answer),
                            None => Err(FriendError::Malformed),
                        })
                        .map_err(|e| e.to_string());
                self.mutual_done(device, outcome);
            }
            (FriendPending::Mutual(device), _) => {
                self.mutual_done(device, Err("the friend did not answer".into()));
            }
            (FriendPending::Heartbeat, FriendResponse::Refused) => {
                if let Some(device) = f.online.remove(&peer)
                    && !f.is_online(&device)
                {
                    self.emit(NodeEvent::FriendOffline { device });
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use fogata_friends::Friend;
    use futures::StreamExt;
    use libp2p::{
        StreamProtocol, SwarmBuilder,
        multiaddr::Protocol,
        request_response::{Message, ProtocolSupport},
        swarm::SwarmEvent,
    };

    use super::*;
    use crate::{
        NodeConfig,
        node::{start, tests::inspect},
    };

    /// A friend link's maker chose its addresses: those of this computer, of this device
    /// itself, names, and local ones of a carrier's or a VPN's network this device is not on
    /// are left out; its public ones and its private IPv4 ones (at the same place, behind
    /// another router maybe) are dialled.
    #[test]
    fn a_link_is_dialled_only_at_addresses_this_device_may_dial() {
        let own = OwnNetworks::of(&[
            "/ip4/192.168.1.20/udp/31000/quic-v1".parse().unwrap(),
            "/ip4/203.0.113.20/udp/31000/quic-v1".parse().unwrap(),
        ]);
        let peer = PeerId::random();
        let link: Vec<Vec<u8>> = [
            "/ip4/198.51.100.7/udp/31000/quic-v1",
            "/ip4/192.168.2.123/udp/31000/quic-v1",
            "/ip4/127.0.0.1/udp/22/quic-v1",
            "/ip6/::1/udp/22/quic-v1",
            "/ip4/100.70.8.9/udp/80/quic-v1",
            "/ip6/fd99:1:2:1::1/udp/443/quic-v1",
            "/ip6/fe80::1/udp/31000/quic-v1",
            "/ip4/192.168.1.20/udp/31000/quic-v1",
            "/dns4/router.example/udp/31000/quic-v1",
        ]
        .iter()
        .map(|a| {
            let a: Multiaddr = a.parse().unwrap();
            a.with(Protocol::P2p(peer)).to_vec()
        })
        .chain([vec![0xff, 0x01]])
        .collect();
        let dialled: Vec<String> = link_dial_addresses(&link, &own)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            dialled,
            [
                "/ip4/198.51.100.7/udp/31000/quic-v1",
                "/ip4/192.168.2.123/udp/31000/quic-v1"
            ]
        );
    }

    /// A friend that stays away is looked up less and less often, up to once an hour; one
    /// that just went at every slot for a while; a change here starts over; and a friend
    /// back online is not looked up.
    #[test]
    fn a_friend_that_stays_away_is_looked_up_less_and_less_often() {
        let ana = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let bea_book = FriendBook::new(&bea, "Bea", 1).unwrap();
        let mut f = FriendContext::new(FriendBook::new(&ana, "Ana", 1).unwrap());
        f.book.friends.insert(
            bea.public_key(),
            Friend {
                profile: bea_book.profile.clone(),
                secret: bea_book.secret.clone(),
                since_ms: 2,
                delivered: true,
                banner: None,
                bio: None,
            },
        );
        let start = Instant::now();
        let looked =
            |f: &mut FriendContext, at: Duration| !f.lookups_at(&ana, start + at).is_empty();
        // At every two-minute slot for two hours: looked up at 0, ~2, ~7, ~22, ~82 minutes.
        let slots: Vec<u64> = (0..60)
            .filter(|i| looked(&mut f, Duration::from_secs(i * 120)))
            .collect();
        assert!((4..=6).contains(&slots.len()), "{slots:?}");
        let gaps: Vec<u64> = slots.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(gaps.windows(2).all(|g| g[0] <= g[1]), "{gaps:?}");
        assert!(*gaps.last().unwrap() >= 30, "{gaps:?}");
        // It just went (as after a connection lost): looked up at every slot for a while.
        let later = Duration::from_secs(2 * 3600);
        f.gone_at.insert(bea.public_key(), start + later);
        let hard = (0..6)
            .filter(|i| looked(&mut f, later + Duration::from_secs(i * 10)))
            .count();
        assert_eq!(hard, 6);
        // A change here starts over.
        f.lookups_at(&ana, start + later + Duration::from_secs(200));
        f.recent = Some(Instant::now());
        assert!(looked(&mut f, later + Duration::from_secs(210)));
        // Online: not looked up.
        f.online.insert(PeerId::random(), bea.public_key());
        assert!(!looked(&mut f, later + Duration::from_secs(4 * 3600)));
    }

    #[test]
    fn friends_are_looked_up_seldom_only_when_nobody_is_missing() {
        let ana = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let bea_book = FriendBook::new(&bea, "Bea", 1).unwrap();
        let mut f = FriendContext::new(FriendBook::new(&ana, "Ana", 1).unwrap());
        assert!(f.nobody_missing());
        // A request waits for its answer, which the device asked announces where both look.
        f.book.request_sent(bea.public_key(), "Bea", 1).unwrap();
        assert!(!f.nobody_missing());
        f.book.outgoing.clear();
        // A friend offline is looked for; online, nobody is missing again.
        f.book.friends.insert(
            bea.public_key(),
            Friend {
                profile: bea_book.profile.clone(),
                secret: bea_book.secret.clone(),
                since_ms: 2,
                delivered: true,
                banner: None,
                bio: None,
            },
        );
        assert!(!f.nobody_missing());
        f.online.insert(PeerId::random(), bea.public_key());
        assert!(f.nobody_missing());
    }

    /// Deliveries meet under the secret two devices share, never under a friend's own secret,
    /// which the friend's other friends know; and only what is found under a friend's own
    /// secret needs its proof.
    #[test]
    fn deliveries_meet_where_only_the_two_can() {
        let ana = DeviceIdentity::generate().unwrap();
        let (bea, cid, dan) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        let bea_book = FriendBook::new(&bea, "Bea", 1).unwrap();
        let mut f = FriendContext::new(FriendBook::new(&ana, "Ana", 1).unwrap());
        // Bea waits for Ana's new secret, a request to Cid waits for its answer, and Dan was
        // removed.
        f.book.friends.insert(
            bea.public_key(),
            Friend {
                profile: bea_book.profile.clone(),
                secret: bea_book.secret.clone(),
                since_ms: 2,
                delivered: false,
                banner: None,
                bio: None,
            },
        );
        let request = f.book.request_sent(cid.public_key(), "Cid", 1).unwrap();
        let dan_secret = FriendSecret::generate().unwrap();
        f.book.departing.insert(
            dan.public_key(),
            fogata_friends::Departing {
                secret: dan_secret.clone(),
                since_ms: 1,
            },
        );
        let pair = |other: &DeviceIdentity| {
            FriendSecret::pair(other, &ana.public_key())
                .unwrap()
                .rendezvous()
        };
        let announced = f.extra_announcements(&ana);
        assert!(announced.contains(&pair(&bea)));
        assert!(announced.contains(&pair(&cid)));
        assert!(!announced.contains(&bea_book.secret.rendezvous()));
        assert!(!announced.contains(&request.rendezvous()));
        let looked: Vec<([u8; 32], Target, Option<[u8; 32]>)> = f.lookups(&ana);
        let bea_own = bea_book.secret.rendezvous();
        let dan_own = dan_secret.rendezvous();
        for (secret, target, proof) in [
            (bea_own, Target::Friend(bea.public_key()), Some(bea_own)),
            (pair(&bea), Target::Friend(bea.public_key()), None),
            (dan_own, Target::Departing(dan.public_key()), Some(dan_own)),
            (pair(&dan), Target::Departing(dan.public_key()), None),
            (pair(&cid), Target::Accepting, None),
        ] {
            assert!(looked.contains(&(secret, target, proof)), "{target:?}");
        }
        assert_eq!(looked.len(), 5);
        // Nothing is looked up under this device's own secrets any more.
        assert!(
            !looked
                .iter()
                .any(|(s, ..)| *s == f.book.secret.rendezvous())
        );
    }

    /// A device that knows Bea's friend secret (another friend of hers) but is not Bea: it
    /// answers probes with her tag, and other requests with `answer`, counting those
    /// `answer` takes. Returns its address, and its task, which ends when that is dropped.
    async fn impostor(
        secret: FriendSecret,
        answer: fn(&DeviceIdentity, &[u8], &[u8], FriendRequest) -> Option<FriendResponse>,
        counted: Arc<AtomicUsize>,
    ) -> (Multiaddr, Impostor) {
        let mut swarm = SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_quic()
            .with_behaviour(|_| {
                request_response::Behaviour::with_codec(
                    crate::behaviour::message_codec::<FriendRequest, FriendResponse>(),
                    [(
                        StreamProtocol::new(fogata_proto::FRIEND_PROTOCOL),
                        ProtocolSupport::Full,
                    )],
                    request_response::Config::default(),
                )
            })
            .unwrap()
            .build();
        swarm
            .listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap())
            .unwrap();
        let address = loop {
            if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
                break address;
            }
        };
        let me = *swarm.local_peer_id();
        let identity = DeviceIdentity::generate().unwrap();
        let task = tokio::spawn(async move {
            loop {
                let SwarmEvent::Behaviour(request_response::Event::Message {
                    peer,
                    message:
                        Message::Request {
                            request, channel, ..
                        },
                    ..
                }) = swarm.select_next_some().await
                else {
                    continue;
                };
                let response = match request {
                    FriendRequest::Probe { nonce } => FriendResponse::ProbeReply {
                        tag: secret
                            .probe_tag(&nonce, &me.to_bytes(), &peer.to_bytes())
                            .to_vec(),
                    },
                    request => match answer(&identity, &me.to_bytes(), &peer.to_bytes(), request) {
                        Some(response) => {
                            counted.fetch_add(1, Ordering::Relaxed);
                            response
                        }
                        None => FriendResponse::Refused,
                    },
                };
                let _ = swarm.behaviour_mut().send_response(channel, response);
            }
        });
        (address.with(Protocol::P2p(me)), Impostor(task))
    }

    /// An impostor's task, aborted when dropped.
    struct Impostor(tokio::task::JoinHandle<()>);

    impl Drop for Impostor {
        fn drop(&mut self) {
            self.0.abort();
        }
    }

    fn start_with(
        identity: Arc<DeviceIdentity>,
        book: FriendBook,
        peer: Multiaddr,
    ) -> crate::NodeHandle {
        crate::start(NodeConfig {
            identity,
            groups: Vec::new(),
            friends: Some(book),
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: vec![peer],
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap()
        .0
    }

    async fn until_counted(counted: &AtomicUsize, what: &str) {
        let done = tokio::time::timeout(Duration::from_secs(30), async {
            while counted.load(Ordering::Relaxed) == 0 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        assert!(done.is_ok(), "{what} never went out");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_acknowledgment_from_someone_else_does_not_stop_a_delivery() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = DeviceIdentity::generate().unwrap();
        let bea_book = FriendBook::new(&bea, "Bea", 1).unwrap();
        let mut book = FriendBook::new(&ana, "Ana", 1).unwrap();
        // Ana's new secret has not reached Bea yet.
        book.friends.insert(
            bea.public_key(),
            Friend {
                profile: bea_book.profile.clone(),
                secret: bea_book.secret.clone(),
                since_ms: 2,
                delivered: false,
                banner: None,
                bio: None,
            },
        );
        let acks = Arc::new(AtomicUsize::new(0));
        let (address, _impostor) = impostor(
            bea_book.secret.clone(),
            |_, _, _, request| {
                matches!(request, FriendRequest::Accept { .. }).then_some(FriendResponse::Ack)
            },
            acks.clone(),
        )
        .await;
        let node = start_with(ana, book, address);
        until_counted(&acks, "the acceptance").await;
        let book = node.friends().await.unwrap().unwrap();
        assert!(!book.friends[&bea.public_key()].delivered);
    }

    /// A removal is over only once the former friend itself acknowledges it, signed: neither
    /// a bare acknowledgment nor one signed by another device ends it.
    #[tokio::test(flavor = "multi_thread")]
    async fn only_the_former_friend_ends_its_removal() {
        type Answer = fn(&DeviceIdentity, &[u8], &[u8], FriendRequest) -> Option<FriendResponse>;
        let bare: Answer = |_, _, _, request| {
            matches!(request, FriendRequest::Unfriend { .. }).then_some(FriendResponse::Ack)
        };
        // It cannot open the removal, sealed to Bea, so it signs a removal it makes up, as
        // the device it is.
        let signed: Answer = |identity, local, remote, request| {
            let FriendRequest::Unfriend { sealed } = request else {
                return None;
            };
            let remover = DeviceIdentity::generate().unwrap().public_key();
            let digest = fogata_friends::removal_digest(&sealed);
            fogata_friends::unfriend_ack(identity, &remover, local, remote, &digest)
                .ok()
                .map(|sealed| FriendResponse::Unfriended { sealed })
        };
        for answer in [bare, signed] {
            let ana = Arc::new(DeviceIdentity::generate().unwrap());
            let bea = DeviceIdentity::generate().unwrap();
            let bea_book = FriendBook::new(&bea, "Bea", 1).unwrap();
            let mut book = FriendBook::new(&ana, "Ana", 1).unwrap();
            book.departing.insert(
                bea.public_key(),
                fogata_friends::Departing {
                    secret: bea_book.secret.clone(),
                    since_ms: now_ms(),
                },
            );
            let told = Arc::new(AtomicUsize::new(0));
            let (address, _impostor) =
                impostor(bea_book.secret.clone(), answer, told.clone()).await;
            let node = start_with(ana, book, address);
            until_counted(&told, "the removal").await;
            let book = node.friends().await.unwrap().unwrap();
            assert!(book.departing.contains_key(&bea.public_key()));
        }
    }

    /// A device reached on the local network answers a code with nonsense: that is not the
    /// device showing the code, and the code is still looked for.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_device_that_answers_the_code_with_nonsense_does_not_end_the_search() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let port = std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let (bea_node, _bea_events) = crate::start(NodeConfig {
            identity: bea.clone(),
            groups: Vec::new(),
            friends: Some(FriendBook::new(&bea, "Bea", 1).unwrap()),
            listen_port: port,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap();
        let code = bea_node.create_friend_code().await.unwrap();

        let nonsense = Arc::new(AtomicUsize::new(0));
        let (liar, _liar) = impostor(
            FriendSecret::generate().unwrap(),
            |_, _, _, request| {
                matches!(request, FriendRequest::CodeStart { .. }).then(|| {
                    FriendResponse::CodeReply {
                        spake: vec![7; 33],
                        sealed: vec![9; 64],
                    }
                })
            },
            nonsense.clone(),
        )
        .await;
        let (ana_node, mut events) = crate::start(NodeConfig {
            identity: ana.clone(),
            groups: Vec::new(),
            friends: Some(FriendBook::new(&ana, "Ana", 1).unwrap()),
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: vec![liar],
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap();
        ana_node.use_friend_code(code).await.unwrap();
        until_counted(&nonsense, "the answer with nonsense").await;
        while let Ok(event) = events.try_recv() {
            assert!(
                !matches!(event, NodeEvent::FriendRequestFailed(_)),
                "{event:?}"
            );
        }

        // The device showing the code turns up afterwards, and is still asked.
        let bea_at = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
            .parse()
            .unwrap();
        ana_node.dial(vec![bea_at], None).await.unwrap();
        let sent = tokio::time::timeout(Duration::from_secs(30), async {
            while let Some(event) = events.recv().await {
                match event {
                    NodeEvent::FriendRequestSent { .. } => return true,
                    NodeEvent::FriendRequestFailed(why) => panic!("{why}"),
                    _ => {}
                }
            }
            false
        })
        .await;
        assert_eq!(sent, Ok(true));
    }

    #[test]
    fn a_friend_asks_about_a_bounded_number_of_keys_a_day() {
        let start = Instant::now();
        let hour = |n: u64| start + Duration::from_secs(n * 3600 + 1);
        let mut answers = Vec::new();
        // A device with up to 112 friends asks with 128 keys: an answer every hour.
        for n in 0..24 {
            assert!(may_answer(&mut answers, hour(n), None), "hour {n}");
            assert!(may_answer(&mut answers, hour(n), Some(128)), "hour {n}");
            assert!(!may_answer(&mut answers, hour(n), None));
        }
        // One asking about a thousand keys each time: four answers a day.
        let mut answers = Vec::new();
        let given = (0..24)
            .filter(|n| may_answer(&mut answers, hour(*n), Some(1008)))
            .count();
        assert_eq!(given, 4);
        // A day later, room again.
        assert!(may_answer(&mut answers, hour(25), Some(1008)));
    }

    /// The name of a friend request's kind. Its match has no wildcard: a kind added to
    /// `FriendRequest` does not compile here until it is named, and then belongs in
    /// `one_friend_request_of_each` below and in the answers expected of it.
    fn friend_request_kind(request: &FriendRequest) -> &'static str {
        match request {
            FriendRequest::Probe { .. } => "Probe",
            FriendRequest::Hello { .. } => "Hello",
            FriendRequest::LinkRequest { .. } => "LinkRequest",
            FriendRequest::CodeStart { .. } => "CodeStart",
            FriendRequest::CodeFinish { .. } => "CodeFinish",
            FriendRequest::Accept { .. } => "Accept",
            FriendRequest::Profile { .. } => "Profile",
            FriendRequest::Group { .. } => "Group",
            FriendRequest::Status { .. } => "Status",
            FriendRequest::Heartbeat => "Heartbeat",
            FriendRequest::Call { .. } => "Call",
            FriendRequest::Banner { .. } => "Banner",
            FriendRequest::Unfriend { .. } => "Unfriend",
            FriendRequest::Mutual { .. } => "Mutual",
            FriendRequest::Direct { .. } => "Direct",
            FriendRequest::Animation { .. } => "Animation",
            FriendRequest::Bio { .. } => "Bio",
        }
    }

    /// The name of a friend answer's kind.
    fn friend_response_kind(response: &FriendResponse) -> &'static str {
        match response {
            FriendResponse::ProbeReply { .. } => "ProbeReply",
            FriendResponse::HelloReply { .. } => "HelloReply",
            FriendResponse::CodeReply { .. } => "CodeReply",
            FriendResponse::Pending => "Pending",
            FriendResponse::Ack => "Ack",
            FriendResponse::Refused => "Refused",
            FriendResponse::Mutual { .. } => "Mutual",
            FriendResponse::Unfriended { .. } => "Unfriended",
        }
    }

    /// A friend request of every kind, each with the least it can hold: what passes or
    /// fails before its sealed content is opened.
    fn one_friend_request_of_each() -> Vec<FriendRequest> {
        let sealed = || vec![1, 2, 3];
        vec![
            FriendRequest::Probe { nonce: vec![1; 16] },
            FriendRequest::Hello {
                mac: sealed(),
                sealed: sealed(),
            },
            FriendRequest::LinkRequest {
                mac: sealed(),
                sealed: sealed(),
            },
            FriendRequest::CodeStart {
                gate: vec![1; 32],
                spake: sealed(),
            },
            FriendRequest::CodeFinish { sealed: sealed() },
            FriendRequest::Accept {
                mac: sealed(),
                sealed: sealed(),
            },
            FriendRequest::Profile { sealed: sealed() },
            FriendRequest::Group {
                entries: Vec::new(),
                declined: None,
                part: None,
            },
            FriendRequest::Status { sealed: sealed() },
            FriendRequest::Heartbeat,
            FriendRequest::Call { sealed: sealed() },
            FriendRequest::Banner { sealed: sealed() },
            FriendRequest::Unfriend { sealed: sealed() },
            FriendRequest::Mutual { sealed: sealed() },
            FriendRequest::Direct {
                entries: Vec::new(),
            },
            FriendRequest::Animation { sealed: sealed() },
            FriendRequest::Bio { sealed: sealed() },
        ]
    }

    /// What a friend request of every kind gets: from a peer that is no friend, from a
    /// friend online, and from anyone when this device keeps no friends. Only a probe is
    /// answered whoever asks; what a friend sends is opened only for a friend online, and
    /// sealed content that does not open is refused. The answers to what opens are in the
    /// tests of each kind.
    #[tokio::test(flavor = "multi_thread")]
    async fn every_friend_request_is_answered_by_who_asks() {
        use fogata_friends::FriendBook;
        let node = |friends: bool| {
            let identity = Arc::new(DeviceIdentity::generate().unwrap());
            let book = friends.then(|| FriendBook::new(&identity, "Ana", 1).unwrap());
            start(NodeConfig {
                identity,
                groups: Vec::new(),
                friends: book,
                listen_port: 0,
                bootstrap: Vec::new(),
                use_dht: false,
                use_mdns: false,
                bridge: false,
                peers: Vec::new(),
                remembered: Vec::new(),
                hidden: false,
            })
            .unwrap()
            .0
        };
        let (ana, cai) = (node(true), node(false));
        let answers = inspect(&ana, |n| {
            let (stranger, bea) = (PeerId::random(), PeerId::random());
            let device = DeviceIdentity::generate().unwrap().public_key();
            n.friends.as_mut().unwrap().online.insert(bea, device);
            one_friend_request_of_each()
                .into_iter()
                .zip(one_friend_request_of_each())
                .map(|(stranger_asks, bea_asks)| {
                    let kind = friend_request_kind(&stranger_asks);
                    let answers = [
                        n.on_friend_request(stranger, stranger_asks),
                        n.on_friend_request(bea, bea_asks),
                    ]
                    .map(|a| friend_response_kind(&a));
                    (kind, answers)
                })
                .collect::<Vec<_>>()
        })
        .await;
        let without = inspect(&cai, |n| {
            let peer = PeerId::random();
            one_friend_request_of_each()
                .into_iter()
                .map(|request| friend_response_kind(&n.on_friend_request(peer, request)))
                .collect::<Vec<_>>()
        })
        .await;
        let refused = ["Refused"; 2];
        let expected = [
            ("Probe", ["ProbeReply"; 2]),
            ("Hello", refused),
            ("LinkRequest", refused),
            ("CodeStart", refused),
            ("CodeFinish", refused),
            ("Accept", refused),
            ("Profile", refused),
            ("Group", refused),
            ("Status", refused),
            ("Heartbeat", ["Refused", "Ack"]),
            ("Call", refused),
            ("Banner", refused),
            ("Unfriend", refused),
            ("Mutual", refused),
            ("Direct", refused),
            ("Animation", refused),
            ("Bio", refused),
        ];
        assert_eq!(answers, expected);
        assert_eq!(without, ["Refused"; 17]);
        ana.shutdown().await.unwrap();
        cai.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_friend_code_outlives_answers_lost_on_the_way() {
        use fogata_friends::{CodeReply, FriendBook};
        use fogata_proto::{FriendRequest, FriendResponse};
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let (node, _events) = start(NodeConfig {
            identity: ana.clone(),
            groups: Vec::new(),
            friends: Some(FriendBook::new(&ana, "Ana", 1).unwrap()),
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap();
        node.create_friend_code().await.unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let bea_book = FriendBook::new(&bea, "Bea", 1).unwrap();
        let outcome = inspect(&node, move |n| {
            let secrets = n
                .friends
                .as_ref()
                .unwrap()
                .code
                .as_ref()
                .unwrap()
                .secrets
                .clone();
            let (bea_peer, ana_peer) = (PeerId::random(), n.local_peer());
            let start_run = |n: &mut Node| {
                let (requester, start) = fogata_friends::code_start(
                    &secrets,
                    &bea_peer.to_bytes(),
                    &ana_peer.to_bytes(),
                );
                let answer = n.on_friend_request(
                    bea_peer,
                    FriendRequest::CodeStart {
                        gate: start.gate.to_vec(),
                        spake: start.spake,
                    },
                );
                (requester, answer)
            };
            // Bea's network drops Ana's answer three times; Bea asks again each time.
            for _ in 0..3 {
                let (_, answer) = start_run(n);
                if !matches!(answer, FriendResponse::CodeReply { .. }) {
                    return "a lost answer cost the code";
                }
            }
            let (requester, FriendResponse::CodeReply { spake, sealed }) = start_run(n) else {
                return "the fourth run was refused";
            };
            let reply = CodeReply { spake, sealed };
            let Ok((_, sealed)) =
                requester.finish(&bea, &bea_book.profile, &bea_book.secret, &reply)
            else {
                return "the run did not open";
            };
            match n.on_friend_request(bea_peer, FriendRequest::CodeFinish { sealed }) {
                FriendResponse::Pending => "asked",
                _ => "the request was refused",
            }
        })
        .await;
        assert_eq!(outcome, "asked");
        node.shutdown().await.unwrap();
    }

    /// A removal sealed to this device's key is answered, signed, only for a device this one
    /// knows: anyone else that learnt the key (a former friend long gone) and sends it to
    /// peers around gets the refusal every other device gives, and cannot tell which peer
    /// this device is. The friend that sent it gets its answer, and again if it lost it.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_removal_is_answered_only_for_a_device_this_one_knows() {
        use fogata_friends::{Friend, FriendBook};
        use fogata_proto::{FriendRequest, FriendResponse};
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = DeviceIdentity::generate().unwrap();
        let bea_book = FriendBook::new(&bea, "Bea", 1).unwrap();
        let mut book = FriendBook::new(&ana, "Ana", 1).unwrap();
        book.friends.insert(
            bea.public_key(),
            Friend {
                profile: bea_book.profile.clone(),
                secret: bea_book.secret.clone(),
                since_ms: 1,
                delivered: true,
                banner: None,
                bio: None,
            },
        );
        let (node, _events) = start(NodeConfig {
            identity: ana.clone(),
            groups: Vec::new(),
            friends: Some(book),
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap();
        let ana_key = ana.public_key();
        let answers = inspect(&node, move |n| {
            let local = n.local_peer();
            let removal = |from: &DeviceIdentity, peer: PeerId| {
                let sealed =
                    fogata_friends::unfriend(from, &ana_key, &peer.to_bytes(), &local.to_bytes())
                        .unwrap();
                FriendRequest::Unfriend { sealed }
            };
            let signed =
                |answer: FriendResponse| matches!(answer, FriendResponse::Unfriended { .. });
            let stranger = DeviceIdentity::generate().unwrap();
            let peer = PeerId::random();
            let from_stranger = n.on_friend_request(peer, removal(&stranger, peer));
            let peer = PeerId::random();
            let from_bea = signed(n.on_friend_request(peer, removal(&bea, peer)));
            let again = signed(n.on_friend_request(peer, removal(&bea, peer)));
            (
                matches!(from_stranger, FriendResponse::Refused),
                from_bea,
                again,
            )
        })
        .await;
        assert_eq!(answers, (true, true, true));
        node.shutdown().await.unwrap();
    }

    /// A former friend whose removal this device took, and whose answer was lost, tells it
    /// again after this device started again: it still gets the signed answer, the book
    /// keeping that it removed this device; a stranger still gets the plain refusal.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_removal_told_again_after_a_restart_is_still_answered() {
        use fogata_friends::{Friend, FriendBook};
        use fogata_proto::{FriendRequest, FriendResponse};
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let bea_book = FriendBook::new(&bea, "Bea", 1).unwrap();
        let mut book = FriendBook::new(&ana, "Ana", 1).unwrap();
        book.friends.insert(
            bea.public_key(),
            Friend {
                profile: bea_book.profile.clone(),
                secret: bea_book.secret.clone(),
                since_ms: 1,
                delivered: true,
                banner: None,
                bio: None,
            },
        );
        let run = |book: FriendBook| {
            start(NodeConfig {
                identity: ana.clone(),
                groups: Vec::new(),
                friends: Some(book),
                listen_port: 0,
                bootstrap: Vec::new(),
                use_dht: false,
                use_mdns: false,
                bridge: false,
                peers: Vec::new(),
                remembered: Vec::new(),
                hidden: false,
            })
            .unwrap()
            .0
        };
        // A removal from `from`, sealed to Ana; whether the answer is signed.
        async fn told(
            node: &crate::NodeHandle,
            from: Arc<DeviceIdentity>,
            ana: DevicePublicKey,
        ) -> Option<bool> {
            inspect(node, move |n| {
                let peer = PeerId::random();
                let sealed = fogata_friends::unfriend(
                    &from,
                    &ana,
                    &peer.to_bytes(),
                    &n.local_peer().to_bytes(),
                )
                .unwrap();
                match n.on_friend_request(peer, FriendRequest::Unfriend { sealed }) {
                    FriendResponse::Unfriended { .. } => Some(true),
                    FriendResponse::Refused => Some(false),
                    _ => None,
                }
            })
            .await
        }
        let ana_key = ana.public_key();
        let node = run(book);
        assert_eq!(told(&node, bea.clone(), ana_key).await, Some(true));
        let saved = node.friends().await.unwrap().unwrap();
        assert!(saved.friends.is_empty());
        node.shutdown().await.unwrap();
        // Started again, with the book as the disk keeps it.
        let node = run(FriendBook::decode(&saved.encode()).unwrap());
        assert_eq!(told(&node, bea.clone(), ana_key).await, Some(true));
        let stranger = Arc::new(DeviceIdentity::generate().unwrap());
        assert_eq!(told(&node, stranger, ana_key).await, Some(false));
        node.shutdown().await.unwrap();
    }
}
