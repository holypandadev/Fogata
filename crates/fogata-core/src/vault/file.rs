//! Data keys kept as files sealed with a secret that lives elsewhere ([`FileKeyStore`]), and
//! the move of keys from one store to another as they are asked for ([`MovingKeyStore`]).
//!
//! The Flatpak keeps its data keys this way, sealed with the secret the Secret portal gives
//! it (its own, which the desktop's keychain keeps and gives to no other application), in
//! place of reaching the whole Secret Service; keys an earlier Flatpak kept in the Secret
//! Service move over the first time they are asked for.

use std::{
    fmt, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use fogata_crypto::at_rest::DataKey;
use zeroize::Zeroizing;

use super::KeyStore;

/// What the key that seals the files is derived for, from the secret given.
const DERIVE_SALT: &[u8] = b"fogata/v1/keychain-files";

/// Keys kept as files in a folder of their own, one per name, each sealed with a key
/// derived from a secret kept elsewhere, and bound to its name: a file moved to another
/// name does not open. Files are written whole or not at all (a new file, then renamed), so a
/// write cut short leaves the key as it was.
pub struct FileKeyStore {
    dir: PathBuf,
    key: DataKey,
}

impl fmt::Debug for FileKeyStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileKeyStore")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

impl FileKeyStore {
    /// The store in `dir`, its files sealed with a key derived from `secret`.
    pub fn new(dir: PathBuf, secret: &[u8]) -> Self {
        let derived = Zeroizing::new(fogata_crypto::derive_secret(
            DERIVE_SALT,
            secret,
            b"data keys",
        ));
        Self {
            dir,
            key: DataKey::from_bytes(&derived),
        }
    }

    /// The file of `name`: names are those the vault makes (`data-` and hex), and anything
    /// else (a `sealed.json` changed by hand) is refused before it can name a path.
    fn path(&self, name: &str) -> Result<PathBuf, String> {
        let plain = !name.is_empty()
            && name.len() <= 64
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        if !plain {
            return Err(format!("{name:?} is not the name of a data key"));
        }
        Ok(self.dir.join(format!("{name}.key")))
    }

    fn context(name: &str) -> String {
        format!("keychain {name}")
    }
}

impl KeyStore for FileKeyStore {
    fn get(&self, name: &str) -> Result<Option<Zeroizing<String>>, String> {
        let path = self.path(name)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
        };
        let plain = self
            .key
            .open_whole(&Self::context(name), &bytes)
            .map_err(|_| {
                format!(
                    "{} does not open with this application's secret",
                    path.display()
                )
            })?;
        let text =
            std::str::from_utf8(&plain).map_err(|_| format!("{} holds no key", path.display()))?;
        Ok(Some(Zeroizing::new(text.to_string())))
    }

    fn set(&self, name: &str, secret: &str) -> Result<(), String> {
        let path = self.path(name)?;
        make_private_dir(&self.dir)?;
        let sealed = self
            .key
            .seal_whole(&Self::context(name), secret.as_bytes())
            .map_err(|e| e.to_string())?;
        crate::store::write_private(&path, &sealed).map_err(|e| format!("{e:#}"))
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        let path = self.path(name)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("cannot remove {}: {e}", path.display())),
        }
    }
}

/// Makes `dir`, readable by its owner only.
fn make_private_dir(dir: &Path) -> Result<(), String> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(dir)
        .map_err(|e| format!("cannot make {}: {e}", dir.display()))
}

/// Keys kept in `new`, and those still in `old` moved into `new` the first time they are
/// asked for: written there, read back the same, and only then given. The key in `old` is
/// left as it was, so a move cut short at any point leaves it where it was found, and the next
/// start moves it again; a key `new` already holds is never asked of `old`. New keys go to
/// `new` only.
#[derive(Debug)]
pub struct MovingKeyStore {
    pub new: Arc<dyn KeyStore>,
    pub old: Arc<dyn KeyStore>,
    /// What to tell the person when `old` cannot be reached and may hold the key.
    pub old_unreachable: String,
}

impl KeyStore for MovingKeyStore {
    fn get(&self, name: &str) -> Result<Option<Zeroizing<String>>, String> {
        let in_new = self.new.get(name);
        if let Ok(Some(secret)) = in_new {
            return Ok(Some(secret));
        }
        match self.old.get(name) {
            Ok(Some(secret)) => {
                // `new` could not give it (it lacks it, or it cannot be reached or opened
                // now): move it, and give it all the same if that fails, from `old`.
                let moved =
                    self.new
                        .set(name, &secret)
                        .and_then(|()| match self.new.get(name)? {
                            Some(back) if *back == *secret => Ok(()),
                            _ => Err("it did not give the key back".to_string()),
                        });
                match moved {
                    Ok(()) => tracing::info!("moved the data key {name} to the new keychain"),
                    Err(e) => tracing::warn!(
                        "the data key {name} could not be moved to the new keychain ({e}); \
                         it was read from the old one"
                    ),
                }
                Ok(Some(secret))
            }
            Ok(None) => in_new,
            Err(old) => match in_new {
                Err(new) => Err(new),
                Ok(_) => Err(format!("{} ({old})", self.old_unreachable)),
            },
        }
    }

    fn set(&self, name: &str, secret: &str) -> Result<(), String> {
        self.new.set(name, secret)
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        self.new.delete(name)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::vault::{Keychain, MemoryKeyStore, open, try_create};

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fogata-keychain-files-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn files_open_with_their_secret_and_name_only() {
        let dir = folder("files");
        let store = FileKeyStore::new(dir.join("keys"), b"the application's secret");
        assert_eq!(store.get("data-00ff").unwrap(), None);
        store.set("data-00ff", "a1b2").unwrap();
        assert_eq!(
            store
                .get("data-00ff")
                .unwrap()
                .as_deref()
                .map(|s| s.as_str()),
            Some("a1b2")
        );
        // Again, with the same secret: the same key.
        let again = FileKeyStore::new(dir.join("keys"), b"the application's secret");
        assert_eq!(
            again
                .get("data-00ff")
                .unwrap()
                .as_deref()
                .map(|s| s.as_str()),
            Some("a1b2")
        );
        // Another secret, or the file under another name: nothing opens.
        let other = FileKeyStore::new(dir.join("keys"), b"another secret");
        assert!(
            other
                .get("data-00ff")
                .unwrap_err()
                .contains("does not open")
        );
        fs::copy(
            dir.join("keys/data-00ff.key"),
            dir.join("keys/data-11ee.key"),
        )
        .unwrap();
        assert!(
            store
                .get("data-11ee")
                .unwrap_err()
                .contains("does not open")
        );
        // Nothing but a key's name makes a path.
        for name in [
            "",
            "../data-00ff",
            "data/00ff",
            "data-00ff.key",
            ".",
            &"a".repeat(65),
        ] {
            assert!(store.get(name).is_err(), "{name:?}");
            assert!(store.set(name, "00").is_err(), "{name:?}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&dir.join("keys")), 0o700);
            assert_eq!(mode(&dir.join("keys/data-00ff.key")), 0o600);
        }
        store.delete("data-00ff").unwrap();
        store.delete("data-00ff").unwrap();
        assert_eq!(store.get("data-00ff").unwrap(), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_write_cut_short_leaves_the_key_as_it_was() {
        let dir = folder("cut");
        let store = FileKeyStore::new(dir.join("keys"), b"secret");
        store.set("data-01", "first").unwrap();
        crate::store::faults::fail_after(Some(0));
        assert!(store.set("data-01", "second").is_err());
        assert!(store.set("data-02", "second").is_err());
        crate::store::faults::fail_after(None);
        assert_eq!(
            store.get("data-01").unwrap().as_deref().map(|s| s.as_str()),
            Some("first")
        );
        assert_eq!(store.get("data-02").unwrap(), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    fn moving(dir: &Path) -> (Arc<MemoryKeyStore>, Arc<FileKeyStore>, MovingKeyStore) {
        let old = Arc::new(MemoryKeyStore::default());
        let new = Arc::new(FileKeyStore::new(dir.join("keys"), b"portal secret"));
        let store = MovingKeyStore {
            new: new.clone(),
            old: old.clone(),
            old_unreachable: "allow the old keychain once".into(),
        };
        (old, new, store)
    }

    /// A folder sealed while its key was kept in the old store opens through the moving
    /// store, which leaves the key in both; it opens from the new one alone after that.
    #[test]
    fn a_sealed_folder_s_key_moves_and_the_folder_opens_after() {
        let dir = folder("moves");
        let data = dir.join("data");
        fs::create_dir_all(&data).unwrap();
        let (old, new, store) = moving(&dir);
        let key = try_create(&data, old.as_ref()).unwrap().unwrap();
        let name = old.names().pop().unwrap();
        assert_eq!(new.get(&name).unwrap(), None);

        let store = Arc::new(store);
        let opened = open(&data, &Keychain::Open(store.clone()))
            .unwrap()
            .unwrap();
        assert_eq!(opened.as_bytes(), key.as_bytes());
        assert!(new.get(&name).unwrap().is_some());
        assert!(old.get(&name).unwrap().is_some(), "the old key stays");

        // The old store out of reach now (its permission gone): the folder opens all the same.
        old.unreachable.store(true, Ordering::Relaxed);
        let opened = open(&data, &Keychain::Open(store.clone()))
            .unwrap()
            .unwrap();
        assert_eq!(opened.as_bytes(), key.as_bytes());
        // New folders' keys go to the new store only.
        let other = dir.join("other");
        fs::create_dir_all(&other).unwrap();
        try_create(&other, store.as_ref()).unwrap().unwrap();
        assert_eq!(old.names().len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A move cut short (the new store's write fails, the computer stops) gives the key from
    /// the old store, loses nothing, and is made again at the next start.
    #[test]
    fn a_move_cut_short_is_made_again() {
        let dir = folder("again");
        let data = dir.join("data");
        fs::create_dir_all(&data).unwrap();
        let (old, new, store) = moving(&dir);
        let key = try_create(&data, old.as_ref()).unwrap().unwrap();
        let name = old.names().pop().unwrap();
        let store = Arc::new(store);

        crate::store::faults::fail_after(Some(0));
        let opened = open(&data, &Keychain::Open(store.clone()));
        crate::store::faults::fail_after(None);
        assert_eq!(opened.unwrap().unwrap().as_bytes(), key.as_bytes());
        assert_eq!(new.get(&name).unwrap(), None);
        assert!(
            fs::read_dir(dir.join("keys")).unwrap().next().is_none(),
            "nothing half written"
        );

        let opened = open(&data, &Keychain::Open(store.clone()))
            .unwrap()
            .unwrap();
        assert_eq!(opened.as_bytes(), key.as_bytes());
        assert!(new.get(&name).unwrap().is_some());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A key only the old store holds, while it cannot be reached: an error that says how to
    /// let it be reached, never a folder taken as without a key.
    #[test]
    fn a_key_left_in_an_old_store_out_of_reach_says_how_to_reach_it() {
        let dir = folder("unreached");
        let data = dir.join("data");
        fs::create_dir_all(&data).unwrap();
        let (old, new, store) = moving(&dir);
        try_create(&data, old.as_ref()).unwrap().unwrap();
        let name = old.names().pop().unwrap();
        old.unreachable.store(true, Ordering::Relaxed);
        let error = format!(
            "{:#}",
            open(&data, &Keychain::Open(Arc::new(store))).unwrap_err()
        );
        assert!(error.contains("allow the old keychain once"), "{error}");
        assert_eq!(new.get(&name).unwrap(), None);
        old.unreachable.store(false, Ordering::Relaxed);
        assert!(old.get(&name).unwrap().is_some());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A new store that cannot open its file (the desktop's secret changed) gives way to the
    /// old store's key, which is written again under the secret it has now.
    #[test]
    fn a_new_store_that_cannot_open_its_file_takes_the_old_key_again() {
        let dir = folder("reset");
        let data = dir.join("data");
        fs::create_dir_all(&data).unwrap();
        let (old, _, store) = moving(&dir);
        let key = try_create(&data, old.as_ref()).unwrap().unwrap();
        let name = old.names().pop().unwrap();
        let store = Arc::new(store);
        open(&data, &Keychain::Open(store.clone()))
            .unwrap()
            .unwrap();
        let reset = Arc::new(FileKeyStore::new(dir.join("keys"), b"a new portal secret"));
        assert!(reset.get(&name).is_err());
        let store = Arc::new(MovingKeyStore {
            new: reset.clone(),
            old: old.clone(),
            old_unreachable: String::new(),
        });
        let opened = open(&data, &Keychain::Open(store)).unwrap().unwrap();
        assert_eq!(opened.as_bytes(), key.as_bytes());
        assert!(reset.get(&name).unwrap().is_some());
        fs::remove_dir_all(&dir).unwrap();
    }
}
