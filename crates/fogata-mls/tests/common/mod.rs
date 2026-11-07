//! A few devices of one group, each with its own log and MLS state, and the MLS messages
//! kept beside the logs, shared as the network will share them.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
use fogata_mls::{ChainLink, Followed, GroupMember, Proposed};
use fogata_state::{
    Body, Chain, Entry, EntryHash, EpochLink, GroupLog, PayloadDigest, Verdict, payload_digest,
};

pub struct Device {
    pub id: DeviceIdentity,
    pub log: GroupLog,
    pub mls: GroupMember,
    pub verdicts: BTreeMap<EntryHash, Verdict>,
}

pub struct World {
    pub devices: Vec<Device>,
    /// Commits and Welcomes by digest, kept beside the logs.
    pub beside: HashMap<PayloadDigest, Vec<u8>>,
    pub now: u64,
}

pub fn identity_copy(id: &DeviceIdentity) -> DeviceIdentity {
    DeviceIdentity::from_secret_bytes(&id.secret_bytes())
}

/// Gives `to` what it lacks of `from`, as a sync does.
pub fn sync(from: &GroupLog, to: &mut GroupLog) {
    let missing: Vec<Entry> = from
        .entries()
        .filter(|e| to.get(&e.hash()).is_none())
        .cloned()
        .collect();
    to.insert_all(missing);
}

/// The newest KeyPackage `device` put in `log`.
pub fn key_package_of(log: &GroupLog, device: &DevicePublicKey) -> Option<Vec<u8>> {
    log.entries_in_order()
        .into_iter()
        .rev()
        .find_map(|e| match &e.body {
            Body::MlsKeyPackage { key_package } | Body::MlsJoin { key_package, .. }
                if e.author == *device =>
            {
                Some(key_package.clone())
            }
            _ => None,
        })
}

/// The links of `chain`, as `follow` takes them.
pub fn links<'a>(
    log: &'a GroupLog,
    chain: &Chain,
    beside: &'a HashMap<PayloadDigest, Vec<u8>>,
) -> Vec<ChainLink<'a>> {
    chain
        .links
        .iter()
        .map(|link| {
            let entry = log.get(&link.entry).expect("in the log");
            let (changes, digest, welcome) = match &entry.body {
                Body::Commit {
                    changes,
                    payload,
                    welcome,
                    ..
                } => (Some(changes), *payload, *welcome),
                Body::Resync { payload, .. } => (None, *payload, None),
                _ => unreachable!("links are commits"),
            };
            ChainLink {
                entry: link.entry,
                author: entry.author,
                digest,
                changes,
                payload: beside.get(&digest).map(Vec::as_slice),
                welcome: welcome.and_then(|w| beside.get(&w)).map(Vec::as_slice),
            }
        })
        .collect()
}

impl World {
    /// Ana's group with `n` devices in it, each of the others with a KeyPackage in the log.
    pub fn new(n: usize) -> Self {
        let ids: Vec<DeviceIdentity> = (0..n)
            .map(|_| DeviceIdentity::generate().unwrap())
            .collect();
        let mut log = GroupLog::create(&ids[0], "g", 1).unwrap();
        for (i, d) in ids.iter().enumerate().skip(1) {
            log.add(&ids[0], d.public_key(), "m", 1 + i as u64).unwrap();
        }
        let group = log.group_id();
        let mut world = Self {
            devices: ids
                .into_iter()
                .map(|id| Device {
                    mls: GroupMember::new(&id, group),
                    id,
                    log: log.clone(),
                    verdicts: BTreeMap::new(),
                })
                .collect(),
            beside: HashMap::new(),
            now: 1000,
        };
        for i in 1..n {
            world.publish_key_package(i);
        }
        world.sync_all();
        world
    }

    pub fn group(&self) -> GroupId {
        self.devices[0].log.group_id()
    }

    pub fn publish_key_package(&mut self, i: usize) {
        let d = &mut self.devices[i];
        let key_package = d.mls.key_package().unwrap();
        let entry = Entry::sign(
            &d.id,
            d.log.group_id(),
            d.log.heads(),
            self.now,
            Body::MlsKeyPackage { key_package },
        );
        d.log.insert(entry).unwrap();
    }

    /// Ana starts the chain with everyone in, and everyone follows.
    pub fn start(&mut self) {
        self.devices[0].mls.start().unwrap();
        self.duty(0);
        self.sync_all();
        let n = self.devices.len();
        assert_eq!(self.catch_up_all(), vec![Followed::UpToDate; n]);
    }

    pub fn chain(&self, i: usize) -> Chain {
        let d = &self.devices[i];
        d.log
            .mls_chain(|h| d.verdicts.get(h).copied().unwrap_or(Verdict::Valid))
    }

    /// Device `i` follows its chain, taking note of Commits that do not check.
    pub fn catch_up(&mut self, i: usize) -> Followed {
        loop {
            let chain = self.chain(i);
            let d = &mut self.devices[i];
            let links = links(&d.log, &chain, &self.beside);
            match d.mls.follow(&links, self.now).unwrap() {
                Followed::Invalid(hash, _) => {
                    d.verdicts.insert(hash, Verdict::Invalid);
                }
                other => return other,
            }
        }
    }

    /// `member` (a state of device `i`'s key) follows device `i`'s chain.
    pub fn follow_with(&self, i: usize, member: &mut GroupMember) -> Followed {
        let chain = self.chain(i);
        let d = &self.devices[i];
        member
            .follow(&links(&d.log, &chain, &self.beside), self.now)
            .unwrap()
    }

    pub fn catch_up_all(&mut self) -> Vec<Followed> {
        (0..self.devices.len()).map(|i| self.catch_up(i)).collect()
    }

    pub fn sync_all(&mut self) {
        let mut all = self.devices[0].log.clone();
        for d in &self.devices[1..] {
            sync(&d.log, &mut all);
        }
        for d in &mut self.devices {
            sync(&all, &mut d.log);
        }
    }

    /// Signs and adds the entry of a Commit `proposed` by device `i`, on its chain's tip.
    pub fn write_commit(&mut self, i: usize, proposed: Proposed) -> EntryHash {
        let chain = self.chain(i);
        let link = EpochLink {
            epoch: chain.epoch(),
            prev: chain.tip().map(|l| l.entry),
        };
        self.write_commit_on(i, proposed, link)
    }

    pub fn write_commit_as(&mut self, i: usize, proposed: Proposed) -> EntryHash {
        self.write_commit(i, proposed)
    }

    pub fn write_commit_on(&mut self, i: usize, proposed: Proposed, link: EpochLink) -> EntryHash {
        let payload = payload_digest(&proposed.payload);
        self.beside.insert(payload, proposed.payload);
        let welcome = proposed.welcome.map(|w| {
            let digest = payload_digest(&w);
            self.beside.insert(digest, w);
            digest
        });
        let d = &mut self.devices[i];
        let entry = Entry::sign(
            &d.id,
            d.log.group_id(),
            d.log.heads(),
            self.now,
            Body::Commit {
                link,
                changes: proposed.changes,
                payload,
                welcome,
            },
        );
        let hash = entry.hash();
        // A log keeps only what its author could make where it signed it (a device no
        // longer a member resyncing, say, is not kept); the chain would not count it anyway.
        let _ = d.log.insert(entry);
        hash
    }

    /// Device `i` does what its chain asks of it: adds and removes if it manages (and
    /// the devices to add have KeyPackages), else updates its keys. Returns the entry.
    pub fn duty(&mut self, i: usize) -> Option<EntryHash> {
        let chain = self.chain(i);
        let d = &self.devices[i];
        let state = d.log.state();
        let me = d.id.public_key();
        let manager = state.is_manager(&me);
        let packages: Vec<(DevicePublicKey, Vec<u8>)> = if manager {
            chain
                .to_add(&state)
                .into_iter()
                .filter(|k| *k != me)
                .filter_map(|k| Some((k, key_package_of(&d.log, &k)?)))
                .collect()
        } else {
            Vec::new()
        };
        let remove = chain.to_remove(&state);
        let add: Vec<(DevicePublicKey, &[u8])> =
            packages.iter().map(|(k, p)| (*k, p.as_slice())).collect();
        let proposed = self.devices[i].mls.commit(&add, &remove).ok()?;
        Some(self.write_commit(i, proposed))
    }

    /// Admin `i` removes `member` from the group (the intent, not the Commit).
    pub fn remove(&mut self, i: usize, member: DevicePublicKey) {
        let d = &mut self.devices[i];
        d.log.remove(&d.id, member, self.now).unwrap();
    }

    /// Device `i` resyncs from device `from`'s GroupInfo and writes its `Resync`.
    pub fn resync(&mut self, i: usize, from: usize) -> EntryHash {
        let info = self.devices[from].mls.group_info().unwrap();
        let chain = self.chain(from);
        let tip = chain.tip().expect("a chain");
        let link = EpochLink {
            epoch: tip.epoch,
            prev: Some(tip.entry),
        };
        let payload = self.devices[i].mls.resync(&info, self.now).unwrap();
        let digest = payload_digest(&payload);
        self.beside.insert(digest, payload);
        sync(&self.devices[from].log.clone(), &mut self.devices[i].log);
        let d = &mut self.devices[i];
        let entry = Entry::sign(
            &d.id,
            d.log.group_id(),
            d.log.heads(),
            self.now,
            Body::Resync {
                link,
                payload: digest,
            },
        );
        let hash = entry.hash();
        // A log keeps only what its author could make where it signed it (a device no
        // longer a member resyncing, say, is not kept); the chain would not count it anyway.
        let _ = d.log.insert(entry);
        hash
    }

    /// The current epoch's id and secret on device `i`.
    pub fn current(&self, i: usize) -> Option<([u8; 16], [u8; 32])> {
        self.devices[i]
            .mls
            .secrets()
            .current()
            .filter(|_| self.devices[i].mls.epoch().is_some())
            .map(|s| (s.id, *s.secret.as_bytes()))
    }

    /// Checks that every device in the tree is at the chain's last epoch with the same
    /// secret, and returns that epoch.
    pub fn agreed(&self) -> u64 {
        let chain = self.chain(0);
        let roster = chain.roster();
        let mut seen = None;
        for (i, d) in self.devices.iter().enumerate() {
            assert_eq!(self.chain(i), chain, "device {i} has another chain");
            if !roster.contains(&d.id.public_key()) {
                continue;
            }
            assert_eq!(
                d.mls.epoch(),
                Some(chain.epoch()),
                "device {i} is elsewhere"
            );
            let current = self.current(i);
            assert!(current.is_some());
            if let Some(seen) = seen {
                assert_eq!(current, seen, "device {i} has other keys");
            }
            seen = Some(current);
        }
        chain.epoch()
    }

    /// Copies of every device, logs and MLS states.
    pub fn copy(&self) -> Self {
        Self {
            devices: self
                .devices
                .iter()
                .map(|d| Device {
                    id: identity_copy(&d.id),
                    log: d.log.clone(),
                    mls: GroupMember::from_bytes(&d.id, &d.mls.to_bytes()).unwrap(),
                    verdicts: d.verdicts.clone(),
                })
                .collect(),
            beside: self.beside.clone(),
            now: self.now,
        }
    }

    /// The devices `which` were with `other` (a copy of this world) and come back: they
    /// take their state there, and the messages beside the logs come too.
    pub fn take_devices_from(&mut self, other: &World, which: &[usize]) {
        for &i in which {
            let d = &other.devices[i];
            self.devices[i] = Device {
                id: identity_copy(&d.id),
                log: d.log.clone(),
                mls: GroupMember::from_bytes(&d.id, &d.mls.to_bytes()).unwrap(),
                verdicts: d.verdicts.clone(),
            };
        }
        for (k, v) in &other.beside {
            self.beside.entry(*k).or_insert_with(|| v.clone());
        }
        self.now = self.now.max(other.now);
    }
}
