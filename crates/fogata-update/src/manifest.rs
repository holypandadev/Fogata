//! The release manifest and its signature.
//!
//! A manifest is a small JSON document. What is signed are its exact bytes, behind a label
//! that says what they are ([`SIGNING_LABEL`]), so the same key signing anything else can
//! never stand for a release. A copy checks the size, the key and the signature before it
//! reads a single field, so nothing unsigned is ever parsed; then every field against the
//! limits below. Whether it takes the release is one more rule ([`Manifest::taken_by`]):
//! only a build above every build it has or runs, so an old release cannot be offered again
//! (no downgrade, no replay); not further above the build it runs than the signing key's role
//! allows, so a leaked release key cannot put a release out of reach of every later one; and,
//! for a release key's release, not made before the release it holds.

use std::collections::BTreeMap;

use fogata_crypto::{DeviceIdentity, DevicePublicKey};
use serde::{Deserialize, Serialize};

use crate::{CHANNEL, KeyRole, MAX_BUILD_STEP, MAX_RESCUE_STEP, PRODUCT, ReleaseKey, role_of};

/// The manifest format this build reads.
pub const FORMAT: u32 = 1;
/// The most bytes a manifest may have: a few kilobytes per package with the chunk hashes of
/// the largest one.
pub const MAX_MANIFEST: usize = 256 * 1024;
/// The largest package a release may hold.
pub const MAX_ARTIFACT: u64 = 400 * 1024 * 1024;
/// Packages are checked in chunks of this size as they arrive.
pub const CHUNK: u64 = 1024 * 1024;
/// The most packages one release names.
pub const MAX_ARTIFACTS: usize = 8;
pub const MAX_VERSION_LEN: usize = 32;
pub const MAX_NOTES: usize = 8 * 1024;
/// How far ahead of a copy's clock a release may say it was made.
pub const MAX_CLOCK_AHEAD: u64 = 24 * 3600;
/// How long a release is taken by default after it is made.
pub const DEFAULT_LIFETIME: u64 = 180 * 24 * 3600;
/// The longest a release may say it is taken for: so no release stands for ever.
pub const MAX_LIFETIME: u64 = 2 * 365 * 24 * 3600;
/// What a signature over a manifest signs, before the manifest's bytes.
pub const SIGNING_LABEL: &[u8] = b"fogata/v1/release-manifest\0";

/// Why a release is not taken.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("the manifest is larger than any release's")]
    TooLarge,
    #[error("the manifest is signed by a key this copy does not know")]
    UnknownKey,
    #[error("the manifest's signature is not good")]
    BadSignature,
    #[error("the manifest cannot be read: {0}")]
    Malformed(String),
    #[error("the manifest is of a format this copy does not know ({0})")]
    UnknownFormat(u32),
    #[error("the manifest is for another application or channel")]
    WrongChannel,
    #[error("the release is not newer than build {have}")]
    NotNewer { have: u64 },
    #[error("the release is further ahead of build {runs} than its key may go")]
    TooFarAhead { runs: u64 },
    #[error("the release was made before the one this copy holds")]
    MadeBefore,
    #[error("a package of the release is larger than {MAX_ARTIFACT} bytes")]
    ArtifactTooLarge,
    #[error("a package arrived damaged")]
    Damaged,
    #[error("the release expired")]
    Expired,
    #[error("the release says it was made later than this computer's time")]
    FromTheFuture,
}

/// A release, as its manifest says.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    /// [`PRODUCT`] and [`CHANNEL`].
    pub product: String,
    pub channel: String,
    /// Goes up with every release.
    pub build: u64,
    /// What people see, such as `0.2.0-beta.3`.
    pub version: String,
    /// The oldest build that still works with this one. A copy below it must update to keep
    /// talking to copies of this release (a change of the network protocol).
    pub min_build: u64,
    /// When it was made, in seconds since 1970. A release made more than a day after a
    /// copy's clock is not taken.
    pub published: u64,
    /// When it stops being taken or installed, in seconds since 1970: a copy that friends
    /// keep from newer releases still stops at an old one at some point.
    pub expires: u64,
    /// What changed, in a few lines.
    pub notes: String,
    /// One package per platform ([`crate::PLATFORMS`]).
    pub artifacts: BTreeMap<String, Artifact>,
}

/// A package of a release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// Its file name, for people.
    pub file: String,
    pub size: u64,
    /// BLAKE3 of the whole package, hex.
    pub blake3: String,
    /// SHA-256 of the whole package, hex: a second hash, of another family.
    pub sha256: String,
    /// BLAKE3 of each [`CHUNK`] of it in order, hex, one after the other.
    pub chunks: String,
}

/// A manifest's bytes with the key that signed them and the signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signed {
    pub body: Vec<u8>,
    pub key: [u8; 32],
    pub signature: [u8; 64],
}

/// The signature file beside a manifest (`release.sig`): the key and the signature, hex.
#[derive(Serialize, Deserialize)]
struct SignatureFile {
    key: String,
    signature: String,
}

impl Signed {
    /// Signs `manifest` with the release key.
    pub fn sign(manifest: &Manifest, key: &DeviceIdentity) -> Result<Signed, Refusal> {
        manifest.check()?;
        let mut body =
            serde_json::to_vec_pretty(manifest).map_err(|e| Refusal::Malformed(e.to_string()))?;
        body.push(b'\n');
        if body.len() > MAX_MANIFEST {
            return Err(Refusal::TooLarge);
        }
        let signature = key.sign(&signed_bytes(&body));
        Ok(Signed {
            body,
            key: *key.public_key().as_bytes(),
            signature,
        })
    }

    /// Checks the signature with the keys in `trusted`, then reads and checks the manifest.
    pub fn verify(&self, trusted: &[ReleaseKey]) -> Result<Manifest, Refusal> {
        if self.body.len() > MAX_MANIFEST {
            return Err(Refusal::TooLarge);
        }
        if self.role(trusted).is_none() {
            return Err(Refusal::UnknownKey);
        }
        let key = DevicePublicKey::from_bytes(&self.key).map_err(|_| Refusal::UnknownKey)?;
        key.verify(&signed_bytes(&self.body), &self.signature)
            .map_err(|_| Refusal::BadSignature)?;
        let manifest: Manifest =
            serde_json::from_slice(&self.body).map_err(|e| Refusal::Malformed(e.to_string()))?;
        manifest.check()?;
        Ok(manifest)
    }

    /// The role of the key that signed it, if `trusted` holds that key.
    pub fn role(&self, trusted: &[ReleaseKey]) -> Option<KeyRole> {
        role_of(trusted, &self.key)
    }

    /// The signature file's text.
    pub fn signature_file(&self) -> String {
        let file = SignatureFile {
            key: hex::encode(self.key),
            signature: hex::encode(self.signature),
        };
        serde_json::to_string_pretty(&file).expect("plain strings") + "\n"
    }

    /// A manifest's bytes with its signature file's text.
    pub fn from_files(body: Vec<u8>, signature_file: &[u8]) -> Result<Signed, Refusal> {
        if body.len() > MAX_MANIFEST || signature_file.len() > 1024 {
            return Err(Refusal::TooLarge);
        }
        let file: SignatureFile = serde_json::from_slice(signature_file)
            .map_err(|e| Refusal::Malformed(e.to_string()))?;
        let key = hex_array::<32>(&file.key).ok_or(Refusal::BadSignature)?;
        let signature = hex_array::<64>(&file.signature).ok_or(Refusal::BadSignature)?;
        Ok(Signed {
            body,
            key,
            signature,
        })
    }
}

fn signed_bytes(body: &[u8]) -> Vec<u8> {
    let mut message = SIGNING_LABEL.to_vec();
    message.extend_from_slice(body);
    message
}

fn hex_array<const N: usize>(text: &str) -> Option<[u8; N]> {
    let bytes = hex::decode(text).ok()?;
    bytes.try_into().ok()
}

impl Manifest {
    /// Checks every field against the format's limits.
    pub fn check(&self) -> Result<(), Refusal> {
        if self.format != FORMAT {
            return Err(Refusal::UnknownFormat(self.format));
        }
        if self.product != PRODUCT || self.channel != CHANNEL {
            return Err(Refusal::WrongChannel);
        }
        let malformed = |what: &str| Err(Refusal::Malformed(what.to_string()));
        if self.build == 0 || self.min_build > self.build {
            return malformed("its builds do not add up");
        }
        if self.expires <= self.published {
            return malformed("it expires before it was made");
        }
        if self.expires - self.published > MAX_LIFETIME {
            return malformed("it would be taken for too long");
        }
        if self.version.is_empty()
            || self.version.len() > MAX_VERSION_LEN
            || !self
                .version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".+-".contains(&b))
        {
            return malformed("its version is not a version");
        }
        if self.notes.len() > MAX_NOTES
            || self
                .notes
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return malformed("its notes are too long or hold control characters");
        }
        if self.artifacts.is_empty() || self.artifacts.len() > MAX_ARTIFACTS {
            return malformed("it holds no packages, or too many");
        }
        for (platform, artifact) in &self.artifacts {
            if platform.is_empty()
                || platform.len() > 32
                || !platform
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return malformed("a platform's name is not one");
            }
            artifact.check()?;
        }
        Ok(())
    }

    /// Whether this release may be taken or installed at `now` (seconds since 1970): not
    /// expired, and not made more than a day ahead of this clock.
    pub fn current(&self, now: u64) -> Result<(), Refusal> {
        if now >= self.expires {
            Err(Refusal::Expired)
        } else if self.published > now.saturating_add(MAX_CLOCK_AHEAD) {
            Err(Refusal::FromTheFuture)
        } else {
            Ok(())
        }
    }

    /// Whether this release is newer than build `have`.
    pub fn newer_than(&self, have: u64) -> Result<(), Refusal> {
        if self.build > have {
            Ok(())
        } else {
            Err(Refusal::NotNewer { have })
        }
    }

    /// Whether this release, signed by a key of `role`, is at most as far above build `runs`
    /// as that role allows ([`MAX_BUILD_STEP`], [`MAX_RESCUE_STEP`]).
    pub fn within_reach(&self, role: KeyRole, runs: u64) -> Result<(), Refusal> {
        let step = match role {
            KeyRole::Release => MAX_BUILD_STEP,
            KeyRole::Rescue => MAX_RESCUE_STEP,
        };
        if self.build.saturating_sub(runs) <= step {
            Ok(())
        } else {
            Err(Refusal::TooFarAhead { runs })
        }
    }

    /// Whether this release, signed by a key of `role`, may be taken by a copy that runs
    /// build `runs` and holds release `held` (checked when it was taken): newer than both,
    /// within reach of `runs`, and, from a release key, made no earlier than `held`. The
    /// rescue key's releases are not held to the time: one that replaces a leaked release
    /// key may have to pass a release the leaked key dated up to a day ahead.
    pub fn taken_by(
        &self,
        role: KeyRole,
        runs: u64,
        held: Option<&Manifest>,
    ) -> Result<(), Refusal> {
        self.newer_than(runs.max(held.map_or(0, |h| h.build)))?;
        self.within_reach(role, runs)?;
        if role == KeyRole::Release && held.is_some_and(|h| self.published < h.published) {
            return Err(Refusal::MadeBefore);
        }
        Ok(())
    }
}

impl Artifact {
    fn check(&self) -> Result<(), Refusal> {
        let malformed = |what: &str| Err(Refusal::Malformed(what.to_string()));
        if self.file.is_empty()
            || self.file.len() > 128
            || self.file.starts_with('.')
            || !self
                .file
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return malformed("a package's file name is not a plain name");
        }
        if self.size > MAX_ARTIFACT {
            return Err(Refusal::ArtifactTooLarge);
        }
        if self.size == 0 {
            return malformed("a package is empty");
        }
        if hex_array::<32>(&self.blake3).is_none() || hex_array::<32>(&self.sha256).is_none() {
            return malformed("a package's hash is not one");
        }
        if self.chunks.len() as u64 != self.chunk_count() * 64 || hex::decode(&self.chunks).is_err()
        {
            return malformed("a package's chunk hashes do not match its size");
        }
        Ok(())
    }

    /// How many chunks the package has.
    pub fn chunk_count(&self) -> u64 {
        self.size.div_ceil(CHUNK)
    }

    /// The length of chunk `index`: [`CHUNK`], less for the last one.
    pub fn chunk_len(&self, index: u64) -> u64 {
        CHUNK.min(self.size.saturating_sub(index * CHUNK))
    }

    /// The BLAKE3 hash chunk `index` must have.
    pub fn chunk_hash(&self, index: u64) -> Option<[u8; 32]> {
        let at = usize::try_from(index).ok()?.checked_mul(64)?;
        hex_array::<32>(self.chunks.get(at..at + 64)?)
    }

    /// The whole package's BLAKE3 hash.
    pub fn hash(&self) -> [u8; 32] {
        hex_array::<32>(&self.blake3).unwrap_or_default()
    }

    /// The whole package's SHA-256 hash.
    pub fn sha256(&self) -> [u8; 32] {
        hex_array::<32>(&self.sha256).unwrap_or_default()
    }

    /// Describes `data` as a package named `file`.
    pub fn of(file: &str, data: &[u8]) -> Artifact {
        let mut builder = Builder::new(file);
        builder.update(data);
        builder.finish()
    }
}

/// Describes a package read piece by piece: its size, hash and chunk hashes.
#[derive(Debug)]
pub struct Builder {
    file: String,
    whole: blake3::Hasher,
    sha256: sha2::Sha256,
    chunk: blake3::Hasher,
    in_chunk: u64,
    size: u64,
    chunks: String,
}

impl Builder {
    pub fn new(file: &str) -> Builder {
        Builder {
            file: file.to_string(),
            whole: blake3::Hasher::new(),
            sha256: sha2::Sha256::default(),
            chunk: blake3::Hasher::new(),
            in_chunk: 0,
            size: 0,
            chunks: String::new(),
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.whole.update(data);
        sha2::Digest::update(&mut self.sha256, data);
        self.size += data.len() as u64;
        while !data.is_empty() {
            let take = ((CHUNK - self.in_chunk) as usize).min(data.len());
            self.chunk.update(&data[..take]);
            self.in_chunk += take as u64;
            data = &data[take..];
            if self.in_chunk == CHUNK {
                self.end_chunk();
            }
        }
    }

    fn end_chunk(&mut self) {
        self.chunks
            .push_str(self.chunk.finalize().to_hex().as_str());
        self.chunk = blake3::Hasher::new();
        self.in_chunk = 0;
    }

    pub fn finish(mut self) -> Artifact {
        if self.in_chunk > 0 {
            self.end_chunk();
        }
        Artifact {
            file: self.file,
            size: self.size,
            blake3: self.whole.finalize().to_hex().to_string(),
            sha256: hex::encode(sha2::Digest::finalize(self.sha256)),
            chunks: self.chunks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(build: u64, data: &[u8]) -> Manifest {
        Manifest {
            format: FORMAT,
            product: PRODUCT.into(),
            channel: CHANNEL.into(),
            build,
            version: format!("0.1.{build}-beta"),
            min_build: 1,
            published: 1_790_000_000,
            expires: 1_790_000_000 + DEFAULT_LIFETIME,
            notes: "Fixes.\nMore fixes.".into(),
            artifacts: BTreeMap::from([(
                "linux-flatpak".into(),
                Artifact::of("fogata.pack", data),
            )]),
        }
    }

    /// The universal Mac app: one package, listed under the platform of each kind of Mac
    /// (tools/beta/release.py), which every copy since the first beta takes.
    #[test]
    fn one_package_may_serve_two_platforms() {
        let key = DeviceIdentity::generate().unwrap();
        let trusted = [ReleaseKey::release(*key.public_key().as_bytes())];
        let mut m = manifest(5, b"package");
        let mac = Artifact::of("Fogata-0.1.5-beta-macos-universal.pack", b"universal app");
        m.artifacts.insert("macos-arm64".into(), mac.clone());
        m.artifacts.insert("macos-x64".into(), mac.clone());
        let signed = Signed::sign(&m, &key).unwrap();
        let read = signed.verify(&trusted).unwrap();
        assert_eq!(read.artifacts["macos-arm64"], mac);
        assert_eq!(read.artifacts["macos-x64"], mac);
        assert!(crate::PLATFORMS.contains(&"macos-x64"));
    }

    #[test]
    fn a_signed_release_verifies_with_its_key_only() {
        let key = DeviceIdentity::generate().unwrap();
        let other = DeviceIdentity::generate().unwrap();
        let trusted = [ReleaseKey::release(*key.public_key().as_bytes())];
        let signed = Signed::sign(&manifest(5, b"package"), &key).unwrap();
        assert_eq!(signed.verify(&trusted).unwrap().build, 5);
        // Through its files.
        let again =
            Signed::from_files(signed.body.clone(), signed.signature_file().as_bytes()).unwrap();
        assert_eq!(again, signed);
        // Another key, even a good signature of it, is unknown.
        let theirs = Signed::sign(&manifest(5, b"package"), &other).unwrap();
        assert_eq!(theirs.verify(&trusted), Err(Refusal::UnknownKey));
        assert_eq!(signed.verify(&[]), Err(Refusal::UnknownKey));
        // A trusted key named with another's signature.
        let mut forged = theirs.clone();
        forged.key = trusted[0].key;
        assert_eq!(forged.verify(&trusted), Err(Refusal::BadSignature));
    }

    #[test]
    fn any_change_to_a_signed_manifest_breaks_its_signature() {
        let key = DeviceIdentity::generate().unwrap();
        let trusted = [ReleaseKey::release(*key.public_key().as_bytes())];
        let signed = Signed::sign(&manifest(5, b"package"), &key).unwrap();
        for at in [0, signed.body.len() / 2, signed.body.len() - 1] {
            let mut changed = signed.clone();
            changed.body[at] ^= 1;
            assert_eq!(changed.verify(&trusted), Err(Refusal::BadSignature));
        }
        let mut changed = signed.clone();
        changed.signature[10] ^= 1;
        assert_eq!(changed.verify(&trusted), Err(Refusal::BadSignature));
        // A higher build in place of the signed one.
        let text = String::from_utf8(signed.body.clone()).unwrap();
        let mut changed = signed.clone();
        changed.body = text.replace("\"build\": 5", "\"build\": 50").into_bytes();
        assert_ne!(changed.body, signed.body);
        assert_eq!(changed.verify(&trusted), Err(Refusal::BadSignature));
        // The signature does not stand for the bytes without the label.
        let bare = key.sign(&signed.body);
        let mut changed = signed.clone();
        changed.signature = bare;
        assert_eq!(changed.verify(&trusted), Err(Refusal::BadSignature));
    }

    #[test]
    fn an_expired_or_future_release_is_not_current() {
        let m = manifest(5, b"package");
        assert!(m.current(m.published).is_ok());
        assert!(m.current(m.expires - 1).is_ok());
        assert_eq!(m.current(m.expires), Err(Refusal::Expired));
        // Made a little ahead of this clock: fine; more than a day: no.
        assert!(m.current(m.published - 3600).is_ok());
        assert_eq!(
            m.current(m.published - MAX_CLOCK_AHEAD - 1),
            Err(Refusal::FromTheFuture)
        );
    }

    #[test]
    fn only_a_newer_build_is_taken() {
        let m = manifest(5, b"package");
        assert!(m.newer_than(4).is_ok());
        assert_eq!(m.newer_than(5), Err(Refusal::NotNewer { have: 5 }));
        assert_eq!(m.newer_than(9), Err(Refusal::NotNewer { have: 9 }));
        let release = KeyRole::Release;
        assert!(m.taken_by(release, 4, None).is_ok());
        assert_eq!(
            m.taken_by(release, 5, None),
            Err(Refusal::NotNewer { have: 5 })
        );
        // A release held counts as much as the build run.
        let held = manifest(7, b"package");
        assert_eq!(
            m.taken_by(release, 4, Some(&held)),
            Err(Refusal::NotNewer { have: 7 })
        );
    }

    #[test]
    fn a_release_key_cannot_jump_far_ahead() {
        let release = KeyRole::Release;
        // Up to MAX_BUILD_STEP above the build run, whatever is held.
        assert!(
            manifest(1 + MAX_BUILD_STEP, b"p")
                .taken_by(release, 1, None)
                .is_ok()
        );
        assert_eq!(
            manifest(2 + MAX_BUILD_STEP, b"p").taken_by(release, 1, None),
            Err(Refusal::TooFarAhead { runs: 1 })
        );
        assert_eq!(
            manifest(u64::MAX - 1, b"p").taken_by(release, 1, None),
            Err(Refusal::TooFarAhead { runs: 1 })
        );
        // A held release does not move the limit: a chain of releases of a leaked key
        // cannot climb past it.
        let held = manifest(MAX_BUILD_STEP, b"p");
        assert_eq!(
            manifest(MAX_BUILD_STEP + 500, b"p").taken_by(release, 1, Some(&held)),
            Err(Refusal::TooFarAhead { runs: 1 })
        );
        // At the top of the numbers, no overflow.
        assert!(
            manifest(u64::MAX, b"p")
                .taken_by(release, u64::MAX - 1, None)
                .is_ok()
        );
        assert_eq!(
            manifest(u64::MAX, b"p").taken_by(release, u64::MAX, None),
            Err(Refusal::NotNewer { have: u64::MAX })
        );
    }

    #[test]
    fn a_release_made_before_the_one_held_is_not_taken() {
        let held = manifest(5, b"p");
        let mut m = manifest(6, b"p");
        m.published = held.published - 1;
        assert_eq!(
            m.taken_by(KeyRole::Release, 4, Some(&held)),
            Err(Refusal::MadeBefore)
        );
        m.published = held.published;
        assert!(m.taken_by(KeyRole::Release, 4, Some(&held)).is_ok());
        m.published = held.published + 3600;
        assert!(m.taken_by(KeyRole::Release, 4, Some(&held)).is_ok());
        // With nothing held there is nothing to be earlier than.
        m.published = 1;
        m.expires = 2;
        assert!(m.taken_by(KeyRole::Release, 4, None).is_ok());
    }

    /// A release key leaks: its holder makes copies hold the highest release they take,
    /// dated a day ahead. The rescue release, numbered one step above the last good build and
    /// made before that date, passes it on every copy up to a step behind, and is in turn
    /// out of the leaked key's reach.
    #[test]
    fn a_rescue_release_passes_whatever_a_leaked_key_made_copies_hold() {
        let (release, rescue) = (KeyRole::Release, KeyRole::Rescue);
        let last_good = 5000u64;
        let now = 1_790_000_000u64;
        let rescued = |runs: u64| {
            let mut leaked = manifest(runs + MAX_BUILD_STEP, b"evil");
            leaked.published = now + MAX_CLOCK_AHEAD;
            leaked.expires = leaked.published + DEFAULT_LIFETIME;
            assert!(leaked.taken_by(release, runs, None).is_ok());
            let mut fix = manifest(last_good + MAX_BUILD_STEP + 1, b"fixed");
            fix.published = now;
            // Not from the release key: too early and, for runs < last_good, too far.
            assert!(fix.taken_by(release, runs, Some(&leaked)).is_err());
            fix.taken_by(rescue, runs, Some(&leaked))?;
            // Once it holds the rescue release, nothing the leaked key signs passes it.
            for build in [leaked.build + 1, runs + MAX_BUILD_STEP + 1, u64::MAX] {
                let mut later = manifest(build, b"evil");
                later.published = now + MAX_CLOCK_AHEAD;
                assert!(later.taken_by(release, runs, Some(&fix)).is_err());
            }
            Ok::<(), Refusal>(())
        };
        for runs in [last_good, last_good - 1, last_good - (MAX_BUILD_STEP - 1)] {
            rescued(runs).unwrap();
        }
        // A copy more than a step behind takes no rescue release so far ahead (nor the
        // releases between, which it never had).
        assert_eq!(
            rescued(last_good - MAX_BUILD_STEP),
            Err(Refusal::TooFarAhead {
                runs: last_good - MAX_BUILD_STEP
            })
        );
    }

    #[test]
    fn only_trusted_keys_have_a_role() {
        let key = DeviceIdentity::generate().unwrap();
        let rescue = DeviceIdentity::generate().unwrap();
        let trusted = [
            ReleaseKey::release(*key.public_key().as_bytes()),
            ReleaseKey::rescue(*rescue.public_key().as_bytes()),
        ];
        let signed = Signed::sign(&manifest(5, b"p"), &key).unwrap();
        assert_eq!(signed.role(&trusted), Some(KeyRole::Release));
        let signed = Signed::sign(&manifest(5, b"p"), &rescue).unwrap();
        assert_eq!(signed.role(&trusted), Some(KeyRole::Rescue));
        assert_eq!(signed.verify(&trusted).unwrap().build, 5);
        assert_eq!(signed.role(&trusted[..1]), None);
        assert_eq!(signed.verify(&trusted[..1]), Err(Refusal::UnknownKey));
    }

    #[test]
    fn an_oversized_release_is_refused_even_when_signed() {
        let key = DeviceIdentity::generate().unwrap();
        let trusted = [ReleaseKey::release(*key.public_key().as_bytes())];
        let mut m = manifest(5, b"package");
        let artifact = m.artifacts.get_mut("linux-flatpak").unwrap();
        artifact.size = MAX_ARTIFACT + 1;
        artifact.chunks = "00".repeat(32).repeat(artifact.chunk_count() as usize);
        assert_eq!(Signed::sign(&m, &key), Err(Refusal::ArtifactTooLarge));
        // Signed by a key that did not check it: still refused.
        let body = serde_json::to_vec(&m).unwrap();
        let signature = key.sign(&signed_bytes(&body));
        let signed = Signed {
            body,
            key: trusted[0].key,
            signature,
        };
        assert_eq!(signed.verify(&trusted), Err(Refusal::ArtifactTooLarge));
        // A manifest past its own limit is not even checked.
        let signed = Signed {
            body: vec![b' '; MAX_MANIFEST + 1],
            key: trusted[0].key,
            signature: [0; 64],
        };
        assert_eq!(signed.verify(&trusted), Err(Refusal::TooLarge));
    }

    #[test]
    fn fields_are_held_to_the_format() {
        let key = DeviceIdentity::generate().unwrap();
        let ok = manifest(5, b"package");
        assert!(ok.check().is_ok());
        type Change = Box<dyn Fn(&mut Manifest)>;
        let bad: Vec<Change> = vec![
            Box::new(|m| m.format = 2),
            Box::new(|m| m.channel = "stable".into()),
            Box::new(|m| m.product = "other".into()),
            Box::new(|m| m.min_build = 6),
            Box::new(|m| m.build = 0),
            Box::new(|m| m.version = "1.0 beta".into()),
            Box::new(|m| m.version = "1".repeat(33)),
            Box::new(|m| m.notes = "bell\u{7}".into()),
            Box::new(|m| m.artifacts.clear()),
            Box::new(|m| {
                let a = m.artifacts.remove("linux-flatpak").unwrap();
                m.artifacts.insert("Linux".into(), a);
            }),
            Box::new(|m| m.artifacts.get_mut("linux-flatpak").unwrap().file = "../x".into()),
            Box::new(|m| m.artifacts.get_mut("linux-flatpak").unwrap().file = ".hidden".into()),
            Box::new(|m| m.artifacts.get_mut("linux-flatpak").unwrap().size = 0),
            Box::new(|m| m.artifacts.get_mut("linux-flatpak").unwrap().blake3 = "zz".into()),
            Box::new(|m| {
                m.artifacts
                    .get_mut("linux-flatpak")
                    .unwrap()
                    .sha256
                    .pop()
                    .map(drop)
                    .unwrap_or(())
            }),
            Box::new(|m| m.expires = m.published),
            Box::new(|m| m.expires = m.published + MAX_LIFETIME + 1),
            Box::new(|m| {
                m.artifacts
                    .get_mut("linux-flatpak")
                    .unwrap()
                    .chunks
                    .push('0')
            }),
            Box::new(|m| {
                m.artifacts.get_mut("linux-flatpak").unwrap().size = CHUNK + 1;
            }),
        ];
        for (i, change) in bad.iter().enumerate() {
            let mut m = ok.clone();
            change(&mut m);
            assert!(m.check().is_err(), "change {i} passed");
            assert!(Signed::sign(&m, &key).is_err(), "change {i} was signed");
        }
        // A newer manifest's extra field is read past; one missing a field is not read.
        let mut value = serde_json::to_value(&ok).unwrap();
        value["later"] = serde_json::json!(1);
        assert!(serde_json::from_value::<Manifest>(value.clone()).is_ok());
        value.as_object_mut().unwrap().remove("min_build");
        assert!(serde_json::from_value::<Manifest>(value).is_err());
    }

    #[test]
    fn chunks_are_described_one_by_one() {
        let data: Vec<u8> = (0..(2 * CHUNK + 5)).map(|i| (i % 251) as u8).collect();
        let artifact = Artifact::of("p", &data);
        assert_eq!(artifact.size, data.len() as u64);
        assert_eq!(artifact.chunk_count(), 3);
        assert_eq!(artifact.chunk_len(0), CHUNK);
        assert_eq!(artifact.chunk_len(2), 5);
        for i in 0..3u64 {
            let start = (i * CHUNK) as usize;
            let end = (start + artifact.chunk_len(i) as usize).min(data.len());
            assert_eq!(
                artifact.chunk_hash(i).unwrap(),
                *blake3::hash(&data[start..end]).as_bytes()
            );
        }
        assert!(artifact.chunk_hash(3).is_none());
        assert_eq!(artifact.hash(), *blake3::hash(&data).as_bytes());
        assert_eq!(
            artifact.sha256(),
            <[u8; 32]>::from(sha2::Digest::finalize(
                <sha2::Sha256 as sha2::Digest>::new_with_prefix(&data)
            ))
        );
        // Fed in odd pieces, the same.
        let mut builder = Builder::new("p");
        for piece in data.chunks(77_777) {
            builder.update(piece);
        }
        assert_eq!(builder.finish(), artifact);
        // Exactly one chunk.
        let one = Artifact::of("p", &vec![1u8; CHUNK as usize]);
        assert_eq!(one.chunk_count(), 1);
        assert_eq!(one.chunks.len(), 64);
    }
}
