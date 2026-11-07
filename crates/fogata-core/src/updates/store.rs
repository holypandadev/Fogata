//! Where releases are kept in the data folder: `updates/`, with the newest release checked
//! (`release.json`, its exact signed bytes, and `release.sig`), its complete packages
//! (`<platform>.pack`) and those on their way (`<platform>.pack.part`), and `installed.json`
//! once an install has started. Nothing in it is secret. Earlier versions of the application
//! never look there.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use fogata_update::{Manifest, Refusal, ReleaseKey, Signed};
use serde::{Deserialize, Serialize};

pub(crate) const DIR: &str = "updates";
const MANIFEST: &str = "release.json";
const SIGNATURE: &str = "release.sig";
const INSTALLED: &str = "installed.json";
/// There while the release kept was put here by `fogata release publish`: this copy is the
/// publisher's and keeps every platform's package.
const PUBLISHED: &str = "published";

/// The `updates/` folder of data folder `data`.
pub(crate) fn dir(data: &Path) -> PathBuf {
    data.join(DIR)
}

pub(crate) fn package_path(dir: &Path, platform: &str) -> PathBuf {
    dir.join(format!("{platform}.pack"))
}

pub(crate) fn part_path(dir: &Path, platform: &str) -> PathBuf {
    dir.join(format!("{platform}.pack.part"))
}

/// The release kept, as its files hold it, unchecked.
pub(crate) fn read_signed(dir: &Path) -> Result<Option<Signed>> {
    let body = match read_small(&dir.join(MANIFEST), fogata_update::manifest::MAX_MANIFEST) {
        Ok(body) => body,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let signature = read_small(&dir.join(SIGNATURE), 1024)?;
    Ok(Some(Signed::from_files(body, &signature)?))
}

/// Reads a file of at most `max` bytes.
fn read_small(path: &Path, max: usize) -> io::Result<Vec<u8>> {
    use io::Read as _;
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(max as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} is too large", path.display()),
        ));
    }
    Ok(bytes)
}

/// Keeps `signed` as the release, in place of any other, and removes the packages of the one
/// it replaces.
pub(crate) fn keep(dir: &Path, signed: &Signed) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("cannot make {}", dir.display()))?;
    let replaced = read_signed(dir).ok().flatten();
    if replaced.as_ref().map(|r| &r.body) != Some(&signed.body) {
        remove_packages(dir, None)?;
        let _ = fs::remove_file(dir.join(PUBLISHED));
    }
    write_new(&dir.join(SIGNATURE), signed.signature_file().as_bytes())?;
    write_new(&dir.join(MANIFEST), &signed.body)?;
    Ok(())
}

/// Removes every package and partial package, except those of `keep`'s platforms.
pub(crate) fn remove_packages(dir: &Path, keep: Option<&Manifest>) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let platform = name
            .strip_suffix(".pack")
            .or_else(|| name.strip_suffix(".pack.part"));
        if let Some(platform) = platform
            && keep.is_none_or(|m| !m.artifacts.contains_key(platform))
        {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

/// Forgets the release kept and its packages.
pub(crate) fn forget(dir: &Path) -> Result<()> {
    remove_packages(dir, None)?;
    for name in [MANIFEST, SIGNATURE, PUBLISHED] {
        match fs::remove_file(dir.join(name)) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Writes `bytes` to `path` through a temporary file, synced, renamed over the old one.
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    use io::Write as _;
    let tmp = path.with_extension("tmp");
    let _ = fs::remove_file(&tmp);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .with_context(|| format!("cannot write {}", tmp.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

/// That an install of `build` started, and when.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Installed {
    pub build: u64,
    pub at_ms: u64,
}

pub(crate) fn note_installing(dir: &Path, build: u64) -> Result<()> {
    let note = Installed {
        build,
        at_ms: crate::now_ms(),
    };
    write_new(&dir.join(INSTALLED), &serde_json::to_vec(&note)?)
}

pub(crate) fn installed(dir: &Path) -> Option<Installed> {
    let bytes = read_small(&dir.join(INSTALLED), 4096).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn forget_installed(dir: &Path) {
    let _ = fs::remove_file(dir.join(INSTALLED));
}

/// Whether the release kept was published from this data folder.
pub(crate) fn published(dir: &Path) -> bool {
    dir.join(PUBLISHED).is_file()
}

/// Puts the release in `from` (a folder with `release.json`, `release.sig` and the packages
/// it names) into data folder `data`, after checking every byte of it with the keys in
/// `keys`: from there the copy running on that folder, which runs build `runs`, gives it to
/// friends and members. Refuses a release older than the one kept there, and one that copy
/// would let go when it starts (older than `runs`, further above it than the key's role
/// allows, expired or dated ahead), which no copy close to it would take either. Returns its
/// manifest.
pub fn publish(data: &Path, from: &Path, keys: &[ReleaseKey], runs: u64) -> Result<Manifest> {
    let signed = read_signed(from)?.with_context(|| {
        format!(
            "no release in {} (release.json and release.sig)",
            from.display()
        )
    })?;
    let manifest = signed.verify(keys)?;
    if manifest.build < runs {
        bail!(
            "build {} is older than the build this copy runs ({runs})",
            manifest.build
        );
    }
    let role = signed.role(keys).unwrap_or(fogata_update::KeyRole::Release);
    manifest.within_reach(role, runs)?;
    manifest.current(super::now_secs())?;
    let dir = dir(data);
    if let Some(kept) = read_signed(&dir)?
        && let Ok(kept) = kept.verify(keys)
        && kept.build > manifest.build
    {
        return Err(Refusal::NotNewer { have: kept.build }.into());
    }
    fs::create_dir_all(&dir)?;
    for (platform, artifact) in &manifest.artifacts {
        let source = from.join(&artifact.file);
        if !fogata_net::updates::check_package(&source, artifact)
            .with_context(|| format!("cannot read {}", source.display()))?
        {
            bail!(
                "{} is not the package the manifest names for {platform}",
                source.display()
            );
        }
    }
    keep(&dir, &signed)?;
    for (platform, artifact) in &manifest.artifacts {
        let source = from.join(&artifact.file);
        let part = part_path(&dir, platform);
        let _ = fs::remove_file(&part);
        fs::copy(&source, &part).with_context(|| format!("cannot copy {}", source.display()))?;
        if !fogata_net::updates::check_package(&part, artifact)? {
            bail!("{} changed while it was copied", source.display());
        }
        fs::rename(&part, package_path(&dir, platform))?;
    }
    fs::write(dir.join(PUBLISHED), b"")?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use fogata_crypto::DeviceIdentity;
    use fogata_update::{Artifact, CHANNEL, PRODUCT, manifest::FORMAT};

    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fogata-updates-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn release(dir: &Path, key: &DeviceIdentity, build: u64, data: &[u8]) -> Manifest {
        let manifest = Manifest {
            format: FORMAT,
            product: PRODUCT.into(),
            channel: CHANNEL.into(),
            build,
            version: format!("0.{build}"),
            min_build: 1,
            published: super::super::now_secs(),
            expires: super::super::now_secs() + fogata_update::manifest::DEFAULT_LIFETIME,
            notes: String::new(),
            artifacts: BTreeMap::from([(
                "windows-x64".to_string(),
                Artifact::of("Fogata-windows.pack", data),
            )]),
        };
        let signed = Signed::sign(&manifest, key).unwrap();
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(MANIFEST), &signed.body).unwrap();
        fs::write(dir.join(SIGNATURE), signed.signature_file()).unwrap();
        fs::write(dir.join("Fogata-windows.pack"), data).unwrap();
        manifest
    }

    #[test]
    fn a_release_is_published_whole_and_checked() {
        let root = folder("publish");
        let key = DeviceIdentity::generate().unwrap();
        let keys = [fogata_update::ReleaseKey::release(
            *key.public_key().as_bytes(),
        )];
        let data = root.join("data");
        release(&root.join("r2"), &key, 2, b"two");
        let manifest = publish(&data, &root.join("r2"), &keys, 1).unwrap();
        assert_eq!(manifest.build, 2);
        let updates = dir(&data);
        assert_eq!(
            fs::read(package_path(&updates, "windows-x64")).unwrap(),
            b"two"
        );
        let kept = read_signed(&updates).unwrap().unwrap();
        assert_eq!(kept.verify(&keys).unwrap(), manifest);
        assert!(published(&updates));
        // Older than what is kept: no.
        release(&root.join("r1"), &key, 1, b"one");
        assert!(publish(&data, &root.join("r1"), &keys, 1).is_err());
        // Not signed with a key given: no.
        assert!(publish(&data, &root.join("r2"), &[], 1).is_err());
        // A package that is not the one named: no, and nothing changes.
        release(&root.join("r3"), &key, 3, b"three");
        fs::write(root.join("r3/Fogata-windows.pack"), b"thrEe").unwrap();
        assert!(publish(&data, &root.join("r3"), &keys, 1).is_err());
        assert_eq!(read_signed(&updates).unwrap().unwrap(), kept);
        // A newer one replaces it.
        release(&root.join("r4"), &key, 4, b"four");
        publish(&data, &root.join("r4"), &keys, 1).unwrap();
        assert_eq!(
            fs::read(package_path(&updates, "windows-x64")).unwrap(),
            b"four"
        );
        let _ = fs::remove_dir_all(&root);
    }

    /// The publisher's own copy would let go at its next start of a release it could not
    /// take: none is published.
    #[test]
    fn a_release_out_of_reach_of_the_publisher_is_not_published() {
        let root = folder("publish-reach");
        let (key, rescue) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        let keys = [
            fogata_update::ReleaseKey::release(*key.public_key().as_bytes()),
            fogata_update::ReleaseKey::rescue(*rescue.public_key().as_bytes()),
        ];
        let data = root.join("data");
        let step = fogata_update::MAX_BUILD_STEP;
        release(&root.join("far"), &key, 7 + step + 1, b"far");
        let error = publish(&data, &root.join("far"), &keys, 7).unwrap_err();
        assert!(error.to_string().contains("further ahead"), "{error}");
        assert!(read_signed(&dir(&data)).unwrap().is_none());
        // Older than the copy runs.
        release(&root.join("old"), &key, 6, b"old");
        assert!(publish(&data, &root.join("old"), &keys, 7).is_err());
        // The rescue key may go further.
        release(&root.join("rescue"), &rescue, 7 + step + 1, b"rescue");
        publish(&data, &root.join("rescue"), &keys, 7).unwrap();
        // An expired release.
        let expired = Manifest {
            published: 1,
            expires: 2,
            ..release(&root.join("expired"), &key, 8 + step + 1, b"expired")
        };
        let signed = Signed::sign(&expired, &key).unwrap();
        fs::write(root.join("expired").join(MANIFEST), &signed.body).unwrap();
        fs::write(
            root.join("expired").join(SIGNATURE),
            signed.signature_file(),
        )
        .unwrap();
        assert!(publish(&data, &root.join("expired"), &keys, 8 + step).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
