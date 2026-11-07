//! One device's MLS state in one group, and how it follows the group's chain of epochs.
//!
//! The chain itself is the log's to decide ([`fogata_state::chain`]); here a device makes
//! Commits, checks the ones of others against the entries that carry them, and moves its
//! state along the chain: on to each new link, back to a snapshot when a branch it was on
//! lost, or into the group from a Welcome. It never moves its state on a Commit of its own
//! until that Commit's entry is in the chain.

use fogata_crypto::{
    DeviceIdentity, DevicePublicKey, GroupId, GroupSecret,
    codec::{DecodeError, Reader, Writer, encode_secret},
};
use fogata_state::{Changes, EntryHash, PayloadDigest, payload_digest};
use openmls::prelude::{
    tls_codec::{Deserialize as _, Serialize as _},
    *,
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{
    identity::{MlsSigner, device_of},
    keys::{EXPORT_LABEL, EpochSecret, KeptSecrets},
    provider::CIPHERSUITE,
    storage::{Provider, decode_values, read_values},
};

/// The most snapshots kept, one per link applied: how far back a device can undo a branch
/// that lost without a resync.
pub const MAX_SNAPSHOTS: usize = 8;

/// How long a snapshot is kept. A snapshot holds the secrets of its epoch, so this is
/// also how long forward secrecy waits for them.
pub const SNAPSHOT_FOR_MS: u64 = 2 * 24 * 60 * 60 * 1000;

const FILE_VERSION: u8 = 1;

/// Something went wrong making or reading an MLS message.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MlsError {
    #[error("not in the group's MLS tree")]
    NotInGroup,
    #[error("{0}")]
    Refused(&'static str),
    #[error("MLS: {0}")]
    Mls(String),
    #[error("damaged MLS state")]
    Damaged,
}

fn mls(e: impl std::fmt::Debug) -> MlsError {
    MlsError::Mls(format!("{e:?}"))
}

impl From<DecodeError> for MlsError {
    fn from(_: DecodeError) -> Self {
        MlsError::Damaged
    }
}

fn create_config() -> MlsGroupCreateConfig {
    MlsGroupCreateConfig::builder()
        .ciphersuite(CIPHERSUITE)
        // A Welcome carries the tree: a joining device needs nothing else.
        .use_ratchet_tree_extension(true)
        // Commits go in the clear (they ride in the members-only log, and every member
        // checks them); no application messages go through MLS.
        .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
        .max_past_epochs(0)
        .build()
}

fn join_config() -> MlsGroupJoinConfig {
    create_config().join_config().clone()
}

/// A Commit this device made: what its entry names, and the messages kept beside the log.
#[derive(Debug)]
pub struct Proposed {
    pub changes: Changes,
    pub payload: Vec<u8>,
    pub welcome: Option<Vec<u8>>,
}

/// One link of the chain, as [`GroupMember::follow`] needs it: from its entry, and the
/// messages kept beside the log, if they arrived.
#[derive(Clone, Copy, Debug)]
pub struct ChainLink<'a> {
    pub entry: EntryHash,
    pub author: DevicePublicKey,
    /// The digest the entry names.
    pub digest: PayloadDigest,
    /// The changes of a `Commit`; `None` for a `Resync`.
    pub changes: Option<&'a Changes>,
    pub payload: Option<&'a [u8]>,
    pub welcome: Option<&'a [u8]>,
}

/// Where following the chain left this device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Followed {
    /// At the chain's last link.
    UpToDate,
    /// The message of this link has not arrived yet.
    Waiting(EntryHash),
    /// This link's Commit does not check, the same for every member of its epoch: the
    /// chain should be worked out again without it ([`fogata_state::Verdict::Invalid`]).
    Invalid(EntryHash, &'static str),
    /// This device's state does not fit the chain any more (a branch longer than its
    /// snapshots lost, its state is behind, or a Commit's secrets for it do not open): it
    /// should resync if it is in the tree, or wait for a manager to add it again.
    Lost,
    /// The tree does not hold this device (yet, or any more).
    NotInTree,
    /// Another state of this device's key wrote this link, a `Resync` or a `Commit` this
    /// state did not make: a copy of the device's data elsewhere, or an older backup put
    /// back. This state is out of the tree.
    Replaced(EntryHash),
}

/// Why a Commit was not taken.
enum Refusal {
    /// The same for every member of its epoch.
    Invalid(&'static str),
    /// This device's own trouble.
    Local,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Own {
    /// A Commit waiting in OpenMLS until its entry is in the chain.
    Pending(PayloadDigest),
    /// An external Commit (resync), already applied here.
    Merged(PayloadDigest),
}

/// The state just before link `before` was applied.
struct Snapshot {
    before: EntryHash,
    taken_ms: u64,
    values: Zeroizing<Vec<u8>>,
    own: Option<Own>,
}

/// One device's MLS state in one group.
pub struct GroupMember {
    group_id: GroupId,
    device: DevicePublicKey,
    signer: MlsSigner,
    credential: CredentialWithKey,
    provider: Provider,
    group: Option<MlsGroup>,
    /// The links applied here, in order, since this device got its group: the first is
    /// the one it joined with; none while it holds the group it started at epoch 0.
    applied: Vec<EntryHash>,
    own: Option<Own>,
    snapshots: Vec<Snapshot>,
    secrets: KeptSecrets,
}

impl std::fmt::Debug for GroupMember {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GroupMember")
            .field("group_id", &self.group_id)
            .field("device", &self.device)
            .field("epoch", &self.epoch())
            .field("applied", &self.applied.len())
            .finish_non_exhaustive()
    }
}

impl GroupMember {
    /// `device`, not in the group's tree yet.
    pub fn new(device: &DeviceIdentity, group_id: GroupId) -> Self {
        let signer = MlsSigner::of(device);
        Self {
            group_id,
            device: device.public_key(),
            credential: signer.credential(device),
            signer,
            provider: Provider::default(),
            group: None,
            applied: Vec::new(),
            own: None,
            snapshots: Vec::new(),
            secrets: KeptSecrets::default(),
        }
    }

    fn mls_group_id(&self) -> openmls::group::GroupId {
        openmls::group::GroupId::from_slice(self.group_id.as_bytes())
    }

    /// The MLS epoch this device is at, if it is in the tree.
    pub fn epoch(&self) -> Option<u64> {
        self.group.as_ref().map(|g| g.epoch().as_u64())
    }

    /// The secrets of the current epoch and of the past ones still kept.
    pub fn secrets(&self) -> &KeptSecrets {
        &self.secrets
    }

    /// The links applied here, the one this device joined with first.
    pub fn applied(&self) -> &[EntryHash] {
        &self.applied
    }

    /// Snapshots kept to undo links with.
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    /// Bytes OpenMLS keeps for this group now.
    pub fn stored_len(&self) -> usize {
        self.provider.stored_len()
    }

    /// A new KeyPackage of this device, to be added to the group with; its secrets stay
    /// here until a Welcome uses them.
    pub fn key_package(&self) -> Result<Vec<u8>, MlsError> {
        let bundle = KeyPackage::builder()
            .build(
                CIPHERSUITE,
                &self.provider,
                &self.signer,
                self.credential.clone(),
            )
            .map_err(mls)?;
        MlsMessageOut::from(bundle.key_package().clone())
            .tls_serialize_detached()
            .map_err(mls)
    }

    /// Starts the group's MLS state at epoch 0, with this device alone: the first Commit
    /// adds the others.
    pub fn start(&mut self) -> Result<(), MlsError> {
        if self.group.is_some() {
            return Err(MlsError::Refused("the group is started already"));
        }
        let group = MlsGroup::new_with_group_id(
            &self.provider,
            &self.signer,
            &create_config(),
            self.mls_group_id(),
            self.credential.clone(),
        )
        .map_err(mls)?;
        self.group = Some(group);
        self.applied.clear();
        Ok(())
    }

    /// The leaf of `device` in the current tree.
    fn leaf_of(&self, device: &DevicePublicKey) -> Option<LeafNodeIndex> {
        self.group
            .as_ref()?
            .members()
            .find(|m| device_of(&m.credential, &m.signature_key).as_ref() == Some(device))
            .map(|m| m.index)
    }

    /// A Commit on the current epoch that adds `add` (devices with their KeyPackages) and
    /// removes `remove`; with neither, one that updates this device's own keys. It waits
    /// here until its entry is in the chain ([`Self::follow`]); a newer one replaces it.
    pub fn commit(
        &mut self,
        add: &[(DevicePublicKey, &[u8])],
        remove: &[DevicePublicKey],
    ) -> Result<Proposed, MlsError> {
        let packages = add
            .iter()
            .map(|(device, bytes)| read_key_package(bytes, device))
            .collect::<Result<Vec<_>, _>>()?;
        let leaves = remove
            .iter()
            .map(|d| {
                self.leaf_of(d)
                    .ok_or(MlsError::Refused("removes a device not in the tree"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let provider = &self.provider;
        let signer = &self.signer;
        let group = self.group.as_mut().ok_or(MlsError::NotInGroup)?;
        group
            .clear_pending_commit(provider.storage())
            .map_err(mls)?;
        let bundle = if packages.is_empty() && leaves.is_empty() {
            group
                .self_update(provider, signer, LeafNodeParameters::default())
                .map_err(mls)?
        } else {
            group
                .commit_builder()
                .propose_adds(packages)
                .propose_removals(leaves)
                .load_psks(provider.storage())
                .map_err(mls)?
                .build(provider.rand(), provider.crypto(), signer, |_| true)
                .map_err(mls)?
                .stage_commit(provider)
                .map_err(mls)?
        };
        let (commit, welcome, _) = bundle.into_messages();
        let payload = commit.tls_serialize_detached().map_err(mls)?;
        let welcome = welcome
            .map(|w| w.tls_serialize_detached())
            .transpose()
            .map_err(mls)?;
        self.own = Some(Own::Pending(payload_digest(&payload)));
        Ok(Proposed {
            changes: Changes::new(add.iter().map(|(d, _)| *d), remove.iter().copied()),
            payload,
            welcome,
        })
    }

    /// The current epoch's GroupInfo, with the tree: what a member that lost its state
    /// resyncs from.
    pub fn group_info(&self) -> Result<Vec<u8>, MlsError> {
        let group = self.group.as_ref().ok_or(MlsError::NotInGroup)?;
        group
            .export_group_info(self.provider.crypto(), &self.signer, true)
            .map_err(mls)?
            .tls_serialize_detached()
            .map_err(mls)
    }

    /// Takes a new place in the tree from `group_info` with an external Commit that drops
    /// this device's old place, throwing away whatever state it had: for a device whose
    /// state no longer fits the chain. Returns the Commit, for a `Resync` entry.
    pub fn resync(&mut self, group_info: &[u8], now_ms: u64) -> Result<Vec<u8>, MlsError> {
        let MlsMessageBodyIn::GroupInfo(info) = MlsMessageIn::tls_deserialize_exact(group_info)
            .map_err(mls)?
            .extract()
        else {
            return Err(MlsError::Refused("not a group info"));
        };
        if info.group_id() != &self.mls_group_id() {
            return Err(MlsError::Refused("a group info of another group"));
        }
        let provider = Provider::default();
        let (group, bundle) = MlsGroup::external_commit_builder()
            .with_config(join_config())
            .skip_lifetime_validation()
            .build_group(&provider, info, self.credential.clone())
            .map_err(mls)?
            .load_psks(provider.storage())
            .map_err(mls)?
            .build(provider.rand(), provider.crypto(), &self.signer, |_| true)
            .map_err(mls)?
            .finalize(&provider)
            .map_err(mls)?;
        if group
            .members()
            .any(|m| device_of(&m.credential, &m.signature_key).is_none())
        {
            return Err(MlsError::Refused("a leaf not tied to its device"));
        }
        let payload = bundle.into_commit().tls_serialize_detached().map_err(mls)?;
        self.provider = provider;
        self.group = Some(group);
        self.applied.clear();
        self.snapshots.clear();
        self.own = Some(Own::Merged(payload_digest(&payload)));
        self.reached(now_ms)?;
        Ok(payload)
    }

    /// Moves this device's state along `links`, the group's chain from its first link (see
    /// [`Followed`]). The caller works the chain out again and calls this again after
    /// `Invalid`, and after anything new arrives.
    pub fn follow(&mut self, links: &[ChainLink<'_>], now_ms: u64) -> Result<Followed, MlsError> {
        let result = self.follow_inner(links, now_ms);
        self.prune(now_ms);
        result
    }

    fn follow_inner(&mut self, links: &[ChainLink<'_>], now_ms: u64) -> Result<Followed, MlsError> {
        // Where this device's links sit in the chain, and how far they agree with it.
        let start = match (self.applied.first(), self.own) {
            (Some(first), _) => links.iter().position(|l| l.entry == *first),
            (None, Some(own)) if self.group.is_some() => {
                let digest = match own {
                    Own::Pending(d) | Own::Merged(d) => d,
                };
                match links
                    .iter()
                    .position(|l| l.digest == digest && l.author == self.device)
                {
                    Some(at) => Some(at),
                    // Its own first link is not (or no longer) in the chain.
                    None if links.is_empty() => return Ok(Followed::UpToDate),
                    None => None,
                }
            }
            (None, None) if self.group.is_some() && links.is_empty() => {
                return Ok(Followed::UpToDate);
            }
            _ => None,
        };
        let Some(start) = start else {
            self.drop_group()?;
            return self.join_from_chain(links, now_ms);
        };
        let agree = self
            .applied
            .iter()
            .zip(&links[start..])
            .take_while(|(mine, link)| **mine == link.entry)
            .count();
        if agree < self.applied.len() && !self.roll_back(self.applied[agree])? {
            self.drop_group()?;
            return self.join_from_chain(links, now_ms);
        }
        self.apply_all(&links[start + self.applied.len()..], now_ms)
    }

    fn apply_all(&mut self, links: &[ChainLink<'_>], now_ms: u64) -> Result<Followed, MlsError> {
        for link in links {
            if link
                .changes
                .is_some_and(|c| c.remove.contains(&self.device))
            {
                self.drop_group()?;
                return Ok(Followed::NotInTree);
            }
            let mine =
                matches!(self.own, Some(Own::Merged(d) | Own::Pending(d)) if d == link.digest);
            if link.author == self.device && !mine {
                let Some(payload) = link.payload.filter(|p| payload_digest(p) == link.digest)
                else {
                    return Ok(Followed::Waiting(link.entry));
                };
                if let Err(Refusal::Invalid(reason)) = self.precheck(link, payload) {
                    return Ok(Followed::Invalid(link.entry, reason));
                }
                self.drop_group()?;
                return Ok(Followed::Replaced(link.entry));
            }
            match self.apply(link, now_ms)? {
                Followed::UpToDate => {}
                other => return Ok(other),
            }
        }
        Ok(Followed::UpToDate)
    }

    /// Applies one link on the state this device is at.
    fn apply(&mut self, link: &ChainLink<'_>, now_ms: u64) -> Result<Followed, MlsError> {
        let own = link.author == self.device
            && matches!(self.own, Some(Own::Pending(d) | Own::Merged(d)) if d == link.digest);
        if own && matches!(self.own, Some(Own::Merged(_))) {
            self.own = None;
            self.applied.push(link.entry);
            return Ok(Followed::UpToDate);
        }
        if !own
            && link
                .payload
                .is_none_or(|p| payload_digest(p) != link.digest)
        {
            return Ok(Followed::Waiting(link.entry));
        }
        self.take_snapshot(link.entry, now_ms);
        let provider = &self.provider;
        let group = self.group.as_mut().ok_or(MlsError::NotInGroup)?;
        if own {
            group.merge_pending_commit(provider).map_err(mls)?;
        } else {
            group
                .clear_pending_commit(provider.storage())
                .map_err(mls)?;
            let payload = link.payload.expect("checked above");
            if let Err(refusal) = self.process(link, payload) {
                let before = self.snapshots.pop().expect("taken above");
                self.restore(before)?;
                return Ok(match refusal {
                    Refusal::Invalid(reason) => Followed::Invalid(link.entry, reason),
                    Refusal::Local => Followed::Lost,
                });
            }
        }
        self.own = None;
        self.applied.push(link.entry);
        self.reached(now_ms)?;
        Ok(Followed::UpToDate)
    }

    /// What every member of the epoch checks the same way before anything else, and even
    /// of a link its own key wrote that it did not make: that the message is a Commit in
    /// the clear, on this group's current epoch, sent by the entry's author as the entry
    /// says. Returns the message, and the devices of the current tree's leaves.
    #[allow(clippy::type_complexity)]
    fn precheck(
        &self,
        link: &ChainLink<'_>,
        payload: &[u8],
    ) -> Result<
        (
            PublicMessageIn,
            Vec<(LeafNodeIndex, Option<DevicePublicKey>)>,
        ),
        Refusal,
    > {
        use Refusal::Invalid;
        let message = MlsMessageIn::tls_deserialize_exact(payload)
            .map_err(|_| Invalid("not an MLS message"))?;
        let MlsMessageBodyIn::PublicMessage(message) = message.extract() else {
            return Err(Invalid("not a commit in the clear"));
        };
        let group = self.group.as_ref().ok_or(Refusal::Local)?;
        let devices: Vec<(LeafNodeIndex, Option<DevicePublicKey>)> = group
            .members()
            .map(|m| (m.index, device_of(&m.credential, &m.signature_key)))
            .collect();
        let sender_ok = match (message.sender(), link.changes.is_none()) {
            (Sender::Member(leaf), false) => devices
                .iter()
                .any(|(i, d)| i == leaf && *d == Some(link.author)),
            (Sender::NewMemberCommit, true) => true,
            _ => false,
        };
        if !sender_ok {
            return Err(Invalid("not sent by the entry's author as its entry says"));
        }
        if message.group_id() != group.group_id() || message.epoch() != group.epoch() {
            return Err(Invalid("made on another group or epoch"));
        }
        if message.content_type() != ContentType::Commit {
            return Err(Invalid("not a commit"));
        }
        Ok((message, devices))
    }

    /// Checks another device's Commit against its entry, and applies it.
    fn process(&mut self, link: &ChainLink<'_>, payload: &[u8]) -> Result<(), Refusal> {
        use Refusal::Invalid;
        let resync = link.changes.is_none();
        let (message, devices_before) = self.precheck(link, payload)?;
        let device_at = |leaf: LeafNodeIndex| {
            devices_before
                .iter()
                .find(|(i, _)| *i == leaf)
                .and_then(|(_, d)| *d)
        };
        let provider = &self.provider;
        let group = self.group.as_mut().ok_or(Refusal::Local)?;
        let processed = group
            .process_message(provider, ProtocolMessage::from(message))
            .map_err(|e| classify(&e))?;
        let ProcessedMessageContent::StagedCommitMessage(staged) = processed.into_content() else {
            return Err(Invalid("not a commit"));
        };
        for proposal in staged.queued_proposals() {
            match proposal.proposal() {
                Proposal::Add(_) | Proposal::Remove(_) => {}
                Proposal::ExternalInit(_) if resync => {}
                _ => return Err(Invalid("a kind of proposal groups do not use")),
            }
        }
        let mut added = Vec::new();
        for add in staged.add_proposals() {
            let leaf = add.add_proposal().key_package().leaf_node();
            let device = device_of(leaf.credential(), leaf.signature_key().as_slice())
                .ok_or(Invalid("a leaf not tied to its device"))?;
            added.push(device);
        }
        let mut removed = Vec::new();
        for remove in staged.remove_proposals() {
            removed.push(
                device_at(remove.remove_proposal().removed())
                    .ok_or(Invalid("removes a leaf not tied to its device"))?,
            );
        }
        let path_device = staged
            .update_path_leaf_node()
            .map(|leaf| device_of(leaf.credential(), leaf.signature_key().as_slice()));
        match link.changes {
            Some(changes) => {
                if Changes::new(added.clone(), removed.clone()) != *changes
                    || added.len() != changes.add.len()
                    || removed.len() != changes.remove.len()
                {
                    return Err(Invalid("changes other than its entry names"));
                }
            }
            None => {
                if !added.is_empty() || removed.iter().any(|d| *d != link.author) {
                    return Err(Invalid(
                        "a resync that changes more than its author's place",
                    ));
                }
                if path_device.is_none() {
                    return Err(Invalid("a resync without a new place"));
                }
            }
        }
        if path_device.is_some_and(|d| d != Some(link.author)) {
            return Err(Invalid("a new leaf not tied to the entry's author"));
        }
        group
            .merge_staged_commit(provider, *staged)
            .map_err(|_| Refusal::Local)
    }

    /// Joins from the Welcome of the last link that adds this device, if no later link
    /// removes it.
    fn join_from_chain(
        &mut self,
        links: &[ChainLink<'_>],
        now_ms: u64,
    ) -> Result<Followed, MlsError> {
        // The last link that says where this device stands: one that adds or removes it, or
        // one it wrote (it was in the tree then).
        let Some(at) = links.iter().rposition(|l| {
            l.author == self.device
                || l.changes.is_some_and(|c| {
                    c.add.contains(&self.device) || c.remove.contains(&self.device)
                })
        }) else {
            return Ok(Followed::NotInTree);
        };
        let link = &links[at];
        if link
            .changes
            .is_some_and(|c| c.remove.contains(&self.device))
        {
            return Ok(Followed::NotInTree);
        }
        if !link.changes.is_some_and(|c| c.add.contains(&self.device)) {
            // In the tree, by a link of its own: only a resync brings it back.
            return Ok(Followed::Lost);
        }
        let Some(welcome) = link.welcome else {
            return Ok(Followed::Waiting(link.entry));
        };
        let MlsMessageBodyIn::Welcome(welcome) = MlsMessageIn::tls_deserialize_exact(welcome)
            .map_err(mls)?
            .extract()
        else {
            return Ok(Followed::Lost);
        };
        let staged = StagedWelcome::build_from_welcome(&self.provider, &join_config(), welcome)
            .and_then(|b| b.replace_old_group().skip_lifetime_validation().build());
        let Ok(staged) = staged else {
            // Its secrets are gone (used, or a newer state replaced them), or it does not
            // open: a manager has to add this device again.
            return Ok(Followed::Lost);
        };
        let from_author = staged
            .welcome_sender()
            .ok()
            .and_then(|l| device_of(l.credential(), l.signature_key().as_slice()))
            == Some(link.author);
        let all_tied = staged
            .members()
            .all(|m| device_of(&m.credential, &m.signature_key).is_some());
        if !from_author || !all_tied {
            return Ok(Followed::Lost);
        }
        let group = staged.into_group(&self.provider).map_err(mls)?;
        self.group = Some(group);
        self.applied = vec![link.entry];
        self.snapshots.clear();
        self.own = None;
        self.reached(now_ms)?;
        self.apply_all(&links[at + 1..], now_ms)
    }

    /// Forgets the group's state (KeyPackages not used yet stay).
    fn drop_group(&mut self) -> Result<(), MlsError> {
        if let Some(mut group) = self.group.take() {
            group.delete(self.provider.storage()).map_err(mls)?;
        }
        self.applied.clear();
        self.own = None;
        self.snapshots.clear();
        Ok(())
    }

    /// The epoch reached: its secret is exported and kept.
    fn reached(&mut self, now_ms: u64) -> Result<(), MlsError> {
        let group = self.group.as_ref().ok_or(MlsError::NotInGroup)?;
        let secret = Zeroizing::new(
            group
                .export_secret(
                    self.provider.crypto(),
                    EXPORT_LABEL,
                    self.group_id.as_bytes(),
                    32,
                )
                .map_err(mls)?,
        );
        let id: [u8; 16] = group.epoch_authenticator().as_slice()[..16]
            .try_into()
            .expect("an epoch authenticator is 32 bytes");
        self.secrets.reached(EpochSecret {
            id,
            secret: GroupSecret::from_bytes(&secret).map_err(mls)?,
            since_ms: now_ms,
        });
        Ok(())
    }

    fn take_snapshot(&mut self, before: EntryHash, now_ms: u64) {
        self.snapshots.push(Snapshot {
            before,
            taken_ms: now_ms,
            values: self.provider.encoded(),
            own: self.own,
        });
    }

    fn restore(&mut self, snapshot: Snapshot) -> Result<(), MlsError> {
        self.provider.restore(decode_values(&snapshot.values)?);
        self.group = load_group(&self.provider, &self.mls_group_id())?;
        self.own = snapshot.own;
        if let Some(at) = self.applied.iter().position(|h| *h == snapshot.before) {
            self.applied.truncate(at);
        }
        Ok(())
    }

    /// Undoes the links from `link` on with the snapshot taken before it. False when it is
    /// no longer kept.
    fn roll_back(&mut self, link: EntryHash) -> Result<bool, MlsError> {
        let Some(at) = self.snapshots.iter().position(|s| s.before == link) else {
            return Ok(false);
        };
        let snapshot = self.snapshots.drain(at..).next().expect("found above");
        self.restore(snapshot)?;
        Ok(true)
    }

    /// Deletes snapshots and past secrets past keeping.
    fn prune(&mut self, now_ms: u64) {
        let excess = self.snapshots.len().saturating_sub(MAX_SNAPSHOTS);
        self.snapshots.drain(..excess);
        self.snapshots
            .retain(|s| now_ms.saturating_sub(s.taken_ms) < SNAPSHOT_FOR_MS);
        self.secrets.prune(now_ms);
    }

    /// This device's state as bytes, to be sealed and written whole.
    pub fn to_bytes(&self) -> Zeroizing<Vec<u8>> {
        let provider = self.provider.encoded();
        // Measured first, with room for the digest, so no copy of a secret is left behind
        // as the buffer grows.
        let mut bytes = encode_secret(32, |w| {
            w.u8(FILE_VERSION)
                .group(&self.group_id)
                .key(&self.device)
                .raw(&provider);
            w.u32(self.applied.len() as u32);
            for hash in &self.applied {
                w.raw(hash);
            }
            write_own(w, self.own);
            w.u32(self.snapshots.len() as u32);
            for s in &self.snapshots {
                w.raw(&s.before).u64(s.taken_ms);
                write_own(w, s.own);
                w.bytes(&s.values);
            }
            w.u32(self.secrets.all().len() as u32);
            for s in self.secrets.all() {
                w.raw(&s.id).raw(s.secret.as_bytes()).u64(s.since_ms);
            }
        });
        let digest = Sha256::digest(bytes.as_slice());
        bytes.extend_from_slice(&digest);
        bytes
    }

    /// The state [`Self::to_bytes`] wrote, for `device`.
    pub fn from_bytes(device: &DeviceIdentity, bytes: &[u8]) -> Result<Self, MlsError> {
        // A digest at the end: damage is told before OpenMLS reads its part (which it
        // trusts, being its own).
        let (bytes, digest) = bytes
            .split_at_checked(bytes.len().saturating_sub(32))
            .ok_or(MlsError::Damaged)?;
        if digest.len() != 32 || Sha256::digest(bytes).as_slice() != digest {
            return Err(MlsError::Damaged);
        }
        let mut r = Reader::new(bytes);
        if r.u8()? != FILE_VERSION {
            return Err(MlsError::Damaged);
        }
        let group_id = r.group()?;
        if r.key()? != device.public_key() {
            return Err(MlsError::Refused("the state of another device"));
        }
        let mut member = Self::new(device, group_id);
        member.provider = Provider::with_values(read_values(&mut r)?);
        let n = r.count(32)?;
        member.applied = (0..n).map(|_| r.array()).collect::<Result<_, _>>()?;
        member.own = read_own(&mut r)?;
        let n = r.count(32 + 8 + 1 + 4)?;
        for _ in 0..n {
            let before = r.array()?;
            let taken_ms = r.u64()?;
            let own = read_own(&mut r)?;
            let values = Zeroizing::new(r.bytes()?.to_vec());
            decode_values(&values)?;
            member.snapshots.push(Snapshot {
                before,
                taken_ms,
                values,
                own,
            });
        }
        let n = r.count(16 + 32 + 8)?;
        let mut secrets = Vec::with_capacity(n);
        for _ in 0..n {
            secrets.push(EpochSecret {
                id: r.array()?,
                secret: GroupSecret::from_bytes(r.take(32)?).map_err(|_| MlsError::Damaged)?,
                since_ms: r.u64()?,
            });
        }
        member.secrets = KeptSecrets::from_vec(secrets);
        r.finish()?;
        member.group = load_group(&member.provider, &member.mls_group_id())?;
        if member.group.is_none() && (!member.applied.is_empty() || member.own.is_some()) {
            return Err(MlsError::Damaged);
        }
        Ok(member)
    }
}

/// The group OpenMLS stored in `provider`. OpenMLS's storage panics on a value it cannot
/// read; that is told as damage, not a crash.
fn load_group(
    provider: &Provider,
    id: &openmls::group::GroupId,
) -> Result<Option<MlsGroup>, MlsError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        MlsGroup::load(provider.storage(), id)
    }))
    .map_err(|_| MlsError::Damaged)?
    .map_err(mls)
}

fn write_own(w: &mut Writer, own: Option<Own>) {
    match own {
        None => w.u8(0),
        Some(Own::Pending(d)) => w.u8(1).raw(&d),
        Some(Own::Merged(d)) => w.u8(2).raw(&d),
    };
}

fn read_own(r: &mut Reader<'_>) -> Result<Option<Own>, DecodeError> {
    Ok(match r.u8()? {
        0 => None,
        1 => Some(Own::Pending(r.array()?)),
        2 => Some(Own::Merged(r.array()?)),
        _ => return Err(DecodeError),
    })
}

/// Sorts what OpenMLS found wrong with a Commit: what every member of the epoch finds the
/// same way (its framing, signatures, proposals, the leaves it brings), against what may
/// be this device's own (secrets sealed to it that do not open, a state that is behind).
fn classify<E>(error: &ProcessMessageError<E>) -> Refusal {
    use StageCommitError as S;
    match error {
        ProcessMessageError::ValidationError(ValidationError::WrongEpoch) => Refusal::Local,
        ProcessMessageError::ValidationError(_)
        | ProcessMessageError::IncompatibleWireFormat
        | ProcessMessageError::UnauthorizedExternalApplicationMessage
        | ProcessMessageError::UnsupportedProposalType => {
            Refusal::Invalid("its framing does not check")
        }
        ProcessMessageError::InvalidCommit(
            S::WrongPlaintextContentType
            | S::PathLeafNodeVerificationFailure
            | S::RequiredPathNotFound
            | S::ConfirmationTagMissing
            | S::AttemptedSelfRemoval
            | S::MissingProposal
            | S::InconsistentSenderIndex
            | S::SenderTypeExternal
            | S::SenderTypeNewMemberProposal
            | S::TooManyNewMembers
            | S::ProposalValidationError(_)
            | S::PskError(_)
            | S::ExternalCommitValidation(_)
            | S::GroupContextExtensionsProposalValidationError(_)
            | S::LeafNodeValidation(_)
            | S::DuplicatePskId(_),
        ) => Refusal::Invalid("it does not check"),
        // The secrets sealed to this device, or this device's state.
        _ => Refusal::Local,
    }
}

/// A KeyPackage, checked: for this ciphersuite, signed, and tied to `device`.
pub fn read_key_package(bytes: &[u8], device: &DevicePublicKey) -> Result<KeyPackage, MlsError> {
    let MlsMessageBodyIn::KeyPackage(package) = MlsMessageIn::tls_deserialize_exact(bytes)
        .map_err(mls)?
        .extract()
    else {
        return Err(MlsError::Refused("not a key package"));
    };
    let package = package
        .validate(&crate::FogataCrypto, ProtocolVersion::Mls10)
        .map_err(mls)?;
    if package.ciphersuite() != CIPHERSUITE {
        return Err(MlsError::Refused("a key package of another ciphersuite"));
    }
    let leaf = package.leaf_node();
    if device_of(leaf.credential(), leaf.signature_key().as_slice()).as_ref() != Some(device) {
        return Err(MlsError::Refused("a key package of another device"));
    }
    Ok(package)
}
