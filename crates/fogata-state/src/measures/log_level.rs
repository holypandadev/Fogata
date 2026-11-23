//! Makers of the level two and level three corpora, run by hand (`--ignored`).

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
