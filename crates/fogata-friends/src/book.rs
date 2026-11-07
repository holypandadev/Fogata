//! What a device knows about its friends. The application keeps it on disk; the network node
//! keeps the copy it works with.
//!
//! A friend request does not carry the requester's friend secret but a one-time request
//! secret, so a device that never accepts cannot follow the requester afterwards. Once both
//! have accepted, each sends the other its friend secret; `delivered` tracks whether a
//! friend has this device's current one.
//!
//! Removing a friend is said to the former friend, who then forgets this device too; until it
//! has heard it, the device stays in `departing`. The former friend notes for a day that it
//! was removed (`removed_us`), so a removal told again, its answer lost, is still answered.
//!
//! A new friend secret reaches each friend the next time both are online. Until every friend
//! has it, the earlier ones are kept in `previous`, so that two friends who both changed
//! theirs while apart still recognise each other.

use std::collections::BTreeMap;

use fogata_crypto::{
    DeviceIdentity, DevicePublicKey,
    codec::{Reader, Writer, encode_secret},
};

use crate::{
    Avatar, Banner, Bio, FriendError, FriendSecret, Introduction, PRESET_AVATARS, Profile,
};

const ENCODING_VERSION: u8 = 6;
/// Versions taken as written by a later Fogata rather than as damage.
const LATER_VERSIONS: u8 = 32;

/// Earlier friend secrets kept at most, and for how long.
pub const MAX_PREVIOUS: usize = 4;
const PREVIOUS_FOR_MS: u64 = 90 * 24 * 3600 * 1000;

/// How long a former friend that removed this device is noted, so that its removal, told
/// again when this device's answer was lost, is still answered; and how many are noted at
/// most (the oldest go first).
pub const REMOVED_US_FOR_MS: u64 = 24 * 3600 * 1000;
pub const MAX_REMOVED_US: usize = 64;

/// How long a request made here waits for its answer. Meanwhile this device announces
/// itself under the request secret, which the other device holds, so one that never
/// answers could otherwise follow it for good.
pub const REQUEST_WAITS_MS: u64 = 30 * 24 * 3600 * 1000;
/// How long a request that came here waits: a day less, so it is not accepted after the
/// device that made it stopped waiting, as its clock may be ahead.
const ANSWER_WAITS_MS: u64 = REQUEST_WAITS_MS - 24 * 3600 * 1000;

#[derive(Clone, Debug)]
pub struct Friend {
    pub profile: Profile,
    /// The friend's friend secret, or its request secret until it sends the real one.
    pub secret: FriendSecret,
    /// When they became friends, in milliseconds since the Unix epoch.
    pub since_ms: u64,
    /// Whether the friend has this device's current friend secret.
    pub delivered: bool,
    /// The friend's banner, once it sent one.
    pub banner: Option<Banner>,
    /// The friend's bio, once it sent one.
    pub bio: Option<Bio>,
}

/// A former friend that has not heard yet that this device removed it.
#[derive(Clone, Debug)]
pub struct Departing {
    /// Its friend secret, to find it.
    pub secret: FriendSecret,
    /// When it was removed, in milliseconds since the Unix epoch.
    pub since_ms: u64,
}

/// A friend secret this device had before its current one.
#[derive(Clone, Debug)]
pub struct Previous {
    pub secret: FriendSecret,
    /// When it was replaced, in milliseconds since the Unix epoch.
    pub until_ms: u64,
}

/// A friend request waiting for an answer.
#[derive(Clone, Debug)]
pub struct Incoming {
    pub profile: Profile,
    /// The requester's request secret.
    pub secret: FriendSecret,
    pub since_ms: u64,
}

/// A friend request this device made, waiting for the other device to accept it.
#[derive(Clone, Debug)]
pub struct Outgoing {
    pub name: String,
    pub since_ms: u64,
    /// The request secret sent with the request; the acceptance comes keyed by it.
    pub secret: FriendSecret,
}

/// What happened to a friend request that arrived.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestOutcome {
    /// It waits for the person to answer.
    New,
    /// It came from a friend that seems to have lost this device; the acceptance will be
    /// sent again.
    AlreadyFriend,
    /// It came from this device itself.
    Ignored,
}

#[derive(Clone, Debug)]
pub struct FriendBook {
    pub profile: Profile,
    pub secret: FriendSecret,
    /// This device's banner, shown to friends only.
    pub banner: Option<Banner>,
    /// This device's bio, shown to friends only; none until the person writes one.
    pub bio: Option<Bio>,
    pub friends: BTreeMap<DevicePublicKey, Friend>,
    pub incoming: BTreeMap<DevicePublicKey, Incoming>,
    pub outgoing: BTreeMap<DevicePublicKey, Outgoing>,
    pub departing: BTreeMap<DevicePublicKey, Departing>,
    /// Earlier friend secrets, newest first, while a friend may know only one of them.
    pub previous: Vec<Previous>,
    /// Former friends that removed this device in the last [`REMOVED_US_FOR_MS`], and when
    /// (milliseconds since the Unix epoch), at most [`MAX_REMOVED_US`].
    pub removed_us: BTreeMap<DevicePublicKey, u64>,
}

impl FriendBook {
    /// A new book with a fresh friend secret and a profile called `name`, with a preset
    /// avatar picked from the device key.
    pub fn new(identity: &DeviceIdentity, name: &str, now_ms: u64) -> Result<Self, FriendError> {
        let avatar = Avatar::Preset(identity.public_key().as_bytes()[0] % PRESET_AVATARS);
        Ok(Self {
            profile: Profile::sign(identity, name, avatar, now_ms)?,
            secret: FriendSecret::generate()?,
            banner: None,
            bio: None,
            friends: BTreeMap::new(),
            incoming: BTreeMap::new(),
            outgoing: BTreeMap::new(),
            departing: BTreeMap::new(),
            previous: Vec::new(),
            removed_us: BTreeMap::new(),
        })
    }

    /// Changes this device's name or avatar.
    pub fn set_profile(
        &mut self,
        identity: &DeviceIdentity,
        name: Option<&str>,
        avatar: Option<Avatar>,
        now_ms: u64,
    ) -> Result<(), FriendError> {
        let name = name.unwrap_or(&self.profile.name).to_string();
        let avatar = avatar.unwrap_or_else(|| self.profile.avatar.clone());
        let version = now_ms.max(self.profile.version_ms + 1);
        self.profile = Profile::sign(identity, &name, avatar, version)?;
        Ok(())
    }

    /// Changes this device's banner. Returns whether it differs from the one it had.
    pub fn set_banner(
        &mut self,
        identity: &DeviceIdentity,
        color: u32,
        picture: Option<Vec<u8>>,
        now_ms: u64,
    ) -> Result<bool, FriendError> {
        if self
            .banner
            .as_ref()
            .is_some_and(|b| b.color == color && b.picture == picture)
        {
            return Ok(false);
        }
        let version = now_ms.max(self.banner.as_ref().map_or(0, |b| b.version_ms + 1));
        self.banner = Some(Banner::sign(identity, color, picture, version)?);
        Ok(true)
    }

    /// Changes this device's bio. Returns whether it differs from the one it had.
    pub fn set_bio(
        &mut self,
        identity: &DeviceIdentity,
        text: &str,
        now_ms: u64,
    ) -> Result<bool, FriendError> {
        let text = crate::clean_bio(text)?;
        if self.bio.as_ref().is_some_and(|b| b.text == text) {
            return Ok(false);
        }
        let version = now_ms.max(self.bio.as_ref().map_or(0, |b| b.version_ms + 1));
        self.bio = Some(Bio::sign(identity, &text, version)?);
        Ok(true)
    }

    /// Records a checked friend request.
    pub fn request_received(&mut self, intro: Introduction, now_ms: u64) -> RequestOutcome {
        let device = intro.profile.device;
        if device == self.profile.device {
            return RequestOutcome::Ignored;
        }
        if let Some(friend) = self.friends.get_mut(&device) {
            if intro.profile.version_ms >= friend.profile.version_ms {
                friend.profile = intro.profile;
            }
            friend.secret = intro.secret;
            friend.delivered = false;
            return RequestOutcome::AlreadyFriend;
        }
        self.outgoing.remove(&device);
        self.incoming.insert(
            device,
            Incoming {
                profile: intro.profile,
                secret: intro.secret,
                since_ms: now_ms,
            },
        );
        RequestOutcome::New
    }

    /// The person answered a request. Returns the new friend if it was accepted.
    pub fn answer(
        &mut self,
        device: &DevicePublicKey,
        accept: bool,
        now_ms: u64,
    ) -> Option<&Friend> {
        let incoming = self.incoming.remove(device)?;
        if !accept {
            return None;
        }
        // Friends again: a removal still on its way to it must not undo that.
        self.departing.remove(device);
        self.friends.insert(
            *device,
            Friend {
                profile: incoming.profile,
                secret: incoming.secret,
                since_ms: now_ms,
                delivered: false,
                banner: None,
                bio: None,
            },
        );
        self.friends.get(device)
    }

    /// Records a request this device makes to `device` and returns the request secret to
    /// send with it: the one of an earlier request to the same device, or a new one.
    pub fn request_sent(
        &mut self,
        device: DevicePublicKey,
        name: &str,
        now_ms: u64,
    ) -> Result<FriendSecret, FriendError> {
        if let Some(outgoing) = self.outgoing.get_mut(&device) {
            outgoing.name = name.to_string();
            outgoing.since_ms = now_ms;
            return Ok(outgoing.secret.clone());
        }
        let secret = FriendSecret::generate()?;
        self.request_sent_with(device, name, secret.clone(), now_ms);
        Ok(secret)
    }

    /// Records a request made with a request secret chosen before the device was known.
    pub fn request_sent_with(
        &mut self,
        device: DevicePublicKey,
        name: &str,
        secret: FriendSecret,
        now_ms: u64,
    ) {
        if self.friends.contains_key(&device) || device == self.profile.device {
            return;
        }
        self.outgoing.insert(
            device,
            Outgoing {
                name: name.to_string(),
                since_ms: now_ms,
                secret,
            },
        );
    }

    /// The friend secret, then the earlier ones a friend may still know, newest first.
    pub fn own_secrets(&self) -> impl Iterator<Item = &FriendSecret> {
        std::iter::once(&self.secret).chain(self.previous.iter().map(|p| &p.secret))
    }

    /// Secrets an acceptance or a friend's update may come keyed by: the friend secrets (see
    /// [`Self::own_secrets`]) and the request secrets of requests still waiting.
    pub fn reply_secrets(&self) -> Vec<FriendSecret> {
        let mut out: Vec<FriendSecret> = self.own_secrets().cloned().collect();
        out.extend(self.outgoing.values().map(|o| o.secret.clone()));
        out
    }

    /// A checked acceptance arrived: a device accepted our request, or a friend sent its
    /// new friend secret. Returns false if this device never asked that device.
    pub fn accept_received(&mut self, intro: Introduction, now_ms: u64) -> bool {
        let device = intro.profile.device;
        let asked = self.outgoing.remove(&device).is_some();
        match self.friends.get_mut(&device) {
            Some(friend) => {
                if intro.profile.version_ms >= friend.profile.version_ms {
                    friend.profile = intro.profile;
                }
                friend.secret = intro.secret;
                true
            }
            None if asked => {
                // Friends again: a removal still on its way to it must not undo that.
                self.departing.remove(&device);
                // The new friend only has our request secret; ours follows.
                self.friends.insert(
                    device,
                    Friend {
                        profile: intro.profile,
                        secret: intro.secret,
                        since_ms: now_ms,
                        delivered: false,
                        banner: None,
                        bio: None,
                    },
                );
                true
            }
            None => false,
        }
    }

    pub fn set_delivered(&mut self, device: &DevicePublicKey) {
        if let Some(friend) = self.friends.get_mut(device) {
            friend.delivered = true;
        }
        if self.friends.values().all(|f| f.delivered) {
            self.previous.clear();
        }
    }

    /// Forgets earlier friend secrets nobody needs any longer: every friend has the current
    /// one, or they were replaced long ago. Returns whether one was forgotten.
    pub fn forget_previous(&mut self, now_ms: u64) -> bool {
        let before = self.previous.len();
        if self.friends.values().all(|f| f.delivered) {
            self.previous.clear();
        }
        self.previous
            .retain(|p| now_ms.saturating_sub(p.until_ms) < PREVIOUS_FOR_MS);
        self.previous.len() != before
    }

    /// Forgets requests made or received longer ago than they wait (see
    /// [`REQUEST_WAITS_MS`]). Returns whether one was forgotten.
    pub fn forget_old_requests(&mut self, now_ms: u64) -> bool {
        let before = self.outgoing.len() + self.incoming.len();
        self.outgoing
            .retain(|_, o| now_ms.saturating_sub(o.since_ms) < REQUEST_WAITS_MS);
        self.incoming
            .retain(|_, i| now_ms.saturating_sub(i.since_ms) < ANSWER_WAITS_MS);
        self.outgoing.len() + self.incoming.len() != before
    }

    /// Keeps a friend's profile if it is newer than the one known. Returns whether it was.
    pub fn update_profile(&mut self, profile: Profile) -> bool {
        match self.friends.get_mut(&profile.device) {
            Some(friend) if profile.version_ms > friend.profile.version_ms => {
                friend.profile = profile;
                true
            }
            _ => false,
        }
    }

    /// Keeps a friend's banner if it is newer than the one known. Returns whether it was.
    pub fn update_banner(&mut self, banner: Banner) -> bool {
        match self.friends.get_mut(&banner.device) {
            Some(friend)
                if friend
                    .banner
                    .as_ref()
                    .is_none_or(|b| banner.version_ms > b.version_ms) =>
            {
                friend.banner = Some(banner);
                true
            }
            _ => false,
        }
    }

    /// Keeps a friend's bio if it is newer than the one known. Returns whether it was.
    pub fn update_bio(&mut self, bio: Bio) -> bool {
        match self.friends.get_mut(&bio.device) {
            Some(friend)
                if friend
                    .bio
                    .as_ref()
                    .is_none_or(|b| bio.version_ms > b.version_ms) =>
            {
                friend.bio = Some(bio);
                true
            }
            _ => false,
        }
    }

    /// Forgets a friend or a request in either direction. Returns whether there was one.
    ///
    /// Forgetting a friend also makes a new friend secret, so that the former friend can no
    /// longer find this device; the remaining friends get it the next time both are online.
    /// The former friend waits in `departing` until it hears that it was removed.
    pub fn remove(&mut self, device: &DevicePublicKey, now_ms: u64) -> Result<bool, FriendError> {
        let friend = self.friends.remove(device);
        let incoming = self.incoming.remove(device).is_some();
        let outgoing = self.outgoing.remove(device).is_some();
        let was_friend = friend.is_some();
        if let Some(friend) = friend {
            self.departing.insert(
                *device,
                Departing {
                    secret: friend.secret,
                    since_ms: now_ms,
                },
            );
            self.rotate_secret(now_ms)?;
        }
        Ok(was_friend || incoming || outgoing)
    }

    /// A friend removed this device: forget it as well, with a new friend secret, and without
    /// telling it back. Returns whether it was a friend.
    pub fn removed_by(
        &mut self,
        device: &DevicePublicKey,
        now_ms: u64,
    ) -> Result<bool, FriendError> {
        self.incoming.remove(device);
        self.outgoing.remove(device);
        if self.friends.remove(device).is_none() {
            return Ok(false);
        }
        self.rotate_secret(now_ms)?;
        self.forget_old_removals(now_ms);
        self.removed_us.insert(*device, now_ms);
        while self.removed_us.len() > MAX_REMOVED_US {
            let oldest = self
                .removed_us
                .iter()
                .min_by_key(|(_, at)| **at)
                .map(|(d, _)| *d);
            if let Some(oldest) = oldest {
                self.removed_us.remove(&oldest);
            }
        }
        Ok(true)
    }

    /// Whether `device` removed this device in the last [`REMOVED_US_FOR_MS`].
    pub fn removed_us_lately(&self, device: &DevicePublicKey, now_ms: u64) -> bool {
        self.removed_us
            .get(device)
            .is_some_and(|at| now_ms.saturating_sub(*at) < REMOVED_US_FOR_MS)
    }

    /// Forgets former friends that removed this device longer ago than
    /// [`REMOVED_US_FOR_MS`]. Returns whether one was forgotten.
    pub fn forget_old_removals(&mut self, now_ms: u64) -> bool {
        let before = self.removed_us.len();
        self.removed_us
            .retain(|_, at| now_ms.saturating_sub(*at) < REMOVED_US_FOR_MS);
        self.removed_us.len() != before
    }

    /// The former friend heard that it was removed. Returns whether it was waiting.
    pub fn departed(&mut self, device: &DevicePublicKey) -> bool {
        self.departing.remove(device).is_some()
    }

    /// Makes a new friend secret; every friend needs to be sent it. The one it replaces is
    /// kept in `previous` while a friend may know only that one.
    pub fn rotate_secret(&mut self, now_ms: u64) -> Result<(), FriendError> {
        let old = std::mem::replace(&mut self.secret, FriendSecret::generate()?);
        self.previous.insert(
            0,
            Previous {
                secret: old,
                until_ms: now_ms,
            },
        );
        self.previous.truncate(MAX_PREVIOUS);
        for friend in self.friends.values_mut() {
            friend.delivered = false;
        }
        Ok(())
    }

    /// The name to show for a device this book knows, if any.
    pub fn name_of(&self, device: &DevicePublicKey) -> Option<&str> {
        if let Some(friend) = self.friends.get(device) {
            return Some(&friend.profile.name);
        }
        if let Some(incoming) = self.incoming.get(device) {
            return Some(&incoming.profile.name);
        }
        self.outgoing.get(device).map(|o| o.name.as_str())
    }

    /// The book as the friends file holds it: every friend secret in it, so it is wiped from
    /// memory when dropped.
    pub fn encode(&self) -> zeroize::Zeroizing<Vec<u8>> {
        // Measured first, so it is written where it stays: no copy of a secret is left
        // behind as the buffer grows.
        encode_secret(0, |w| self.write(w))
    }

    fn write(&self, w: &mut Writer) {
        w.u8(ENCODING_VERSION)
            .bytes(&self.profile.encode())
            .raw(self.secret.as_bytes());
        write_banner(w, self.banner.as_ref());
        write_bio(w, self.bio.as_ref());
        w.u32(self.friends.len() as u32);
        for friend in self.friends.values() {
            w.bytes(&friend.profile.encode())
                .raw(friend.secret.as_bytes())
                .u64(friend.since_ms)
                .u8(friend.delivered.into());
            write_banner(w, friend.banner.as_ref());
            write_bio(w, friend.bio.as_ref());
        }
        w.u32(self.incoming.len() as u32);
        for incoming in self.incoming.values() {
            w.bytes(&incoming.profile.encode())
                .raw(incoming.secret.as_bytes())
                .u64(incoming.since_ms);
        }
        w.u32(self.outgoing.len() as u32);
        for (device, outgoing) in &self.outgoing {
            w.key(device)
                .str(&outgoing.name)
                .u64(outgoing.since_ms)
                .raw(outgoing.secret.as_bytes());
        }
        w.u32(self.departing.len() as u32);
        for (device, departing) in &self.departing {
            w.key(device)
                .raw(departing.secret.as_bytes())
                .u64(departing.since_ms);
        }
        w.u32(self.previous.len() as u32);
        for previous in &self.previous {
            w.raw(previous.secret.as_bytes()).u64(previous.until_ms);
        }
        w.u32(self.removed_us.len() as u32);
        for (device, at) in &self.removed_us {
            w.key(device).u64(*at);
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FriendError> {
        let mut r = Reader::new(bytes);
        let version = r.u8()?;
        // A version a little past this one is a later Fogata's; a byte far past it is damage.
        if (ENCODING_VERSION + 1..=LATER_VERSIONS).contains(&version) {
            return Err(FriendError::NewerVersion);
        }
        if !(1..=ENCODING_VERSION).contains(&version) {
            return Err(FriendError::Malformed);
        }
        // Version 3 added banners and the former friends still to tell.
        let banners = version >= 3;
        let profile = Profile::decode(r.bytes()?)?;
        let secret = FriendSecret::from_bytes(r.take(32)?)?;
        let banner = if banners { read_banner(&mut r)? } else { None };
        // Version 5 added bios.
        let bios = version >= 5;
        let bio = if bios { read_bio(&mut r)? } else { None };
        let mut friends = BTreeMap::new();
        for _ in 0..r.count(4 + 32 + 8 + 1)? {
            let profile = Profile::decode(r.bytes()?)?;
            let friend = Friend {
                secret: FriendSecret::from_bytes(r.take(32)?)?,
                since_ms: r.u64()?,
                delivered: r.u8()? != 0,
                banner: if banners { read_banner(&mut r)? } else { None },
                bio: if bios { read_bio(&mut r)? } else { None },
                profile,
            };
            if friend
                .banner
                .as_ref()
                .is_some_and(|b| b.device != friend.profile.device)
                || friend
                    .bio
                    .as_ref()
                    .is_some_and(|b| b.device != friend.profile.device)
            {
                return Err(FriendError::Malformed);
            }
            friends.insert(friend.profile.device, friend);
        }
        let mut incoming = BTreeMap::new();
        for _ in 0..r.count(4 + 32 + 8)? {
            let profile = Profile::decode(r.bytes()?)?;
            let request = Incoming {
                secret: FriendSecret::from_bytes(r.take(32)?)?,
                since_ms: r.u64()?,
                profile,
            };
            incoming.insert(request.profile.device, request);
        }
        let mut outgoing = BTreeMap::new();
        for _ in 0..r.count(32 + 4 + 8)? {
            let device = r.key()?;
            let name = r.str()?;
            let since_ms = r.u64()?;
            // Version 1 requests carried the friend secret itself.
            let request_secret = if version == 1 {
                secret.clone()
            } else {
                FriendSecret::from_bytes(r.take(32)?)?
            };
            outgoing.insert(
                device,
                Outgoing {
                    name,
                    since_ms,
                    secret: request_secret,
                },
            );
        }
        let mut departing = BTreeMap::new();
        if banners {
            for _ in 0..r.count(32 + 32 + 8)? {
                let device = r.key()?;
                departing.insert(
                    device,
                    Departing {
                        secret: FriendSecret::from_bytes(r.take(32)?)?,
                        since_ms: r.u64()?,
                    },
                );
            }
        }
        // Version 4 added the earlier friend secrets.
        let mut previous = Vec::new();
        if version >= 4 {
            for _ in 0..r.count(32 + 8)? {
                previous.push(Previous {
                    secret: FriendSecret::from_bytes(r.take(32)?)?,
                    until_ms: r.u64()?,
                });
            }
        }
        if previous.len() > MAX_PREVIOUS {
            return Err(FriendError::Malformed);
        }
        // Version 6 added the former friends that removed this device lately.
        let mut removed_us = BTreeMap::new();
        if version >= 6 {
            for _ in 0..r.count(32 + 8)? {
                removed_us.insert(r.key()?, r.u64()?);
            }
        }
        if removed_us.len() > MAX_REMOVED_US {
            return Err(FriendError::Malformed);
        }
        r.finish()?;
        if banner.as_ref().is_some_and(|b| b.device != profile.device)
            || bio.as_ref().is_some_and(|b| b.device != profile.device)
        {
            return Err(FriendError::Malformed);
        }
        Ok(Self {
            profile,
            secret,
            banner,
            bio,
            friends,
            incoming,
            outgoing,
            departing,
            previous,
            removed_us,
        })
    }
}

fn write_banner(w: &mut Writer, banner: Option<&Banner>) {
    match banner {
        Some(banner) => w.u8(1).bytes(&banner.encode()),
        None => w.u8(0),
    };
}

fn write_bio(w: &mut Writer, bio: Option<&Bio>) {
    match bio {
        Some(bio) => w.u8(1).bytes(&bio.encode()),
        None => w.u8(0),
    };
}

fn read_bio(r: &mut Reader<'_>) -> Result<Option<Bio>, FriendError> {
    match r.u8()? {
        0 => Ok(None),
        1 => Ok(Some(Bio::decode(r.bytes()?)?)),
        _ => Err(FriendError::Malformed),
    }
}

fn read_banner(r: &mut Reader<'_>) -> Result<Option<Banner>, FriendError> {
    match r.u8()? {
        0 => Ok(None),
        1 => Ok(Some(Banner::decode(r.bytes()?)?)),
        _ => Err(FriendError::Malformed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Damaged copies of `good`, as a hostile peer could send: bits flipped, bytes and
    /// lengths changed, cut short, grown, and plain noise. The same ones every run.
    fn damaged(good: &[u8], seed: u64) -> Vec<Vec<u8>> {
        let mut x = seed | 1;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as usize
        };
        (0..500)
            .map(|_| {
                let mut bytes = good.to_vec();
                let at = next() % (bytes.len() + 1);
                match next() % 6 {
                    0 if at < bytes.len() => bytes[at] ^= 1 << (next() % 8),
                    1 if at < bytes.len() => bytes[at] = next() as u8,
                    2 => bytes.truncate(at),
                    3 => {
                        let extra: Vec<u8> = (0..next() % 64).map(|_| next() as u8).collect();
                        bytes.splice(at..at, extra);
                    }
                    4 if at + 4 <= bytes.len() => {
                        let big = [u32::MAX, 0x8000_0000, 0x0100_0000][next() % 3];
                        bytes[at..at + 4].copy_from_slice(&big.to_be_bytes());
                    }
                    _ => bytes = (0..next() % 300).map(|_| next() as u8).collect(),
                }
                bytes
            })
            .collect()
    }

    fn intro(identity: &DeviceIdentity, name: &str, version: u64) -> Introduction {
        Introduction {
            profile: Profile::sign(identity, name, Avatar::Preset(0), version).unwrap(),
            secret: FriendSecret::generate().unwrap(),
        }
    }

    #[test]
    fn requests_become_friends() {
        let me = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let cid = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();

        assert_eq!(
            book.request_received(intro(&me, "Ana", 1), 2),
            RequestOutcome::Ignored
        );
        assert_eq!(
            book.request_received(intro(&bea, "Bea", 1), 2),
            RequestOutcome::New
        );
        assert!(book.answer(&bea.public_key(), true, 3).is_some());
        assert!(!book.friends[&bea.public_key()].delivered);
        book.set_delivered(&bea.public_key());
        assert!(book.friends[&bea.public_key()].delivered);

        // A request from a friend means the acceptance got lost on its side.
        assert_eq!(
            book.request_received(intro(&bea, "Bea", 2), 4),
            RequestOutcome::AlreadyFriend
        );
        assert!(!book.friends[&bea.public_key()].delivered);

        // Declined requests are forgotten.
        book.request_received(intro(&cid, "Cid", 1), 5);
        assert!(book.answer(&cid.public_key(), false, 6).is_none());
        assert!(book.incoming.is_empty());
        assert!(!book.friends.contains_key(&cid.public_key()));
    }

    #[test]
    fn requests_wait_a_month_at_most() {
        let me = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let cid = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        book.request_sent(bea.public_key(), "Bea", 10).unwrap();
        book.request_received(intro(&cid, "Cid", 1), 10);
        assert!(!book.forget_old_requests(10 + ANSWER_WAITS_MS - 1));
        // The one that came goes first: its maker may stop waiting a little earlier.
        assert!(book.forget_old_requests(10 + ANSWER_WAITS_MS));
        assert!(book.incoming.is_empty() && !book.outgoing.is_empty());
        assert!(book.forget_old_requests(10 + REQUEST_WAITS_MS));
        assert!(book.outgoing.is_empty());
        assert!(
            !book
                .reply_secrets()
                .iter()
                .any(|s| s.as_bytes() != book.secret.as_bytes())
        );
    }

    #[test]
    fn acceptances_need_a_request() {
        let me = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        assert!(!book.accept_received(intro(&bea, "Bea", 1), 2));
        let secret = book.request_sent(bea.public_key(), "Bea", 2).unwrap();
        assert_ne!(secret.as_bytes(), book.secret.as_bytes());
        // Asking again reuses the request secret.
        let again = book.request_sent(bea.public_key(), "Bea", 3).unwrap();
        assert_eq!(again.as_bytes(), secret.as_bytes());
        assert_eq!(book.reply_secrets().len(), 2);
        assert_eq!(book.name_of(&bea.public_key()), Some("Bea"));
        assert!(book.accept_received(intro(&bea, "Bea", 1), 3));
        assert!(book.outgoing.is_empty());
        // Bea only has the request secret so far.
        assert!(!book.friends[&bea.public_key()].delivered);
    }

    #[test]
    fn removing_a_friend_makes_a_new_secret() {
        let me = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let cid = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        for (who, name) in [(&bea, "Bea"), (&cid, "Cid")] {
            book.request_received(intro(who, name, 1), 2);
            book.answer(&who.public_key(), true, 3);
            book.set_delivered(&who.public_key());
        }
        let cid_secret = book.friends[&cid.public_key()].secret.clone();
        let before = *book.secret.as_bytes();
        assert!(book.remove(&cid.public_key(), 4).unwrap());
        assert_ne!(*book.secret.as_bytes(), before);
        assert!(!book.friends[&bea.public_key()].delivered);

        // Forgetting a request does not.
        let dan = DeviceIdentity::generate().unwrap();
        book.request_sent(dan.public_key(), "Dan", 4).unwrap();
        let before = *book.secret.as_bytes();
        assert!(book.remove(&dan.public_key(), 5).unwrap());
        assert_eq!(*book.secret.as_bytes(), before);
        assert!(!book.remove(&dan.public_key(), 6).unwrap());
        // Only the former friend waits to hear it.
        assert_eq!(book.departing.len(), 1);
        assert_eq!(
            book.departing[&cid.public_key()].secret.as_bytes(),
            cid_secret.as_bytes()
        );
        assert!(book.departed(&cid.public_key()));
        assert!(!book.departed(&cid.public_key()));
    }

    #[test]
    fn befriending_again_cancels_a_removal_not_delivered_yet() {
        let me = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        book.request_received(intro(&bea, "Bea", 1), 2);
        book.answer(&bea.public_key(), true, 3);
        // Removed while Bea was away: the removal waits to reach her.
        book.remove(&bea.public_key(), 4).unwrap();
        assert!(book.departing.contains_key(&bea.public_key()));
        // Before it does, they become friends again with a new request from her.
        book.request_received(intro(&bea, "Bea", 5), 6);
        book.answer(&bea.public_key(), true, 7);
        assert!(book.friends.contains_key(&bea.public_key()));
        assert!(book.departing.is_empty(), "the old removal would undo it");

        // The same when this device asks and she accepts.
        book.remove(&bea.public_key(), 8).unwrap();
        book.request_sent(bea.public_key(), "Bea", 9).unwrap();
        assert!(book.accept_received(intro(&bea, "Bea", 10), 11));
        assert!(book.departing.is_empty(), "the old removal would undo it");
    }

    #[test]
    fn a_friend_that_removed_us_is_forgotten_without_telling_back() {
        let me = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        book.request_received(intro(&bea, "Bea", 1), 2);
        book.answer(&bea.public_key(), true, 3);
        let before = *book.secret.as_bytes();
        assert!(book.removed_by(&bea.public_key(), 5).unwrap());
        assert!(book.friends.is_empty());
        assert!(book.departing.is_empty());
        assert_ne!(*book.secret.as_bytes(), before);
        assert!(!book.removed_by(&bea.public_key(), 5).unwrap());
    }

    /// A book saved by the version before (5, written by that version's code) opens with
    /// everything in it, and is written in this version with nothing lost.
    #[test]
    fn a_book_of_version_5_opens_with_nothing_lost() {
        let old = include_bytes!("../tests/data/book-v5.bin");
        assert_eq!(old[0], 5);
        let book = FriendBook::decode(old).unwrap();
        let key = |n: u8| DeviceIdentity::from_secret_bytes(&[n; 32]).public_key();
        assert_eq!(book.profile.name, "Ana");
        assert_eq!(book.profile.device, key(1));
        // Signed into the fixture by the version that wrote it, so it stays as it was.
        assert_eq!(book.bio.as_ref().unwrap().text, "Hola");
        assert_eq!(book.banner.as_ref().unwrap().color, 0xFF3D21AD);
        let bea = &book.friends[&key(2)];
        assert_eq!(bea.profile.name, "Bea");
        assert_eq!(bea.bio.as_ref().unwrap().text, "Bea's bio");
        assert_eq!(bea.banner.as_ref().unwrap().color, 0xFF0E7C66);
        assert_eq!(book.incoming[&key(3)].profile.name, "Cid");
        assert_eq!(book.outgoing[&key(4)].name, "Dan");
        assert_eq!(book.departing[&key(5)].since_ms, 17);
        assert_eq!(book.previous.len(), 1);
        assert!(book.removed_us.is_empty());
        // Written now, it reads back the same, every field of the old one in it.
        let new = book.encode();
        assert_eq!(new[0], ENCODING_VERSION);
        let again = FriendBook::decode(&new).unwrap();
        assert_eq!(*again.encode(), *new);
        assert_eq!(&new[1..old.len()], &old[1..]);
    }

    #[test]
    fn a_former_friend_that_removed_us_is_noted_for_a_day_and_kept_with_the_book() {
        let me = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        book.request_received(intro(&bea, "Bea", 1), 2);
        book.answer(&bea.public_key(), true, 3);
        assert!(book.removed_by(&bea.public_key(), 1_000).unwrap());
        let book = FriendBook::decode(&book.encode()).unwrap();
        assert!(book.removed_us_lately(&bea.public_key(), 1_000 + REMOVED_US_FOR_MS - 1));
        assert!(!book.removed_us_lately(&bea.public_key(), 1_000 + REMOVED_US_FOR_MS));
        let mut book = book;
        assert!(!book.forget_old_removals(1_000 + REMOVED_US_FOR_MS - 1));
        assert!(book.forget_old_removals(1_000 + REMOVED_US_FOR_MS));
        assert!(book.removed_us.is_empty());
    }

    #[test]
    fn only_so_many_former_friends_that_removed_us_are_noted() {
        let me = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        let mut first = None;
        for n in 0..MAX_REMOVED_US as u64 + 5 {
            let friend = DeviceIdentity::generate().unwrap();
            first.get_or_insert(friend.public_key());
            book.request_received(intro(&friend, "F", 1), 2);
            book.answer(&friend.public_key(), true, 3);
            assert!(book.removed_by(&friend.public_key(), 10 + n).unwrap());
        }
        assert_eq!(book.removed_us.len(), MAX_REMOVED_US);
        // The oldest went first.
        assert!(!book.removed_us.contains_key(&first.unwrap()));
        // A book noting more than that is damaged.
        let mut bytes = book.encode().to_vec();
        let at = bytes.len() - 4 - MAX_REMOVED_US * 40;
        bytes[at..at + 4].copy_from_slice(&(MAX_REMOVED_US as u32 + 1).to_be_bytes());
        bytes.extend_from_slice(&[0; 40]);
        assert!(FriendBook::decode(&bytes).is_err());
    }

    #[test]
    fn banners_change_only_when_they_differ() {
        let me = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        assert!(book.set_banner(&me, 0xFF3D21AD, None, 10).unwrap());
        assert!(!book.set_banner(&me, 0xFF3D21AD, None, 11).unwrap());
        assert!(book.set_banner(&me, 0xFF2563EB, None, 5).unwrap());
        // Newer than the last one even with a clock gone back.
        assert_eq!(book.banner.as_ref().unwrap().version_ms, 11);

        book.request_received(intro(&bea, "Bea", 1), 2);
        book.answer(&bea.public_key(), true, 3);
        let theirs = Banner::sign(&bea, 0xFF0E7C66, None, 7).unwrap();
        assert!(book.update_banner(theirs.clone()));
        assert!(!book.update_banner(theirs));
        assert!(!book.update_banner(Banner::sign(&bea, 0, None, 6).unwrap()));
        // Not from strangers.
        let cid = DeviceIdentity::generate().unwrap();
        assert!(!book.update_banner(Banner::sign(&cid, 0, None, 9).unwrap()));
    }

    #[test]
    fn newer_profiles_replace_older_ones() {
        let me = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        book.request_received(intro(&bea, "Bea", 5), 2);
        book.answer(&bea.public_key(), true, 3);
        let old = Profile::sign(&bea, "Old", Avatar::Preset(0), 4).unwrap();
        assert!(!book.update_profile(old));
        let new = Profile::sign(&bea, "Beatriz", Avatar::Preset(2), 6).unwrap();
        assert!(book.update_profile(new));
        assert_eq!(book.name_of(&bea.public_key()), Some("Beatriz"));

        book.set_profile(&me, Some("Ana M."), None, 0).unwrap();
        assert_eq!(book.profile.name, "Ana M.");
        assert_eq!(book.profile.version_ms, 2);
        assert!(book.profile.verify().is_ok());
    }

    /// The book is written where it stays, of exactly its size: a buffer that grew as it
    /// was written would have left copies of friend secrets behind.
    #[test]
    fn a_book_is_encoded_without_copies_left_behind() {
        let me = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        for i in 0..20 {
            let friend = DeviceIdentity::generate().unwrap();
            book.request_received(intro(&friend, &format!("Bea {i}"), 1), 2);
            book.answer(&friend.public_key(), true, 3);
        }
        let leo = DeviceIdentity::generate().unwrap();
        book.request_received(intro(&leo, "Leo", 1), 4);
        let sofia = DeviceIdentity::generate().unwrap();
        book.request_sent(sofia.public_key(), "Sofía", 5).unwrap();
        book.set_banner(&me, 0xFF3D21AD, None, 6).unwrap();
        book.set_bio(&me, "Hi", 7).unwrap();
        book.rotate_secret(8).unwrap();
        let bytes = book.encode();
        assert_eq!(bytes.capacity(), bytes.len());
        let back = FriendBook::decode(&bytes).unwrap();
        assert_eq!(back.friends.len(), 20);
        assert_eq!(back.previous.len(), 1);
    }

    #[test]
    fn books_round_trip() {
        let me = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let cid = DeviceIdentity::generate().unwrap();
        let dan = DeviceIdentity::generate().unwrap();
        book.request_received(intro(&bea, "Bea", 1), 2);
        book.answer(&bea.public_key(), true, 3);
        book.request_received(intro(&cid, "Cid", 1), 4);
        book.request_sent(dan.public_key(), "Dan", 5).unwrap();

        // All its friend secrets: wiped from memory when the bytes go.
        let bytes: zeroize::Zeroizing<Vec<u8>> = book.encode();
        let back = FriendBook::decode(&bytes).unwrap();
        assert_eq!(back.profile, book.profile);
        assert_eq!(back.secret.as_bytes(), book.secret.as_bytes());
        assert_eq!(back.friends.len(), 1);
        assert_eq!(back.friends[&bea.public_key()].since_ms, 3);
        assert_eq!(back.incoming[&cid.public_key()].profile.name, "Cid");
        let (theirs, ours) = (
            &back.outgoing[&dan.public_key()],
            &book.outgoing[&dan.public_key()],
        );
        assert_eq!((&theirs.name, theirs.since_ms), (&ours.name, ours.since_ms));
        assert_eq!(theirs.secret.as_bytes(), ours.secret.as_bytes());
        assert!(FriendBook::decode(&book.encode()[..20]).is_err());

        // Banners and former friends still to tell come back too.
        book.set_banner(&me, 0xFF3D21AD, None, 6).unwrap();
        book.update_banner(Banner::sign(&bea, 0xFF0E7C66, None, 7).unwrap());
        let eva = DeviceIdentity::generate().unwrap();
        book.request_received(intro(&eva, "Eva", 1), 8);
        book.answer(&eva.public_key(), true, 9);
        book.remove(&eva.public_key(), 10).unwrap();
        let back = FriendBook::decode(&book.encode()).unwrap();
        assert_eq!(back.banner, book.banner);
        assert_eq!(
            back.friends[&bea.public_key()].banner,
            book.friends[&bea.public_key()].banner
        );
        assert_eq!(back.departing[&eva.public_key()].since_ms, 10);
        // So do the earlier friend secrets.
        assert_eq!(back.previous.len(), 1);
        assert_eq!(
            back.previous[0].secret.as_bytes(),
            book.previous[0].secret.as_bytes()
        );
        assert_eq!(back.previous[0].until_ms, 10);

        // And bios, this device's and its friends'.
        assert!(book.set_bio(&me, "Hey there!", 11).unwrap());
        book.update_bio(Bio::sign(&bea, "Bea here", 12).unwrap());
        let back = FriendBook::decode(&book.encode()).unwrap();
        assert_eq!(back.bio, book.bio);
        assert_eq!(back.bio.as_ref().unwrap().text, "Hey there!");
        assert_eq!(
            back.friends[&bea.public_key()].bio.as_ref().unwrap().text,
            "Bea here"
        );
    }

    #[test]
    fn bios_change_only_when_they_do() {
        let me = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        assert!(book.bio.is_none());
        assert!(book.set_bio(&me, "Hey there!", 5).unwrap());
        // The same once tidied: no change, nothing to send.
        assert!(!book.set_bio(&me, "  Hey there! ", 6).unwrap());
        // Newer even with the clock behind.
        assert!(book.set_bio(&me, "", 2).unwrap());
        assert_eq!(book.bio.as_ref().unwrap().version_ms, 6);
        assert_eq!(
            book.set_bio(&me, "Ana\u{202E}", 7),
            Err(FriendError::BadBio)
        );

        book.request_received(intro(&bea, "Bea", 1), 2);
        book.answer(&bea.public_key(), true, 3);
        assert!(book.update_bio(Bio::sign(&bea, "New", 9).unwrap()));
        assert!(!book.update_bio(Bio::sign(&bea, "Old", 8).unwrap()));
        assert_eq!(
            book.friends[&bea.public_key()].bio.as_ref().unwrap().text,
            "New"
        );
        // Only friends' bios are kept.
        let eva = DeviceIdentity::generate().unwrap();
        assert!(!book.update_bio(Bio::sign(&eva, "Eva", 9).unwrap()));
    }

    #[test]
    fn books_saved_before_bios_still_open() {
        let me = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        book.request_received(intro(&bea, "Bea", 1), 2);
        book.answer(&bea.public_key(), true, 3);
        book.set_banner(&me, 0xFF3D21AD, None, 4).unwrap();
        // Version 4: the same layout without bios.
        let mut w = Writer::default();
        w.u8(4)
            .bytes(&book.profile.encode())
            .raw(book.secret.as_bytes());
        write_banner(&mut w, book.banner.as_ref());
        w.u32(1);
        let friend = &book.friends[&bea.public_key()];
        w.bytes(&friend.profile.encode())
            .raw(friend.secret.as_bytes())
            .u64(friend.since_ms)
            .u8(0);
        write_banner(&mut w, None);
        w.u32(0).u32(0).u32(0).u32(0);
        let back = FriendBook::decode(&w.into_bytes()).unwrap();
        assert_eq!(back.friends.len(), 1);
        assert_eq!(back.banner, book.banner);
        assert!(back.bio.is_none() && back.friends[&bea.public_key()].bio.is_none());
    }

    #[test]
    fn earlier_secrets_stay_until_every_friend_has_the_new_one() {
        let me = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        let (bea, cid) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        for (who, name) in [(&bea, "Bea"), (&cid, "Cid")] {
            book.request_received(intro(who, name, 1), 2);
            book.answer(&who.public_key(), true, 3);
            book.set_delivered(&who.public_key());
        }
        let first = book.secret.clone();
        book.rotate_secret(10).unwrap();
        book.rotate_secret(20).unwrap();
        let mine: Vec<_> = book.own_secrets().map(|s| *s.as_bytes()).collect();
        assert_eq!(mine.len(), 3);
        assert_eq!(mine[2], *first.as_bytes());
        assert!(
            book.reply_secrets()
                .iter()
                .any(|s| s.as_bytes() == first.as_bytes())
        );
        // Kept while Cid may know only an old one, and at most a few.
        book.set_delivered(&bea.public_key());
        assert_eq!(book.previous.len(), 2);
        for t in 0..10 {
            book.rotate_secret(30 + t).unwrap();
        }
        assert_eq!(book.previous.len(), MAX_PREVIOUS);
        // Forgotten once too old, or once everyone has the current one.
        assert!(!book.forget_previous(40));
        assert!(book.forget_previous(40 + PREVIOUS_FOR_MS));
        assert!(book.previous.is_empty());
        book.rotate_secret(50).unwrap();
        book.set_delivered(&bea.public_key());
        book.set_delivered(&cid.public_key());
        assert!(book.previous.is_empty());
    }

    #[test]
    fn books_saved_before_banners_still_open() {
        let me = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        book.request_received(intro(&bea, "Bea", 1), 2);
        book.answer(&bea.public_key(), true, 3);
        // Version 2: the same layout without banners and former friends.
        let mut w = Writer::default();
        w.u8(2)
            .bytes(&book.profile.encode())
            .raw(book.secret.as_bytes());
        w.u32(1);
        let friend = &book.friends[&bea.public_key()];
        w.bytes(&friend.profile.encode())
            .raw(friend.secret.as_bytes())
            .u64(friend.since_ms)
            .u8(0);
        w.u32(0).u32(0);
        let back = FriendBook::decode(&w.into_bytes()).unwrap();
        assert_eq!(back.friends.len(), 1);
        assert!(back.banner.is_none() && back.departing.is_empty());
    }

    #[test]
    fn damaged_books_profiles_and_banners_are_refused_without_a_panic() {
        let me = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let cid = DeviceIdentity::generate().unwrap();
        let dan = DeviceIdentity::generate().unwrap();
        let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
        book.request_received(intro(&bea, "Bea", 1), 2);
        book.answer(&bea.public_key(), true, 3);
        book.request_received(intro(&cid, "Cid", 1), 4);
        book.request_sent(dan.public_key(), "Dan", 5).unwrap();
        book.remove(&bea.public_key(), 6).unwrap();
        book.banner =
            Some(Banner::sign(&me, 0xFF3D21AD, Some(b"\x89PNG\r\n\x1a\n".to_vec()), 7).unwrap());
        for bytes in damaged(&book.encode(), 1) {
            let _ = FriendBook::decode(&bytes);
        }
        for bytes in damaged(&book.profile.encode(), 2) {
            let _ = Profile::decode(&bytes);
        }
        for bytes in damaged(&book.banner.as_ref().unwrap().encode(), 3) {
            let _ = Banner::decode(&bytes);
        }
    }
}
