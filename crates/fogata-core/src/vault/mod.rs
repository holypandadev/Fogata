//! The data key of a data folder, which the system keychain keeps: Keychain Services on
//! macOS, the Credential Manager on Windows (protected with the user's DPAPI key), the
//! Secret Service elsewhere (GNOME Keyring, KWallet). The Flatpak, which is not let reach the
//! Secret Service (it would reach every secret of the person there), keeps each key in a file
//! of its own sealed with the secret the Secret portal gives it alone ([`file`],
//! `secret_portal`), and moves there a key an earlier Flatpak kept in the Secret Service the
//! first time it is asked for. With it the folder's secrets are
//! sealed on the disk (see [`crate::store`]): the identity, the friends file, and direct
//! chats' journals, pins, profiles and attachments.
//!
//! A folder whose data is sealed holds `sealed.json`, which names its key in the keychain and
//! holds a box that only that key opens. Such a folder never opens without its key: a
//! keychain that cannot be reached, or that lacks the key or holds another one, is an error
//! that says so, never a reason to start the device afresh. The identity and the friends
//! file name the key too, so a folder that lost `sealed.json` finds it again: keychains
//! cannot list what they hold, only give what is asked for by name.
//!
//! A folder without `sealed.json` is sealed the first time an application that may seal
//! (see [`Keychain`]) opens it while no other copy runs on it; when there is no keychain to
//! keep a key (a computer without a Secret Service, or one that refuses), it stays as it is
//! and is tried again at the next start.

use std::{
    collections::HashMap,
    fmt, fs,
    path::Path,
    sync::{Arc, Mutex, OnceLock, PoisonError},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use fogata_crypto::{
    at_rest::{self, DataKey, sealed_form},
    random_bytes,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub mod file;
#[cfg(target_os = "linux")]
mod secret_portal;

/// The service under which data keys are kept in the keychain.
pub const SERVICE: &str = "Fogata";

/// The file that marks a sealed folder.
pub const SEALED_FILE: &str = "sealed.json";

const SEALED_VERSION: u32 = 1;

/// What `sealed.json`'s box holds, sealed with the folder's key.
const CHECK: &[u8] = b"the data key of a Fogata folder";

/// How long a call to the keychain may take: it may be waiting for the person to unlock it.
const KEYCHAIN_WAIT: Duration = Duration::from_secs(120);

/// Where data keys are kept, by name. Keys travel as hex text, which every keychain takes.
pub trait KeyStore: Send + Sync + fmt::Debug {
    /// The secret kept under `name`, or `None` when there is none.
    fn get(&self, name: &str) -> Result<Option<Zeroizing<String>>, String>;
    fn set(&self, name: &str, secret: &str) -> Result<(), String>;
    fn delete(&self, name: &str) -> Result<(), String>;
}

/// How a process uses the keychain for the data folders it opens.
#[derive(Clone, Debug, Default)]
pub enum Keychain {
    /// No keychain: folders stay as they are, and a sealed one does not open.
    #[default]
    Off,
    /// Sealed folders open with their key (sealing one that was cut short goes on); plain
    /// ones stay plain.
    Open(Arc<dyn KeyStore>),
    /// Sealed folders open with their key, and plain ones are sealed.
    Seal(Arc<dyn KeyStore>),
}

impl Keychain {
    pub(crate) fn store(&self) -> Option<&Arc<dyn KeyStore>> {
        match self {
            Keychain::Off => None,
            Keychain::Open(store) | Keychain::Seal(store) => Some(store),
        }
    }
}

static CHOSEN: OnceLock<Mutex<Keychain>> = OnceLock::new();

/// Sets how the data folders this process opens from now on use the keychain. Applications
/// call it once at start; without it no keychain is used.
pub fn use_keychain(keychain: Keychain) {
    *CHOSEN
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = keychain;
}

pub(crate) fn chosen() -> Keychain {
    CHOSEN
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// The system's keychain: the platform's (below), or in the Flatpak the files sealed with
/// the Secret portal's secret, which take over the keys an earlier Flatpak kept in the Secret
/// Service. Each call to a keychain runs on a thread of its own and is given up after
/// [`KEYCHAIN_WAIT`], so a keychain that never answers cannot hold the application.
#[derive(Debug, Default)]
pub struct SystemKeychain;

impl KeyStore for SystemKeychain {
    fn get(&self, name: &str) -> Result<Option<Zeroizing<String>>, String> {
        system().get(name)
    }

    fn set(&self, name: &str, secret: &str) -> Result<(), String> {
        system().set(name, secret)
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        system().delete(name)
    }
}

/// The keychain [`SystemKeychain`] uses in this process.
fn system() -> &'static dyn KeyStore {
    static SYSTEM: OnceLock<Box<dyn KeyStore>> = OnceLock::new();
    SYSTEM
        .get_or_init(|| {
            #[cfg(target_os = "linux")]
            if Path::new("/.flatpak-info").exists() {
                return Box::new(flatpak::keychain());
            }
            Box::new(PlatformKeychain)
        })
        .as_ref()
}

/// Runs `f` on a thread of its own, given up after [`KEYCHAIN_WAIT`].
#[cfg(target_os = "linux")]
fn on_thread<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("keychain".into())
        .spawn(move || {
            let _ = tx.send(f());
        })
        .map_err(|e| e.to_string())?;
    rx.recv_timeout(KEYCHAIN_WAIT)
        .unwrap_or_else(|_| Err("the keychain did not answer".into()))
}

/// The platform's keychain, as the `keyring` crate reaches it.
#[derive(Debug, Default)]
struct PlatformKeychain;

impl PlatformKeychain {
    fn call<T: Send + 'static>(
        name: &str,
        f: impl FnOnce(keyring::Entry) -> keyring::Result<T> + Send + 'static,
    ) -> Result<T, keyring::Error> {
        let name = name.to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("keychain".into())
            .spawn(move || {
                let _ = tx.send(keyring::Entry::new(SERVICE, &name).and_then(f));
            })
            .map_err(|e| keyring::Error::PlatformFailure(Box::new(e)))?;
        rx.recv_timeout(KEYCHAIN_WAIT).unwrap_or_else(|_| {
            Err(keyring::Error::PlatformFailure(
                "the keychain did not answer".into(),
            ))
        })
    }
}

impl KeyStore for PlatformKeychain {
    fn get(&self, name: &str) -> Result<Option<Zeroizing<String>>, String> {
        match Self::call(name, |entry| entry.get_password()) {
            Ok(secret) => Ok(Some(Zeroizing::new(secret))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, name: &str, secret: &str) -> Result<(), String> {
        let secret = Zeroizing::new(secret.to_string());
        Self::call(name, move |entry| entry.set_password(&secret)).map_err(|e| e.to_string())
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        match Self::call(name, |entry| entry.delete_credential()) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// The Flatpak's keychain.
#[cfg(target_os = "linux")]
mod flatpak {
    use std::{path::PathBuf, sync::Arc};

    use zeroize::Zeroizing;

    use super::{
        KeyStore, Mutex, PlatformKeychain, PoisonError, file::FileKeyStore, file::MovingKeyStore,
        on_thread, secret_portal,
    };

    /// What to do when a key an earlier Flatpak kept in the Secret Service must move and this
    /// one cannot reach it.
    const ALLOW_ONCE: &str = "an earlier version of Fogata kept this key in the system \
        keychain (the Secret Service), which this version is not let reach: allow it once \
        with `flatpak override --user --talk-name=org.freedesktop.secrets \
        dev.holypanda.fogata`, start Fogata, which moves the key, and then take that back \
        with `flatpak override --user --no-talk-name=org.freedesktop.secrets \
        dev.holypanda.fogata`";

    pub(super) fn keychain() -> MovingKeyStore {
        MovingKeyStore {
            new: Arc::new(PortalKeychain::default()),
            old: Arc::new(PlatformKeychain),
            old_unreachable: ALLOW_ONCE.into(),
        }
    }

    /// Keys in files under the application's data folder (`$XDG_DATA_HOME`, which the
    /// Flatpak sets to `~/.var/app/dev.holypanda.fogata/data`), sealed with the Secret
    /// portal's secret, asked for once per process.
    #[derive(Debug, Default)]
    struct PortalKeychain {
        files: Mutex<Option<Arc<FileKeyStore>>>,
    }

    impl PortalKeychain {
        fn files(&self) -> Result<Arc<FileKeyStore>, String> {
            let mut files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(files) = files.as_ref() {
                return Ok(files.clone());
            }
            let secret: Zeroizing<Vec<u8>> = on_thread(secret_portal::retrieve)?;
            let dir = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .filter(|dir| dir.is_absolute())
                .or_else(|| {
                    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
                })
                .ok_or("no folder for the application's data")?
                .join("fogata-keychain");
            let store = Arc::new(FileKeyStore::new(dir, &secret));
            *files = Some(store.clone());
            Ok(store)
        }
    }

    impl KeyStore for PortalKeychain {
        fn get(&self, name: &str) -> Result<Option<Zeroizing<String>>, String> {
            self.files()?.get(name)
        }

        fn set(&self, name: &str, secret: &str) -> Result<(), String> {
            self.files()?.set(name, secret)
        }

        fn delete(&self, name: &str) -> Result<(), String> {
            self.files()?.delete(name)
        }
    }
}

/// Keys kept in memory, for tests: no keychain of the computer is touched.
#[derive(Debug, Default)]
pub struct MemoryKeyStore {
    keys: Mutex<HashMap<String, String>>,
    /// Every call fails, as with a keychain that cannot be reached.
    pub unreachable: std::sync::atomic::AtomicBool,
    /// Keeping a key fails, as with a locked keychain the person did not unlock.
    pub locked: std::sync::atomic::AtomicBool,
    /// What is read back is not what was kept.
    pub garbles: std::sync::atomic::AtomicBool,
}

impl MemoryKeyStore {
    fn reach(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, String>>, String> {
        if self.unreachable.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("the keychain cannot be reached".into());
        }
        Ok(self.keys.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// The names of the keys kept.
    pub fn names(&self) -> Vec<String> {
        self.keys
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .cloned()
            .collect()
    }
}

impl KeyStore for MemoryKeyStore {
    fn get(&self, name: &str) -> Result<Option<Zeroizing<String>>, String> {
        let got = self.reach()?.get(name).cloned();
        if self.garbles.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(got.map(|s| Zeroizing::new(s.chars().rev().collect())));
        }
        Ok(got.map(Zeroizing::new))
    }

    fn set(&self, name: &str, secret: &str) -> Result<(), String> {
        let mut keys = self.reach()?;
        if self.locked.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("the keychain is locked".into());
        }
        keys.insert(name.to_string(), secret.to_string());
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        self.reach()?.remove(name);
        Ok(())
    }
}

/// `sealed.json`.
#[derive(Serialize, Deserialize)]
struct SealedFile {
    version: u32,
    /// The key's name in the keychain, under [`SERVICE`].
    key: String,
    /// [`CHECK`] sealed with the key, in hex.
    check: String,
}

/// The name in the keychain of the key that opens the folder `dir`, if it is sealed.
pub(crate) fn key_name(dir: &Path) -> Option<String> {
    let bytes = fs::read(dir.join(SEALED_FILE)).ok()?;
    serde_json::from_slice::<SealedFile>(&bytes)
        .ok()
        .map(|s| s.key)
}

/// The key of the folder `dir`, if it is sealed. Errors when it is sealed and its key cannot
/// be had.
pub(crate) fn open(dir: &Path, keychain: &Keychain) -> Result<Option<DataKey>> {
    let path = dir.join(SEALED_FILE);
    let bytes = match fs::read(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        bytes => bytes.with_context(|| format!("cannot read {}", path.display()))?,
    };
    let sealed: SealedFile =
        serde_json::from_slice(&bytes).with_context(|| format!("{} is damaged", path.display()))?;
    if sealed.version > SEALED_VERSION {
        bail!(
            "{} was saved by a newer version of Fogata; update Fogata to open it",
            path.display()
        );
    }
    let Some(store) = keychain.store() else {
        bail!(
            "the data in {} is sealed with a key in the system keychain, and the keychain is \
             not in use",
            dir.display()
        );
    };
    let secret = match store.get(&sealed.key) {
        Ok(Some(secret)) => secret,
        Ok(None) => bail!(
            "the system keychain does not hold the key that opens the data in {}",
            dir.display()
        ),
        Err(e) => bail!(
            "the system keychain cannot be reached ({e}); it holds the key that opens the \
             data in {}",
            dir.display()
        ),
    };
    let key = parse_key(&secret)
        .filter(|key| fits(key, &sealed.check))
        .with_context(|| {
            format!(
                "the key in the system keychain does not open the data in {}",
                dir.display()
            )
        })?;
    Ok(Some(named(key, &sealed.key)))
}

/// Looks for the key of a folder that holds sealed secrets but lost its `sealed.json`: the
/// identity and the friends file name their key ([`Form::Named`]), and the keychain is asked
/// for it by that name, as it cannot list what it holds. A key that opens one of them is put
/// back in a new `sealed.json`. `None` when the folder holds nothing sealed (a key may be
/// made for it); an error, naming the key looked for, when it does and that key cannot be
/// had: making a new key then would leave those files unreadable for good.
///
/// [`Form::Named`]: fogata_crypto::at_rest::Form::Named
pub(crate) fn recover(dir: &Path, store: &dyn KeyStore) -> Result<Option<DataKey>> {
    let files: Vec<(&str, Vec<u8>)> = [("identity.key", "identity"), ("friends.bin", "friends")]
        .into_iter()
        .filter_map(|(file, context)| {
            let bytes = fs::read(dir.join(file)).ok()?;
            sealed_form(&bytes).is_some().then_some((context, bytes))
        })
        .collect();
    if files.is_empty() {
        return Ok(None);
    }
    let mut names: Vec<&str> = files
        .iter()
        .filter_map(|(_, bytes)| at_rest::key_name(bytes))
        .collect();
    names.dedup();
    let mut missed = Vec::new();
    for name in &names {
        let secret = match store.get(name) {
            Ok(Some(secret)) => secret,
            Ok(None) => {
                missed.push(format!("it does not hold {name}"));
                continue;
            }
            Err(e) => {
                missed.push(format!("{name} cannot be read from it ({e})"));
                continue;
            }
        };
        let Some(key) = parse_key(&secret).map(|key| named(key, name)) else {
            missed.push(format!("{name} is no data key"));
            continue;
        };
        if !files
            .iter()
            .any(|(context, bytes)| key.open_whole(context, bytes).is_ok())
        {
            missed.push(format!("{name} does not open it"));
            continue;
        }
        let sealed = SealedFile {
            version: SEALED_VERSION,
            key: name.to_string(),
            check: hex::encode(key.seal_whole("check", CHECK)?),
        };
        crate::store::write_private(&dir.join(SEALED_FILE), &serde_json::to_vec_pretty(&sealed)?)?;
        tracing::warn!(
            "{SEALED_FILE} was missing in {}: put it back",
            dir.display()
        );
        return Ok(Some(key));
    }
    let candidates = if names.is_empty() {
        "its files do not name which".to_string()
    } else {
        missed.join(", and ")
    };
    bail!(
        "the data in {} is sealed but {SEALED_FILE} is missing, so the data key that opens it \
         is not known: it is one of the \"data-...\" keys of the \"{SERVICE}\" service in the \
         system keychain, and {candidates}. Put {SEALED_FILE} back from a copy of the folder",
        dir.display()
    )
}

/// The file of a data folder that says its person chose to keep it unencrypted while no
/// keychain keeps a key for it ([`allow_plain`]).
pub const PLAIN_FILE: &str = "unencrypted.json";

/// The system keychain could not keep a key for a data folder that holds nothing of an
/// account yet, and its person has not chosen to keep it unencrypted ([`allow_plain`]):
/// nothing secret is written in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoKeychain(pub String);

impl fmt::Display for NoKeychain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the system keychain cannot keep the key that encrypts this computer's data ({})",
            self.0
        )
    }
}

impl std::error::Error for NoKeychain {}

/// The person of the folder `dir` chose to keep it unencrypted while no keychain keeps a
/// key for it. It is still encrypted at the first start a keychain keeps one.
pub fn allow_plain(dir: &Path) -> Result<()> {
    // Made as the store makes it, readable by its owner only.
    crate::store::Store::open(dir.to_path_buf())?;
    crate::store::write_private(
        &dir.join(PLAIN_FILE),
        &serde_json::to_vec_pretty(&serde_json::json!({ "at_ms": crate::now_ms() }))?,
    )
}

/// Whether the person of the folder `dir` chose to keep it unencrypted ([`allow_plain`]).
pub(crate) fn plain_allowed(dir: &Path) -> bool {
    dir.join(PLAIN_FILE).is_file()
}

/// Whether what comes to the folder `dir` may be written unencrypted when no keychain keeps
/// a key: its person chose so, or it already holds an identity that is not encrypted.
pub(crate) fn may_stay_plain(dir: &Path) -> bool {
    plain_allowed(dir)
        || fs::read(dir.join("identity.key")).is_ok_and(|bytes| sealed_form(&bytes).is_none())
}

/// Makes a key for the folder `dir`, keeps it in the keychain and marks the folder as
/// sealed. `None` when the keychain cannot keep it: the folder then stays as it is.
#[cfg(test)]
pub(crate) fn create(dir: &Path, store: &dyn KeyStore) -> Result<Option<DataKey>> {
    Ok(try_create(dir, store)?.ok())
}

/// [`create`], saying why the keychain did not keep the key when it did not.
pub(crate) fn try_create(
    dir: &Path,
    store: &dyn KeyStore,
) -> Result<std::result::Result<DataKey, String>> {
    try_create_with(dir, store, |_| Ok(()))
}

/// [`try_create`], running `before` with the new key once the keychain kept it and before
/// `sealed.json` marks the folder as sealed: what it writes is there whenever the folder
/// has a key. When it fails, the keychain forgets the key and the folder stays as it is.
pub(crate) fn try_create_with(
    dir: &Path,
    store: &dyn KeyStore,
    before: impl FnOnce(&DataKey) -> Result<()>,
) -> Result<std::result::Result<DataKey, String>> {
    let mut id = [0u8; 16];
    random_bytes(&mut id)?;
    let name = format!("data-{}", hex::encode(id));
    let key = DataKey::generate()?.named(&name)?;
    let secret = Zeroizing::new(hex::encode(key.as_bytes()));
    if let Err(e) = store.set(&name, &secret) {
        tracing::warn!("no system keychain keeps a key ({e}): the data stays as it is");
        return Ok(Err(e));
    }
    // Read back: a keychain that keeps something else than it was given would lock the
    // folder for good.
    match store.get(&name) {
        Ok(Some(back)) if *back == *secret => {}
        got => {
            let why = match got {
                Err(e) => e,
                Ok(None) => "it did not give the key back".into(),
                Ok(Some(_)) => "it gave another value back".into(),
            };
            tracing::warn!(
                "the system keychain did not keep the key ({why}): the data stays as it is"
            );
            let _ = store.delete(&name);
            return Ok(Err(why));
        }
    }
    let sealed = SealedFile {
        version: SEALED_VERSION,
        key: name.clone(),
        check: hex::encode(key.seal_whole("check", CHECK)?),
    };
    if let Err(e) = before(&key).and_then(|()| {
        crate::store::write_private(&dir.join(SEALED_FILE), &serde_json::to_vec_pretty(&sealed)?)
    }) {
        let _ = store.delete(&name);
        return Err(e);
    }
    Ok(Ok(key))
}

/// `key`, writing `name` in the whole files it seals; a name too long to write (from a
/// `sealed.json` changed by hand) is left out.
fn named(key: DataKey, name: &str) -> DataKey {
    key.clone().named(name).unwrap_or(key)
}

fn parse_key(secret: &str) -> Option<DataKey> {
    let bytes: Zeroizing<Vec<u8>> = Zeroizing::new(hex::decode(secret.trim()).ok()?);
    let bytes: &[u8; 32] = bytes.as_slice().try_into().ok()?;
    Some(DataKey::from_bytes(bytes))
}

fn fits(key: &DataKey, check: &str) -> bool {
    hex::decode(check)
        .ok()
        .and_then(|sealed| key.open_whole("check", &sealed).ok())
        .is_some_and(|plain| *plain == *CHECK)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("fogata-vault-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn what_comes_stays_plain_only_where_the_person_chose_or_it_already_was() {
        let dir = folder("may-stay-plain");
        assert!(!may_stay_plain(&dir));
        fs::write(dir.join("identity.key"), b"0a1b").unwrap();
        assert!(may_stay_plain(&dir));
        let key = DataKey::generate().unwrap();
        fs::write(
            dir.join("identity.key"),
            key.seal_whole("identity", b"x").unwrap(),
        )
        .unwrap();
        assert!(!may_stay_plain(&dir));
        allow_plain(&dir).unwrap();
        assert!(may_stay_plain(&dir));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_folder_opens_with_the_key_it_was_sealed_with_only() {
        let dir = folder("open");
        let store = Arc::new(MemoryKeyStore::default());
        let keychain = Keychain::Seal(store.clone());
        assert!(open(&dir, &keychain).unwrap().is_none());
        let key = create(&dir, store.as_ref()).unwrap().unwrap();
        assert_eq!(
            open(&dir, &keychain).unwrap().unwrap().as_bytes(),
            key.as_bytes()
        );
        // Only opening, too.
        assert!(
            open(&dir, &Keychain::Open(store.clone()))
                .unwrap()
                .is_some()
        );

        // No keychain in use, one that cannot be reached, one without the key, one with
        // another key: errors, never a folder taken as plain.
        let error = |keychain: &Keychain| format!("{:#}", open(&dir, keychain).unwrap_err());
        assert!(error(&Keychain::Off).contains("not in use"));
        store
            .unreachable
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(error(&keychain).contains("cannot be reached"));
        store
            .unreachable
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let name = store.names().pop().unwrap();
        store
            .set(&name, &hex::encode(DataKey::generate().unwrap().as_bytes()))
            .unwrap();
        assert!(error(&keychain).contains("does not open"));
        store.delete(&name).unwrap();
        assert!(error(&keychain).contains("does not hold"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_keychain_that_keeps_nothing_leaves_the_folder_plain() {
        let dir = folder("none");
        let store = MemoryKeyStore::default();
        store
            .unreachable
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(create(&dir, &store).unwrap().is_none());
        assert!(!dir.join(SEALED_FILE).exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The computer's keychain, only in a session made for it, never the person's own:
    /// `FOGATA_TEST_KEYCHAIN=private` in a private D-Bus session with a Secret Service of
    /// its own (keys are kept, given back and removed, and a folder is sealed), `=none` with
    /// no session bus (`DBUS_SESSION_BUS_ADDRESS=disabled:`: every call fails, and a folder
    /// stays plain). Anything else skips it.
    #[test]
    #[ignore]
    fn the_system_keychain() {
        let mode = std::env::var("FOGATA_TEST_KEYCHAIN").unwrap_or_default();
        if mode != "private" && mode != "none" {
            eprintln!("skipped: FOGATA_TEST_KEYCHAIN is neither private nor none");
            return;
        }
        let keys = Arc::new(SystemKeychain);
        let dir = folder("system");
        let store = crate::Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        let started = std::time::Instant::now();
        let sealing = store.sealing().unwrap();
        eprintln!(
            "{mode}: first look for the key took {:.0} ms",
            started.elapsed().as_secs_f64() * 1000.0
        );
        if mode == "none" {
            assert!(keys.set("data-test", "00").is_err());
            assert!(keys.get("data-test").is_err());
            // Nothing is written until the person chooses to go on unencrypted.
            assert!(matches!(sealing, crate::store::Sealing::NoKeychain(_)));
            assert!(store.identity().is_err());
            assert!(!dir.join("identity.key").exists());
            allow_plain(&dir).unwrap();
            let store = crate::Store::open_with(dir.clone(), Keychain::Seal(keys)).unwrap();
            store.identity().unwrap();
            assert!(!store.sealed());
            assert!(!dir.join(SEALED_FILE).exists());
        } else {
            assert_eq!(sealing, crate::store::Sealing::Sealed);
            let (identity, _) = store.identity().unwrap();
            assert!(store.sealed());
            let sealed: SealedFile =
                serde_json::from_slice(&fs::read(dir.join(SEALED_FILE)).unwrap()).unwrap();
            let again = crate::Store::open_with(dir.clone(), Keychain::Open(keys.clone())).unwrap();
            assert_eq!(
                again.identity().unwrap().0.public_key(),
                identity.public_key()
            );
            // Without `sealed.json`, the key is asked for by the name the identity holds.
            fs::remove_file(dir.join(SEALED_FILE)).unwrap();
            let found = crate::Store::open_with(dir.clone(), Keychain::Open(keys.clone())).unwrap();
            assert_eq!(
                found.identity().unwrap().0.public_key(),
                identity.public_key()
            );
            assert_eq!(key_name(&dir), Some(sealed.key.clone()));
            keys.delete(&sealed.key).unwrap();
            // A key gone. In a Flatpak that does not reach the Secret Service (where an
            // earlier Flatpak kept its keys) nothing can tell it is not there: the error says
            // how to let it be looked for.
            let flatpak = Path::new("/.flatpak-info").exists();
            match keys.get(&sealed.key) {
                Ok(None) => {}
                Err(e) if flatpak && e.contains("flatpak override") => {}
                other => panic!("a key removed: {other:?}"),
            }
            let gone = crate::Store::open_with(dir.clone(), Keychain::Open(keys)).unwrap();
            let error = format!("{:#}", gone.identity().unwrap_err());
            assert!(
                error.contains("does not hold") || (flatpak && error.contains("flatpak override")),
                "{error}"
            );
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    /// For tools/beta/test-flatpak-keychain.sh, which runs it on the computer and in a
    /// Flatpak, in a session of its own: `FOGATA_TEST_KEYCHAIN=seal-folder` or `open-folder`
    /// opens the data folder `FOGATA_TEST_KEYCHAIN_DIR` with the system keychain (sealing it,
    /// when it is new, with the first only) and prints its identity's public key. Anything
    /// else skips it.
    #[test]
    #[ignore]
    fn a_folder_with_the_system_keychain() {
        let mode = std::env::var("FOGATA_TEST_KEYCHAIN").unwrap_or_default();
        let keychain = match mode.as_str() {
            "seal-folder" => Keychain::Seal(Arc::new(SystemKeychain)),
            "open-folder" => Keychain::Open(Arc::new(SystemKeychain)),
            _ => {
                eprintln!("skipped: FOGATA_TEST_KEYCHAIN is neither seal-folder nor open-folder");
                return;
            }
        };
        let dir = std::path::PathBuf::from(std::env::var("FOGATA_TEST_KEYCHAIN_DIR").unwrap());
        let store = crate::Store::open_with(dir, keychain).unwrap();
        match store.identity() {
            Ok((identity, _)) => println!(
                "identity: {} sealed: {}",
                hex::encode(identity.public_key().as_bytes()),
                store.sealed()
            ),
            Err(e) => panic!("no identity: {e:#}"),
        }
    }

    #[test]
    fn a_later_sealed_file_is_left_alone() {
        let dir = folder("later");
        fs::write(
            dir.join(SEALED_FILE),
            br#"{"version": 2, "key": "x", "check": ""}"#,
        )
        .unwrap();
        let error = open(&dir, &Keychain::Off).unwrap_err().to_string();
        assert!(
            error.contains("saved by a newer version of Fogata"),
            "{error}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
