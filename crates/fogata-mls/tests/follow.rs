//! Devices with their own copies of a group's log and their own MLS state, following the
//! log's chain of epochs (`fogata_state::chain`) with real MLS Commits, as the network will
//! carry them: entries in the log, the MLS messages beside it.

mod common;

use common::*;
use fogata_mls::{Followed, KEPT_EPOCHS, MAX_SNAPSHOTS};
use fogata_state::{Body, MIN_UPDATE_GAP_MS};

#[test]
fn a_group_starts_its_chain_and_everyone_shares_each_epoch() {
    let mut w = World::new(3);
    w.start();
    assert_eq!(w.agreed(), 1);
    // Bea updates her keys; everyone moves on to the same epoch.
    w.duty(1);
    w.sync_all();
    assert_eq!(w.catch_up_all(), vec![Followed::UpToDate; 3]);
    assert_eq!(w.agreed(), 2);
    // Leo's state, written out and read back, follows on as before.
    let saved = w.devices[2].mls.to_bytes();
    // Written where it stays, of exactly its size: a buffer that grew as it was written
    // would have left copies of the group's secrets behind.
    assert_eq!(saved.capacity(), saved.len());
    w.devices[2].mls = fogata_mls::GroupMember::from_bytes(&w.devices[2].id, &saved).unwrap();
    w.now += MIN_UPDATE_GAP_MS;
    w.duty(0);
    w.sync_all();
    w.catch_up_all();
    assert_eq!(w.agreed(), 3);
}

#[test]
fn two_updates_at_once_settle_and_the_loser_undoes_its_own() {
    let mut w = World::new(3);
    w.start();
    // Bea and Leo update on the same epoch without seeing each other.
    w.duty(1);
    w.duty(2);
    assert_eq!(w.catch_up(1), Followed::UpToDate);
    assert_eq!(w.catch_up(2), Followed::UpToDate);
    assert_ne!(w.current(1), w.current(2));
    w.sync_all();
    assert_eq!(w.catch_up_all(), vec![Followed::UpToDate; 3]);
    assert_eq!(w.agreed(), 2);
    // A longer line made apart (Ana, then Leo) wins over Bea's one update, and everyone
    // follows it, Bea undoing her own.
    let mut apart = w.copy();
    for i in [0, 2] {
        apart.now += MIN_UPDATE_GAP_MS;
        apart.duty(i);
        apart.sync_all();
        apart.catch_up_all();
    }
    w.now += MIN_UPDATE_GAP_MS;
    w.duty(1);
    w.sync_all();
    w.catch_up_all();
    assert_eq!(w.agreed(), 3);
    w.take_devices_from(&apart, &[0, 2]);
    w.sync_all();
    assert_eq!(w.catch_up_all(), vec![Followed::UpToDate; 3]);
    assert_eq!(w.agreed(), 4);
    assert_eq!(w.current(0), apart.current(0));
}

#[test]
fn a_commit_that_does_not_match_its_entry_counts_for_nobody() {
    let mut w = World::new(3);
    w.start();
    // Leo signs an entry for a Commit Bea made: its sender is not the entry's author.
    let proposed = w.devices[1].mls.commit(&[], &[]).unwrap();
    w.write_commit_as(2, proposed);
    w.sync_all();
    for i in 0..3 {
        w.catch_up(i);
    }
    let first = w.agreed();
    assert_eq!(first, 1);
    assert!(w.devices.iter().all(|d| {
        d.verdicts
            .values()
            .any(|v| *v == fogata_state::Verdict::Invalid)
    }));
    // The chain goes on without it.
    w.duty(0);
    w.sync_all();
    w.catch_up_all();
    assert_eq!(w.agreed(), 2);
}

#[test]
fn a_device_that_lost_its_state_resyncs_and_its_old_copy_is_left_out() {
    let mut w = World::new(3);
    w.start();
    // A copy of Leo's state, as a thief with his disk has it.
    let copy = w.devices[2].mls.to_bytes();
    // Leo loses his state: the group's Welcome no longer opens for him.
    w.devices[2].mls = fogata_mls::GroupMember::new(&w.devices[2].id, w.group());
    assert_eq!(w.catch_up(2), Followed::Lost);
    w.resync(2, 1);
    w.sync_all();
    assert_eq!(w.catch_up_all(), vec![Followed::UpToDate; 3]);
    assert_eq!(w.agreed(), 2);
    // The copy sees another copy of its device take its place.
    let mut thief = fogata_mls::GroupMember::from_bytes(&w.devices[2].id, &copy).unwrap();
    let followed = w.follow_with(2, &mut thief);
    assert!(matches!(followed, Followed::Replaced(_)), "{followed:?}");
    assert!(thief.epoch().is_none());
}

#[test]
fn a_removed_device_gets_no_newer_epoch() {
    let mut w = World::new(3);
    w.start();
    let leo = w.devices[2].id.public_key();
    w.remove(0, leo);
    w.duty(0);
    w.sync_all();
    assert_eq!(w.catch_up(2), Followed::NotInTree);
    assert_eq!(w.catch_up(0), Followed::UpToDate);
    assert_eq!(w.catch_up(1), Followed::UpToDate);
    assert_eq!(w.current(0), w.current(1));
    let theirs = w.current(0).unwrap();
    assert!(
        w.devices[2]
            .mls
            .secrets()
            .all()
            .iter()
            .all(|s| s.id != theirs.0)
    );
    // Leo leaves no trace in the tree either.
    let chain = w.chain(0);
    assert!(!chain.roster().contains(&leo));
    assert!(w.devices[0].log.state().members.len() == 2);
    let _ = Body::Leave;
}

#[test]
fn old_states_and_secrets_go_once_past_keeping() {
    let mut w = World::new(2);
    w.start();
    for round in 0..12 {
        w.now += MIN_UPDATE_GAP_MS;
        w.duty(round % 2);
        w.sync_all();
        assert_eq!(w.catch_up_all(), vec![Followed::UpToDate; 2]);
    }
    assert_eq!(w.agreed(), 13);
    for d in &w.devices {
        assert!(d.mls.secrets().all().len() <= KEPT_EPOCHS + 1);
        assert!(d.mls.snapshot_count() <= MAX_SNAPSHOTS);
    }
    // Two days later, with no new epoch: no snapshot is left, and a week later only the
    // current secret.
    w.now += 7 * 24 * 60 * 60 * 1000;
    w.catch_up_all();
    for d in &w.devices {
        assert_eq!(d.mls.snapshot_count(), 0);
        assert_eq!(d.mls.secrets().all().len(), 1);
    }
}

#[test]
fn a_damaged_state_file_reads_as_damaged() {
    let mut w = World::new(2);
    w.start();
    let id = &w.devices[1].id;
    let good = w.devices[1].mls.to_bytes();
    assert!(fogata_mls::GroupMember::from_bytes(id, &good).is_ok());
    assert!(fogata_mls::GroupMember::from_bytes(&w.devices[0].id, &good).is_err());
    for at in [0, 40, good.len() / 2, good.len() - 1] {
        let mut bad = good.to_vec();
        bad[at] ^= 1;
        assert_eq!(
            fogata_mls::GroupMember::from_bytes(id, &bad).unwrap_err(),
            fogata_mls::MlsError::Damaged
        );
    }
    // Damage OpenMLS's own part, with a digest that fits: told as damage too. It comes after
    // the version, the group id and the device key, whose random bytes may hold a `{` too.
    let body = &good[..good.len() - 32];
    let after_key = 1 + 16 + 32;
    let at = after_key
        + body[after_key..]
            .iter()
            .position(|b| *b == b'{')
            .expect("a stored value");
    let mut bad = body.to_vec();
    bad[at] = b'[';
    let digest = <sha2::Sha256 as sha2::Digest>::digest(&bad);
    bad.extend_from_slice(&digest);
    assert_eq!(
        fogata_mls::GroupMember::from_bytes(id, &bad).unwrap_err(),
        fogata_mls::MlsError::Damaged
    );
    assert!(fogata_mls::GroupMember::from_bytes(id, &[]).is_err());
}

/// A device's state put back from an older copy (a restore), behind the group's chain:
/// while the links it missed are others' Commits it follows on from where it was, by itself.
/// Once a Commit of its own came after the copy (its update, an add or a removal it made),
/// the state put back is not the one that made it: it is out of the tree, as a stolen copy
/// would be, and only a resync from a member's GroupInfo brings it back.
#[test]
fn an_older_copy_put_back_catches_up_unless_it_committed() {
    let mut w = World::new(3);
    w.start();
    let copy = w.devices[2].mls.to_bytes();
    // After the copy, Ana and Bea move the group on two epochs.
    for i in [0, 1] {
        w.now += MIN_UPDATE_GAP_MS;
        w.duty(i);
        w.sync_all();
        assert_eq!(w.catch_up_all(), vec![Followed::UpToDate; 3]);
    }
    assert_eq!(w.agreed(), 3);
    // Leo's state from the copy, with the log as it is now (a member's sync brings it).
    w.devices[2].mls = fogata_mls::GroupMember::from_bytes(&w.devices[2].id, &copy).unwrap();
    assert_eq!(w.devices[2].mls.epoch(), Some(1));
    assert_eq!(w.catch_up(2), Followed::UpToDate);
    assert_eq!(w.agreed(), 3);

    // Now Leo updates his own keys after a copy is made.
    let copy = w.devices[2].mls.to_bytes();
    w.now += MIN_UPDATE_GAP_MS;
    w.duty(2);
    w.sync_all();
    assert_eq!(w.catch_up_all(), vec![Followed::UpToDate; 3]);
    assert_eq!(w.agreed(), 4);
    w.devices[2].mls = fogata_mls::GroupMember::from_bytes(&w.devices[2].id, &copy).unwrap();
    let followed = w.catch_up(2);
    assert!(matches!(followed, Followed::Replaced(_)), "{followed:?}");
    assert!(w.devices[2].mls.epoch().is_none());
    // A resync takes it back in, and everyone moves on with it.
    w.resync(2, 1);
    w.sync_all();
    assert_eq!(w.catch_up_all(), vec![Followed::UpToDate; 3]);
    assert_eq!(w.agreed(), 5);
}
