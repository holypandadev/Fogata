//! A release as a copy of Fogata takes one from a friend or the releases page, through this
//! crate's public calls only and with keys made for the test (never the real ones): the
//! manifest and its signature file read back from their bytes, the signature checked against
//! the keys the copy trusts, the release held to its dates and builds, and each package
//! checked chunk by chunk as it arrives.

use std::collections::BTreeMap;

use fogata_crypto::DeviceIdentity;
use fogata_update::{
    CHANNEL, KeyRole, MAX_BUILD_STEP, PRODUCT, RELEASE_KEYS, ReleaseKey,
    manifest::{Artifact, Builder, CHUNK, DEFAULT_LIFETIME, FORMAT, Manifest, Refusal, Signed},
};

const NOW: u64 = 1_790_000_000;

fn package(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

fn manifest(build: u64, artifacts: BTreeMap<String, Artifact>) -> Manifest {
    Manifest {
        format: FORMAT,
        product: PRODUCT.into(),
        channel: CHANNEL.into(),
        build,
        version: format!("0.1.0-beta.{build}"),
        min_build: build - 1,
        published: NOW,
        expires: NOW + DEFAULT_LIFETIME,
        notes: "Fixes.".into(),
        artifacts,
    }
}

struct Keys {
    release: DeviceIdentity,
    rescue: DeviceIdentity,
    trusted: [ReleaseKey; 2],
}

fn keys() -> Keys {
    let release = DeviceIdentity::generate().unwrap();
    let rescue = DeviceIdentity::generate().unwrap();
    let trusted = [
        ReleaseKey::release(*release.public_key().as_bytes()),
        ReleaseKey::rescue(*rescue.public_key().as_bytes()),
    ];
    Keys {
        release,
        rescue,
        trusted,
    }
}

/// What a copy reads from a friend or the releases page: `release.json` and `release.sig`.
fn as_files(signed: &Signed) -> (Vec<u8>, Vec<u8>) {
    (signed.body.clone(), signed.signature_file().into_bytes())
}

/// What the copy makes of the two files: the manifest and the role of its key.
fn read(
    trusted: &[ReleaseKey],
    body: &[u8],
    signature: &[u8],
) -> Result<(Manifest, KeyRole), Refusal> {
    let signed = Signed::from_files(body.to_vec(), signature)?;
    let manifest = signed.verify(trusted)?;
    Ok((manifest, signed.role(trusted).expect("verified")))
}

#[test]
fn a_release_signed_with_a_trusted_key_is_taken_from_its_files() {
    let keys = keys();
    let mac = package(3 * CHUNK as usize + 17, 1);
    let release = manifest(
        12,
        BTreeMap::from([
            ("macos-arm64".into(), Artifact::of("Fogata.pack", &mac)),
            ("macos-x86-64".into(), Artifact::of("Fogata.pack", &mac)),
        ]),
    );
    let signed = Signed::sign(&release, &keys.release).unwrap();
    let (body, signature) = as_files(&signed);
    let (read_back, role) = read(&keys.trusted, &body, &signature).unwrap();
    assert_eq!(
        (read_back.clone(), role),
        (release.clone(), KeyRole::Release)
    );
    assert!(read_back.current(NOW).is_ok());
    assert!(read_back.taken_by(role, 11, None).is_ok());
    // A copy that runs it, or a newer one, does not take it.
    assert_eq!(
        read_back.taken_by(role, 12, None),
        Err(Refusal::NotNewer { have: 12 })
    );
    // Keys made here are not the ones any build trusts.
    assert_eq!(
        Signed::from_files(body.clone(), &signature)
            .unwrap()
            .verify(RELEASE_KEYS),
        Err(Refusal::UnknownKey)
    );
    // Nor is a key that trusts only itself enough: the key must be one the copy holds.
    let stranger = DeviceIdentity::generate().unwrap();
    let forged = Signed::sign(&release, &stranger).unwrap();
    let (body, signature) = as_files(&forged);
    assert_eq!(
        read(&keys.trusted, &body, &signature).unwrap_err(),
        Refusal::UnknownKey
    );
}

/// Each field of the manifest changed as it travels, the signature kept: refused for the
/// signature, whatever the field.
#[test]
fn any_field_changed_after_signing_is_refused() {
    let keys = keys();
    let release = manifest(
        12,
        BTreeMap::from([(
            "windows-x86-64".into(),
            Artifact::of("Fogata.pack", &package(5000, 2)),
        )]),
    );
    let signed = Signed::sign(&release, &keys.release).unwrap();
    let (body, signature) = as_files(&signed);
    let original: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let mut changes: Vec<(String, serde_json::Value)> = Vec::new();
    let serde_json::Value::Object(fields) = &original else {
        panic!("a manifest is an object");
    };
    for (name, value) in fields {
        let mut changed = original.clone();
        changed[name] = match value {
            serde_json::Value::Number(n) => (n.as_u64().unwrap() + 1).into(),
            serde_json::Value::String(s) => format!("{s}x").into(),
            serde_json::Value::Object(_) => {
                let mut artifacts = value.clone();
                let one = artifacts
                    .as_object_mut()
                    .unwrap()
                    .values_mut()
                    .next()
                    .unwrap();
                one["size"] = (one["size"].as_u64().unwrap() + 1).into();
                artifacts
            }
            other => panic!("{name}: {other:?}"),
        };
        changes.push((name.clone(), changed));
    }
    // Each field of the package too.
    for field in ["file", "size", "blake3", "sha256", "chunks"] {
        let mut changed = original.clone();
        let one = &mut changed["artifacts"]["windows-x86-64"][field];
        *one = match one {
            serde_json::Value::Number(n) => (n.as_u64().unwrap() - 1).into(),
            serde_json::Value::String(s) => s.replacen('a', "b", 1).replacen('0', "1", 1).into(),
            _ => unreachable!(),
        };
        changes.push((format!("artifacts.{field}"), changed));
    }
    // Another platform for the same package.
    let mut changed = original.clone();
    let package = changed["artifacts"]["windows-x86-64"].clone();
    changed["artifacts"]["windows-arm64"] = package;
    changes.push(("artifacts (a platform added)".into(), changed));
    assert!(changes.len() >= 15, "{} changes", changes.len());

    for (what, changed) in changes {
        let mut body = serde_json::to_vec_pretty(&changed).unwrap();
        body.push(b'\n');
        assert_ne!(body, signed.body, "{what}: not changed");
        assert_eq!(
            read(&keys.trusted, &body, &signature).unwrap_err(),
            Refusal::BadSignature,
            "{what}"
        );
    }
    // And the signature file's own fields.
    let file: serde_json::Value = serde_json::from_slice(&signature).unwrap();
    for field in ["key", "signature"] {
        let mut changed = file.clone();
        let hex = changed[field].as_str().unwrap().to_string();
        let flipped = if hex.starts_with('0') { "1" } else { "0" };
        changed[field] = format!("{flipped}{}", &hex[1..]).into();
        let refused = read(&keys.trusted, &body, changed.to_string().as_bytes()).unwrap_err();
        assert!(
            matches!(refused, Refusal::BadSignature | Refusal::UnknownKey),
            "{field}: {refused:?}"
        );
    }
}

/// The signature is checked before anything else, and the manifest after it: one signed with
/// the right key that breaks the format is still refused.
#[test]
fn a_manifest_signed_as_it_is_but_out_of_its_format_is_refused() {
    let keys = keys();
    let good = manifest(
        12,
        BTreeMap::from([(
            "linux-flatpak".into(),
            Artifact::of("fogata.pack", b"package"),
        )]),
    );
    let signed_as = |manifest: &Manifest| {
        // Signed::sign checks the format first; a publisher's tool that did not would sign
        // the bytes as they are.
        let mut body = serde_json::to_vec_pretty(manifest).unwrap();
        body.push(b'\n');
        let mut message = fogata_update::manifest::SIGNING_LABEL.to_vec();
        message.extend_from_slice(&body);
        let signed = Signed {
            body,
            key: *keys.release.public_key().as_bytes(),
            signature: keys.release.sign(&message),
        };
        as_files(&signed)
    };
    let (body, signature) = signed_as(&good);
    assert!(read(&keys.trusted, &body, &signature).is_ok());

    let mut bad = Vec::new();
    let mut m = good.clone();
    m.min_build = m.build + 1;
    bad.push(("an oldest build above its own", m));
    let mut m = good.clone();
    m.expires = m.published;
    bad.push(("expiring as it is made", m));
    let mut m = good.clone();
    m.channel = "stable".into();
    bad.push(("another channel", m));
    let mut m = good.clone();
    m.format = FORMAT + 1;
    bad.push(("a later format", m));
    let mut m = good.clone();
    m.artifacts.values_mut().next().unwrap().size += CHUNK;
    bad.push(("a package's size its chunks do not cover", m));
    let mut m = good.clone();
    m.artifacts.values_mut().next().unwrap().file = "../fogata.pack".into();
    bad.push(("a package's name that climbs out", m));
    for (what, m) in bad {
        let (body, signature) = signed_as(&m);
        let refused = read(&keys.trusted, &body, &signature).unwrap_err();
        assert!(
            !matches!(refused, Refusal::BadSignature | Refusal::UnknownKey),
            "{what}: refused for {refused:?}, not for its format"
        );
    }
}

#[test]
fn a_release_is_taken_only_between_its_dates() {
    let keys = keys();
    let release = manifest(
        12,
        BTreeMap::from([(
            "linux-flatpak".into(),
            Artifact::of("fogata.pack", b"package"),
        )]),
    );
    let (body, signature) = as_files(&Signed::sign(&release, &keys.release).unwrap());
    let (read_back, _) = read(&keys.trusted, &body, &signature).unwrap();
    assert!(read_back.current(read_back.expires - 1).is_ok());
    assert_eq!(read_back.current(read_back.expires), Err(Refusal::Expired));
    // A clock up to a day behind the publisher's still takes it; further behind does not.
    assert!(read_back.current(NOW - 23 * 3600).is_ok());
    assert_eq!(
        read_back.current(NOW - 25 * 3600),
        Err(Refusal::FromTheFuture)
    );
}

/// A release key leaked, and its holder made copies hold a release far ahead, dated a day
/// ahead: the release of the rescue key, read from its files as any other, passes it; the
/// same release from the leaked key does not.
#[test]
fn a_rescue_release_passes_one_a_leaked_key_made_copies_hold() {
    let keys = keys();
    let runs = 40;
    let mut leaked = manifest(
        runs + MAX_BUILD_STEP,
        BTreeMap::from([("linux-flatpak".into(), Artifact::of("fogata.pack", b"evil"))]),
    );
    leaked.published = NOW + 24 * 3600;
    leaked.expires = leaked.published + DEFAULT_LIFETIME;
    let (body, signature) = as_files(&Signed::sign(&leaked, &keys.release).unwrap());
    let (held, role) = read(&keys.trusted, &body, &signature).unwrap();
    assert!(held.taken_by(role, runs, None).is_ok());

    let fix = manifest(
        runs + MAX_BUILD_STEP + 1,
        BTreeMap::from([(
            "linux-flatpak".into(),
            Artifact::of("fogata.pack", b"fixed"),
        )]),
    );
    let (body, signature) = as_files(&Signed::sign(&fix, &keys.rescue).unwrap());
    let (rescue, role) = read(&keys.trusted, &body, &signature).unwrap();
    assert_eq!(role, KeyRole::Rescue);
    assert!(rescue.current(NOW).is_ok());
    assert!(rescue.taken_by(role, runs, Some(&held)).is_ok());

    let (body, signature) = as_files(&Signed::sign(&fix, &keys.release).unwrap());
    let (same, role) = read(&keys.trusted, &body, &signature).unwrap();
    assert_eq!(role, KeyRole::Release);
    assert!(same.taken_by(role, runs, Some(&held)).is_err());
}

/// A package as it arrives, chunk by chunk: each chunk is checked against its hash in the
/// signed manifest, so one changed byte is caught in its own chunk and in the whole.
#[test]
fn a_package_with_a_changed_byte_is_caught_in_its_chunk() {
    let data = package(2 * CHUNK as usize + 1000, 3);
    let described = Artifact::of("Fogata.pack", &data);
    assert_eq!(described.chunk_count(), 3);
    assert_eq!(described.chunk_len(2), 1000);
    assert_eq!(described.chunk_hash(3), None);
    // Read piece by piece, as it is written to the disk, it is described the same.
    let mut builder = Builder::new("Fogata.pack");
    for piece in data.chunks(65_537) {
        builder.update(piece);
    }
    assert_eq!(builder.finish(), described);

    let chunk_ok = |bytes: &[u8], index: u64| {
        let from = (index * CHUNK) as usize;
        let to = from + described.chunk_len(index) as usize;
        Some(*blake3::hash(&bytes[from..to]).as_bytes()) == described.chunk_hash(index)
    };
    assert!((0..3).all(|i| chunk_ok(&data, i)));
    assert_eq!(*blake3::hash(&data).as_bytes(), described.hash());
    for at in [0, CHUNK as usize - 1, CHUNK as usize, data.len() - 1] {
        let mut changed = data.clone();
        changed[at] ^= 0x10;
        let damaged: Vec<u64> = (0..3).filter(|&i| !chunk_ok(&changed, i)).collect();
        assert_eq!(damaged, [at as u64 / CHUNK], "a byte changed at {at}");
        assert_ne!(*blake3::hash(&changed).as_bytes(), described.hash());
        assert_ne!(
            Artifact::of("Fogata.pack", &changed).sha256(),
            described.sha256()
        );
    }
    // A package cut short, or longer, is not the one described.
    assert_ne!(
        Artifact::of("Fogata.pack", &data[..data.len() - 1]),
        described
    );
}
