//! `fogata transfer` asks before what cannot be undone, and takes the code from its input.

use std::{
    io::Write,
    process::{Command, Stdio},
};

fn fogata(dir: &std::path::Path, args: &[&str], input: &str) -> (bool, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fogata"))
        .arg("--data-dir")
        .arg(dir)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

#[test]
fn replacing_a_profile_or_keeping_a_moving_account_is_asked_first() {
    let dir = std::env::temp_dir().join(format!("fogata-cli-transfer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (made, _) = fogata(&dir, &["profile", "--name", "Leo"], "");
    assert!(made);
    // A no leaves the profile as it is, and shows no code.
    let (ok, said) = fogata(
        &dir,
        &["transfer", "receive", "--no-dht", "--no-mdns"],
        "n\n",
    );
    assert!(
        !ok && said.contains("replaces Leo") && said.contains("nothing changed"),
        "{said}"
    );
    assert!(!said.contains("Code:"));
    // A folder that is not moving has nothing to keep.
    let (ok, said) = fogata(&dir, &["transfer", "keep"], "n\n");
    assert!(!ok && said.contains("nothing changed"), "{said}");
    // The code is read from the input when not given; a mistyped one goes no further.
    let (ok, said) = fogata(
        &dir,
        &["transfer", "send", "--no-dht", "--no-mdns"],
        "ABCDE-FGHJK\n",
    );
    assert!(
        !ok && said.contains("Code shown on the new computer"),
        "{said}"
    );
    assert!(said.contains("whole code"), "{said}");
    std::fs::remove_dir_all(&dir).unwrap();
}
