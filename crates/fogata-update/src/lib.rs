//! Releases of Fogata as they travel between friends, with no server: a release is a manifest
//! signed by a release key (an Ed25519 key only the publisher holds, whose public half is
//! compiled in here), naming one package per platform by size and BLAKE3 hash, with the hash of
//! every 1 MiB chunk so that a damaged chunk is caught as it arrives. A copy takes a release
//! only when the signature is good, the key is one it knows, the build is newer than every
//! build it has or runs but not too far above the one it runs, and it was not made before the
//! release it holds ([`manifest`]). The packages of Windows and macOS, and the Flatpak
//! repository of Linux, travel as one file each in a simple format of this crate ([`pack`]).
//! The first installs are listed with their SHA-256 in a file signed by the same key
//! ([`sums`]).
//!
//! Nothing here reaches the network; the files it reads and writes are the ones it is given.

#![forbid(unsafe_code)]

pub mod key;
pub mod manifest;
pub mod pack;
pub mod sums;

pub use manifest::{Artifact, Manifest, Refusal, Signed};

/// What this build of Fogata is, as releases count it: a number that only goes up from one
/// release to the next, and the version people see.
pub const THIS_BUILD: u64 = 1;
pub const THIS_VERSION: &str = "0.1.0-beta.1";

/// The application releases are for, and the channel this build follows.
pub const PRODUCT: &str = "dev.holypanda.fogata";
pub const CHANNEL: &str = "beta";

/// What a release key is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyRole {
    /// Signs every release.
    Release,
    /// Kept offline, and used only when a release key leaked or was lost: a release it signs
    /// may go further above the build a copy runs ([`MAX_RESCUE_STEP`]) than any release of a
    /// release key, so it is taken over whatever a leaked release key made a copy hold, and
    /// the build in it trusts a new release key in place of the old one.
    Rescue,
}

/// A public key this build trusts, and what for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReleaseKey {
    pub role: KeyRole,
    pub key: [u8; 32],
}

impl ReleaseKey {
    pub const fn release(key: [u8; 32]) -> ReleaseKey {
        ReleaseKey {
            role: KeyRole::Release,
            key,
        }
    }

    pub const fn rescue(key: [u8; 32]) -> ReleaseKey {
        ReleaseKey {
            role: KeyRole::Rescue,
            key,
        }
    }
}

/// The keys this build trusts: the public halves of the Ed25519 keys that sign releases
/// (`fogata release keygen` prints the line for one, `--rescue` for the rescue key). Either
/// none, and no release is ever taken, or at least one of each role.
pub const RELEASE_KEYS: &[ReleaseKey] = &[];

/// How far above the build a copy runs a release signed by a release key may go: a key that
/// leaked can make copies hold a release at most this far ahead, never one no later release
/// could pass.
pub const MAX_BUILD_STEP: u64 = 1000;
/// How far above the build a copy runs a release signed by the rescue key may go: a rescue
/// release numbered [`MAX_BUILD_STEP`] + 1 above the last good build is above anything a leaked
/// release key can have made a copy hold, and still taken by copies up to
/// [`MAX_BUILD_STEP`] - 1 builds behind.
pub const MAX_RESCUE_STEP: u64 = 2 * MAX_BUILD_STEP;

/// The role of `key` among `trusted`, if it is one of them.
pub fn role_of(trusted: &[ReleaseKey], key: &[u8; 32]) -> Option<KeyRole> {
    trusted.iter().find(|k| k.key == *key).map(|k| k.role)
}

/// Bytes the release tooling looks for in a built library, to make sure a package holds
/// the build its manifest names (`fogata release pack --expect-build`): a package that held
/// an older build would be offered again and again once installed.
pub const BUILD_MARKER: &str = concat!("fogata-build-marker:", "1", ";");

/// What starts [`KEYS_MARKER`].
pub const KEYS_MARKER_PREFIX: &[u8] = b"fogata-keys-marker:";
const KEYS_MARKER_LEN: usize = KEYS_MARKER_PREFIX.len() + 1 + 33 * RELEASE_KEYS.len() + 1;

/// [`RELEASE_KEYS`] as bytes the release tooling finds in a built library, and the copy of
/// them the library trusts ([`compiled_keys`]): the prefix, the number of keys, then each
/// key as its role (`R` release, `S` rescue) and its 32 bytes, then `;`. A package built
/// before the keys changed holds other keys, and is refused even when its build is right.
pub const KEYS_MARKER: [u8; KEYS_MARKER_LEN] = keys_marker();

const fn keys_marker() -> [u8; KEYS_MARKER_LEN] {
    assert!(RELEASE_KEYS.len() < 256);
    let mut out = [0u8; KEYS_MARKER_LEN];
    let mut at = 0;
    while at < KEYS_MARKER_PREFIX.len() {
        out[at] = KEYS_MARKER_PREFIX[at];
        at += 1;
    }
    out[at] = RELEASE_KEYS.len() as u8;
    at += 1;
    let mut k = 0;
    while k < RELEASE_KEYS.len() {
        out[at] = role_byte(RELEASE_KEYS[k].role);
        at += 1;
        let mut i = 0;
        while i < 32 {
            out[at] = RELEASE_KEYS[k].key[i];
            at += 1;
            i += 1;
        }
        k += 1;
    }
    out[at] = b';';
    out
}

/// How a key's role is written in [`KEYS_MARKER`].
pub const fn role_byte(role: KeyRole) -> u8 {
    match role {
        KeyRole::Release => b'R',
        KeyRole::Rescue => b'S',
    }
}

/// Reads the keys out of the bytes of a keys marker that start at its prefix: `None` when
/// they are not one. Also the release tooling's reader.
pub fn read_keys_marker(bytes: &[u8]) -> Option<Vec<ReleaseKey>> {
    let rest = bytes.strip_prefix(KEYS_MARKER_PREFIX)?;
    let (&count, rest) = rest.split_first()?;
    let len = 33 * count as usize;
    if rest.get(len) != Some(&b';') {
        return None;
    }
    rest[..len]
        .as_chunks::<33>()
        .0
        .iter()
        .map(|chunk| {
            let role = match chunk[0] {
                b'R' => KeyRole::Release,
                b'S' => KeyRole::Rescue,
                _ => return None,
            };
            Some(ReleaseKey {
                role,
                key: chunk[1..].try_into().ok()?,
            })
        })
        .collect()
}

/// The keys this build trusts, read from [`KEYS_MARKER`] as the library holds it: what the
/// release tooling checks in a package is what the package trusts.
pub fn compiled_keys() -> Vec<ReleaseKey> {
    read_keys_marker(std::hint::black_box(&KEYS_MARKER[..]))
        .expect("the keys marker is made from RELEASE_KEYS")
}

/// What starts [`COMMIT_MARKER`].
pub const COMMIT_MARKER_PREFIX: &str = "fogata-commit-marker:";

/// The commit a release build of the library was made from, for the release tooling: the
/// prefix, the commit's 40 hex digits (`+dirty` after them when files the build reads had
/// changes not committed, or a new one that git does not keep yet), `;`. Debug builds, and builds made where git cannot say, write
/// `unknown` in place of the commit.
pub const COMMIT_MARKER: &str = concat!("fogata-commit-marker:", env!("FOGATA_COMMIT"), ";");

/// The platforms releases have packages for. A copy fetches only its own; a name it does not
/// know (from a later manifest) is carried but never fetched.
pub const PLATFORMS: &[&str] = &["linux-flatpak", "windows-x64", "macos-arm64", "macos-x64"];

/// The platform of this build, if releases have one for it.
pub fn this_platform() -> Option<&'static str> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("linux-flatpak")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Some("windows-x64")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("macos-arm64")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("macos-x64")
    } else {
        None
    }
}

/// Keeps [`BUILD_MARKER`] in the compiled library, where the release tooling finds it.
#[inline(never)]
pub fn build_marker() -> &'static str {
    std::hint::black_box(BUILD_MARKER)
}

/// Keeps [`COMMIT_MARKER`] in the compiled library, where the release tooling finds it.
#[inline(never)]
pub fn commit_marker() -> &'static str {
    std::hint::black_box(COMMIT_MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marker_names_this_build() {
        assert_eq!(build_marker(), format!("fogata-build-marker:{THIS_BUILD};"));
        assert!(THIS_VERSION.len() <= manifest::MAX_VERSION_LEN);
        let commit = commit_marker()
            .strip_prefix(COMMIT_MARKER_PREFIX)
            .and_then(|rest| rest.strip_suffix(';'))
            .unwrap();
        assert!(commit == "unknown" || commit.len() >= 40, "{commit}");
    }

    /// The macOS updater, which installs a new version only within the steps a release may
    /// take, knows the greatest of them.
    #[test]
    fn the_macos_updater_allows_the_rescue_step() {
        let checks = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../app/macos/Updater/Checks.swift"
        ))
        .unwrap();
        assert!(
            checks.contains(&format!(
                "static let maxBuildStep: UInt64 = {MAX_RESCUE_STEP}\n"
            )),
            "set maxBuildStep in app/macos/Updater/Checks.swift to MAX_RESCUE_STEP"
        );
    }

    /// The keys a build trusts are the ones RELEASE_KEYS names, read back from the bytes
    /// the release tooling checks; anything else in those bytes is no marker.
    #[test]
    fn the_keys_marker_holds_the_keys_compiled_in() {
        assert_eq!(compiled_keys(), RELEASE_KEYS);
        let keys = [ReleaseKey::release([7; 32]), ReleaseKey::rescue([9; 32])];
        let mut marker = KEYS_MARKER_PREFIX.to_vec();
        marker.push(2);
        for key in keys {
            marker.push(role_byte(key.role));
            marker.extend_from_slice(&key.key);
        }
        marker.push(b';');
        assert_eq!(read_keys_marker(&marker), Some(keys.to_vec()));
        let mut extra = marker.clone();
        extra.extend_from_slice(b"anything after");
        assert_eq!(read_keys_marker(&extra), Some(keys.to_vec()));
        assert_eq!(read_keys_marker(&marker[..marker.len() - 1]), None);
        let mut wrong_role = marker.clone();
        wrong_role[KEYS_MARKER_PREFIX.len() + 1] = b'X';
        assert_eq!(read_keys_marker(&wrong_role), None);
        let mut short = marker.clone();
        short[KEYS_MARKER_PREFIX.len()] = 3;
        assert_eq!(read_keys_marker(&short), None);
    }

    /// The application's version (app/pubspec.yaml, which the Windows and macOS builds name
    /// their files with) is this build's: THIS_VERSION without its pre-release part, and
    /// THIS_BUILD as its build number (0.1.0-beta.1 and build 1 make 0.1.0+1).
    #[test]
    fn the_application_version_is_this_build() {
        let pubspec = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../app/pubspec.yaml"
        ))
        .unwrap();
        let version = pubspec
            .lines()
            .find_map(|line| line.strip_prefix("version:"))
            .expect("app/pubspec.yaml names a version")
            .trim();
        let release = THIS_VERSION.split(['-', '+']).next().unwrap();
        assert_eq!(
            version,
            format!("{release}+{THIS_BUILD}"),
            "set version in app/pubspec.yaml to {release}+{THIS_BUILD}, as THIS_VERSION and \
             THIS_BUILD say"
        );
    }

    /// A build that trusts a release key trusts a rescue key too, so a release key that
    /// leaks can be replaced; no key has two roles.
    #[test]
    fn the_keys_compiled_in_have_both_roles_or_none() {
        check_keys(RELEASE_KEYS).unwrap();
        let key = || {
            *fogata_crypto::DeviceIdentity::generate()
                .unwrap()
                .public_key()
                .as_bytes()
        };
        let (a, b) = (key(), key());
        assert!(check_keys(&[]).is_ok());
        assert!(check_keys(&[ReleaseKey::release(a), ReleaseKey::rescue(b)]).is_ok());
        assert!(check_keys(&[ReleaseKey::release(a)]).is_err());
        assert!(check_keys(&[ReleaseKey::rescue(b)]).is_err());
        assert!(check_keys(&[ReleaseKey::release(a), ReleaseKey::rescue(a)]).is_err());
        assert!(
            check_keys(&[
                ReleaseKey::release(a),
                ReleaseKey::release(b),
                ReleaseKey::rescue(key())
            ])
            .is_ok()
        );
    }

    fn check_keys(keys: &[ReleaseKey]) -> Result<(), String> {
        let has = |role| keys.iter().any(|k| k.role == role);
        if !keys.is_empty() && !(has(KeyRole::Release) && has(KeyRole::Rescue)) {
            return Err("RELEASE_KEYS needs a release key and a rescue key".into());
        }
        let mut seen = std::collections::HashSet::new();
        if !keys.iter().all(|k| seen.insert(k.key)) {
            return Err("a key is named twice in RELEASE_KEYS".into());
        }
        if keys
            .iter()
            .any(|k| fogata_crypto::DevicePublicKey::from_bytes(&k.key).is_err())
        {
            return Err("a key in RELEASE_KEYS is not an Ed25519 public key".into());
        }
        Ok(())
    }
}
