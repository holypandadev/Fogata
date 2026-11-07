//! `fogata release` as the publisher runs it: the passphrase of a release key comes from the
//! environment only in the network lab (`release --lab`, in a build with the `lab` feature),
//! and the program that ships has none of the lab's subcommands and flags.

use std::process::{Command, Stdio};

fn folder(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("fogata-cli-release-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn keygen(lab: bool, dir: &std::path::Path) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fogata"));
    command.arg("release");
    if lab {
        command.arg("--lab");
    }
    command
        .args(["keygen", "--dir"])
        .arg(dir)
        .env(
            "FOGATA_RELEASE_PASSPHRASE",
            "a passphrase in the environment",
        )
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn the_passphrase_does_not_come_from_the_environment_without_the_lab_flag() {
    let refused = folder("refused");
    let output = keygen(false, &refused);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("network lab only"), "{stderr}");
    assert!(!refused.join("release-key").exists());
    let _ = std::fs::remove_dir_all(&refused);
}

#[cfg(feature = "lab")]
#[test]
fn the_passphrase_comes_from_the_environment_in_the_lab() {
    let lab = folder("lab");
    let output = keygen(true, &lab);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fogata_update::key::is_sealed(&lab.join("release-key")).unwrap());
    let _ = std::fs::remove_dir_all(&lab);
}

/// What `fogata <args>` says, and whether it ran.
fn fogata(args: &[&str]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_fogata"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Every subcommand and flag of the lab: asked for help, each is there with the `lab`
/// feature and unknown to the program that ships.
const LAB: [&[&str]; 14] = [
    &["lab-dht", "--help"],
    &["lab-liar", "--help"],
    &["lab-flood", "--help"],
    &["lab-silent", "--help"],
    &["lab-mdns-flood", "--help"],
    &["lab-open", "--help"],
    &["lab-plant-release", "--help"],
    &["lab-sign-raw", "--help"],
    &["lab-update-probe", "--help"],
    &["lab-soak", "--help"],
    &["release", "--lab", "keygen", "--help"],
    &["update", "run", "--lab-unchecked", "--help"],
    &["update", "run", "--lab-damage-chunks", "--help"],
    &["media-bridge", "x", "--lab-loss", "0.1", "--help"],
];

#[cfg(not(feature = "lab"))]
#[test]
fn the_program_that_ships_has_no_lab_subcommands_or_flags() {
    for args in LAB {
        let (ran, stderr) = fogata(args);
        assert!(!ran, "{args:?} ran");
        assert!(
            stderr.contains("unrecognized subcommand") || stderr.contains("unexpected argument"),
            "{args:?}: {stderr}"
        );
    }
}

#[cfg(feature = "lab")]
#[test]
fn the_lab_build_has_every_lab_subcommand_and_flag() {
    for args in LAB {
        let (ran, stderr) = fogata(args);
        assert!(ran, "{args:?}: {stderr}");
    }
}

/// `fogata update status` on an empty data folder, with `args` and `env`.
fn status(args: &[&str], env: &[(&str, &str)]) -> std::process::Output {
    let dir = folder("status");
    let mut command = Command::new(env!("CARGO_BIN_EXE_fogata"));
    command
        .arg("--data-dir")
        .arg(&dir)
        .args(["update", "status"])
        .args(args);
    for (key, value) in env {
        command.env(key, value);
    }
    let output = command.stdin(Stdio::null()).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    output
}

/// What a copy counts as (its build, its platform) and the keys it trusts can be changed
/// for tests only in the network lab's copy (the `lab` feature): the copy that ships takes
/// neither the options nor the variables, and trusts the keys compiled in only.
#[test]
fn only_the_labs_copy_counts_as_another_build_or_trusts_other_keys() {
    let key = "00".repeat(32);
    let options: [&[&str]; 4] = [
        &["--build", "77"],
        &["--platform", "windows-x64"],
        &["--release-key", &key],
        &["--rescue-key", &key],
    ];
    for args in options {
        let output = status(args, &[]);
        let said = String::from_utf8_lossy(&output.stderr);
        if cfg!(feature = "lab") {
            assert!(!said.contains("unexpected argument"), "{args:?}: {said}");
        } else {
            assert!(!output.status.success(), "{args:?} taken");
            assert!(said.contains("unexpected argument"), "{args:?}: {said}");
        }
    }
    let output = status(&[], &[("FOGATA_TEST_BUILD", "77")]);
    let said = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{said}");
    assert_eq!(
        said.contains("This copy: build 77 "),
        cfg!(feature = "lab"),
        "{said}"
    );
}
