//! The seeds writer and the long runs of every fuzz target, run by hand (`--ignored`) by `tools/fuzz.sh`.

use super::runs::run;
use super::*;
use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

fn long_run(name: &str) {
    let target = TARGETS.iter().find(|t| t.name == name).expect("a target");
    let secs = std::env::var("FOGATA_FUZZ_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let seed = std::env::var("FOGATA_FUZZ_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
        });
    eprintln!("{name}: seed {seed}, {secs} s");
    let done = run(target, seed, 0, Some(Duration::from_secs(secs)));
    eprintln!("{name}: {done} inputs, none failed");
}

/// Writes the seeds of every target as files, one folder per target, into
/// `FOGATA_FUZZ_SEEDS`: where libFuzzer starts from.
#[test]
#[ignore = "writes files; tools/fuzz.sh runs it"]
fn write_seeds() {
    let dir = PathBuf::from(std::env::var_os("FOGATA_FUZZ_SEEDS").expect("FOGATA_FUZZ_SEEDS"));
    for target in TARGETS {
        let dir = dir.join(target.name);
        std::fs::create_dir_all(&dir).expect("a folder");
        for (n, seed) in (target.seeds)().into_iter().enumerate() {
            std::fs::write(dir.join(format!("seed-{n}")), seed).expect("written");
        }
    }
}

macro_rules! long_runs {
    ($($name:ident),* $(,)?) => {
        /// The targets with a long run below.
        pub(super) const LONG_RUNS: &[&str] = &[$(stringify!($name)),*];
        $(
            #[test]
            #[ignore = "long; tools/fuzz.sh runs it"]
            fn $name() {
                long_run(stringify!($name));
            }
        )*
    };
}

long_runs!(
    entry,
    log,
    proof,
    member_wire,
    node_request,
    node_join,
    node_friend,
    friend_wire,
    friend_sealed,
    room_event,
    parts,
    turn,
    direct_payload,
    typing,
    sealed_name,
    invite,
    records,
    at_rest,
    mls,
    ogg_read,
    voice_play,
    wsola_step,
    resample,
    recorder,
    transfer_wire,
);
