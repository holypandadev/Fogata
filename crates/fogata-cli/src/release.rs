//! Pre-releases: the publisher makes the release key once, packs and signs each
//! release, and publishes it into a data folder of his, from which the copy running there
//! gives it to friends and members (`fogata release`). Testers see and install what came
//! (`fogata update`). See docs/releasing-beta.md.

use std::{
    collections::BTreeMap,
    io::{BufWriter, Read as _},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use fogata_core::{
    Core, CoreEvent, Store,
    updates::{self, UpdateOptions, UpdateState},
};
#[cfg(feature = "lab")]
use fogata_net::updates::Misbehave;
use fogata_update::{
    Artifact, CHANNEL, KeyRole, Manifest, PLATFORMS, PRODUCT, ReleaseKey, Signed,
    manifest::{self, Builder},
    sums,
};

use zeroize::Zeroizing;

use crate::chat::NetArgs;

#[derive(Args)]
pub struct ReleaseArgs {
    /// Network lab and tests only: take the passphrase of every key from
    /// FOGATA_RELEASE_PASSPHRASE instead of asking for it.
    #[cfg(feature = "lab")]
    #[arg(long, hide = true)]
    lab: bool,
    #[command(subcommand)]
    command: ReleaseCommand,
}

#[derive(Subcommand)]
pub enum ReleaseCommand {
    /// Make the release key: a secret file readable only by you, sealed with a passphrase
    /// asked on the terminal, and its public half to compile into the application. Run
    /// once; keep a copy of the folder somewhere safe and offline. With --rescue, make the
    /// rescue key instead, on a drive kept offline.
    Keygen {
        /// Where to keep it (by default ~/.config/fogata-release; the rescue key has no
        /// default).
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Keep the secret plain (hex), with no passphrase: anyone who reads the file has
        /// the key.
        #[arg(long)]
        plain: bool,
        /// Make the rescue key (rescue-key, rescue-key.pub), which signs only the release
        /// that replaces a release key that leaked or was lost.
        #[arg(long)]
        rescue: bool,
    },
    /// Pack a folder (the Windows application folder, Fogata.app, a Flatpak repository)
    /// into one file for a release.
    Pack {
        folder: PathBuf,
        /// The file to write.
        #[arg(long, short)]
        out: PathBuf,
        /// Check that the folder holds this build of the library (its build marker), and
        /// no other, trusting exactly the keys given with --expect-key and
        /// --expect-rescue-key (none given: no key).
        #[arg(long)]
        expect_build: Option<u64>,
        /// With --expect-build: a release key (hex) the library must trust. Repeatable.
        #[arg(long, requires = "expect_build")]
        expect_key: Vec<String>,
        /// With --expect-build: a rescue key (hex) the library must trust. Repeatable.
        #[arg(long, requires = "expect_build")]
        expect_rescue_key: Vec<String>,
        /// With --expect-build: the commit (40 hex digits) the library must be built from,
        /// with no changes.
        #[arg(long, requires = "expect_build")]
        expect_commit: Option<String>,
    },
    /// Write and sign a release's manifest, with the packages beside it, into a folder.
    Sign {
        /// The release key's secret file.
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        build: u64,
        #[arg(long)]
        version: String,
        /// The oldest build that still works with this one.
        #[arg(long)]
        min_build: u64,
        /// A text file with what changed.
        #[arg(long)]
        notes: Option<PathBuf>,
        /// Days the release is taken for; after that copies no longer take or install it.
        #[arg(long, default_value_t = 180)]
        days: u64,
        /// The folder to write (made if missing).
        #[arg(long)]
        out: PathBuf,
        /// Packages, as platform=file (platforms: linux-flatpak, windows-x64, macos-arm64,
        /// macos-x64).
        #[arg(required = true)]
        packages: Vec<String>,
        /// Also list the files of this folder (the first installs) in SHA256SUMS there and
        /// sign it with the same key, as sign-sums does.
        #[arg(long)]
        sums_of: Option<PathBuf>,
    },
    /// Check a release folder: the signature, the manifest and every package.
    Verify {
        dir: PathBuf,
        #[command(flatten)]
        keys: KeyArgs,
    },
    /// List every file of a folder (the first installs) with its SHA-256 in SHA256SUMS
    /// there, and sign the list (SHA256SUMS.sig, an SSH signature `ssh-keygen -Y verify`
    /// also checks).
    SignSums {
        /// The release key's secret file.
        #[arg(long)]
        key: PathBuf,
        dir: PathBuf,
    },
    /// Check a folder of first installs: the signature of SHA256SUMS by a release key, every
    /// file it lists, and that the folder holds nothing it does not list.
    VerifySums {
        dir: PathBuf,
        #[command(flatten)]
        keys: KeyArgs,
    },
    /// Put a release into this data folder, from where the copy running on it gives it to
    /// friends and members. Close Fogata on that folder first.
    Publish {
        dir: PathBuf,
        #[command(flatten)]
        keys: KeyArgs,
    },
}

#[derive(Subcommand)]
pub enum UpdateCommand {
    /// Where updates stand in this data folder.
    Status {
        #[command(flatten)]
        this: ThisArgs,
    },
    /// Run online for friends and members, taking and giving new versions, and print what
    /// happens.
    Run {
        #[command(flatten)]
        net: NetArgs,
        #[command(flatten)]
        this: ThisArgs,
        /// Stop after this many seconds.
        #[arg(long = "for")]
        seconds: Option<u64>,
        /// Stop once a newer release is here, ready to install (exit 1 if it never is).
        #[arg(long)]
        until_ready: bool,
        /// Network lab only: send damaged chunks.
        #[cfg(feature = "lab")]
        #[arg(long, hide = true)]
        lab_damage_chunks: bool,
        /// Network lab only: offer the release kept whatever the other side has, and keep
        /// it unchecked.
        #[cfg(feature = "lab")]
        #[arg(long, hide = true)]
        lab_unchecked: bool,
    },
    /// Install the newer release that is here, then quit; the new version starts.
    Install {
        #[command(flatten)]
        this: ThisArgs,
    },
}

/// The release keys trusted: the ones compiled in, and with the `lab` feature (the network
/// lab's copy, never the one that ships) test keys besides them. A copy that ships trusts
/// the compiled-in keys only, whatever its command line or environment say.
#[derive(Args, Clone)]
pub struct KeyArgs {
    /// A release public key (hex) to trust besides the compiled-in ones. Repeatable.
    #[cfg(feature = "lab")]
    #[arg(
        long = "release-key",
        env = "FOGATA_RELEASE_KEY",
        value_delimiter = ','
    )]
    release_keys: Vec<String>,
    /// A rescue public key (hex) to trust besides the compiled-in ones. Repeatable.
    #[cfg(feature = "lab")]
    #[arg(long = "rescue-key", env = "FOGATA_RESCUE_KEY", value_delimiter = ',')]
    rescue_keys: Vec<String>,
}

impl KeyArgs {
    fn keys(&self) -> Result<Vec<ReleaseKey>> {
        #[allow(unused_mut)]
        let mut keys = fogata_update::compiled_keys();
        #[cfg(feature = "lab")]
        keys.extend(public_keys(&self.release_keys, &self.rescue_keys)?);
        Ok(keys)
    }
}

/// Release and rescue public keys, each in hex.
fn public_keys(release: &[String], rescue: &[String]) -> Result<Vec<ReleaseKey>> {
    let mut keys = Vec::new();
    for (texts, role) in [(release, KeyRole::Release), (rescue, KeyRole::Rescue)] {
        for text in texts {
            let key = fogata_update::key::parse_public(text)
                .with_context(|| format!("{text} is not a public key"))?;
            keys.push(ReleaseKey { role, key });
        }
    }
    Ok(keys)
}

/// What this copy counts as: the build and platform compiled in, and the keys (see
/// [`KeyArgs`]). With the `lab` feature, for tests, another build or platform.
#[derive(Args, Clone)]
pub struct ThisArgs {
    #[command(flatten)]
    keys: KeyArgs,
    /// Where new versions are looked for besides friends and members: `off`, or the address
    /// of an API like GitHub's followed by the repository (`https://api.github.com
    /// owner/name`). The project's releases page by default.
    #[arg(long, env = "FOGATA_UPDATE_WEB", value_name = "off|API REPO")]
    web: Option<String>,
    #[cfg(feature = "lab")]
    #[arg(long, hide = true, env = "FOGATA_TEST_BUILD")]
    build: Option<u64>,
    #[cfg(feature = "lab")]
    #[arg(long, hide = true, env = "FOGATA_TEST_PLATFORM")]
    platform: Option<String>,
}

impl ThisArgs {
    fn options(&self) -> Result<UpdateOptions> {
        let mut options = UpdateOptions::compiled_in();
        options.keys = self.keys.keys()?;
        match self.web.as_deref().map(str::trim) {
            None => {}
            Some("off") => options.web = None,
            Some(other) => {
                let Some((api, repo)) = other.split_once(char::is_whitespace) else {
                    bail!("--web takes `off`, or an address and a repository: {other}");
                };
                options.web = Some(fogata_core::updates::WebSource {
                    api: api.to_string(),
                    repo: repo.trim().to_string(),
                });
            }
        }
        #[cfg(feature = "lab")]
        {
            if let Some(build) = self.build {
                options.build = build;
            }
            if let Some(platform) = &self.platform {
                options.platform = Some(platform.clone());
            }
        }
        Ok(options)
    }
}

pub async fn release(store: impl FnOnce() -> Result<Store>, args: ReleaseArgs) -> Result<()> {
    // Without the lab feature there is no --lab: passphrases are always asked on the terminal.
    #[cfg(feature = "lab")]
    let lab = args.lab;
    #[cfg(not(feature = "lab"))]
    let lab = false;
    match args.command {
        ReleaseCommand::Keygen { dir, plain, rescue } => {
            let role = if rescue {
                KeyRole::Rescue
            } else {
                KeyRole::Release
            };
            let dir = match dir {
                Some(dir) => dir,
                None if rescue => bail!(
                    "give --dir for the rescue key: a folder on a drive kept offline, apart \
                     from the release key"
                ),
                None => home()?.join(".config/fogata-release"),
            };
            let passphrase = if plain {
                None
            } else {
                let first = passphrase(
                    if rescue {
                        "Passphrase for the rescue key: "
                    } else {
                        "Passphrase for the release key: "
                    },
                    lab,
                )?;
                fogata_update::key::check_passphrase(&first)?;
                if !(lab && std::env::var_os(PASSPHRASE_VAR).is_some()) {
                    let again = passphrase("The same passphrase again: ", lab)?;
                    if *again != *first {
                        bail!("the two passphrases differ");
                    }
                }
                Some(first)
            };
            let public =
                fogata_update::key::generate_as(&dir, role, passphrase.as_deref().map(|p| &**p))?;
            println!(
                "Made the {} key in {} (the secret is {}, readable by you only{}; keep a \
                 copy of the folder somewhere safe and offline).",
                if rescue { "rescue" } else { "release" },
                dir.display(),
                fogata_update::key::files(role).0,
                if plain {
                    ", PLAIN: anyone who reads it has the key"
                } else {
                    ", sealed with the passphrase: without it the file is no key, and there \
                     is no way to recover it"
                }
            );
            println!("Public key: {}", hex::encode(public));
            println!("For crates/fogata-update/src/lib.rs, RELEASE_KEYS:");
            let bytes: Vec<String> = public.iter().map(|b| format!("0x{b:02x}")).collect();
            println!(
                "    ReleaseKey::{}([{}]),",
                if rescue { "rescue" } else { "release" },
                bytes.join(", ")
            );
            println!("For an allowed signers file (ssh-keygen -Y verify of SHA256SUMS):");
            println!("    {}", sums::allowed_signers_line(&public));
        }
        ReleaseCommand::Pack {
            folder,
            out,
            expect_build,
            expect_key,
            expect_rescue_key,
            expect_commit,
        } => {
            if let Some(build) = expect_build {
                let keys = public_keys(&expect_key, &expect_rescue_key)?;
                check_marker(&folder, build, &keys, expect_commit.as_deref())?;
            }
            if out.exists() {
                bail!("{} is there already", out.display());
            }
            let file = std::fs::File::create_new(&out)
                .with_context(|| format!("cannot write {}", out.display()))?;
            let mut writer = BufWriter::new(file);
            let packed = fogata_update::pack::write(&folder, &mut writer)?;
            writer
                .into_inner()
                .map_err(|e| e.into_error())?
                .sync_all()?;
            let size = std::fs::metadata(&out)?.len();
            println!(
                "{}: {} files, {} bytes of them, {size} bytes packed",
                out.display(),
                packed.files,
                packed.bytes
            );
            if size > manifest::MAX_ARTIFACT {
                bail!("that is larger than a release may carry");
            }
        }
        ReleaseCommand::Sign {
            key,
            build,
            version,
            min_build,
            notes,
            days,
            out,
            packages,
            sums_of,
        } => {
            let key = load_key(&key, lab)?;
            // A release out of reach of the build it is made from: no copy would take it (the
            // network lab signs some on purpose).
            if !lab {
                let role = fogata_update::role_of(
                    &fogata_update::compiled_keys(),
                    key.public_key().as_bytes(),
                )
                .unwrap_or(KeyRole::Release);
                reachable(build, role, fogata_update::THIS_BUILD)?;
            }
            sign(
                &key,
                (build, &version, min_build, days),
                notes.as_deref(),
                &out,
                &packages,
            )?;
            if let Some(dir) = sums_of {
                sign_sums(&key, &dir)?;
            }
        }
        ReleaseCommand::SignSums { key, dir } => sign_sums(&load_key(&key, lab)?, &dir)?,
        ReleaseCommand::VerifySums { dir, keys } => verify_sums(&dir, &keys.keys()?)?,
        ReleaseCommand::Verify { dir, keys } => {
            let keys = keys.keys()?;
            let (signed, manifest) = updates::verify_release(&dir, &keys)?;
            for (platform, artifact) in &manifest.artifacts {
                let path = dir.join(&artifact.file);
                if !fogata_net::updates::check_package(&path, artifact)
                    .with_context(|| format!("cannot read {}", path.display()))?
                {
                    bail!("{} is not the package the manifest names", path.display());
                }
                println!(
                    "{platform}: {} ({} bytes) good",
                    artifact.file, artifact.size
                );
            }
            println!(
                "Build {} ({}), signed with the {} key {}: good.",
                manifest.build,
                manifest.version,
                match signed.role(&keys) {
                    Some(KeyRole::Rescue) => "rescue",
                    _ => "release",
                },
                hex::encode(signed.key)
            );
        }
        ReleaseCommand::Publish { dir, keys } => {
            let store = store()?;
            let _running = store.hold_running()?;
            let manifest =
                updates::publish(store.dir(), &dir, &keys.keys()?, fogata_update::THIS_BUILD)?;
            println!(
                "Build {} ({}) is in {}; the copy running on that folder gives it to its \
                 friends and members.",
                manifest.build,
                manifest.version,
                store.dir().display()
            );
        }
    }
    Ok(())
}

/// The release key's passphrase for the network lab and tests, taken only with `release
/// --lab`: a real key's passphrase is asked on the terminal, never kept in the environment.
const PASSPHRASE_VAR: &str = "FOGATA_RELEASE_PASSPHRASE";

/// [`PASSPHRASE_VAR`], when it is set and `lab` lets it count.
fn given_passphrase(lab: bool) -> Option<Zeroizing<String>> {
    let given = std::env::var_os(PASSPHRASE_VAR)?;
    if !lab {
        eprintln!(
            "{PASSPHRASE_VAR} is for the network lab only (release --lab, in a build made for it): not used"
        );
        return None;
    }
    Some(Zeroizing::new(given.to_string_lossy().into_owned()))
}

/// The release key's passphrase: [`PASSPHRASE_VAR`] with `lab`, else asked on the terminal
/// with echo off.
fn passphrase(prompt: &str, lab: bool) -> Result<Zeroizing<String>> {
    if let Some(given) = given_passphrase(lab) {
        return Ok(given);
    }
    read_hidden(prompt).context("cannot read the passphrase from the terminal")
}

/// Reads a line from the terminal without showing it: echo is turned off with `stty` on
/// the terminal itself for as long as the line is typed.
#[cfg(unix)]
fn read_hidden(prompt: &str) -> std::io::Result<Zeroizing<String>> {
    use std::io::{BufRead as _, IsTerminal as _, Write as _};
    // Asked only of someone at the terminal, not of a script or a test with no input.
    if !std::io::stdin().is_terminal() {
        return Err(std::io::Error::other(
            "the passphrase is asked on a terminal, and there is none",
        ));
    }
    let mut tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")?;
    tty.write_all(prompt.as_bytes())?;
    tty.flush()?;
    let echo = |on: bool| -> std::io::Result<()> {
        let status = std::process::Command::new("stty")
            .arg(if on { "echo" } else { "-echo" })
            .stdin(tty.try_clone()?)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("stty cannot set the terminal"))
        }
    };
    echo(false)?;
    let mut line = Zeroizing::new(String::new());
    let read = std::io::BufReader::new(&tty).read_line(&mut line);
    let restored = echo(true);
    tty.write_all(b"\n")?;
    read?;
    restored?;
    while line.ends_with('\n') || line.ends_with('\r') {
        line.pop();
    }
    Ok(line)
}

/// On this system the terminal's echo is not turned off here, so a passphrase is not asked
/// for: it would show on the screen as it is typed.
#[cfg(not(unix))]
fn read_hidden(_prompt: &str) -> std::io::Result<Zeroizing<String>> {
    Err(std::io::Error::other(
        "this command asks for a passphrase only on Linux and macOS, where it can keep it off \
         the screen; make and use release keys there",
    ))
}

/// Opens the release or rescue key at `path`, asking for its passphrase when it is sealed.
fn load_key(path: &Path, lab: bool) -> Result<fogata_crypto::DeviceIdentity> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    fogata_update::key::load(path, || {
        passphrase(&format!("Passphrase of {name}: "), lab).map_err(std::io::Error::other)
    })
    .with_context(|| format!("cannot open the release key {}", path.display()))
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .context("no home folder")
}

/// Checks that `folder` holds build `build` of the library and no other, that the library
/// trusts exactly `keys` (a package built before the keys changed trusts others, and a copy
/// that installed it would never take a release signed by the new ones), and, when
/// `commit` is given, that it was built from that commit with no changes: a package holding
/// an older build would be offered again and again once installed.
fn check_marker(
    folder: &Path,
    build: u64,
    keys: &[ReleaseKey],
    commit: Option<&str>,
) -> Result<()> {
    const BUILD: &[u8] = b"fogata-build-marker:";
    let commit_prefix = fogata_update::COMMIT_MARKER_PREFIX.as_bytes();
    let mut builds = std::collections::BTreeSet::new();
    let mut key_sets = std::collections::BTreeSet::new();
    let mut commits = std::collections::BTreeSet::new();
    let mut stack = vec![folder.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                let mut bytes = Vec::new();
                std::fs::File::open(entry.path())?.read_to_end(&mut bytes)?;
                // The text up to `;` after `prefix`, at most `max` bytes.
                let field = |at: usize, prefix: &[u8], max: usize| {
                    let rest = &bytes[at + prefix.len()..];
                    let end = rest.iter().take(max + 1).position(|b| *b == b';')?;
                    std::str::from_utf8(&rest[..end]).ok().map(str::to_string)
                };
                for at in memmem(&bytes, BUILD) {
                    if let Some(n) = field(at, BUILD, 20).and_then(|n| n.parse::<u64>().ok()) {
                        builds.insert(n);
                    }
                }
                for at in memmem(&bytes, fogata_update::KEYS_MARKER_PREFIX) {
                    if let Some(found) = fogata_update::read_keys_marker(&bytes[at..]) {
                        let mut found: Vec<_> = found
                            .iter()
                            .map(|k| (fogata_update::role_byte(k.role), k.key))
                            .collect();
                        found.sort();
                        key_sets.insert(found);
                    }
                }
                for at in memmem(&bytes, commit_prefix) {
                    if let Some(text) = field(at, commit_prefix, 64) {
                        commits.insert(text);
                    }
                }
            }
        }
    }
    if builds.is_empty() {
        bail!(
            "no Fogata library with a build marker in {}",
            folder.display()
        );
    }
    if builds != std::collections::BTreeSet::from([build]) {
        bail!(
            "{} holds build(s) {builds:?}, not {build}: build it again from the commit that sets THIS_BUILD = {build}",
            folder.display()
        );
    }
    let mut expected: Vec<_> = keys
        .iter()
        .map(|k| (fogata_update::role_byte(k.role), k.key))
        .collect();
    expected.sort();
    expected.dedup();
    let show = |set: &[(u8, [u8; 32])]| {
        set.iter()
            .map(|(role, key)| {
                let role = if *role == b'S' { "rescue" } else { "release" };
                format!("{role} {}", hex::encode(key))
            })
            .collect::<Vec<_>>()
    };
    match key_sets.len() {
        0 => bail!(
            "{} holds no keys marker: it was built before releases named their keys; build \
             it again from this commit",
            folder.display()
        ),
        1 if key_sets.contains(&expected) => {}
        _ => bail!(
            "{} trusts the keys {:?}, not {:?}: build it again from the commit whose \
             RELEASE_KEYS names these",
            folder.display(),
            key_sets.iter().map(|set| show(set)).collect::<Vec<_>>(),
            show(&expected)
        ),
    }
    if let Some(commit) = commit {
        let commit = commit.to_ascii_lowercase();
        if commits != std::collections::BTreeSet::from([commit.clone()]) {
            bail!(
                "{} was built from {commits:?}, not from commit {commit} with no changes: \
                 build it again from that commit",
                folder.display()
            );
        }
    }
    Ok(())
}

fn memmem(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    haystack
        .windows(needle.len())
        .enumerate()
        .filter(|(_, w)| *w == needle)
        .map(|(i, _)| i)
        .collect()
}

fn sign(
    key: &fogata_crypto::DeviceIdentity,
    (build, version, min_build, days): (u64, &str, u64, u64),
    notes: Option<&Path>,
    out: &Path,
    packages: &[String],
) -> Result<()> {
    if days == 0 || days * 24 * 3600 > manifest::MAX_LIFETIME {
        bail!(
            "--days must be between 1 and {}",
            manifest::MAX_LIFETIME / (24 * 3600)
        );
    }
    let notes = match notes {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?
            .trim_end()
            .to_string(),
        None => String::new(),
    };
    std::fs::create_dir_all(out)?;
    let mut artifacts = BTreeMap::new();
    for package in packages {
        let (platform, path) = package
            .split_once('=')
            .with_context(|| format!("{package}: give packages as platform=file"))?;
        if !PLATFORMS.contains(&platform) {
            bail!("{platform} is not a platform ({})", PLATFORMS.join(", "));
        }
        let path = Path::new(path);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .with_context(|| format!("{} has no plain file name", path.display()))?
            .to_string();
        let artifact = describe(path, &name)?;
        let dest = out.join(&name);
        if dest != path && std::fs::canonicalize(&dest).ok() != std::fs::canonicalize(path).ok() {
            std::fs::copy(path, &dest).with_context(|| {
                format!("cannot copy {} into {}", path.display(), out.display())
            })?;
        }
        println!("{platform}: {name}, {} bytes", artifact.size);
        artifacts.insert(platform.to_string(), artifact);
    }
    let published = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let manifest = Manifest {
        format: manifest::FORMAT,
        product: PRODUCT.into(),
        channel: CHANNEL.into(),
        build,
        version: version.into(),
        min_build,
        published,
        expires: published + days * 24 * 3600,
        notes,
        artifacts,
    };
    let signed = Signed::sign(&manifest, key)?;
    // What was signed reads back the same.
    signed.verify(&[ReleaseKey::release(*key.public_key().as_bytes())])?;
    std::fs::write(out.join("release.sig"), signed.signature_file())?;
    std::fs::write(out.join("release.json"), &signed.body)?;
    println!(
        "Signed build {build} ({version}) in {} with key {}.",
        out.display(),
        hex::encode(key.public_key().as_bytes())
    );
    Ok(())
}

/// Refuses to sign build `build` with a key of `role` when a copy of build `runs` (the one
/// signing) would not take it: older, or further above than the role allows.
fn reachable(build: u64, role: KeyRole, runs: u64) -> Result<()> {
    if build < runs {
        bail!("build {build} is older than this program's build ({runs}): no copy would take it");
    }
    let step = match role {
        KeyRole::Release => fogata_update::MAX_BUILD_STEP,
        KeyRole::Rescue => fogata_update::MAX_RESCUE_STEP,
    };
    if build - runs > step {
        bail!(
            "build {build} is more than {step} builds above this program's build ({runs}): no \
             copy would take it; build this program from the commit of the release"
        );
    }
    Ok(())
}

/// Lists every file of `dir` with its SHA-256 in SHA256SUMS there and signs the list with
/// `key` into SHA256SUMS.sig. Refuses anything in it but plain files with plain names.
fn sign_sums(key: &fogata_crypto::DeviceIdentity, dir: &Path) -> Result<()> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == sums::SUMS_FILE || name == sums::SIGNATURE_FILE {
            continue;
        }
        if !entry.file_type()?.is_file() || !sums::plain_name(&name) {
            bail!(
                "{}: the folder must hold only files with plain names (letters, digits, . _ + -)",
                entry.path().display()
            );
        }
        names.push(name);
    }
    names.sort();
    let mut files = Vec::new();
    for name in names {
        let digest = sha256_of(&dir.join(&name))?;
        files.push((name, digest));
    }
    let text = sums::write(&files)?;
    let signature = sums::sign(text.as_bytes(), key);
    let public = *key.public_key().as_bytes();
    sums::verify(text.as_bytes(), &signature, &[ReleaseKey::release(public)])?;
    std::fs::write(dir.join(sums::SUMS_FILE), &text)?;
    std::fs::write(dir.join(sums::SIGNATURE_FILE), &signature)?;
    println!(
        "{} and {} in {}, signed with key {}:",
        sums::SUMS_FILE,
        sums::SIGNATURE_FILE,
        dir.display(),
        hex::encode(public)
    );
    print!("{text}");
    Ok(())
}

/// Checks SHA256SUMS in `dir` against its signature and every file it lists.
fn verify_sums(dir: &Path, keys: &[ReleaseKey]) -> Result<()> {
    let read_small = |name: &str, max: u64| -> Result<Vec<u8>> {
        let path = dir.join(name);
        let mut bytes = Vec::new();
        std::fs::File::open(&path)
            .with_context(|| format!("cannot read {}", path.display()))?
            .take(max + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > max {
            bail!("{} is larger than any list", path.display());
        }
        Ok(bytes)
    };
    let text = read_small(sums::SUMS_FILE, 64 * 1024)?;
    let signature = String::from_utf8(read_small(sums::SIGNATURE_FILE, 4096)?)
        .context("the signature file is not text")?;
    let signer = sums::verify(&text, &signature, keys)?;
    let text = String::from_utf8(text).context("the list is not text")?;
    let files = sums::read(&text)?;
    println!(
        "{}: signed with the {} key {}",
        sums::SUMS_FILE,
        match signer.role {
            KeyRole::Release => "release",
            KeyRole::Rescue => "rescue",
        },
        hex::encode(signer.key)
    );
    let mut bad = 0;
    for (name, digest) in &files {
        let path = dir.join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_file() => {
                if sha256_of(&path)? == *digest {
                    println!("  {name}: good");
                } else {
                    println!("  {name}: DIFFERENT from the signed list");
                    bad += 1;
                }
            }
            _ => {
                println!("  {name}: missing");
                bad += 1;
            }
        }
    }
    // Anything else beside them was not signed, and someone may open it believing it was.
    for entry in std::fs::read_dir(dir)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if name != sums::SUMS_FILE
            && name != sums::SIGNATURE_FILE
            && !files.iter().any(|(listed, _)| *listed == name)
        {
            println!("  {name}: NOT in the signed list");
            bad += 1;
        }
    }
    if bad > 0 {
        bail!("{bad} file(s) are not the ones signed");
    }
    println!("Every file listed is the one signed.");
    Ok(())
}

fn sha256_of(path: &Path) -> Result<[u8; 32]> {
    let file =
        std::fs::File::open(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(sums::sha256(file)?)
}

fn describe(path: &Path, name: &str) -> Result<Artifact> {
    let mut file =
        std::fs::File::open(path).with_context(|| format!("cannot read {}", path.display()))?;
    let mut builder = Builder::new(name);
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        builder.update(&buf[..n]);
    }
    let artifact = builder.finish();
    if artifact.size > manifest::MAX_ARTIFACT {
        bail!("{} is larger than a release may carry", path.display());
    }
    Ok(artifact)
}

pub async fn update(store: Store, command: UpdateCommand) -> Result<()> {
    match command {
        UpdateCommand::Status { this } => {
            let status = updates::status_offline(store.dir(), &this.options()?)?;
            print_status(&status);
        }
        UpdateCommand::Install { this } => {
            let manifest = updates::install_offline(store.dir(), &this.options()?).await?;
            println!(
                "Installing build {} ({}); the new version starts by itself.",
                manifest.build, manifest.version
            );
        }
        UpdateCommand::Run {
            net,
            this,
            seconds,
            until_ready,
            #[cfg(feature = "lab")]
            lab_damage_chunks,
            #[cfg(feature = "lab")]
            lab_unchecked,
        } => {
            #[cfg_attr(not(feature = "lab"), allow(unused_mut))]
            let mut options = this.options()?;
            #[cfg(feature = "lab")]
            options.lab(
                Misbehave {
                    damage_chunks: lab_damage_chunks,
                    offer_always: lab_unchecked,
                },
                lab_unchecked,
            );
            run(store, net, options, seconds, until_ready).await?;
        }
    }
    Ok(())
}

fn print_status(status: &updates::UpdateStatus) {
    println!(
        "This copy: build {} ({})",
        status.this_build, status.this_version
    );
    match &status.release {
        Some(r) => println!(
            "Newest release here: build {} ({}), works with build {} and later",
            r.build, r.version, r.min_build
        ),
        None => println!("Newest release here: none"),
    }
    println!("State: {}", state_text(&status.state));
    if status.required {
        println!("This copy must update to keep talking to copies of the newest release.");
    }
}

fn state_text(state: &UpdateState) -> String {
    match state {
        UpdateState::UpToDate => "up to date".into(),
        UpdateState::Checking => "checking".into(),
        UpdateState::Downloading { done, total } => format!("downloading {done} of {total} bytes"),
        UpdateState::Verifying => "verifying".into(),
        UpdateState::Ready => "ready to install".into(),
        UpdateState::Installing => "installing".into(),
        UpdateState::Failed { kind, message } => format!("failed ({kind:?}): {message}"),
    }
}

async fn run(
    store: Store,
    net: NetArgs,
    options: UpdateOptions,
    seconds: Option<u64>,
    until_ready: bool,
) -> Result<()> {
    let (core, mut events) = Core::start(store.dir().to_path_buf(), net.options())?;
    core.start_updates(options)?;
    say!("· online for friends and members, taking and giving new versions");
    let deadline = seconds.map(|s| Instant::now() + Duration::from_secs(s));
    let mut shown_state = String::new();
    let mut shown_notes = 0usize;
    let mut ready = false;
    loop {
        let wait = async {
            match deadline {
                Some(at) => tokio::time::sleep_until(at.into()).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            _ = wait => break,
            event = events.recv() => {
                match event {
                    None => {
                        say!("· the core stopped");
                        break;
                    }
                    Some(CoreEvent::FriendOnline { device }) => {
                        say!("· friend {} online", device.fingerprint());
                    }
                    Some(_) => {}
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(500)) => {}
        }
        let notes = core.update_notes();
        if notes.len() < shown_notes {
            shown_notes = 0;
        }
        for note in &notes[shown_notes..] {
            let text = note.split_once(' ').map(|(_, t)| t).unwrap_or(note);
            say!("update: {text}");
        }
        shown_notes = notes.len();
        if let Some(status) = core.update_status() {
            let text = state_text(&status.state);
            let shown = match &status.state {
                // Progress in steps of a tenth, not every chunk.
                UpdateState::Downloading { done, total } if *total > 0 => {
                    format!("downloading {}0 %", done * 10 / total)
                }
                _ => text.clone(),
            };
            if shown != shown_state {
                say!("state: {text}");
                shown_state = shown;
            }
            if status.state == UpdateState::Ready {
                ready = true;
                if until_ready {
                    break;
                }
            }
        }
    }
    core.shutdown().await?;
    if until_ready && !ready {
        bail!("no newer release became ready");
    }
    Ok(())
}

/// Network lab only: put a release folder into a data folder as it is, unchecked.
#[cfg(feature = "lab")]
pub fn plant(store: Store, from: &Path) -> Result<()> {
    updates::plant_unchecked(store.dir(), from)?;
    println!("planted the release of {} unchecked", from.display());
    Ok(())
}

/// Network lab only: sign `body` as it is, with no check of what it says, into `out` (to
/// see a copy refuse what a release may not hold).
#[cfg(feature = "lab")]
pub fn sign_raw(key: &Path, body: &Path, out: &Path) -> Result<()> {
    use std::io::Write as _;
    let key = load_key(key, true)?;
    let body = std::fs::read(body)?;
    let mut message = manifest::SIGNING_LABEL.to_vec();
    message.extend_from_slice(&body);
    let signed = Signed {
        signature: key.sign(&message),
        key: *key.public_key().as_bytes(),
        body,
    };
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("release.sig"), signed.signature_file())?;
    let mut file = std::fs::File::create(out.join("release.json"))?;
    file.write_all(&signed.body)?;
    Ok(())
}

/// Network lab only: a device that is neither friend nor member dials `peers` and asks each
/// device that takes the update protocol for a release and for a package; prints what it
/// got.
#[cfg(feature = "lab")]
pub async fn probe(net: NetArgs, platform: String) -> Result<()> {
    let identity = fogata_crypto::DeviceIdentity::generate()?;
    let (node, _events) = fogata_net::start(net.node_config(identity, Vec::new(), None))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    // A stranger is not told what the device it reached takes: it asks all the same, as
    // anyone who knows the protocol's name can.
    let peer = loop {
        if let Some(peer) = node.identified_peers().into_iter().next() {
            break peer;
        }
        if Instant::now() > deadline {
            bail!("reached no device");
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    };
    say!("· connected to {}", fogata_net::short_peer(&peer));
    match fogata_net::updates::ask(&node, peer, 0, &platform).await {
        Ok(Some(_)) => say!("ask: offered a release"),
        Ok(None) => say!("ask: nothing"),
        Err(e) => say!("ask: refused ({e})"),
    }
    let artifact = Artifact::of("probe", b"probe");
    let part = std::env::temp_dir().join(format!("fogata-probe-{}.part", std::process::id()));
    let result =
        fogata_net::updates::fetch(&node, peer, 0, &platform, &artifact, &part, |_| {}).await;
    let _ = std::fs::remove_file(&part);
    match result {
        Ok(()) => say!("fetch: got a package"),
        Err(e) => say!("fetch: refused ({e})"),
    }
    node.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    /// A folder holding a library with the markers of `build`, `keys` and `commit`.
    fn package(name: &str, build: u64, keys: &[ReleaseKey], commit: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fogata-release-marker-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("lib")).unwrap();
        let mut library = b"\x7fELF... code ...".to_vec();
        library.extend_from_slice(format!("fogata-build-marker:{build};").as_bytes());
        library.extend_from_slice(b"... more code ...");
        library.extend_from_slice(fogata_update::KEYS_MARKER_PREFIX);
        library.push(keys.len() as u8);
        for key in keys {
            library.push(fogata_update::role_byte(key.role));
            library.extend_from_slice(&key.key);
        }
        library.push(b';');
        library.extend_from_slice(format!("fogata-commit-marker:{commit};").as_bytes());
        std::fs::write(dir.join("lib/libfogata_ffi.so"), library).unwrap();
        std::fs::write(dir.join("fogata"), b"the runner").unwrap();
        dir
    }

    fn key() -> [u8; 32] {
        *fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key()
            .as_bytes()
    }

    #[test]
    fn a_package_must_trust_the_keys_of_the_release() {
        let (release, rescue, other) = (key(), key(), key());
        let keys = [ReleaseKey::release(release), ReleaseKey::rescue(rescue)];
        let good = package("good", 2, &keys, COMMIT);
        check_marker(&good, 2, &keys, Some(COMMIT)).unwrap();
        // The order the keys are given in does not matter.
        check_marker(&good, 2, &[keys[1], keys[0]], None).unwrap();
        // The right build with other keys: built before the keys changed.
        let stale = package(
            "stale",
            2,
            &[ReleaseKey::release(other), ReleaseKey::rescue(rescue)],
            COMMIT,
        );
        let error = check_marker(&stale, 2, &keys, Some(COMMIT)).unwrap_err();
        assert!(error.to_string().contains("trusts the keys"), "{error}");
        // No keys at all, a key more, a key in another role.
        let none = package("none", 2, &[], COMMIT);
        assert!(check_marker(&none, 2, &keys, None).is_err());
        check_marker(&none, 2, &[], None).unwrap();
        let more = package(
            "more",
            2,
            &[keys[0], keys[1], ReleaseKey::release(other)],
            COMMIT,
        );
        assert!(check_marker(&more, 2, &keys, None).is_err());
        let swapped = package(
            "swapped",
            2,
            &[ReleaseKey::rescue(release), ReleaseKey::release(rescue)],
            COMMIT,
        );
        assert!(check_marker(&swapped, 2, &keys, None).is_err());
        // The build still counts.
        assert!(check_marker(&good, 3, &keys, None).is_err());
        for dir in [good, stale, none, more, swapped] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn a_package_must_come_from_the_commit_released() {
        let keys = [ReleaseKey::release(key()), ReleaseKey::rescue(key())];
        let other = package(
            "other",
            2,
            &keys,
            "fedcba9876543210fedcba9876543210fedcba98",
        );
        assert!(check_marker(&other, 2, &keys, Some(COMMIT)).is_err());
        check_marker(&other, 2, &keys, None).unwrap();
        let dirty = package("dirty", 2, &keys, &format!("{COMMIT}+dirty"));
        assert!(check_marker(&dirty, 2, &keys, Some(COMMIT)).is_err());
        let unknown = package("unknown", 2, &keys, "unknown");
        assert!(check_marker(&unknown, 2, &keys, Some(COMMIT)).is_err());
        // Upper case given is the same commit.
        let good = package("upper", 2, &keys, COMMIT);
        check_marker(&good, 2, &keys, Some(&COMMIT.to_ascii_uppercase())).unwrap();
        for dir in [other, dirty, unknown, good] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// A file put beside the first installs after they were signed is not taken as one of
    /// them.
    #[test]
    fn a_file_the_signed_list_does_not_name_fails_the_check() {
        let key = fogata_crypto::DeviceIdentity::generate().unwrap();
        let keys = [ReleaseKey::release(*key.public_key().as_bytes())];
        let dir = std::env::temp_dir().join(format!("fogata-release-sums-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Fogata-0.1.0-beta.2-linux.flatpak"), b"flatpak").unwrap();
        std::fs::write(dir.join("README.txt"), b"readme").unwrap();
        sign_sums(&key, &dir).unwrap();
        verify_sums(&dir, &keys).unwrap();
        std::fs::write(
            dir.join("Fogata-0.1.0-beta.2-windows-x64.zip"),
            b"not signed",
        )
        .unwrap();
        assert!(verify_sums(&dir, &keys).is_err());
        std::fs::remove_file(dir.join("Fogata-0.1.0-beta.2-windows-x64.zip")).unwrap();
        verify_sums(&dir, &keys).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_release_is_signed_only_within_reach_of_this_build() {
        let step = fogata_update::MAX_BUILD_STEP;
        reachable(5, KeyRole::Release, 5).unwrap();
        reachable(5 + step, KeyRole::Release, 5).unwrap();
        assert!(reachable(5 + step + 1, KeyRole::Release, 5).is_err());
        reachable(5 + step + 1, KeyRole::Rescue, 5).unwrap();
        assert!(reachable(5 + 2 * step + 1, KeyRole::Rescue, 5).is_err());
        assert!(reachable(4, KeyRole::Release, 5).is_err());
    }

    /// A library from before the keys marker existed.
    #[test]
    fn a_package_with_no_keys_marker_is_refused() {
        let dir =
            std::env::temp_dir().join(format!("fogata-release-marker-old-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lib.so"), b"xx fogata-build-marker:2; yy").unwrap();
        let error = check_marker(&dir, 2, &[], None).unwrap_err();
        assert!(error.to_string().contains("no keys marker"), "{error}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
