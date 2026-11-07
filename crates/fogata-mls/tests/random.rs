//! Random histories with real MLS: devices that update, remove, leave, come back, lose their
//! state and meet only now and then, each following the chain its own log gives. Once they
//! all meet and do their part, every device in the tree is at the same epoch with the same
//! keys, and the tree holds exactly the group's members.

mod common;

use common::*;
use fogata_mls::{Followed, GroupMember};
use fogata_state::{Body, MIN_UPDATE_GAP_MS};

/// xorshift64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Device `i` brings itself in line with its chain, as an application would after each
/// change: follows; resyncs from `helper` when its state no longer fits but it is in the
/// tree; puts a new KeyPackage in the log when it is a member the tree lacks.
fn tend(w: &mut World, i: usize, helper: usize) -> Followed {
    let followed = w.catch_up(i);
    let me = w.devices[i].id.public_key();
    let state = w.devices[i].log.state();
    let chain = w.chain(i);
    match followed {
        Followed::Lost | Followed::Replaced(_) if chain.roster().contains(&me) => {
            if helper != i && w.catch_up(helper) == Followed::UpToDate && w.chain(helper) == chain {
                w.resync(i, helper);
            }
        }
        Followed::NotInTree | Followed::Lost
            if state.is_member(&me) && !chain.roster().contains(&me) =>
        {
            // A KeyPackage the tree has not used yet: a new one after each try.
            let mine = w.devices[i]
                .log
                .entries()
                .filter(|e| e.author == me && matches!(e.body, Body::MlsKeyPackage { .. }))
                .count();
            let tries = chain
                .links
                .iter()
                .filter(|l| {
                    w.devices[i].log.get(&l.entry).is_some_and(|e| {
                        matches!(&e.body, Body::Commit { changes, .. } if changes.add.contains(&me))
                    })
                })
                .count();
            if mine <= tries {
                w.publish_key_package(i);
            }
        }
        _ => {}
    }
    followed
}

/// Device `i` commits what its chain asks of it, if it is caught up and in the tree.
fn commit(w: &mut World, i: usize) {
    if w.catch_up(i) != Followed::UpToDate {
        return;
    }
    let me = w.devices[i].id.public_key();
    if !w.chain(i).roster().contains(&me) || w.devices[i].mls.epoch().is_none() {
        return;
    }
    w.duty(i);
}

/// How a history ended.
#[derive(Debug, PartialEq, Eq)]
enum End {
    Settled,
    /// No device holds the last epoch of the chain any more (those that made it lost their
    /// state, the others were on a branch that lost, for longer than their snapshots go
    /// back): nobody can give a GroupInfo to resync from.
    NoHolder,
}

fn run(seed: u64) -> End {
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let n = 5;
    let mut w = World::new(n);
    let ana = identity_copy(&w.devices[0].id);
    // Only the first two manage, so that some members do not.
    let first_two = Body::SetManagers { first: Some(2) };
    w.devices[0].log.append(&ana, first_two, w.now).unwrap();
    w.sync_all();
    w.start();
    for _ in 0..60 {
        w.now += rng.below(30) as u64 * 3_600_000;
        let i = rng.below(n);
        let me = w.devices[i].id.public_key();
        let state = w.devices[i].log.state();
        match rng.below(100) {
            0..30 => commit(&mut w, i),
            30..36 => {
                let target = w.devices[rng.below(n)].id.public_key();
                if state.outranks(&me, &target) {
                    w.remove(i, target);
                }
            }
            // The first stays, so that someone always manages.
            36..40 if state.rank(&me).is_some_and(|r| r > 0) => {
                let id = identity_copy(&w.devices[i].id);
                let _ = w.devices[i].log.append(&id, Body::Leave, w.now);
            }
            40..48 if state.is_manager(&me) => {
                let target = w.devices[rng.below(n)].id.public_key();
                if !state.is_member(&target) {
                    let id = identity_copy(&w.devices[i].id);
                    let _ = w.devices[i].log.add(&id, target, "m", w.now);
                }
            }
            48..50 => {
                // Its MLS state is lost (a damaged disk, say).
                w.devices[i].mls = GroupMember::new(&w.devices[i].id, w.group());
            }
            50..70 => {
                let helper = rng.below(n);
                tend(&mut w, i, helper);
            }
            70.. => {
                let j = rng.below(n);
                if i != j {
                    let from = w.devices[i].log.clone();
                    sync(&from, &mut w.devices[j].log);
                    if rng.below(2) == 0 {
                        let back = w.devices[j].log.clone();
                        sync(&back, &mut w.devices[i].log);
                    }
                }
            }
            _ => {}
        }
    }
    // Everyone meets, again and again, and does its part, until nothing is left to do.
    for round in 0..12 {
        w.now += MIN_UPDATE_GAP_MS;
        w.sync_all();
        let Some(helper) = (0..n)
            .find(|i| w.catch_up(*i) == Followed::UpToDate && w.devices[*i].mls.epoch().is_some())
        else {
            // Every device works the chain out the same way, all the same.
            let chain = w.chain(0);
            assert!((1..n).all(|i| w.chain(i) == chain), "seed {seed}");
            return End::NoHolder;
        };
        for i in 0..n {
            tend(&mut w, i, helper);
        }
        w.sync_all();
        let state = w.devices[0].log.state();
        let chain = w.chain(0);
        let pending = !chain.to_add(&state).is_empty() || !chain.to_remove(&state).is_empty();
        let all = (0..n).map(|i| w.catch_up(i)).collect::<Vec<_>>();
        if std::env::var_os("FOGATA_MLS_DEBUG").is_some() {
            eprintln!(
                "round {round}: followed {all:?}, add {:?}, remove {:?}, roster {:?}, members {:?}",
                chain.to_add(&state),
                chain.to_remove(&state),
                chain.roster(),
                state.members.keys().collect::<Vec<_>>()
            );
        }
        let settled = (0..n).all(|i| {
            let me = w.devices[i].id.public_key();
            !chain.roster().contains(&me) || all[i] == Followed::UpToDate
        });
        if !pending && settled {
            w.agreed();
            assert_eq!(
                chain.roster(),
                state.members.keys().copied().collect(),
                "seed {seed}"
            );
            return End::Settled;
        }
        if pending {
            // A manager if one is up to date in the tree, else any member there (who may
            // drop devices that left).
            let ready = |i: &usize, manager: bool| {
                let me = w.devices[*i].id.public_key();
                (!manager || state.is_manager(&me))
                    && state.is_member(&me)
                    && chain.roster().contains(&me)
                    && all[*i] == Followed::UpToDate
            };
            let committer = (0..n)
                .find(|i| ready(i, true))
                .or_else(|| (0..n).find(|i| ready(i, false)));
            if let Some(i) = committer {
                commit(&mut w, i);
            }
        }
    }
    panic!("seed {seed}: never settled");
}

/// `FOGATA_MLS_SEEDS` sets how many histories run (4 by default), `FOGATA_MLS_FIRST` the
/// first one's seed (1).
#[test]
fn every_device_in_the_tree_ends_with_the_same_keys() {
    let seeds = std::env::var("FOGATA_MLS_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4u64);
    let first = std::env::var("FOGATA_MLS_FIRST")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1u64);
    let mut no_holder = 0;
    for seed in first..first + seeds {
        if run(seed) == End::NoHolder {
            no_holder += 1;
        }
    }
    eprintln!("{seeds} histories, {no_holder} where no device held the last epoch");
}
