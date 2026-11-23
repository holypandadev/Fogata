//! Makers of the level two, three and five corpora, run by hand (`--ignored`).

use super::*;

/// Makes the corpus of level two and what it judges of it, into the folder
/// `FOGATA_LEVEL_TWO_CORPUS` names; only when the corpus is made, and never while level two
/// judges the corpus there otherwise than [`JUDGED`] says.
#[test]
#[ignore]
fn write_the_level_two_corpus() {
    let dir = std::env::var_os("FOGATA_LEVEL_TWO_CORPUS").expect("FOGATA_LEVEL_TWO_CORPUS");
    if !CORPUS.is_empty() {
        assert_eq!(as_level_two(|| judged(&corpus_of(CORPUS))), judged_then());
    }
    let mut worlds: Vec<World> = [1, 2, 4].map(corpus_world).into();
    worlds.push(crafted_world());
    let bytes = corpus_bytes(&worlds);
    assert_eq!(corpus_of(&bytes).len(), worlds.len());
    let dir = std::path::Path::new(&dir);
    std::fs::write(dir.join("level-two.bin"), bytes).unwrap();
    std::fs::write(dir.join("level-two.txt"), as_level_two(|| judged(&worlds))).unwrap();
}

/// Makes the corpus of level three and what it judges of it, into the folder
/// `FOGATA_LEVEL_THREE_CORPUS` names: by the version that brought level three only, before
/// any later level, and never while level three judges the corpus there otherwise than
/// [`JUDGED_THREE`] says. Its worlds hold what level three was made for and what it left
/// open: rotations and removals for a few that it keeps and applies.
#[test]
#[ignore]
fn write_the_level_three_corpus() {
    let dir = std::env::var_os("FOGATA_LEVEL_THREE_CORPUS").expect("FOGATA_LEVEL_THREE_CORPUS");
    assert_eq!(
        Level::CURRENT,
        Level::Three,
        "made by the version of level three only"
    );
    if !CORPUS_THREE.is_empty() {
        let worlds = corpus_of(CORPUS_THREE);
        assert_eq!(as_level_three(|| judged_with(&worlds, false)), JUDGED_THREE);
    }
    // Even seeds: a run of the usual room, where the raise is not past its author's run
    // (the corpus of level two holds those).
    let worlds: Vec<World> = [2, 4, 6].map(level_three_world).into();
    // Each kind it holds: a group raised to three, named invites, and rotations and
    // removals for a few that apply there.
    let (mut rotations, mut removals) = (0, 0);
    for world in &worlds {
        let corpus = world.with_variants();
        let mut log = GroupLog::empty(corpus[0].group_id);
        log.set_run_room(world.run, RUN_ROOM.bytes);
        log.insert_all(corpus.iter().cloned());
        let state = log.state();
        assert_eq!(state.level, Level::Three);
        assert!(log.entries().any(|e| NamedInvite::of(&e.body).is_some()));
        for hash in &state.applied {
            if leaves_out_its_view(&log, hash) {
                match log.entries[hash].body {
                    Body::Rotate { .. } => rotations += 1,
                    Body::Remove { .. } => removals += 1,
                    _ => {}
                }
            }
        }
    }
    assert!(rotations > 0 && removals > 0, "{rotations} {removals}");
    let bytes = corpus_bytes(&worlds);
    assert_eq!(corpus_of(&bytes).len(), worlds.len());
    let dir = std::path::Path::new(&dir);
    std::fs::write(dir.join("level-three.bin"), bytes).unwrap();
    std::fs::write(
        dir.join("level-three.txt"),
        as_level_three(|| judged_with(&worlds, false)),
    )
    .unwrap();
}

/// Makes the corpus of level five and what it judges of it, into the folder
/// `FOGATA_LEVEL_FIVE_CORPUS` names: by the version that brought level five only, before
/// any later level, and never while level five judges the corpus there otherwise than
/// [`JUDGED_FIVE`] says. Its worlds hold what level five was made for: a log at its
/// ceiling, entries past it, a removal that still applies, a move, what is written apart
/// from it and a move forged by a member that is not first.
#[test]
#[ignore]
fn write_the_level_five_corpus() {
    let dir = std::env::var_os("FOGATA_LEVEL_FIVE_CORPUS").expect("FOGATA_LEVEL_FIVE_CORPUS");
    assert_eq!(
        Level::CURRENT,
        Level::Five,
        "made by the version of level five only"
    );
    if !CORPUS_FIVE.is_empty() {
        let worlds = corpus_five_of(CORPUS_FIVE);
        assert_eq!(as_level_five(|| judged_with(&worlds, false)), JUDGED_FIVE);
    }
    // The first three histories that reach five and move, judged as written: in some,
    // nobody manages by the time of the raise, or the first member cannot move.
    let judged = |world: &World| {
        let corpus = world.with_variants();
        let mut log = GroupLog::empty(corpus[0].group_id);
        world.rooms().set(&mut log);
        log.insert_all(corpus.iter().cloned());
        log
    };
    let plain: Vec<World> = (1..64)
        .map(level_five_corpus_world)
        .filter(|world| {
            let log = judged(world);
            log.state().log_level == Level::Five && log.moved().is_some()
        })
        .take(3)
        .collect();
    assert_eq!(plain.len(), 3);
    // Each also with its variants: entries like each, which push runs and the ceiling
    // further, and may leave the raise or the move past them.
    let mut worlds = Vec::new();
    for world in plain {
        let with = World {
            variants: true,
            entries: world.entries.clone(),
            devices: (world.devices.iter())
                .map(|d| DeviceIdentity::from_secret_bytes(&d.secret_bytes()))
                .collect(),
            ..world
        };
        worlds.extend([world, with]);
    }
    // In some world, a log at its ceiling with entries refused past it.
    let mut refused: BTreeMap<&str, usize> = BTreeMap::new();
    for world in &worlds {
        for (_, why) in &judged(world).state().rejected {
            *refused.entry(why).or_default() += 1;
        }
    }
    assert!(
        refused.contains_key(crate::budget::PAST_CEILING),
        "{refused:?}"
    );
    let bytes = corpus_five_bytes(&worlds);
    assert_eq!(corpus_five_of(&bytes).len(), worlds.len());
    let dir = std::path::Path::new(&dir);
    std::fs::write(dir.join("level-five.bin"), bytes).unwrap();
    std::fs::write(
        dir.join("level-five.txt"),
        as_level_five(|| judged_with(&worlds, false)),
    )
    .unwrap();
}
