//! Replay of stored group logs under the newer rules, run by hand (`--ignored`).

use super::*;
use fogata_state::Body;

/// Reads every group log under the folders in `FOGATA_REPLAY_LOGS` (separated by `:`,
/// searched to any depth) and fails if one has an entry ignored only for a rule that
/// versions before it did not have: such a log would read differently on older copies.
/// Run by hand on copies of real data folders:
/// `FOGATA_REPLAY_LOGS=<dir> cargo test -p fogata-core replay -- --ignored --nocapture`.
#[test]
#[ignore]
fn replay_logs_under_the_newer_rules() {
    fn logs(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                logs(&path, out);
            } else if path.extension().is_some_and(|e| e == "log")
                && path.parent().is_some_and(|p| p.ends_with("logs"))
            {
                out.push(path);
            }
        }
    }
    let mut found = Vec::new();
    for dir in std::env::var("FOGATA_REPLAY_LOGS")
        .unwrap_or_default()
        .split(':')
    {
        logs(Path::new(dir), &mut found);
    }
    let mut differ = 0;
    for path in &found {
        let (entries, _) = decode_entries(&fs::read(path).unwrap());
        let Some(group) = entries.first().map(|e| e.group_id) else {
            continue;
        };
        let mut log = GroupLog::empty(group);
        log.insert_all(entries);
        let state = log.state();
        let newer: Vec<_> = state
            .rejected
            .iter()
            .filter(|(_, r)| {
                [
                    fogata_state::ROTATE_NOT_NEEDED,
                    fogata_state::WELCOME_NOT_MANAGER,
                ]
                .contains(r)
            })
            .collect();
        println!(
            "{}: {} entries, {} epochs, {} ignored by newer rules",
            path.display(),
            log.len(),
            state.epochs.len(),
            newer.len()
        );
        // The entries that appointed and demoted admins read as moves on the ladder.
        let moves = log
            .entries()
            .filter(|e| matches!(e.body, Body::Promote { .. } | Body::Demote { .. }))
            .count();
        let mut refused: BTreeMap<&str, usize> = BTreeMap::new();
        for (_, reason) in &state.rejected {
            *refused.entry(reason).or_default() += 1;
        }
        println!(
            "  {} members on the ladder, managers {:?}, {moves} moves, refused {refused:?}",
            state.ladder.len(),
            state.managers
        );
        differ += newer.len();
    }
    println!("{} logs read", found.len());
    assert_eq!(differ, 0);
}
