//! Log entries and invite tickets: what they contain, how they are signed and encoded.

use fogata_crypto::{
    CryptoError, DeviceIdentity, DevicePublicKey, GroupId, GroupSecret, SIGNATURE_LEN, Sealed,
    derive_secret, random_bytes,
};
use sha2::{Digest, Sha256};

use fogata_crypto::codec::{DecodeError, Reader, Writer};

/// Hash of an encoded entry; entries refer to their parents by it.
pub type EntryHash = [u8; 32];

/// Longest encoded entry accepted. A rotation seals the secret to every member (about 140
/// bytes each), so this leaves room for hundreds of members; anything longer is not a
/// group change but a way to fill everyone's disk.
pub const MAX_ENTRY_BYTES: usize = 64 * 1024;

/// Random identifier of a key epoch, chosen by whoever starts it.
pub type EpochId = [u8; 16];

const ENTRY_LABEL: &[u8] = b"fogata/v1/entry";
const TICKET_LABEL: &[u8] = b"fogata/v1/ticket";
const SECRET_LABEL: &[u8] = b"fogata/v1/epoch-secret";
const COMMIT_LABEL: &[u8] = b"fogata/v1/epoch-commitment";
const INVITE_LABEL: &[u8] = b"fogata/v1/invite";
const PAYLOAD_LABEL: &[u8] = b"fogata/v3/mls-payload";

/// Longest MLS KeyPackage an entry carries. One is about 370 bytes.
pub const MAX_KEY_PACKAGE_BYTES: usize = 2048;

/// Length of the ciphertext of a box holding one epoch secret (32 bytes and the
/// authentication tag): what every box in a `Create`, `Add`, `Welcome`, `Remove` and
/// `Rotate` holds.
pub const SEALED_SECRET_LEN: usize = 32 + 16;

/// The first kind of entry for MLS groups (see [`Body::MlsKeyPackage`] and after): versions
/// before them read these as [`Body::Unknown`].
pub const FIRST_MLS_KIND: u8 = 20;

/// The kind of [`Body::SetManagers`].
const SET_MANAGERS_KIND: u8 = 24;

/// Digest of an MLS message kept beside the log (a Commit, a Welcome): the entry that
/// carries it signs this, not the bytes, which can go once nobody needs them.
pub type PayloadDigest = [u8; 32];

/// The digest an entry names for `payload`.
pub fn payload_digest(payload: &[u8]) -> PayloadDigest {
    let mut h = Sha256::new();
    h.update(PAYLOAD_LABEL);
    h.update(payload);
    h.finalize().into()
}

/// Random token of an invite; the device that uses the invite and the managers of the group
/// meet on the DHT under a rendezvous secret derived from it, never under the group's own.
pub type InviteToken = [u8; 16];

/// The rendezvous secret of an invite, from its token.
pub fn invite_rendezvous(token: &InviteToken) -> [u8; 32] {
    derive_secret(INVITE_LABEL, token, b"rendezvous")
}

/// Short numeric form of an epoch id, used by the key schedule and in message headers.
pub fn epoch_number(id: &EpochId) -> u32 {
    u32::from_be_bytes([id[0], id[1], id[2], id[3]])
}

/// Commitment to an epoch secret, published in the log so that every recipient can check
/// that the secret it was given is the real one.
pub fn commitment(id: &EpochId, secret: &GroupSecret) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(COMMIT_LABEL);
    h.update(id);
    h.update(secret.as_bytes());
    h.finalize().into()
}

/// What a sealed epoch secret is bound to.
pub(crate) fn secret_context(group_id: &GroupId, epoch: &EpochId) -> Vec<u8> {
    let mut c = SECRET_LABEL.to_vec();
    c.extend_from_slice(group_id.as_bytes());
    c.extend_from_slice(epoch);
    c
}

/// The start of a key epoch: a fresh secret sealed to every member who should have it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rotation {
    pub epoch: EpochId,
    pub commitment: [u8; 32],
    pub sealed: Vec<(DevicePublicKey, Sealed)>,
}

impl Rotation {
    /// A new epoch for `members`, returning the rotation and its secret.
    pub fn new(
        group_id: &GroupId,
        members: impl IntoIterator<Item = DevicePublicKey>,
    ) -> Result<(Self, GroupSecret), CryptoError> {
        let mut epoch = [0u8; 16];
        random_bytes(&mut epoch)?;
        let secret = GroupSecret::generate()?;
        let context = secret_context(group_id, &epoch);
        let mut sealed = Vec::new();
        for member in members {
            sealed.push((member, Sealed::seal(&member, &context, secret.as_bytes())?));
        }
        sealed.sort_by_key(|(k, _)| *k);
        sealed.dedup_by_key(|(k, _)| *k);
        Ok((
            Self {
                epoch,
                commitment: commitment(&epoch, &secret),
                sealed,
            },
            secret,
        ))
    }

    fn write(&self, w: &mut Writer) {
        w.raw(&self.epoch).raw(&self.commitment);
        w.u32(self.sealed.len() as u32);
        for (member, sealed) in &self.sealed {
            w.key(member).sealed(sealed);
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let epoch = r.array()?;
        let commitment = r.array()?;
        let n = r.count(32 + 32 + 24 + 4)?;
        let mut sealed = Vec::with_capacity(n);
        for _ in 0..n {
            sealed.push((r.key()?, r.sealed()?));
        }
        Ok(Self {
            epoch,
            commitment,
            sealed,
        })
    }
}

/// Permission for one device to join, issued by a manager and carried in an invite link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub group_id: GroupId,
    pub ticket_id: [u8; 16],
    pub issuer: DevicePublicKey,
    pub expires_ms: u64,
    pub signature: [u8; SIGNATURE_LEN],
}

impl Ticket {
    pub fn issue(
        issuer: &DeviceIdentity,
        group_id: GroupId,
        expires_ms: u64,
    ) -> Result<Self, CryptoError> {
        let mut ticket_id = [0u8; 16];
        random_bytes(&mut ticket_id)?;
        let mut ticket = Self {
            group_id,
            ticket_id,
            issuer: issuer.public_key(),
            expires_ms,
            signature: [0u8; SIGNATURE_LEN],
        };
        ticket.signature = issuer.sign(&ticket.signed_bytes());
        Ok(ticket)
    }

    fn signed_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.raw(TICKET_LABEL)
            .group(&self.group_id)
            .raw(&self.ticket_id)
            .key(&self.issuer)
            .u64(self.expires_ms);
        w.into_bytes()
    }

    /// Whether the issuer really signed this ticket (not whether it may still be used).
    pub fn signature_valid(&self) -> bool {
        self.issuer
            .verify(&self.signed_bytes(), &self.signature)
            .is_ok()
    }

    /// Whether `entry` is this ticket's issuer cancelling it (`Revoke`), signed. The device
    /// holding the ticket knows no other member, so this is the only refusal it can check:
    /// anyone else could say no in the group's name.
    pub fn revoked_by(&self, entry: &Entry) -> bool {
        entry.group_id == self.group_id
            && entry.author == self.issuer
            && matches!(&entry.body, Body::Revoke { ticket_id } if *ticket_id == self.ticket_id)
            && entry.signature_valid()
    }

    fn write(&self, w: &mut Writer) {
        w.group(&self.group_id)
            .raw(&self.ticket_id)
            .key(&self.issuer)
            .u64(self.expires_ms)
            .raw(&self.signature);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            group_id: r.group()?,
            ticket_id: r.array()?,
            issuer: r.key()?,
            expires_ms: r.u64()?,
            signature: r.array()?,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        self.write(&mut w);
        w.into_bytes()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        let t = Self::read(&mut r)?;
        r.finish()?;
        Ok(t)
    }
}

/// The devices an MLS Commit adds to the group's tree and removes from it, as the log reads
/// it; each list sorted, without duplicates. A Commit that adds and removes nobody updates
/// its author's own keys. The members check that the Commit itself does just this.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub add: Vec<DevicePublicKey>,
    pub remove: Vec<DevicePublicKey>,
}

impl Changes {
    /// The changes with each list sorted and without duplicates.
    pub fn new(
        add: impl IntoIterator<Item = DevicePublicKey>,
        remove: impl IntoIterator<Item = DevicePublicKey>,
    ) -> Self {
        let tidy = |keys: &mut Vec<DevicePublicKey>| {
            keys.sort();
            keys.dedup();
        };
        let mut changes = Self {
            add: add.into_iter().collect(),
            remove: remove.into_iter().collect(),
        };
        tidy(&mut changes.add);
        tidy(&mut changes.remove);
        changes
    }

    /// Whether it only updates its author's keys.
    pub fn is_update(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }

    fn write(&self, w: &mut Writer) {
        for keys in [&self.add, &self.remove] {
            w.u32(keys.len() as u32);
            for key in keys {
                w.key(key);
            }
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let mut read_keys = || -> Result<Vec<DevicePublicKey>, DecodeError> {
            let n = r.count(32)?;
            let mut keys = Vec::with_capacity(n);
            for _ in 0..n {
                keys.push(r.key()?);
            }
            if keys.windows(2).any(|k| k[0] >= k[1]) {
                return Err(DecodeError);
            }
            Ok(keys)
        };
        Ok(Self {
            add: read_keys()?,
            remove: read_keys()?,
        })
    }
}

/// Where an MLS Commit stands in the group's chain of epochs: the epoch it was made on, and
/// the `Commit` or `Resync` entry that started that epoch (`None` for the first Commit, made
/// on the group the creator started alone, epoch 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EpochLink {
    pub epoch: u64,
    pub prev: Option<EntryHash>,
}

impl EpochLink {
    fn write(&self, w: &mut Writer) {
        w.u64(self.epoch);
        match &self.prev {
            Some(prev) => w.u8(1).raw(prev),
            None => w.u8(0),
        };
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            epoch: r.u64()?,
            prev: match r.u8()? {
                0 => None,
                1 => Some(r.array()?),
                _ => return Err(DecodeError),
            },
        })
    }
}

/// A voice room's id, fixed when it is opened.
pub type VoiceRoomId = [u8; 8];

/// The group's first voice room, which every group has and keeps.
pub const FIRST_ROOM: VoiceRoomId = [0; 8];

/// The change an entry makes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    /// Root of the log. The author is the first and only member on the ladder.
    Create {
        name: String,
        start: Rotation,
    },
    Rename {
        name: String,
    },
    /// Move a member below the author on the ladder up to just below the author.
    Promote {
        member: DevicePublicKey,
    },
    /// Move a member below the author on the ladder down to the end.
    Demote {
        member: DevicePublicKey,
    },
    /// Who manages the group: everyone (`None`), or the first `n` on the ladder, the author
    /// among them. Managers only.
    SetManagers {
        first: Option<u8>,
    },
    /// Add a device directly, giving it the current epoch secret. Managers only.
    Add {
        member: DevicePublicKey,
        name: String,
        epoch: EpochId,
        secret: Sealed,
    },
    /// Join with an invite ticket. Written by the joining device.
    Join {
        ticket: Ticket,
        name: String,
    },
    /// Give a member an epoch secret it lacks. Managers only.
    Welcome {
        member: DevicePublicKey,
        epoch: EpochId,
        secret: Sealed,
    },
    /// Remove a device below the author on the ladder and start a new epoch without it.
    Remove {
        member: DevicePublicKey,
        rotation: Rotation,
    },
    /// The author leaves the group.
    Leave,
    /// Start a new epoch with the same members.
    Rotate {
        rotation: Rotation,
    },
    /// Choose the group's icon, one of the preset icons the application draws. Managers only.
    SetIcon {
        icon: u8,
    },
    /// An invite is open: managers announce themselves under its token until it is used or
    /// expires, so the invited device finds one of them. Written by the issuing manager.
    Invite {
        ticket: Ticket,
        token: InviteToken,
    },
    /// Choose the group's picture, a small PNG or JPEG shown instead of the icon, or none
    /// to show the icon again. Managers only.
    SetPicture {
        picture: Option<Vec<u8>>,
    },
    /// The most people in the group, and in each of its voice rooms at once. Managers only.
    SetLimits {
        members: u8,
        room: u8,
    },
    /// Open a voice room, or rename one. Managers only.
    SetRoom {
        id: VoiceRoomId,
        name: String,
    },
    /// Close a voice room; the first one stays. Managers only.
    RemoveRoom {
        id: VoiceRoomId,
    },
    /// Cancel an invite before it is used: nobody can join with it any more. Managers only.
    Revoke {
        ticket_id: [u8; 16],
    },
    /// Who may enter a voice room: everyone in the group (`None`), or only these members,
    /// the author and those above it. Managers only. A room rule (see
    /// [`Body::is_room_rule`]).
    RoomAccess {
        room: VoiceRoomId,
        members: Option<Vec<DevicePublicKey>>,
    },
    /// Keep a member out of a voice room until `until_ms` (by the author's clock), or until
    /// someone above it lets it back (`None`). Only of a member below the author on the
    /// ladder. A room rule.
    KeepOut {
        room: VoiceRoomId,
        member: DevicePublicKey,
        until_ms: Option<u64>,
    },
    /// End a member's time out of a voice room before it runs out. A room rule, with the
    /// rights of [`Body::KeepOut`].
    LetBack {
        room: VoiceRoomId,
        member: DevicePublicKey,
    },
    /// An MLS KeyPackage of the author for this group: what a manager adds it to the group's
    /// tree with, when it lost its place in it or the group moves to MLS. For MLS groups only.
    MlsKeyPackage {
        key_package: Vec<u8>,
    },
    /// [`Body::Join`] with the joining device's MLS KeyPackage, for MLS groups.
    MlsJoin {
        ticket: Ticket,
        name: String,
        key_package: Vec<u8>,
    },
    /// An MLS Commit, which starts epoch `link.epoch + 1`: what it adds and removes, the
    /// digest of the Commit itself, and, when it adds devices, of the Welcome they join
    /// with. Both messages are kept beside the log. For MLS groups only.
    Commit {
        link: EpochLink,
        changes: Changes,
        payload: PayloadDigest,
        welcome: Option<PayloadDigest>,
    },
    /// An MLS external Commit by a member that lost its MLS state: it takes a new place in
    /// the tree and drops its old one, starting epoch `link.epoch + 1`. For MLS groups only.
    Resync {
        link: EpochLink,
        payload: PayloadDigest,
    },
    /// A kind of entry this version does not know, written by a newer one: its body as it
    /// came. It is kept and passed on like any other entry, so the entries written on top of
    /// it still apply, but it changes nothing here.
    Unknown {
        kind: u8,
        bytes: Vec<u8>,
    },
}

impl Body {
    /// Whether only a manager, or a member above another one, may write this: what a
    /// demotion of its author outranks (see [`crate::GroupLog::state`]).
    pub fn needs_rank(&self) -> bool {
        matches!(
            self,
            Body::Rename { .. }
                | Body::SetIcon { .. }
                | Body::Promote { .. }
                | Body::Demote { .. }
                | Body::SetManagers { .. }
                | Body::Add { .. }
                | Body::Remove { .. }
                | Body::Invite { .. }
                | Body::SetPicture { .. }
                | Body::SetLimits { .. }
                | Body::SetRoom { .. }
                | Body::RemoveRoom { .. }
                | Body::Revoke { .. }
                | Body::Welcome { .. }
                | Body::RoomAccess { .. }
                | Body::KeepOut { .. }
                | Body::LetBack { .. }
        )
    }

    /// Whether every box this carries holds one epoch secret, sealed to a distinct device:
    /// what every version writes. A box of another length, or two to one device, only
    /// makes the entry longer.
    pub fn boxes_hold_one_secret(&self) -> bool {
        let rotation_fits = |rotation: &Rotation| {
            rotation
                .sealed
                .iter()
                .all(|(_, s)| s.ciphertext.len() == SEALED_SECRET_LEN)
                && rotation.sealed.windows(2).all(|w| w[0].0 < w[1].0)
        };
        match self {
            Body::Create {
                start: rotation, ..
            }
            | Body::Remove { rotation, .. }
            | Body::Rotate { rotation } => rotation_fits(rotation),
            Body::Add { secret, .. } | Body::Welcome { secret, .. } => {
                secret.ciphertext.len() == SEALED_SECRET_LEN
            }
            _ => true,
        }
    }

    /// Whether this is a room rule: who may enter the voice rooms. Rules apply on top of
    /// what every other entry makes of the group (see [`crate::GroupLog::state`]).
    pub fn is_room_rule(&self) -> bool {
        matches!(
            self,
            Body::RoomAccess { .. } | Body::KeepOut { .. } | Body::LetBack { .. }
        )
    }

    /// Whether this is one of the kinds for MLS groups.
    pub fn is_mls(&self) -> bool {
        matches!(
            self,
            Body::MlsKeyPackage { .. }
                | Body::MlsJoin { .. }
                | Body::Commit { .. }
                | Body::Resync { .. }
        )
    }

    pub(crate) fn write(&self, w: &mut Writer) {
        match self {
            Body::Create { name, start } => {
                w.u8(1).str(name);
                start.write(w);
            }
            Body::Rename { name } => {
                w.u8(2).str(name);
            }
            // One kind for both, with a flag that says which.
            Body::Promote { member } => {
                w.u8(3).key(member).u8(1);
            }
            Body::Demote { member } => {
                w.u8(3).key(member).u8(0);
            }
            Body::Add {
                member,
                name,
                epoch,
                secret,
            } => {
                w.u8(4).key(member).str(name).raw(epoch).sealed(secret);
            }
            Body::Join { ticket, name } => {
                w.u8(5);
                ticket.write(w);
                w.str(name);
            }
            Body::Welcome {
                member,
                epoch,
                secret,
            } => {
                w.u8(6).key(member).raw(epoch).sealed(secret);
            }
            Body::Remove { member, rotation } => {
                w.u8(7).key(member);
                rotation.write(w);
            }
            Body::Leave => {
                w.u8(8);
            }
            Body::Rotate { rotation } => {
                w.u8(9);
                rotation.write(w);
            }
            Body::SetIcon { icon } => {
                w.u8(10).u8(*icon);
            }
            Body::Invite { ticket, token } => {
                w.u8(11);
                ticket.write(w);
                w.raw(token);
            }
            Body::SetPicture { picture } => {
                w.u8(12);
                match picture {
                    Some(picture) => w.u8(1).bytes(picture),
                    None => w.u8(0),
                };
            }
            Body::SetLimits { members, room } => {
                w.u8(13).u8(*members).u8(*room);
            }
            Body::SetRoom { id, name } => {
                w.u8(14).raw(id).str(name);
            }
            Body::RemoveRoom { id } => {
                w.u8(15).raw(id);
            }
            Body::Revoke { ticket_id } => {
                w.u8(16).raw(ticket_id);
            }
            Body::RoomAccess { room, members } => {
                w.u8(17).raw(room);
                match members {
                    Some(members) => {
                        w.u8(1).u32(members.len() as u32);
                        for member in members {
                            w.key(member);
                        }
                    }
                    None => {
                        w.u8(0);
                    }
                }
            }
            Body::KeepOut {
                room,
                member,
                until_ms,
            } => {
                w.u8(18).raw(room).key(member);
                match until_ms {
                    Some(until) => w.u8(1).u64(*until),
                    None => w.u8(0),
                };
            }
            Body::LetBack { room, member } => {
                w.u8(19).raw(room).key(member);
            }
            Body::MlsKeyPackage { key_package } => {
                w.u8(20).bytes(key_package);
            }
            Body::MlsJoin {
                ticket,
                name,
                key_package,
            } => {
                w.u8(21);
                ticket.write(w);
                w.str(name).bytes(key_package);
            }
            Body::Commit {
                link,
                changes,
                payload,
                welcome,
            } => {
                w.u8(22);
                link.write(w);
                changes.write(w);
                w.raw(payload);
                match welcome {
                    Some(welcome) => w.u8(1).raw(welcome),
                    None => w.u8(0),
                };
            }
            Body::Resync { link, payload } => {
                w.u8(23);
                link.write(w);
                w.raw(payload);
            }
            Body::SetManagers { first } => {
                w.u8(SET_MANAGERS_KIND);
                match first {
                    Some(n) => w.u8(1).u8(*n),
                    None => w.u8(0),
                };
            }
            Body::Unknown { kind, bytes } => {
                w.u8(*kind).raw(bytes);
            }
        }
    }

    /// Reads the body of a kind newer than the room rules (the MLS kinds and
    /// [`Body::SetManagers`]) from `bytes`, all of it. Only the one encoding this version
    /// writes is taken: what else comes under these kinds (damaged, or from a newer version)
    /// is kept as [`Body::Unknown`], as versions before them keep all of it. So every version
    /// keeps the same entries, and none waits for ever on one it refused.
    fn read_newer(kind: u8, bytes: &[u8]) -> Option<Self> {
        let mut r = Reader::new(bytes);
        let key_package = |r: &mut Reader<'_>| -> Result<Vec<u8>, DecodeError> {
            let bytes = r.bytes()?;
            if bytes.len() > MAX_KEY_PACKAGE_BYTES {
                return Err(DecodeError);
            }
            Ok(bytes.to_vec())
        };
        let body = match kind {
            20 => Body::MlsKeyPackage {
                key_package: key_package(&mut r).ok()?,
            },
            21 => Body::MlsJoin {
                ticket: Ticket::read(&mut r).ok()?,
                name: r.str().ok()?,
                key_package: key_package(&mut r).ok()?,
            },
            22 => Body::Commit {
                link: EpochLink::read(&mut r).ok()?,
                changes: Changes::read(&mut r).ok()?,
                payload: r.array().ok()?,
                welcome: match r.u8().ok()? {
                    0 => None,
                    1 => Some(r.array().ok()?),
                    _ => return None,
                },
            },
            23 => Body::Resync {
                link: EpochLink::read(&mut r).ok()?,
                payload: r.array().ok()?,
            },
            SET_MANAGERS_KIND => Body::SetManagers {
                first: match r.u8().ok()? {
                    0 => None,
                    1 => Some(r.u8().ok()?),
                    _ => return None,
                },
            },
            _ => return None,
        };
        r.finish().ok()?;
        // Lists out of order or repeated, say: another encoding of the same body.
        let mut w = Writer::default();
        body.write(&mut w);
        (w.into_bytes()[1..] == *bytes).then_some(body)
    }

    /// Reads a body that runs up to the entry's signature, the last thing in an entry: a
    /// kind this version does not know keeps those bytes whole, so the entry's hash and
    /// signature stay the same when it is passed on.
    fn read(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(match r.u8()? {
            1 => Body::Create {
                name: r.str()?,
                start: Rotation::read(r)?,
            },
            2 => Body::Rename { name: r.str()? },
            3 => {
                let member = r.key()?;
                match r.u8()? {
                    0 => Body::Demote { member },
                    1 => Body::Promote { member },
                    _ => return Err(DecodeError),
                }
            }
            4 => Body::Add {
                member: r.key()?,
                name: r.str()?,
                epoch: r.array()?,
                secret: r.sealed()?,
            },
            5 => Body::Join {
                ticket: Ticket::read(r)?,
                name: r.str()?,
            },
            6 => Body::Welcome {
                member: r.key()?,
                epoch: r.array()?,
                secret: r.sealed()?,
            },
            7 => Body::Remove {
                member: r.key()?,
                rotation: Rotation::read(r)?,
            },
            8 => Body::Leave,
            9 => Body::Rotate {
                rotation: Rotation::read(r)?,
            },
            10 => Body::SetIcon { icon: r.u8()? },
            11 => Body::Invite {
                ticket: Ticket::read(r)?,
                token: r.array()?,
            },
            12 => Body::SetPicture {
                picture: match r.u8()? {
                    0 => None,
                    _ => Some(r.bytes()?.to_vec()),
                },
            },
            13 => Body::SetLimits {
                members: r.u8()?,
                room: r.u8()?,
            },
            14 => Body::SetRoom {
                id: r.array()?,
                name: r.str()?,
            },
            15 => Body::RemoveRoom { id: r.array()? },
            16 => Body::Revoke {
                ticket_id: r.array()?,
            },
            17 => Body::RoomAccess {
                room: r.array()?,
                members: match r.u8()? {
                    0 => None,
                    _ => {
                        let n = r.count(32)?;
                        let mut members = Vec::with_capacity(n);
                        for _ in 0..n {
                            members.push(r.key()?);
                        }
                        Some(members)
                    }
                },
            },
            18 => Body::KeepOut {
                room: r.array()?,
                member: r.key()?,
                until_ms: match r.u8()? {
                    0 => None,
                    _ => Some(r.u64()?),
                },
            },
            19 => Body::LetBack {
                room: r.array()?,
                member: r.key()?,
            },
            kind => {
                let len = r
                    .remaining()
                    .checked_sub(SIGNATURE_LEN)
                    .ok_or(DecodeError)?;
                let bytes = r.take(len)?;
                Body::read_newer(kind, bytes).unwrap_or_else(|| Body::Unknown {
                    kind,
                    bytes: bytes.to_vec(),
                })
            }
        })
    }
}

/// One signed change to the group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub group_id: GroupId,
    /// Sorted, without duplicates.
    pub parents: Vec<EntryHash>,
    pub author: DevicePublicKey,
    pub time_ms: u64,
    pub body: Body,
    pub signature: [u8; SIGNATURE_LEN],
}

impl Entry {
    /// Builds and signs an entry.
    pub fn sign(
        identity: &DeviceIdentity,
        group_id: GroupId,
        mut parents: Vec<EntryHash>,
        time_ms: u64,
        body: Body,
    ) -> Self {
        parents.sort_unstable();
        parents.dedup();
        let mut entry = Self {
            group_id,
            parents,
            author: identity.public_key(),
            time_ms,
            body,
            signature: [0u8; SIGNATURE_LEN],
        };
        entry.signature = identity.sign(&entry.signed_bytes());
        entry
    }

    fn write_unsigned(&self, w: &mut Writer) {
        w.group(&self.group_id).u32(self.parents.len() as u32);
        for p in &self.parents {
            w.raw(p);
        }
        w.key(&self.author).u64(self.time_ms);
        self.body.write(w);
    }

    fn signed_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.raw(ENTRY_LABEL);
        self.write_unsigned(&mut w);
        w.into_bytes()
    }

    pub fn signature_valid(&self) -> bool {
        let sorted = self.parents.windows(2).all(|p| p[0] < p[1]);
        sorted
            && self
                .author
                .verify(&self.signed_bytes(), &self.signature)
                .is_ok()
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        self.write_unsigned(&mut w);
        w.raw(&self.signature);
        w.into_bytes()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() > MAX_ENTRY_BYTES {
            return Err(DecodeError);
        }
        let mut r = Reader::new(bytes);
        let group_id = r.group()?;
        let n = r.count(32)?;
        let mut parents = Vec::with_capacity(n);
        for _ in 0..n {
            parents.push(r.array()?);
        }
        let entry = Self {
            group_id,
            parents,
            author: r.key()?,
            time_ms: r.u64()?,
            body: Body::read(&mut r)?,
            signature: r.array()?,
        };
        r.finish()?;
        Ok(entry)
    }

    pub fn hash(&self) -> EntryHash {
        Sha256::digest(self.encode()).into()
    }
}

#[cfg(test)]
pub(crate) mod tests {
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
        (0..2000)
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

    #[test]
    fn entries_round_trip_and_verify() {
        let me = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        let (start, _) = Rotation::new(&group, [me.public_key()]).unwrap();
        let entry = Entry::sign(
            &me,
            group,
            vec![],
            5,
            Body::Create {
                name: "friends".into(),
                start,
            },
        );
        assert!(entry.signature_valid());
        let back = Entry::decode(&entry.encode()).unwrap();
        assert_eq!(back, entry);
        assert_eq!(back.hash(), entry.hash());
    }

    #[test]
    fn the_newer_bodies_round_trip() {
        let me = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        for body in [
            Body::SetPicture {
                picture: Some(vec![7; 300]),
            },
            Body::SetPicture { picture: None },
            Body::SetLimits {
                members: 12,
                room: 6,
            },
            Body::SetRoom {
                id: [3; 8],
                name: "Music".into(),
            },
            Body::RemoveRoom { id: [3; 8] },
            Body::Revoke { ticket_id: [9; 16] },
            Body::RoomAccess {
                room: [3; 8],
                members: Some(vec![me.public_key()]),
            },
            Body::RoomAccess {
                room: [3; 8],
                members: None,
            },
            Body::KeepOut {
                room: [3; 8],
                member: me.public_key(),
                until_ms: Some(99),
            },
            Body::KeepOut {
                room: [3; 8],
                member: me.public_key(),
                until_ms: None,
            },
            Body::LetBack {
                room: [3; 8],
                member: me.public_key(),
            },
            Body::Promote {
                member: me.public_key(),
            },
            Body::Demote {
                member: me.public_key(),
            },
            Body::SetManagers { first: None },
            Body::SetManagers { first: Some(3) },
        ] {
            let entry = Entry::sign(&me, group, vec![], 5, body);
            assert_eq!(Entry::decode(&entry.encode()).unwrap(), entry);
        }
    }

    #[test]
    fn moves_on_the_ladder_keep_the_kind_and_flag_of_their_first_encoding() {
        let me = DeviceIdentity::generate().unwrap();
        let member = DeviceIdentity::generate().unwrap().public_key();
        let mut w = Writer::default();
        Body::Promote { member }.write(&mut w);
        let mut expected = vec![3];
        expected.extend_from_slice(member.as_bytes());
        expected.push(1);
        assert_eq!(w.into_bytes(), expected);
        let mut w = Writer::default();
        Body::Demote { member }.write(&mut w);
        *expected.last_mut().unwrap() = 0;
        assert_eq!(w.into_bytes(), expected);
        // Any other flag reads as nothing.
        let group = GroupId::random().unwrap();
        let entry = Entry::sign(&me, group, vec![], 5, Body::Promote { member });
        let mut bytes = entry.encode();
        let flag = bytes.len() - SIGNATURE_LEN - 1;
        bytes[flag] = 2;
        assert_eq!(Entry::decode(&bytes), Err(DecodeError));
        // A managers setting written otherwise is kept whole, as a kind not known.
        assert_eq!(Body::read_newer(SET_MANAGERS_KIND, &[2]), None);
        assert_eq!(Body::read_newer(SET_MANAGERS_KIND, &[1, 3, 0]), None);
        assert_eq!(
            Body::read_newer(SET_MANAGERS_KIND, &[1, 3]),
            Some(Body::SetManagers { first: Some(3) })
        );
    }

    #[test]
    fn tampering_breaks_the_signature() {
        let me = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        let mut entry = Entry::sign(
            &me,
            group,
            vec![[1; 32]],
            5,
            Body::Rename { name: "a".into() },
        );
        entry.body = Body::Rename { name: "b".into() };
        assert!(!entry.signature_valid());
    }

    #[test]
    fn tickets_round_trip_and_verify() {
        let admin = DeviceIdentity::generate().unwrap();
        let ticket = Ticket::issue(&admin, GroupId::random().unwrap(), 99).unwrap();
        assert!(ticket.signature_valid());
        let back = Ticket::decode(&ticket.encode()).unwrap();
        assert_eq!(back, ticket);
        let mut forged = back;
        forged.expires_ms = 1000;
        assert!(!forged.signature_valid());
    }

    #[test]
    fn oversized_entries_do_not_decode() {
        let me = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        let entry = Entry::sign(
            &me,
            group,
            vec![],
            5,
            Body::Rename {
                name: "x".repeat(MAX_ENTRY_BYTES),
            },
        );
        assert_eq!(Entry::decode(&entry.encode()), Err(DecodeError));
        let fits = Entry::sign(
            &me,
            group,
            vec![],
            5,
            Body::Rename {
                name: "x".repeat(1000),
            },
        );
        assert!(Entry::decode(&fits.encode()).is_ok());
    }

    #[test]
    fn garbage_does_not_decode() {
        assert_eq!(Entry::decode(&[1, 2, 3]), Err(DecodeError));
        let mut huge_count = vec![0u8; 16];
        huge_count.extend_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(Entry::decode(&huge_count), Err(DecodeError));
    }

    #[test]
    fn damaged_entries_and_tickets_are_refused_without_a_panic() {
        let me = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        let (start, _) = Rotation::new(&group, [me.public_key(), me.public_key()]).unwrap();
        let ticket = Ticket::issue(&me, group, 99).unwrap();
        let bodies = [
            Body::Create {
                name: "friends".into(),
                start: start.clone(),
            },
            Body::Rotate { rotation: start },
            Body::Join {
                ticket: ticket.clone(),
                name: "Leo".into(),
            },
            Body::Invite {
                ticket: ticket.clone(),
                token: [4; 16],
            },
            Body::SetPicture {
                picture: Some(vec![7; 300]),
            },
            Body::SetRoom {
                id: [3; 8],
                name: "Music".into(),
            },
            Body::RoomAccess {
                room: [3; 8],
                members: Some(vec![me.public_key(), me.public_key()]),
            },
            Body::KeepOut {
                room: [3; 8],
                member: me.public_key(),
                until_ms: Some(99),
            },
        ];
        for (i, body) in bodies.into_iter().enumerate() {
            let entry = Entry::sign(&me, group, vec![[1; 32], [2; 32]], 5, body);
            for bytes in damaged(&entry.encode(), i as u64 + 1) {
                let _ = Entry::decode(&bytes);
            }
        }
        for bytes in damaged(&ticket.encode(), 99) {
            let _ = Ticket::decode(&bytes);
        }
    }

    /// One entry of each kind for MLS groups.
    pub(crate) fn mls_bodies(me: &DeviceIdentity, group: GroupId) -> Vec<Body> {
        let other = DeviceIdentity::generate().unwrap().public_key();
        vec![
            Body::MlsKeyPackage {
                key_package: vec![1; 369],
            },
            Body::MlsJoin {
                ticket: Ticket::issue(me, group, 99).unwrap(),
                name: "Leo".into(),
                key_package: vec![2; 369],
            },
            Body::Commit {
                link: EpochLink {
                    epoch: 0,
                    prev: None,
                },
                changes: Changes::new([other, me.public_key()], []),
                payload: payload_digest(b"commit"),
                welcome: Some(payload_digest(b"welcome")),
            },
            Body::Commit {
                link: EpochLink {
                    epoch: 7,
                    prev: Some([5; 32]),
                },
                changes: Changes::new([], [other]),
                payload: payload_digest(b"commit"),
                welcome: None,
            },
            Body::Commit {
                link: EpochLink {
                    epoch: 8,
                    prev: Some([6; 32]),
                },
                changes: Changes::default(),
                payload: payload_digest(b"update"),
                welcome: None,
            },
            Body::Resync {
                link: EpochLink {
                    epoch: 9,
                    prev: Some([7; 32]),
                },
                payload: payload_digest(b"external commit"),
            },
        ]
    }

    #[test]
    fn mls_entries_round_trip_and_say_so() {
        let me = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        for body in mls_bodies(&me, group) {
            assert!(body.is_mls());
            let entry = Entry::sign(&me, group, vec![[1; 32]], 5, body);
            let back = Entry::decode(&entry.encode()).unwrap();
            assert_eq!(back, entry);
            assert!(back.signature_valid());
        }
        assert!(!Body::Leave.is_mls());
    }

    #[test]
    fn mls_entries_written_otherwise_are_kept_as_they_came() {
        let me = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        let (a, b) = (
            DeviceIdentity::generate().unwrap().public_key(),
            DeviceIdentity::generate().unwrap().public_key(),
        );
        let (low, high) = (a.min(b), a.max(b));
        // A body under an MLS kind as this version would not write it: lists out of order
        // or repeated, a flag that is neither 0 nor 1, a byte too many, a KeyPackage too
        // long. Each is kept whole as a kind it does not know, as older versions keep it.
        let commit = |add: &[DevicePublicKey], flag: u8, extra: &[u8]| {
            let mut w = Writer::default();
            w.u64(3).u8(1).raw(&[4; 32]).u32(add.len() as u32);
            for key in add {
                w.key(key);
            }
            w.u32(0).raw(&[5; 32]).u8(flag);
            if flag == 1 {
                w.raw(&[6; 32]);
            }
            w.raw(extra);
            w.into_bytes()
        };
        let mut too_long = Writer::default();
        too_long.bytes(&vec![0; MAX_KEY_PACKAGE_BYTES + 1]);
        let odd = [
            (22, commit(&[high, low], 0, &[])),
            (22, commit(&[low, low], 0, &[])),
            (22, commit(&[low], 2, &[])),
            (22, commit(&[low], 1, &[0])),
            (20, too_long.into_bytes()),
            (23, vec![1, 2, 3]),
        ];
        // And the encodings this version writes do read as what they are.
        assert!(Body::read_newer(22, &commit(&[low, high], 0, &[])).is_some());
        assert!(Body::read_newer(22, &commit(&[low], 1, &[])).is_some());
        for (kind, bytes) in odd {
            assert_eq!(Body::read_newer(kind, &bytes), None);
            let entry = Entry::sign(
                &me,
                group,
                vec![],
                5,
                Body::Unknown {
                    kind,
                    bytes: bytes.clone(),
                },
            );
            let back = Entry::decode(&entry.encode()).unwrap();
            assert_eq!(back.body, Body::Unknown { kind, bytes });
            assert_eq!(back.hash(), entry.hash());
            assert!(back.signature_valid());
        }
    }

    #[test]
    fn damaged_mls_entries_are_refused_or_kept_whole_without_a_panic() {
        let me = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        for (i, body) in mls_bodies(&me, group).into_iter().enumerate() {
            let entry = Entry::sign(&me, group, vec![[1; 32]], 5, body);
            for bytes in damaged(&entry.encode(), 100 + i as u64) {
                if let Ok(entry) = Entry::decode(&bytes) {
                    assert_eq!(entry.encode(), bytes);
                }
            }
        }
    }

    #[test]
    fn a_newer_kind_reads_whole_and_signs_the_same() {
        let me = DeviceIdentity::generate().unwrap();
        let group = GroupId::random().unwrap();
        for bytes in [vec![], vec![1, 2, 3], vec![9; 500]] {
            let entry = Entry::sign(
                &me,
                group,
                vec![[1; 32]],
                5,
                Body::Unknown {
                    kind: 200,
                    bytes: bytes.clone(),
                },
            );
            let back = Entry::decode(&entry.encode()).unwrap();
            assert_eq!(back, entry);
            assert!(back.signature_valid());
            assert_eq!(back.hash(), entry.hash());
        }
        // Too short to hold a signature after the kind.
        let entry = Entry::sign(&me, group, vec![], 5, Body::Leave);
        let mut bytes = entry.encode();
        let at = bytes.len() - SIGNATURE_LEN - 1;
        bytes[at] = 200;
        bytes.truncate(bytes.len() - 1);
        assert_eq!(Entry::decode(&bytes), Err(DecodeError));
        // A kind this version knows still has to read as that kind.
        let rename = Entry::sign(&me, group, vec![], 5, Body::Rename { name: "a".into() });
        let mut bytes = rename.encode();
        bytes.insert(bytes.len() - SIGNATURE_LEN, 0);
        assert_eq!(Entry::decode(&bytes), Err(DecodeError));
    }
}
