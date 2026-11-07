//! The release key on the publisher's computer: a folder outside any repository with the
//! secret (`release-key`, readable by its owner only) and the public half
//! (`release-key.pub`, hex). The rescue key is made the same way, as `rescue-key` and
//! `rescue-key.pub`, in a folder of its own on a drive kept offline. The secret is never
//! printed.
//!
//! The secret file is sealed with a passphrase unless the publisher asks for it plain: the
//! passphrase is stretched with Argon2id (a random salt, [`ARGON2_MEMORY_KIB`] of memory,
//! [`ARGON2_PASSES`] passes) into a key that seals the 32 secret bytes with
//! XChaCha20-Poly1305, so a copy of the file (a backup, a synced folder, malware reading the
//! disk) is no release key without the passphrase. A plain file is the secret in hex.

use std::{
    fs,
    io::{self, Write as _},
    path::Path,
};

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    KeyInit as _, XChaCha20Poly1305, XNonce,
    aead::{Aead as _, Payload},
};
use fogata_crypto::DeviceIdentity;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::KeyRole;

pub const SECRET_FILE: &str = "release-key";
pub const PUBLIC_FILE: &str = "release-key.pub";
pub const RESCUE_SECRET_FILE: &str = "rescue-key";
pub const RESCUE_PUBLIC_FILE: &str = "rescue-key.pub";

/// The secret and public files of a key of `role`.
pub fn files(role: KeyRole) -> (&'static str, &'static str) {
    match role {
        KeyRole::Release => (SECRET_FILE, PUBLIC_FILE),
        KeyRole::Rescue => (RESCUE_SECRET_FILE, RESCUE_PUBLIC_FILE),
    }
}

/// What a sealed secret file says it is.
const SEALED_FORMAT: &str = "fogata-release-key/1";
/// What the seal binds the secret to, so the same passphrase sealing anything else can
/// never stand for a release key.
const SEAL_LABEL: &[u8] = b"fogata/v1/release-key";
/// Argon2id cost: 128 MiB and three passes, well under a second on a laptop, long for a
/// program guessing passphrases.
pub const ARGON2_MEMORY_KIB: u32 = 128 * 1024;
pub const ARGON2_PASSES: u32 = 3;
/// The fewest characters a passphrase may have.
pub const MIN_PASSPHRASE: usize = 8;

/// The sealed secret file.
#[derive(Serialize, Deserialize)]
struct Sealed {
    format: String,
    kdf: String,
    memory_kib: u32,
    passes: u32,
    /// Hex.
    salt: String,
    nonce: String,
    sealed: String,
}

/// What the Flatpak signing key leaves in a folder: its GnuPG home and its public half.
const FLATPAK_KEY_FILES: [&str; 2] = ["gnupg", "flatpak-signing-key.gpg"];

/// Makes a new release key in `dir`, which is made readable by its owner only, sealed with
/// `passphrase` (plain, in hex, with `None`). Refuses to replace a key already there.
/// Returns the public half.
pub fn generate(dir: &Path, passphrase: Option<&str>) -> io::Result<[u8; 32]> {
    generate_as(dir, KeyRole::Release, passphrase)
}

/// Makes a new key of `role` in `dir`, as [`generate`]. Refuses a folder that holds a key of
/// either role: the rescue key is kept apart from the release key, so that whoever takes one
/// does not have the other.
pub fn generate_as(dir: &Path, role: KeyRole, passphrase: Option<&str>) -> io::Result<[u8; 32]> {
    let (secret_file, public_file) = files(role);
    let secret_path = dir.join(secret_file);
    let taken = [KeyRole::Release, KeyRole::Rescue]
        .into_iter()
        .flat_map(|role| <[&str; 2]>::from(files(role)))
        .any(|name| dir.join(name).exists());
    if taken {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "a release or rescue key is already in {}: each key has a folder of its own",
                dir.display()
            ),
        ));
    }
    // Nor beside the Flatpak signing key (tools/beta/flatpak-key.sh), which on Linux can
    // make copies run code by itself.
    if FLATPAK_KEY_FILES.iter().any(|name| dir.join(name).exists()) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "the Flatpak signing key is in {}: each key has a folder of its own",
                dir.display()
            ),
        ));
    }
    if let Some(passphrase) = passphrase {
        check_passphrase(passphrase)?;
    }
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        // A FAT or exFAT drive refuses (or ignores) modes: what is made there is checked
        // below.
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}. {DRIVES}", dir.display())))?;
    }
    let identity = DeviceIdentity::generate().map_err(io::Error::other)?;
    let text = match passphrase {
        Some(passphrase) => seal(&identity.secret_bytes(), passphrase)?,
        None => Zeroizing::new(hex::encode(identity.secret_bytes().as_ref()) + "\n"),
    };
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(&secret_path)?;
    // A drive that shows every file to others (FAT, exFAT) would keep a key that no command
    // here opens: none is made there.
    if let Err(e) = check_private(&secret_path) {
        drop(file);
        let _ = fs::remove_file(&secret_path);
        return Err(e);
    }
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    let public = *identity.public_key().as_bytes();
    fs::write(dir.join(public_file), hex::encode(public) + "\n")?;
    Ok(public)
}

/// Whether the secret file at `path` is sealed with a passphrase.
pub fn is_sealed(path: &Path) -> io::Result<bool> {
    let text = Zeroizing::new(fs::read_to_string(path)?);
    Ok(text.trim_start().starts_with('{'))
}

/// Reads the release key from its secret file, asking `passphrase` for the passphrase when
/// the file is sealed. Refuses a file others may read.
pub fn load(
    path: &Path,
    passphrase: impl FnOnce() -> io::Result<Zeroizing<String>>,
) -> io::Result<DeviceIdentity> {
    check_private(path)?;
    let text = Zeroizing::new(fs::read_to_string(path)?);
    let secret = if text.trim_start().starts_with('{') {
        open(&text, &passphrase()?).map_err(|e| match e.kind() {
            io::ErrorKind::InvalidInput => e,
            _ => bad(path),
        })?
    } else {
        let bytes = Zeroizing::new(hex::decode(text.trim()).map_err(|_| bad(path))?);
        let secret: &[u8; 32] = bytes.as_slice().try_into().map_err(|_| bad(path))?;
        Zeroizing::new(*secret)
    };
    Ok(DeviceIdentity::from_secret_bytes(&secret))
}

/// Where a key can be kept readable by its owner only.
const DRIVES: &str = "Keep keys on a drive that keeps files private: ext4 on Linux, APFS on \
                      macOS, or a FAT or exFAT stick mounted with fmask=077,dmask=077, where \
                      chmod does nothing";

/// Refuses the secret file at `path` when others may read it.
fn check_private(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        private_mode(fs::metadata(path)?.permissions().mode(), path)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Refuses a secret file whose `mode` lets others read it, saying how to keep it private.
#[cfg_attr(not(unix), allow(dead_code))]
fn private_mode(mode: u32, path: &Path) -> io::Result<()> {
    if mode & 0o077 == 0 {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!(
            "{} may be read by others (mode {:o}): chmod 600 it. {DRIVES}.",
            path.display(),
            mode & 0o777
        ),
    ))
}

/// Refuses a passphrase too short to be one.
pub fn check_passphrase(passphrase: &str) -> io::Result<()> {
    if passphrase.chars().count() < MIN_PASSPHRASE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("the passphrase must have at least {MIN_PASSPHRASE} characters"),
        ));
    }
    Ok(())
}

/// Seals `secret` with `passphrase` into the text of a sealed secret file.
fn seal(secret: &[u8; 32], passphrase: &str) -> io::Result<Zeroizing<String>> {
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    fogata_crypto::random_bytes(&mut salt).map_err(io::Error::other)?;
    fogata_crypto::random_bytes(&mut nonce).map_err(io::Error::other)?;
    let key = stretch(passphrase, &salt, ARGON2_MEMORY_KIB, ARGON2_PASSES)?;
    let sealed = XChaCha20Poly1305::new(&(*key).into())
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: secret,
                aad: SEAL_LABEL,
            },
        )
        .map_err(|_| io::Error::other("cannot seal the release key"))?;
    let file = Sealed {
        format: SEALED_FORMAT.into(),
        kdf: "argon2id".into(),
        memory_kib: ARGON2_MEMORY_KIB,
        passes: ARGON2_PASSES,
        salt: hex::encode(salt),
        nonce: hex::encode(nonce),
        sealed: hex::encode(sealed),
    };
    let text = serde_json::to_string_pretty(&file).map_err(io::Error::other)?;
    Ok(Zeroizing::new(text + "\n"))
}

/// Opens the text of a sealed secret file with `passphrase`. A wrong passphrase is an
/// `InvalidInput` error; anything else wrong with the file, `InvalidData`.
fn open(text: &str, passphrase: &str) -> io::Result<Zeroizing<[u8; 32]>> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "not a sealed release key");
    let file: Sealed = serde_json::from_str(text).map_err(|_| invalid())?;
    if file.format != SEALED_FORMAT || file.kdf != "argon2id" {
        return Err(invalid());
    }
    // The cost written is read back, so a key made with other costs still opens, but never
    // one that would make this program allocate without bound.
    if file.memory_kib > 4 * 1024 * 1024 || file.passes > 64 {
        return Err(invalid());
    }
    let salt = hex::decode(&file.salt).map_err(|_| invalid())?;
    let nonce: [u8; 24] = hex::decode(&file.nonce)
        .map_err(|_| invalid())?
        .try_into()
        .map_err(|_| invalid())?;
    let sealed = hex::decode(&file.sealed).map_err(|_| invalid())?;
    if salt.len() != 16 || sealed.len() != 32 + 16 {
        return Err(invalid());
    }
    let key = stretch(passphrase, &salt, file.memory_kib, file.passes)?;
    let opened = XChaCha20Poly1305::new(&(*key).into())
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: &sealed,
                aad: SEAL_LABEL,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "the passphrase is wrong, or the key file is damaged",
            )
        })?;
    let secret: &[u8; 32] = opened.as_slice().try_into().map_err(|_| invalid())?;
    Ok(Zeroizing::new(*secret))
}

fn stretch(
    passphrase: &str,
    salt: &[u8],
    memory_kib: u32,
    passes: u32,
) -> io::Result<Zeroizing<[u8; 32]>> {
    let params = Params::new(memory_kib, passes, 1, Some(32))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, key.as_mut())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    Ok(key)
}

/// Reads a public key: 64 hex characters, from text or a `release-key.pub` file.
pub fn parse_public(text: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(text.trim()).ok()?;
    let key: [u8; 32] = bytes.try_into().ok()?;
    fogata_crypto::DevicePublicKey::from_bytes(&key).ok()?;
    Some(key)
}

fn bad(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{} is not a release key", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fogata-release-key-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn never() -> io::Result<Zeroizing<String>> {
        panic!("a plain key asks for no passphrase")
    }

    #[test]
    fn a_plain_key_is_made_once_and_kept_private() {
        let dir = folder("plain");
        let public = generate(&dir, None).unwrap();
        assert!(generate(&dir, None).is_err());
        assert!(!is_sealed(&dir.join(SECRET_FILE)).unwrap());
        let key = load(&dir.join(SECRET_FILE), never).unwrap();
        assert_eq!(*key.public_key().as_bytes(), public);
        let text = fs::read_to_string(dir.join(PUBLIC_FILE)).unwrap();
        assert_eq!(parse_public(&text), Some(public));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let secret = dir.join(SECRET_FILE);
            assert_eq!(
                fs::metadata(&secret).unwrap().permissions().mode() & 0o777,
                0o600
            );
            fs::set_permissions(&secret, fs::Permissions::from_mode(0o644)).unwrap();
            assert!(load(&secret, never).is_err());
        }
        let _ = fs::remove_dir_all(&dir);
    }

    /// A FAT or exFAT stick shows every file with the mask it was mounted with, and chmod
    /// changes nothing there: the key's mode as such a drive gives it is refused, and the
    /// error says which drives and mounts keep it private.
    #[test]
    fn a_drive_that_shows_keys_to_others_is_named_as_the_problem() {
        let path = Path::new("/media/bea/RESCUE/fogata-rescue/rescue-key");
        // vfat and exfat with the default masks (0022), and with fmask=077,dmask=077.
        for mode in [0o100755, 0o100644, 0o100777] {
            let error = private_mode(mode, path).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            let text = error.to_string();
            assert!(text.contains("fmask=077,dmask=077"), "{text}");
            assert!(text.contains("ext4") && text.contains("APFS"), "{text}");
        }
        private_mode(0o100600, path).unwrap();
        private_mode(0o100700, path).unwrap();
    }

    #[test]
    fn a_rescue_key_is_made_apart_from_the_release_key() {
        let dir = folder("rescue");
        let public = generate_as(&dir, KeyRole::Rescue, None).unwrap();
        let key = load(&dir.join(RESCUE_SECRET_FILE), never).unwrap();
        assert_eq!(*key.public_key().as_bytes(), public);
        let text = fs::read_to_string(dir.join(RESCUE_PUBLIC_FILE)).unwrap();
        assert_eq!(parse_public(&text), Some(public));
        assert!(!dir.join(SECRET_FILE).exists());
        // Not beside it, in either order.
        assert!(generate(&dir, None).is_err());
        let other = folder("rescue-other");
        generate(&other, None).unwrap();
        assert!(generate_as(&other, KeyRole::Rescue, None).is_err());
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&other);
    }

    #[test]
    fn a_key_is_not_made_beside_the_flatpak_signing_key() {
        for name in FLATPAK_KEY_FILES {
            let dir = folder(&format!("beside-{name}"));
            fs::create_dir_all(dir.join(name)).unwrap();
            let error = generate(&dir, None).unwrap_err();
            assert!(error.to_string().contains("Flatpak signing key"), "{error}");
            assert!(generate_as(&dir, KeyRole::Rescue, None).is_err());
            assert!(!dir.join(SECRET_FILE).exists());
            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn a_sealed_key_opens_with_its_passphrase_only() {
        let dir = folder("sealed");
        assert!(generate(&dir, Some("short")).is_err());
        assert!(!dir.join(SECRET_FILE).exists());
        let public = generate(&dir, Some("correct horse battery")).unwrap();
        let secret = dir.join(SECRET_FILE);
        assert!(is_sealed(&secret).unwrap());
        let text = fs::read_to_string(&secret).unwrap();
        assert!(text.contains("fogata-release-key/1"));
        // The secret is not in the file in any plain form.
        let key = load(&secret, || {
            Ok(Zeroizing::new("correct horse battery".into()))
        })
        .unwrap();
        assert_eq!(*key.public_key().as_bytes(), public);
        assert!(!text.contains(&hex::encode(key.secret_bytes().as_ref())));
        let wrong = load(&secret, || {
            Ok(Zeroizing::new("correct horse batterx".into()))
        });
        assert_eq!(wrong.unwrap_err().kind(), io::ErrorKind::InvalidInput);
        // A changed byte of the sealed secret is caught.
        let mut file: Sealed = serde_json::from_str(&text).unwrap();
        let mut bytes = hex::decode(&file.sealed).unwrap();
        bytes[5] ^= 1;
        file.sealed = hex::encode(bytes);
        fs::write(&secret, serde_json::to_string(&file).unwrap()).unwrap();
        assert!(
            load(&secret, || Ok(Zeroizing::new(
                "correct horse battery".into()
            )))
            .is_err()
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
