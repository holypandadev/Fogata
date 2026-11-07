//! What of a data folder moves, how each file is read as plain on the old computer and
//! written on the new one, sealed with the new folder's own key.
//!
//! A file travels in the form a folder without a key keeps it: a sealed one is opened with
//! the old folder's key first. The new computer works out from the file's path alone what it
//! is and how to seal it, never from what the old one says, and takes only paths of the
//! kinds below.

use std::{
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use fogata_crypto::at_rest::{DataKey, SealWriter};
use fogata_net::files::kept_context;
use zeroize::Zeroizing;

use crate::{
    direct::store::{
        CHAT_CHAINS, CHAT_OPEN, CHAT_PROFILE, CHAT_TAILS, plain_records, sealed_records,
    },
    store::{DAMAGED, SEALING_FILE, SET_ASIDE_FILE, is_temp_name, mls_context, write_private},
    vault::SEALED_FILE,
};

/// Files the move writes or reads in the data folder itself, never moved.
pub(crate) const MOVED_FILE: &str = "moved.json";
pub(crate) const MOVED_IN_FILE: &str = "moved-in.json";
/// How far a move into this folder got: beside `incoming/`, never in it, so nothing the old
/// computer sends can stand for it.
pub(crate) const STAGE_FILE: &str = "incoming.json";
pub(crate) const INCOMING_DIR: &str = "incoming";
pub(crate) const REPLACED_DIR: &str = "replaced";

/// Most bytes of a file read whole into memory (the identity, the friends file, a chat's
/// journal): far above what one holds.
pub(crate) const MAX_WHOLE: u64 = 64 * 1024 * 1024;
/// Longest path the new computer takes, and the most folders deep.
const MAX_PATH: usize = 300;
const MAX_DEPTH: usize = 4;

/// What a file of the data folder is, for moving it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Kept as it is in every folder: group logs, settings, pictures of the application.
    Plain,
    /// One box for this context in a sealed folder.
    Whole(String),
    /// A chat's journal or pins (`file`), a chain of boxes in a sealed folder.
    Chain { file: &'static str, chat: String },
    /// An attachment kept here, a stream for this context in a sealed folder.
    Stream(String),
}

impl Kind {
    /// Whether the file is read whole into memory to be opened or sealed.
    pub(crate) fn whole(&self) -> bool {
        matches!(self, Kind::Whole(_) | Kind::Chain { .. })
    }
}

/// What the file at `rel` (relative to the data folder, `/` between names) is, or `None`
/// for one that does not move: what belongs to this computer (its key's name, its locks,
/// copies made while sealing and how far sealing got, videos fetched to play, downloads cut short, how chats end as
/// written with this folder's key, what was judged of group logs, its own log, the new versions of Fogata it keeps for this
/// system), the move's own files, and names that start with a dot (nothing the application
/// writes does).
pub(crate) fn kind_of(rel: &str) -> Option<Kind> {
    let parts: Vec<&str> = rel.split('/').collect();
    let name = *parts.last()?;
    if parts.iter().any(|p| p.is_empty() || p.starts_with('.'))
        || is_temp_name(name)
        || name.ends_with(".part")
    {
        return None;
    }
    match parts.as_slice() {
        // What was judged of a log is sealed with this folder's key; judged again there.
        ["logs", file] | ["direct", _, file] if file.ends_with(crate::store::JUDGED_SUFFIX) => None,
        [
            SEALED_FILE
            | SEALING_FILE
            | SET_ASIDE_FILE
            | "running.lock"
            | MOVED_FILE
            | MOVED_IN_FILE
            | STAGE_FILE
            | crate::recovery::RESTORED_FILE,
        ] => None,
        [
            crate::store::BACKUP_DIR
            | INCOMING_DIR
            | REPLACED_DIR
            | "videos"
            | crate::shares::COPIES_DIR
            | "window"
            | "diagnostics"
            | crate::updates::store::DIR,
            ..,
        ] => None,
        ["identity.key"] => Some(Kind::Whole("identity".into())),
        [crate::recovery::KEY_FILE] => Some(Kind::Whole(crate::recovery::KEY_CONTEXT.into())),
        // A key file set aside as damaged opens nowhere: copies are turned on again instead.
        [name] if name.starts_with(&format!("{}.damaged", crate::recovery::KEY_FILE)) => None,
        ["friends.bin" | "friends.bin.bak"] => Some(Kind::Whole("friends".into())),
        [name] if name.starts_with("friends.bin.damaged") => Some(Kind::Whole(DAMAGED.into())),
        ["mls", file] => file
            .strip_suffix(".bin")
            .map(|group| Kind::Whole(mls_context(group))),
        ["mls", ..] => None,
        ["direct", dir, ..] if *dir == CHAT_OPEN => None,
        ["direct", chat, file] if is_chat(chat) => {
            if let Some(chain) = CHAT_CHAINS.iter().find(|c| *c == file) {
                Some(Kind::Chain {
                    file: chain,
                    chat: chat.to_string(),
                })
            } else if *file == CHAT_PROFILE {
                Some(Kind::Whole(CHAT_PROFILE.into()))
            } else if *file == CHAT_TAILS {
                // Made again from the files as written here.
                None
            } else if CHAT_CHAINS
                .iter()
                .any(|c| file.starts_with(&format!("{c}.damaged")))
            {
                Some(Kind::Whole(DAMAGED.into()))
            } else {
                Some(Kind::Plain)
            }
        }
        ["direct", chat, "files", file] if is_chat(chat) => Some(match attachment_id(file) {
            Some(id) => Kind::Stream(kept_context(&id)),
            None => Kind::Plain,
        }),
        _ => Some(Kind::Plain),
    }
}

/// A folder none of whose files move.
fn skipped_dir(rel: &str) -> bool {
    matches!(
        rel,
        crate::store::BACKUP_DIR
            | INCOMING_DIR
            | REPLACED_DIR
            | "videos"
            | crate::shares::COPIES_DIR
            | "window"
            | "diagnostics"
            | crate::updates::store::DIR
    ) || rel == format!("direct/{CHAT_OPEN}")
}

/// A chat's folder: its friend's device key in hex.
fn is_chat(name: &str) -> bool {
    name.len() == 64 && name.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The id of an attachment from its file's name: its id in hex, maybe with an ending.
fn attachment_id(name: &str) -> Option<[u8; 32]> {
    let stem = name.split_once('.').map_or(name, |(stem, _)| stem);
    hex::decode(stem).ok()?.try_into().ok()
}

/// A path the new computer takes: names only (no root, no `..`), not too long nor deep, and
/// of a kind that moves.
///
/// Names are taken as every system the new computer may run would take them. They are plain
/// ASCII letters, digits, `.`, `_` and `-` (the application names its files so; only an
/// attachment's ending, after its id, may be anything else), so that a disk that does not
/// tell cases apart (Windows', macOS's by default) folds them as ASCII does, and no name can
/// be a short form Windows gives another (`INCOMI~1.JSO`). On such a disk `SEALED.JSON` is
/// `sealed.json`, so a name that is one of the names the move knows (or keeps for itself) in
/// another case is refused, and so are names Windows cannot keep or would read as another (a
/// `:`, a trailing dot or space, a device name such as `CON` or `COM1`).
pub(crate) fn check_path(rel: &str) -> Result<Kind> {
    let path = Path::new(rel);
    let refused = || anyhow::anyhow!("refused a file named {rel:?}");
    let parts: Vec<&str> = rel.split('/').collect();
    let attachment = matches!(
        parts.as_slice(),
        ["direct", chat, "files", name] if is_chat(chat) && attachment_id(name).is_some()
    );
    let last = parts.len().saturating_sub(1);
    let names_ok = parts
        .iter()
        .enumerate()
        .all(|(i, name)| portable_name(name) && ((attachment && i == last) || plain_name(name)));
    if rel.len() > MAX_PATH
        || path.components().count() > MAX_DEPTH
        || !path.components().all(|c| matches!(c, Component::Normal(_)))
        || !names_ok
    {
        return Err(refused());
    }
    let kind = kind_of(rel).ok_or_else(refused)?;
    // The same name in lower case is what a disk without cases holds: it must be the very
    // same file, or one the move takes as it is.
    let lower = rel.to_ascii_lowercase();
    if lower != rel && kind_of(&lower) != Some(Kind::Plain) {
        return Err(refused());
    }
    Ok(kind)
}

/// A name of the characters the application names its files with.
fn plain_name(name: &str) -> bool {
    name.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// A name every desktop system keeps as it is: no character Windows forbids or that ends a
/// path, no trailing dot or space (which Windows drops), not a device name.
fn portable_name(name: &str) -> bool {
    const DEVICES: [&str; 4] = ["con", "prn", "aux", "nul"];
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_lowercase();
    let device = DEVICES.contains(&stem.as_str())
        || ((stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit())
        || [
            "com\u{b9}",
            "com\u{b2}",
            "com\u{b3}",
            "lpt\u{b9}",
            "lpt\u{b2}",
            "lpt\u{b3}",
        ]
        .contains(&stem.as_str());
    !name.is_empty()
        && !name.ends_with('.')
        && !name.ends_with(' ')
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '\\' | '|' | '?' | '*'))
        && !device
}

/// A file to move: where it is in the folder, its kind and its size on the disk.
#[derive(Clone, Debug)]
pub(crate) struct Item {
    pub(crate) rel: String,
    pub(crate) kind: Kind,
    pub(crate) len: u64,
}

/// Every file of the folder `dir` that moves, the identity and the friends first and the
/// attachments last, so a move cut short has gone over the small files.
pub(crate) fn list(dir: &Path) -> Result<Vec<Item>> {
    let mut out = Vec::new();
    walk(dir, String::new(), 0, &mut out)?;
    out.sort_by_key(|item| {
        let rank = match (&item.kind, item.rel.as_str()) {
            (_, "identity.key") => 0,
            (_, "friends.bin") => 1,
            (Kind::Stream(_), _) => 4,
            (Kind::Plain, rel) if rel.starts_with("direct/") && rel.contains("/files/") => 4,
            (Kind::Chain { .. }, _) => 3,
            _ => 2,
        };
        (rank, item.rel.clone())
    });
    Ok(out)
}

fn walk(dir: &Path, prefix: String, depth: usize, out: &mut Vec<Item>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        // Links are left alone: nothing the application writes is one.
        let meta = fs::symlink_metadata(entry.path())?;
        if meta.is_dir() {
            if depth + 1 < MAX_DEPTH && !skipped_dir(&rel) {
                walk(&entry.path(), rel, depth + 1, out)?;
            }
        } else if meta.is_file()
            && let Some(kind) = kind_of(&rel)
        {
            // What the new computer would refuse (a name another system cannot keep) does
            // not stop the move: it stays here.
            if check_path(&rel).is_err() {
                tracing::warn!("{rel:?} does not move: its name cannot be kept everywhere");
                continue;
            }
            out.push(Item {
                rel,
                kind,
                len: meta.len(),
            });
        }
    }
    Ok(())
}

/// What a file holds, plain, read from the old folder (sealed with `key` when it has one).
pub(crate) enum Source {
    Whole(Zeroizing<Vec<u8>>),
    Reader(Box<dyn Read + Send>),
}

pub(crate) fn open(dir: &Path, item: &Item, key: Option<&DataKey>) -> Result<Source> {
    let path = dir.join(&item.rel);
    let cannot = || format!("cannot read {}", item.rel);
    if item.kind.whole() {
        if item.len > MAX_WHOLE {
            bail!("{} is too large to move", item.rel);
        }
        let bytes = Zeroizing::new(fs::read(&path).with_context(cannot)?);
        let plain = match &item.kind {
            Kind::Whole(context) => crate::store::from_disk(key, context, &bytes)
                .with_context(|| format!("{} does not open", item.rel))?,
            Kind::Chain { file, chat } => Zeroizing::new(
                plain_records(key, file, chat, &bytes)
                    .with_context(|| format!("{} does not open", item.rel))?,
            ),
            _ => unreachable!("whole kinds only"),
        };
        return Ok(Source::Whole(plain));
    }
    // An attachment is read as the store reads it: through its seal when it opens as one,
    // else as it is (one kept before the folder was sealed may start like a sealed file).
    if let Kind::Stream(_) = &item.kind {
        let name = item.rel.rsplit('/').next().unwrap_or_default();
        let id =
            attachment_id(name).with_context(|| format!("{} is not an attachment", item.rel))?;
        let (kept, _) = fogata_net::files::open_kept(&path, &id, key).with_context(cannot)?;
        return Ok(Source::Reader(Box::new(kept)));
    }
    // Any other file goes as it is: what it holds may come from others (a group's picture,
    // say) and start like a sealed file, which must not keep the account from moving.
    let file = fs::File::open(&path).with_context(cannot)?;
    Ok(Source::Reader(Box::new(file)))
}

/// A file being written on the new computer, under the folder being filled.
pub(crate) enum Sink {
    Whole {
        path: PathBuf,
        rel: String,
        kind: Kind,
        bytes: Zeroizing<Vec<u8>>,
    },
    Plain {
        tmp: PathBuf,
        path: PathBuf,
        file: fs::File,
    },
    Sealed {
        tmp: PathBuf,
        path: PathBuf,
        writer: SealWriter<fs::File>,
    },
}

/// Starts writing the file `rel` of kind `kind` into `stage`.
pub(crate) fn create(stage: &Path, rel: &str, kind: Kind, key: Option<&DataKey>) -> Result<Sink> {
    let path = stage.join(rel);
    if let Some(parent) = path.parent() {
        make_dirs(stage, parent)?;
    }
    if kind.whole() || rel == "app.json" || rel == "meta.json" {
        return Ok(Sink::Whole {
            path,
            rel: rel.to_string(),
            kind,
            bytes: Zeroizing::new(Vec::new()),
        });
    }
    let tmp = path.with_extension("moving.tmp");
    #[cfg(test)]
    crate::store::faults::check().with_context(|| format!("cannot write {rel}"))?;
    let file = new_private(&tmp).with_context(|| format!("cannot write {rel}"))?;
    Ok(match (kind, key) {
        (Kind::Stream(context), Some(key)) => Sink::Sealed {
            writer: SealWriter::new(key, &context, file)?,
            tmp,
            path,
        },
        _ => Sink::Plain { tmp, path, file },
    })
}

impl Sink {
    pub(crate) fn write(&mut self, data: &[u8]) -> Result<()> {
        match self {
            Sink::Whole { bytes, rel, .. } => {
                if (bytes.len() + data.len()) as u64 > MAX_WHOLE {
                    bail!("{rel} is too large");
                }
                // Grown into a new buffer by hand, so the old one is wiped as it goes (a
                // vector would move it and leave the copy).
                if bytes.len() + data.len() > bytes.capacity() {
                    let mut grown = Zeroizing::new(Vec::with_capacity(
                        (bytes.len() + data.len()).max(2 * bytes.capacity()),
                    ));
                    grown.extend_from_slice(bytes);
                    *bytes = grown;
                }
                bytes.extend_from_slice(data);
            }
            Sink::Plain { file, .. } => file.write_all(data)?,
            Sink::Sealed { writer, .. } => writer.write_all(data)?,
        }
        Ok(())
    }

    /// Writes the file whole, sealed with `key` as its kind is sealed; `adjust` may change a
    /// small plain file first (the settings, say).
    pub(crate) fn finish(
        self,
        key: Option<&DataKey>,
        adjust: impl FnOnce(&str, &[u8]) -> Option<Vec<u8>>,
    ) -> Result<()> {
        match self {
            Sink::Whole {
                path,
                rel,
                kind,
                bytes,
            } => {
                let out = match &kind {
                    Kind::Whole(context) => {
                        Zeroizing::new(crate::store::to_disk(key, context, &bytes)?)
                    }
                    Kind::Chain { file, chat } => {
                        Zeroizing::new(sealed_records(key, file, chat, &bytes)?)
                    }
                    _ => Zeroizing::new(adjust(&rel, &bytes).unwrap_or_else(|| bytes.to_vec())),
                };
                write_private(&path, &out)
            }
            Sink::Plain { tmp, path, file } => {
                file.sync_all()?;
                drop(file);
                fs::rename(&tmp, &path)?;
                Ok(())
            }
            Sink::Sealed { tmp, path, writer } => {
                let file = writer.finish()?;
                file.sync_all()?;
                drop(file);
                fs::rename(&tmp, &path)?;
                Ok(())
            }
        }
    }
}

/// Makes the folders down to `dir` inside `stage`, readable by their owner only.
fn make_dirs(stage: &Path, dir: &Path) -> Result<()> {
    let rel = dir.strip_prefix(stage).context("outside the folder")?;
    let mut made = stage.to_path_buf();
    for part in rel.components() {
        made.push(part);
        match fs::symlink_metadata(&made) {
            Ok(meta) if meta.is_dir() => continue,
            Ok(_) => bail!("{} is in the way", made.display()),
            Err(_) => {
                fs::create_dir(&made)
                    .with_context(|| format!("cannot create {}", made.display()))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&made, fs::Permissions::from_mode(0o700));
                }
            }
        }
    }
    Ok(())
}

/// A new file readable by its owner only (on Unix); never one that is there already.
fn new_private(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHAT: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    /// An attachment kept plain whose bytes start like a sealed file (a friend chooses what
    /// it sends) moves as the store reads it, in a plain folder and in a sealed one.
    #[test]
    fn a_plain_attachment_that_starts_like_a_sealed_one_moves() {
        let id = [0xab; 32];
        let name = format!("{}.bin", hex::encode(id));
        let mut looks_sealed = fogata_crypto::at_rest::MAGIC.to_vec();
        looks_sealed.push(3);
        looks_sealed.extend_from_slice(&[7u8; 200]);
        for key in [None, Some(DataKey::generate().unwrap())] {
            let dir = std::env::temp_dir().join(format!(
                "fogata-looks-sealed-{}-{}",
                key.is_some(),
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            let files = dir.join("direct").join(CHAT).join("files");
            fs::create_dir_all(&files).unwrap();
            fs::write(files.join(&name), &looks_sealed).unwrap();
            let items = list(&dir).unwrap();
            let item = items.iter().find(|i| i.rel.ends_with(&name)).unwrap();
            let Source::Reader(mut reader) = open(&dir, item, key.as_ref()).unwrap() else {
                panic!("an attachment is read as it goes");
            };
            let mut plain = Vec::new();
            reader.read_to_end(&mut plain).unwrap();
            assert_eq!(plain, looks_sealed);
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    /// Any other file goes as it is, whatever it starts with: a group's picture a member
    /// chose cannot keep the account from moving.
    #[test]
    fn a_plain_file_that_starts_like_a_sealed_one_goes_as_it_is() {
        let dir = std::env::temp_dir().join(format!("fogata-plain-looks-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("logs")).unwrap();
        let mut looks_sealed = fogata_crypto::at_rest::MAGIC.to_vec();
        looks_sealed.extend_from_slice(&[1, 9, 9, 9]);
        fs::write(dir.join("logs/ab.banner"), &looks_sealed).unwrap();
        let items = list(&dir).unwrap();
        let Source::Reader(mut reader) = open(&dir, &items[0], None).unwrap() else {
            panic!("read as it goes");
        };
        let mut back = Vec::new();
        reader.read_to_end(&mut back).unwrap();
        assert_eq!(back, looks_sealed);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn each_file_is_known_by_its_path() {
        assert_eq!(
            kind_of("identity.key"),
            Some(Kind::Whole("identity".into()))
        );
        assert_eq!(
            kind_of("recovery.bin"),
            Some(Kind::Whole("recovery".into()))
        );
        assert_eq!(
            kind_of("friends.bin.bak"),
            Some(Kind::Whole("friends".into()))
        );
        assert_eq!(
            kind_of("friends.bin.damaged.1"),
            Some(Kind::Whole(DAMAGED.into()))
        );
        assert_eq!(
            kind_of("mls/abcd.bin"),
            Some(Kind::Whole("mls/abcd".into()))
        );
        assert_eq!(kind_of("logs/crew.log"), Some(Kind::Plain));
        assert_eq!(kind_of("app.json"), Some(Kind::Plain));
        assert_eq!(
            kind_of(&format!("direct/{CHAT}/journal")),
            Some(Kind::Chain {
                file: "journal",
                chat: CHAT.into()
            })
        );
        assert_eq!(
            kind_of(&format!("direct/{CHAT}/profile")),
            Some(Kind::Whole("profile".into()))
        );
        assert_eq!(
            kind_of(&format!("direct/{CHAT}/group.log")),
            Some(Kind::Plain)
        );
        let id = "ab".repeat(32);
        assert_eq!(
            kind_of(&format!("direct/{CHAT}/files/{id}.jpg")),
            Some(Kind::Stream(kept_context(&[0xab; 32])))
        );
        for left in [
            "sealed.json",
            "sealing.json",
            "running.lock",
            "moved.json",
            "moved-in.json",
            "restored.json",
            "recovery.bin.damaged",
            "recovery.bin.damaged.2",
            "before-sealing/identity.key",
            "incoming/identity.key",
            "replaced/identity.key",
            "videos/x.mp4",
            "window/restart",
            "diagnostics/fogata-2026-10-30.log",
            "updates/linux.pack",
            "updates/manifest",
            "direct/open/x.mp4",
            "friends.bin.123.4.tmp",
            &format!("direct/{CHAT}/tails"),
            &format!("direct/{CHAT}/group.log.judged"),
            "logs/crew.log.judged",
            &format!("direct/{CHAT}/files/{id}.jpg.abcd1234.part"),
            "a//b",
            "../x",
            "incoming.json",
            ".transfer.json",
            ".aside",
            "logs/.hidden",
            ".git/config",
        ] {
            assert_eq!(kind_of(left), None, "{left}");
        }
        // Names in another case, which a disk without cases takes for the move's own or for
        // a sealed file, and names Windows cannot keep or reads as another.
        for bad in [
            "SEALED.JSON",
            "Moved.json",
            "MOVED-IN.JSON",
            "RUNNING.LOCK",
            "INCOMING/identity.key",
            "Replaced/identity.key",
            "Before-Sealing/identity.key",
            "Videos/x.mp4",
            "IDENTITY.KEY",
            "Friends.bin",
            "MLS/abcd.bin",
            &format!("direct/{}/journal", CHAT.to_uppercase()),
            &format!("direct/{CHAT}/JOURNAL"),
            "logs/CON",
            "logs/NUL.log",
            "logs/com1",
            "logs/x.log.",
            "logs/x.log ",
            "logs/...",
            "ab:stream",
            "logs/a?b",
            "logs/a\u{1}b",
            // The move's own records, as they are, in another case, or as Windows' short
            // form of a name.
            ".transfer.json",
            ".TRANSFER.JSON",
            ".aside",
            "incoming.json",
            "INCOMING.JSON",
            "Incoming.Json",
            "INCOMI~1.JSO",
            "MOVED-~1.JSO",
            "logs/.x",
            // Letters a disk without cases may fold onto ASCII ones (a Kelvin sign is a
            // `k`), or that another system may write in another form.
            "logs/\u{212a}ey.log",
            "\u{212a}eys.json",
            "logs/caf\u{e9}.log",
            "logs/a b.log",
        ] {
            assert!(check_path(bad).is_err(), "{bad}");
        }
        // An attachment's ending may be anything a name may hold.
        let id = "ab".repeat(32);
        assert!(matches!(
            check_path(&format!("direct/{CHAT}/files/{id}.caf\u{e9}")),
            Ok(Kind::Stream(_))
        ));
        assert!(check_path(&format!("direct/{CHAT}/files/x\u{e9}.jpg")).is_err());
        // Plain files keep any case.
        assert_eq!(check_path("logs/Notes.log").unwrap(), Kind::Plain);
        for bad in [
            "/etc/passwd",
            "../identity.key",
            "a/../../b",
            "a\\b",
            "a/b/c/d/e",
        ] {
            assert!(check_path(bad).is_err(), "{bad}");
        }
        assert!(check_path("logs/x.log").is_ok());
    }
}
