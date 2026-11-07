//! `fogata group join` and `fogata friends add` take the link from their input, so it shows
//! neither in the list of processes nor in the shell's history.

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
fn links_are_read_from_the_input_when_not_given() {
    let dir = std::env::temp_dir().join(format!("fogata-cli-links-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (made, _) = fogata(&dir, &["profile", "--name", "Leo"], "");
    assert!(made);
    // A link that is not one goes no further than its check.
    let (ok, said) = fogata(
        &dir,
        &["group", "join", "--no-dht", "--no-mdns"],
        "not an invite\n",
    );
    assert!(!ok && said.contains("Invite:"), "{said}");
    assert!(!said.contains("required"), "{said}");
    let (ok, said) = fogata(
        &dir,
        &["friends", "add", "--no-dht", "--no-mdns"],
        "fogata:not-a-friend-link\n",
    );
    assert!(
        !ok && said.contains("Link or code") && said.contains("fogata group join"),
        "{said}"
    );
    // Nothing typed: nothing done.
    let (ok, said) = fogata(&dir, &["group", "join", "--no-dht", "--no-mdns"], "");
    assert!(!ok && said.contains("nothing was typed"), "{said}");
    std::fs::remove_dir_all(&dir).unwrap();
}
