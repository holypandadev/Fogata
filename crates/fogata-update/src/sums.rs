//! The first installs of a release, for people who get them by hand from a shared folder:
//! `SHA256SUMS` names each file with its SHA-256, and `SHA256SUMS.sig` is a release key's
//! signature over it in the format of `ssh-keygen -Y sign` (namespace [`NAMESPACE`]), so it
//! can be checked with `fogata release verify-sums`, or with the `ssh-keygen` of Linux,
//! macOS and Windows and the line [`allowed_signers_line`] gives for the key.
//!
//! The signature is over `SSHSIG`, the namespace and the SHA-512 of the file, a message no
//! manifest signature ever covers (those start with [`crate::manifest::SIGNING_LABEL`]), so
//! the same key signing both can never make one stand for the other.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use fogata_crypto::{DeviceIdentity, DevicePublicKey};
use sha2::Digest as _;

use crate::{ReleaseKey, role_of};

pub const SUMS_FILE: &str = "SHA256SUMS";
pub const SIGNATURE_FILE: &str = "SHA256SUMS.sig";
/// What the signature says it is for, as `ssh-keygen -Y` names it.
pub const NAMESPACE: &str = "file";
/// The name the pre-release keys have in an allowed signers file. It is not
/// `fogata-release`, the name of the SSH key of public releases (docs/releasing.md), so that
/// a line trusting a pre-release key never lets it vouch for a public release, nor the other
/// way round.
pub const PRINCIPAL: &str = "fogata-prerelease";
/// The most files a list names, and the longest a name may be.
pub const MAX_FILES: usize = 256;
pub const MAX_NAME: usize = 128;

const MAGIC: &[u8; 6] = b"SSHSIG";
const VERSION: u32 = 1;
const KEY_TYPE: &str = "ssh-ed25519";
const BEGIN: &str = "-----BEGIN SSH SIGNATURE-----";
const END: &str = "-----END SSH SIGNATURE-----";
/// The longest signature file read: an Ed25519 signature takes under 300 bytes.
const MAX_SIGNATURE_FILE: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SumsError {
    #[error("{0}")]
    Malformed(String),
    #[error("the list is signed by a key this copy does not know")]
    UnknownKey,
    #[error("the list's signature is not good")]
    BadSignature,
}

fn malformed(what: &str) -> SumsError {
    SumsError::Malformed(what.to_string())
}

/// A file name the list may hold: letters, digits and `._+-`, not starting with a dot,
/// which reads the same in the list, in a shell and on every system.
pub fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
}

/// The SHA-256 of everything `reader` gives.
pub fn sha256(mut reader: impl std::io::Read) -> std::io::Result<[u8; 32]> {
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().into())
}

/// The text of `SHA256SUMS` for `files` (name, SHA-256), in the order given, as `sha256sum`
/// writes it.
pub fn write(files: &[(String, [u8; 32])]) -> Result<String, SumsError> {
    if files.is_empty() || files.len() > MAX_FILES {
        return Err(malformed("a list names between one and 256 files"));
    }
    let mut text = String::new();
    let mut seen = std::collections::HashSet::new();
    for (name, digest) in files {
        if !plain_name(name) || !seen.insert(name.to_ascii_lowercase()) {
            return Err(SumsError::Malformed(format!(
                "{name:?} cannot be in the list"
            )));
        }
        text.push_str(&format!("{}  {name}\n", hex::encode(digest)));
    }
    Ok(text)
}

/// Reads the text of `SHA256SUMS`: each line a lowercase SHA-256 in hex, two spaces and a
/// plain name, names not repeated (not even in another case).
pub fn read(text: &str) -> Result<Vec<(String, [u8; 32])>, SumsError> {
    let mut files = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if !text.ends_with('\n') {
        return Err(malformed("the list does not end with a new line"));
    }
    for line in text[..text.len() - 1].split('\n') {
        let (digest, name) = line
            .split_once("  ")
            .ok_or_else(|| SumsError::Malformed(format!("not a line of a list: {line:?}")))?;
        let digest: [u8; 32] = (digest.len() == 64
            && digest
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .then(|| hex::decode(digest).ok()?.try_into().ok())
        .flatten()
        .ok_or_else(|| SumsError::Malformed(format!("not a SHA-256: {digest:?}")))?;
        if !plain_name(name) || !seen.insert(name.to_ascii_lowercase()) {
            return Err(SumsError::Malformed(format!(
                "{name:?} cannot be in the list"
            )));
        }
        files.push((name.to_string(), digest));
    }
    if files.is_empty() || files.len() > MAX_FILES {
        return Err(malformed("a list names between one and 256 files"));
    }
    Ok(files)
}

/// Signs `message` (the bytes of `SHA256SUMS`) with `key`: the text of `SHA256SUMS.sig`.
pub fn sign(message: &[u8], key: &DeviceIdentity) -> String {
    let public = *key.public_key().as_bytes();
    let signature = key.sign(&signed_data(NAMESPACE, "sha512", message));
    let mut blob = MAGIC.to_vec();
    blob.extend_from_slice(&VERSION.to_be_bytes());
    put_string(&mut blob, &key_blob(&public));
    put_string(&mut blob, NAMESPACE.as_bytes());
    put_string(&mut blob, b"");
    put_string(&mut blob, b"sha512");
    let mut signature_blob = Vec::new();
    put_string(&mut signature_blob, KEY_TYPE.as_bytes());
    put_string(&mut signature_blob, &signature);
    put_string(&mut blob, &signature_blob);
    let encoded = STANDARD.encode(&blob);
    let mut text = format!("{BEGIN}\n");
    for line in encoded.as_bytes().chunks(70) {
        text.push_str(std::str::from_utf8(line).expect("base64 is ASCII"));
        text.push('\n');
    }
    text.push_str(END);
    text.push('\n');
    text
}

/// Checks `signature` (the text of `SHA256SUMS.sig`) over `message` with the keys in
/// `trusted`; gives the key that signed it.
pub fn verify(
    message: &[u8],
    signature: &str,
    trusted: &[ReleaseKey],
) -> Result<ReleaseKey, SumsError> {
    if signature.len() > MAX_SIGNATURE_FILE {
        return Err(malformed("the signature file is too large"));
    }
    let body = signature
        .trim()
        .strip_prefix(BEGIN)
        .and_then(|rest| rest.strip_suffix(END))
        .ok_or_else(|| malformed("not an SSH signature"))?;
    let encoded: String = body.split_whitespace().collect();
    let blob = STANDARD
        .decode(encoded)
        .map_err(|_| malformed("not an SSH signature"))?;
    let mut reader = Reader(&blob);
    if reader.take(MAGIC.len())? != MAGIC || reader.u32()? != VERSION {
        return Err(malformed(
            "not an SSH signature of a version this copy reads",
        ));
    }
    let public = {
        let mut key = Reader(reader.string()?);
        if key.string()? != KEY_TYPE.as_bytes() {
            return Err(SumsError::UnknownKey);
        }
        let public: [u8; 32] = key
            .string()?
            .try_into()
            .map_err(|_| SumsError::UnknownKey)?;
        key.end()?;
        public
    };
    let namespace = reader.string()?;
    let _reserved = reader.string()?;
    let hash = reader.string()?;
    let signature: [u8; 64] = {
        let mut sig = Reader(reader.string()?);
        if sig.string()? != KEY_TYPE.as_bytes() {
            return Err(SumsError::BadSignature);
        }
        let bytes = sig
            .string()?
            .try_into()
            .map_err(|_| SumsError::BadSignature)?;
        sig.end()?;
        bytes
    };
    reader.end()?;
    let role = role_of(trusted, &public).ok_or(SumsError::UnknownKey)?;
    if namespace != NAMESPACE.as_bytes() {
        return Err(malformed("the signature is not for a file"));
    }
    let hash = match hash {
        b"sha512" => "sha512",
        b"sha256" => "sha256",
        _ => {
            return Err(malformed(
                "the signature uses a hash this copy does not know",
            ));
        }
    };
    DevicePublicKey::from_bytes(&public)
        .and_then(|key| key.verify(&signed_data(NAMESPACE, hash, message), &signature))
        .map_err(|_| SumsError::BadSignature)?;
    Ok(ReleaseKey { role, key: public })
}

/// The key as OpenSSH writes a public key: `ssh-ed25519 AAAA...`.
pub fn ssh_public(key: &[u8; 32]) -> String {
    format!("{KEY_TYPE} {}", STANDARD.encode(key_blob(key)))
}

/// The line of an allowed signers file (`ssh-keygen -Y verify -f <file> -I fogata-prerelease
/// -n file`) that trusts `key` for these lists.
pub fn allowed_signers_line(key: &[u8; 32]) -> String {
    format!("{PRINCIPAL} namespaces=\"{NAMESPACE}\" {}", ssh_public(key))
}

/// What an SSH signature signs: the magic, the namespace, an empty reserved field, the hash's
/// name and the message's hash.
fn signed_data(namespace: &str, hash: &str, message: &[u8]) -> Vec<u8> {
    let digest = match hash {
        "sha256" => sha2::Sha256::digest(message).to_vec(),
        _ => sha2::Sha512::digest(message).to_vec(),
    };
    let mut data = MAGIC.to_vec();
    put_string(&mut data, namespace.as_bytes());
    put_string(&mut data, b"");
    put_string(&mut data, hash.as_bytes());
    put_string(&mut data, &digest);
    data
}

fn key_blob(key: &[u8; 32]) -> Vec<u8> {
    let mut blob = Vec::new();
    put_string(&mut blob, KEY_TYPE.as_bytes());
    put_string(&mut blob, key);
    blob
}

fn put_string(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
}

/// Reads the fields of an SSH wire blob.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], SumsError> {
        if self.0.len() < n {
            return Err(malformed("the signature is cut short"));
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }

    fn u32(&mut self) -> Result<u32, SumsError> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes(bytes.try_into().expect("four bytes")))
    }

    fn string(&mut self) -> Result<&'a [u8], SumsError> {
        let len = self.u32()? as usize;
        self.take(len)
    }

    fn end(&self) -> Result<(), SumsError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(malformed("the signature has bytes after its end"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KeyRole;

    /// A key made with `ssh-keygen -t ed25519` for this test only, its seed, and the
    /// signature `ssh-keygen -Y sign -n file` made with it over [`MESSAGE`].
    const SEED: &str = "2f5fc17b91ece436a50429a9f0866d6c55d4083dd44bb69c3ffef71580c35c98";
    const PUBLIC_LINE: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIKBYbws15Z38EYbr1Z/rnH2JFrKFvBc8cQ9UiG5VcJZU";
    const MESSAGE: &[u8] = b"test vector for the SHA256SUMS signature\n";
    const SSH_KEYGEN_SIGNATURE: &str = concat!(
        "-----BEGIN SSH SIGNATURE-----\n",
        "U1NIU0lHAAAAAQAAADMAAAALc3NoLWVkMjU1MTkAAAAgoFhvCzXlnfwRhuvVn+ucfYkWso\n",
        "W8FzxxD1SIblVwllQAAAAEZmlsZQAAAAAAAAAGc2hhNTEyAAAAUwAAAAtzc2gtZWQyNTUx\n",
        "OQAAAEDd6goBnDBtpKYFJM3IEcm3V48OQYbVUPI5ur1ZnlI4oW3M866qD+16L3QHZL8Z93\n",
        "/Wy/xOLYw3toykMu6fR6sP\n",
        "-----END SSH SIGNATURE-----\n",
    );

    fn test_key() -> DeviceIdentity {
        DeviceIdentity::from_secret_bytes(&hex::decode(SEED).unwrap().try_into().unwrap())
    }

    #[test]
    fn signatures_are_those_of_ssh_keygen() {
        let key = test_key();
        let public = *key.public_key().as_bytes();
        assert_eq!(ssh_public(&public), PUBLIC_LINE);
        // Ed25519 is deterministic: the same key and message give ssh-keygen's bytes.
        assert_eq!(sign(MESSAGE, &key), SSH_KEYGEN_SIGNATURE);
        let trusted = [ReleaseKey::release(public)];
        assert_eq!(
            verify(MESSAGE, SSH_KEYGEN_SIGNATURE, &trusted),
            Ok(trusted[0])
        );
        assert_eq!(
            allowed_signers_line(&public),
            format!("fogata-prerelease namespaces=\"file\" {PUBLIC_LINE}")
        );
    }

    #[test]
    fn a_signature_holds_for_its_message_and_key_only() {
        let key = DeviceIdentity::generate().unwrap();
        let rescue = DeviceIdentity::generate().unwrap();
        let trusted = [
            ReleaseKey::release(*key.public_key().as_bytes()),
            ReleaseKey::rescue(*rescue.public_key().as_bytes()),
        ];
        let text = write(&[
            ("Fogata-0.1.0-beta.2-windows-x64.zip".into(), [7; 32]),
            ("README.txt".into(), [9; 32]),
        ])
        .unwrap();
        let signature = sign(text.as_bytes(), &key);
        assert_eq!(
            verify(text.as_bytes(), &signature, &trusted),
            Ok(trusted[0])
        );
        let by_rescue = sign(text.as_bytes(), &rescue);
        assert_eq!(
            verify(text.as_bytes(), &by_rescue, &trusted).map(|k| k.role),
            Ok(KeyRole::Rescue)
        );
        // Another key.
        assert_eq!(
            verify(text.as_bytes(), &signature, &trusted[1..]),
            Err(SumsError::UnknownKey)
        );
        // Another message.
        let changed = text.replace("README", "READMF");
        assert_eq!(
            verify(changed.as_bytes(), &signature, &trusted),
            Err(SumsError::BadSignature)
        );
        // A trusted key named over another key's signature.
        let other = DeviceIdentity::generate().unwrap();
        let theirs = sign(text.as_bytes(), &other);
        let body: String = theirs.lines().filter(|l| !l.starts_with("-----")).collect();
        let mut blob = STANDARD.decode(body).unwrap();
        let at = blob
            .windows(32)
            .position(|w| w == other.public_key().as_bytes())
            .unwrap();
        blob[at..at + 32].copy_from_slice(key.public_key().as_bytes());
        let swapped = format!("{BEGIN}\n{}\n{END}\n", STANDARD.encode(&blob));
        assert_eq!(
            verify(text.as_bytes(), &swapped, &trusted),
            Err(SumsError::BadSignature)
        );
        // A manifest signature of the same key over the same bytes is no list signature.
        let mut labelled = crate::manifest::SIGNING_LABEL.to_vec();
        labelled.extend_from_slice(text.as_bytes());
        assert_ne!(
            key.sign(&labelled),
            key.sign(&signed_data(NAMESPACE, "sha512", text.as_bytes()))
        );
        // Not a signature at all, or cut short.
        assert!(verify(text.as_bytes(), "hello", &trusted).is_err());
        let short = format!("{BEGIN}\n{}\n{END}\n", STANDARD.encode(b"SSHSIG\0\0\0\x01"));
        assert!(verify(text.as_bytes(), &short, &trusted).is_err());
    }

    #[test]
    fn lists_are_plain() {
        let files = vec![("a.zip".to_string(), [1u8; 32]), ("b.txt".into(), [2; 32])];
        let text = write(&files).unwrap();
        assert_eq!(read(&text).unwrap(), files);
        for bad in [
            "",
            "0101  a.zip\n",
            &format!("{}  ../a.zip\n", "01".repeat(32)),
            &format!("{}  .hidden\n", "01".repeat(32)),
            &format!("{}  a b.zip\n", "01".repeat(32)),
            &format!("{} a.zip\n", "01".repeat(32)),
            &format!("{}  a.zip", "01".repeat(32)),
            &format!("{}  a.zip\n", "0A".repeat(32)),
            &format!("{0}  a.zip\n{0}  A.zip\n", "01".repeat(32)),
        ] {
            assert!(read(bad).is_err(), "{bad:?} was read");
        }
        assert!(write(&[("../x".into(), [0; 32])]).is_err());
        assert!(write(&[]).is_err());
    }
}
