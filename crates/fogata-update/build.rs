//! Writes the commit a release build is made from into FOGATA_COMMIT, for the library's
//! commit marker (`fogata_update::COMMIT_MARKER`). Release builds only: they run again when
//! the commit or the sources change; a debug build says `unknown` and never runs git.

#![forbid(unsafe_code)]

use std::{path::Path, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let release = std::env::var("PROFILE").as_deref() == Ok("release");
    let commit = if release { commit() } else { None };
    println!(
        "cargo:rustc-env=FOGATA_COMMIT={}",
        commit.as_deref().unwrap_or("unknown")
    );
}

/// The native parts of the application, from the top of the checkout.
const NATIVE: [&str; 4] = ["app/linux", "app/windows", "app/macos", "app/third_party"];

/// What to watch of [`NATIVE`]: every file git keeps or does not ignore, and every folder
/// with nothing ignored below it, so a new file there counts too. A folder with something
/// ignored in it is left to its files: cargo watches a folder by everything below it, and
/// what a build writes there (Flutter's ephemeral folders, the unpacked libwebrtc) would run
/// this again every time.
fn native_sources(top: &str, git: &dyn Fn(&[&str]) -> Option<String>) -> Option<Vec<String>> {
    let list = |extra: &[&str]| -> Option<Vec<String>> {
        let mut args = vec!["-C", top, "ls-files", "-z", "--exclude-standard"];
        args.extend(extra);
        args.push("--");
        args.extend(NATIVE);
        let text = git(&args)?;
        Some(
            text.split('\0')
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
        )
    };
    let files = list(&["--cached", "--others"])?;
    let ignored = list(&["--others", "--ignored", "--directory"])?;
    let mut folders = std::collections::BTreeSet::new();
    for file in &files {
        for folder in Path::new(file).ancestors().skip(1) {
            let folder = folder.to_string_lossy();
            if NATIVE
                .iter()
                .any(|root| folder == *root || folder.starts_with(&format!("{root}/")))
            {
                folders.insert(folder.into_owned());
            }
        }
    }
    let mut watched = files;
    watched.extend(folders.into_iter().filter(|folder| {
        !ignored
            .iter()
            .any(|path| path.starts_with(&format!("{folder}/")))
    }));
    Some(watched)
}

/// The commit checked out, with `+dirty` when the files the library is built from differ
/// from it (a file git does not keep yet counts); `None` outside a git checkout.
fn commit() -> Option<String> {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let git = |args: &[&str]| -> Option<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&manifest)
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    let top = git(&["rev-parse", "--show-toplevel"])?;
    let sha = git(&["rev-parse", "--verify", "HEAD^{commit}"])?;
    // Run again when HEAD moves (a commit, a checkout) or what the library is built from
    // changes. A path that does not exist would run this on every build, so only the ones
    // there are named.
    let mut watched = vec![
        git(&["rev-parse", "--path-format=absolute", "--git-path", "HEAD"])?,
        git(&[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "packed-refs",
        ])?,
    ];
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
        watched.push(git(&[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            &branch,
        ])?);
    }
    // What a release is built from: the Rust code, the Flutter application and its native
    // runners and plugins, and the Rust toolchain.
    let sources = [
        "crates",
        "vendor",
        "app",
        "Cargo.toml",
        "Cargo.lock",
        ".cargo",
        "rust-toolchain.toml",
    ];
    // `app` holds build output and downloads that change on every build (a Flutter build
    // writes app/linux/flutter/ephemeral, the libwebrtc download is unpacked into
    // app/third_party) and would run this again each time, so only its sources are watched
    // (git status below looks at all of it): the Dart code, and the files and folders of the
    // native parts that git keeps or does not ignore.
    let app_watched = ["app/lib", "app/pubspec.yaml", "app/pubspec.lock"];
    watched.extend(
        sources
            .iter()
            .filter(|name| **name != "app")
            .chain(&app_watched)
            .map(|name| format!("{top}/{name}")),
    );
    watched.extend(
        native_sources(&top, &git)?
            .into_iter()
            .map(|name| format!("{top}/{name}")),
    );
    for path in watched {
        if Path::new(&path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    // Paths from the top of the checkout, not from this crate's folder.
    let mut status = vec![
        "-C",
        top.as_str(),
        "status",
        "--porcelain",
        // A source file not added to git is built in all the same.
        "--untracked-files=normal",
        "--",
    ];
    status.extend(sources);
    let dirty = !git(&status)?.is_empty();
    Some(if dirty { format!("{sha}+dirty") } else { sha })
}
