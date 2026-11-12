//! The commands that change a folder's files themselves (the profile, the friend book, the
//! groups) refuse a folder that a running copy holds: its next save would write over the
//! change. Those that only read it still run.

use std::process::Command;

fn fogata(dir: &std::path::Path, args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_fogata"))
        .arg("--data-dir")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

#[test]
fn what_changes_the_folder_waits_for_the_running_copy() {
    let dir = std::env::temp_dir().join(format!("fogata-cli-running-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (made, said) = fogata(&dir, &["profile", "--name", "Leo"]);
    assert!(made, "{said}");
    let (made, said) = fogata(&dir, &["group", "create", "Crew"]);
    assert!(made, "{said}");

    let running = fogata_core::Store::open(dir.clone())
        .unwrap()
        .hold_running()
        .unwrap();
    for args in [
        &["profile", "--name", "Ana"][..],
        &["friends", "decline", "Bea"],
        &["friends", "remove", "Bea"],
        &["group", "rename", "Crew", "Gang"],
        &["group", "leave", "Crew"],
    ] {
        let (ok, said) = fogata(&dir, args);
        assert!(!ok && said.contains("already running"), "{args:?}: {said}");
    }
    for args in [&["profile"][..], &["friends"], &["group", "list"]] {
        let (ok, said) = fogata(&dir, args);
        assert!(ok, "{args:?}: {said}");
    }
    let (_, said) = fogata(&dir, &["profile"]);
    assert!(said.contains("Leo"), "{said}");

    drop(running);
    let (ok, said) = fogata(&dir, &["profile", "--name", "Ana"]);
    assert!(ok && said.contains("Ana"), "{said}");
    std::fs::remove_dir_all(&dir).unwrap();
}
