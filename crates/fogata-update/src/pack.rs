//! A folder as one file, for the packages of a release: the Windows application folder, the
//! macOS bundle (with its links and executable bits), the Linux Flatpak repository.
//!
//! The format is plain and read in one pass: `FGTPACK1`, then entries, each a kind byte, a
//! path (`/`-separated, relative), permission bits and, for a file, its size and bytes, for a
//! link its target; a zero kind ends it. Folders come before what is in them.
//!
//! The package was checked against the signed manifest before it is unpacked, so what is in
//! it is what the publisher put there; unpacking still trusts nothing in it: every path is a
//! plain relative name that means the same on every system, goes into a folder made new for
//! it, never through a link and never over anything, and links point inside it.

use std::{
    collections::HashSet,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

const MAGIC: &[u8; 8] = b"FGTPACK1";
const END: u8 = 0;
const DIR: u8 = 1;
const FILE: u8 = 2;
const LINK: u8 = 3;
/// Limits that no application folder comes near.
const MAX_ENTRIES: usize = 100_000;
const MAX_PATH: usize = 1024;
const MAX_DEPTH: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("the package holds a name that cannot be unpacked safely: {0:?}")]
    BadName(String),
    #[error("the package is not one or is cut short")]
    Malformed,
    #[error("the package holds more than it may")]
    TooLarge,
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// What a package held.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unpacked {
    pub files: usize,
    pub bytes: u64,
}

/// Writes the contents of folder `dir` (not the folder itself) as a package to `out`. Links
/// are kept as links; anything else that is neither a file nor a folder is refused.
pub fn write(dir: &Path, out: &mut impl Write) -> Result<Unpacked, PackError> {
    out.write_all(MAGIC)?;
    let mut summary = Unpacked::default();
    write_dir(dir, "", out, &mut summary)?;
    out.write_all(&[END])?;
    Ok(summary)
}

fn write_dir(
    root: &Path,
    prefix: &str,
    out: &mut impl Write,
    summary: &mut Unpacked,
) -> Result<(), PackError> {
    let mut names: Vec<_> = fs::read_dir(root.join(prefix))?
        .map(|e| e.map(|e| e.file_name()))
        .collect::<Result<_, _>>()?;
    names.sort();
    for name in names {
        let name = name
            .into_string()
            .map_err(|n| PackError::BadName(n.to_string_lossy().into_owned()))?;
        let path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        check_path(&path)?;
        let full = root.join(&path);
        let meta = fs::symlink_metadata(&full)?;
        let mode = mode_of(&meta);
        if meta.file_type().is_symlink() {
            let target = fs::read_link(&full)?;
            let target = target
                .to_str()
                .ok_or_else(|| PackError::BadName(path.clone()))?
                .to_string();
            check_link(&path, &target)?;
            header(out, LINK, &path, mode)?;
            out.write_all(&(target.len() as u16).to_be_bytes())?;
            out.write_all(target.as_bytes())?;
        } else if meta.is_dir() {
            header(out, DIR, &path, mode)?;
            write_dir(root, &path, out, summary)?;
        } else if meta.is_file() {
            header(out, FILE, &path, mode)?;
            out.write_all(&meta.len().to_be_bytes())?;
            let copied = io::copy(&mut fs::File::open(&full)?.take(meta.len()), out)?;
            if copied != meta.len() {
                return Err(PackError::Io(io::Error::other(format!(
                    "{path} changed while it was packed"
                ))));
            }
            summary.files += 1;
            summary.bytes += copied;
        } else {
            return Err(PackError::BadName(path));
        }
    }
    Ok(())
}

fn header(out: &mut impl Write, kind: u8, path: &str, mode: u32) -> io::Result<()> {
    out.write_all(&[kind])?;
    out.write_all(&(path.len() as u16).to_be_bytes())?;
    out.write_all(path.as_bytes())?;
    out.write_all(&mode.to_be_bytes())
}

#[cfg(unix)]
fn mode_of(meta: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    meta.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn mode_of(meta: &fs::Metadata) -> u32 {
    if meta.is_dir() { 0o755 } else { 0o644 }
}

/// Unpacks a package from `input` into `dest`, a folder made here (its parent must exist,
/// and nothing may be at `dest`). Holds it to `max_bytes` of file contents. On an error,
/// what was unpacked stays for the caller to remove.
pub fn extract(input: &mut impl Read, dest: &Path, max_bytes: u64) -> Result<Unpacked, PackError> {
    let mut magic = [0u8; 8];
    read_exact(input, &mut magic)?;
    if &magic != MAGIC {
        return Err(PackError::Malformed);
    }
    fs::create_dir(dest)?;
    // Readable by its owner only until it is put in place.
    set_mode(dest, 0o700)?;
    // Folders this package made, where its entries may go ("" is `dest`), and every name
    // taken, as systems that ignore case would see it.
    let mut dirs: HashSet<String> = HashSet::from([String::new()]);
    let mut taken: HashSet<String> = HashSet::new();
    let mut summary = Unpacked::default();
    let mut entries = 0usize;
    loop {
        let kind = read_u8(input)?;
        if kind == END {
            break;
        }
        entries += 1;
        if entries > MAX_ENTRIES {
            return Err(PackError::TooLarge);
        }
        let len = read_u16(input)? as usize;
        if len > MAX_PATH {
            return Err(PackError::Malformed);
        }
        let mut path = vec![0u8; len];
        read_exact(input, &mut path)?;
        let path = String::from_utf8(path).map_err(|_| PackError::Malformed)?;
        let parts = check_path(&path)?;
        let mode = read_u32(input)?;
        let parent = path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        if !dirs.contains(parent) || !taken.insert(folded(&path)) {
            return Err(PackError::BadName(path));
        }
        let full = parts
            .iter()
            .fold(dest.to_path_buf(), |p, part| p.join(part));
        match kind {
            DIR => {
                fs::create_dir(&full)?;
                set_mode(&full, 0o755)?;
                dirs.insert(path);
            }
            FILE => {
                let size = read_u64(input)?;
                summary.bytes = summary.bytes.saturating_add(size);
                if summary.bytes > max_bytes {
                    return Err(PackError::TooLarge);
                }
                let mut file = create_new(&full, mode & 0o111 != 0)?;
                let copied = io::copy(&mut input.take(size), &mut file)?;
                if copied != size {
                    return Err(PackError::Malformed);
                }
                file.flush()?;
                summary.files += 1;
            }
            LINK => {
                let len = read_u16(input)? as usize;
                if len > MAX_PATH {
                    return Err(PackError::Malformed);
                }
                let mut target = vec![0u8; len];
                read_exact(input, &mut target)?;
                let target = String::from_utf8(target).map_err(|_| PackError::Malformed)?;
                check_link(&path, &target)?;
                make_link(&target, &full)?;
            }
            _ => return Err(PackError::Malformed),
        }
    }
    // Nothing after the end.
    if input.read(&mut [0u8; 1])? != 0 {
        return Err(PackError::Malformed);
    }
    Ok(summary)
}

/// A name as systems that ignore case see it.
fn folded(path: &str) -> String {
    path.to_lowercase()
}

/// Checks a path inside a package and gives its parts: plain names, the same on every
/// system, nothing that climbs out or names a device.
fn check_path(path: &str) -> Result<Vec<&str>, PackError> {
    let bad = || PackError::BadName(path.to_string());
    if path.is_empty() || path.len() > MAX_PATH {
        return Err(bad());
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() > MAX_DEPTH || !parts.iter().all(|p| plain_name(p)) {
        return Err(bad());
    }
    Ok(parts)
}

/// One part of a path: no separators or characters some system refuses or reads otherwise,
/// not `.` nor `..`, no trailing dot or space (Windows drops them), no device name.
fn plain_name(name: &str) -> bool {
    if name.is_empty()
        || name.len() > 255
        || name == "."
        || name == ".."
        || name.ends_with('.')
        || name.ends_with(' ')
        || name
            .chars()
            .any(|c| c.is_control() || "\\/:*?\"<>|".contains(c))
    {
        return false;
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    !device
}

/// A link at `path` to `target`: relative, and never leading out of the package. Steps up
/// (`..`) come only first, from the link's own folder, which is a real folder of the
/// package: after a name, which may be another link, a step up would climb from wherever
/// that one leads.
fn check_link(path: &str, target: &str) -> Result<(), PackError> {
    let bad = || PackError::BadName(format!("{path} -> {target}"));
    if target.is_empty() || target.len() > MAX_PATH || target.starts_with('/') {
        return Err(bad());
    }
    let mut depth = path.split('/').count() as i64 - 1;
    let mut named = false;
    for part in target.split('/') {
        if part == ".." && !named {
            depth -= 1;
            if depth < 0 {
                return Err(bad());
            }
        } else if plain_name(part) {
            named = true;
        } else {
            return Err(bad());
        }
    }
    Ok(())
}

fn create_new(path: &Path, executable: bool) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(if executable { 0o755 } else { 0o644 });
    }
    #[cfg(not(unix))]
    let _ = executable;
    options.open(path)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_mode(_: &Path, _: u32) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn make_link(target: &str, at: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

/// Windows packages hold no links.
#[cfg(not(unix))]
fn make_link(_: &str, at: &Path) -> io::Result<()> {
    Err(io::Error::other(format!(
        "{} is a link, which packages for this system do not hold",
        at.display()
    )))
}

fn read_exact(input: &mut impl Read, buf: &mut [u8]) -> Result<(), PackError> {
    input.read_exact(buf).map_err(|e| match e.kind() {
        io::ErrorKind::UnexpectedEof => PackError::Malformed,
        _ => PackError::Io(e),
    })
}

fn read_u8(input: &mut impl Read) -> Result<u8, PackError> {
    let mut b = [0u8; 1];
    read_exact(input, &mut b)?;
    Ok(b[0])
}

fn read_u16(input: &mut impl Read) -> Result<u16, PackError> {
    let mut b = [0u8; 2];
    read_exact(input, &mut b)?;
    Ok(u16::from_be_bytes(b))
}

fn read_u32(input: &mut impl Read) -> Result<u32, PackError> {
    let mut b = [0u8; 4];
    read_exact(input, &mut b)?;
    Ok(u32::from_be_bytes(b))
}

fn read_u64(input: &mut impl Read) -> Result<u64, PackError> {
    let mut b = [0u8; 8];
    read_exact(input, &mut b)?;
    Ok(u64::from_be_bytes(b))
}

/// Where a file of a package went: `dest` joined with its parts.
pub fn place(dest: &Path, path: &str) -> PathBuf {
    path.split('/')
        .fold(dest.to_path_buf(), |p, part| p.join(part))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fogata-pack-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A package by hand: entries of (kind, path, mode, contents or link target).
    fn by_hand(entries: &[(u8, &str, u32, &[u8])]) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        for (kind, path, mode, data) in entries {
            header(&mut out, *kind, path, *mode).unwrap();
            match *kind {
                FILE => {
                    out.extend_from_slice(&(data.len() as u64).to_be_bytes());
                    out.extend_from_slice(data);
                }
                LINK => {
                    out.extend_from_slice(&(data.len() as u16).to_be_bytes());
                    out.extend_from_slice(data);
                }
                _ => {}
            }
        }
        out.push(END);
        out
    }

    #[test]
    fn a_folder_comes_back_as_it_was() {
        let dir = folder("roundtrip");
        let src = dir.join("src");
        fs::create_dir_all(src.join("Contents/MacOS")).unwrap();
        fs::write(src.join("Contents/Info.plist"), "plist").unwrap();
        fs::write(src.join("Contents/MacOS/Fogata"), vec![7u8; 100_000]).unwrap();
        fs::create_dir(src.join("empty")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(
                src.join("Contents/MacOS/Fogata"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
            std::os::unix::fs::symlink("MacOS/Fogata", src.join("Contents/Current")).unwrap();
        }
        let mut packed = Vec::new();
        let written = write(&src, &mut packed).unwrap();
        assert_eq!(written.files, 2);
        let out = dir.join("out");
        let read = extract(&mut packed.as_slice(), &out, u64::MAX).unwrap();
        assert_eq!(read, written);
        assert_eq!(
            fs::read(out.join("Contents/MacOS/Fogata")).unwrap(),
            vec![7u8; 100_000]
        );
        assert!(out.join("empty").is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = |p: &str| fs::metadata(out.join(p)).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode("Contents/MacOS/Fogata"), 0o755);
            assert_eq!(mode("Contents/Info.plist"), 0o644);
            assert_eq!(
                fs::read_link(out.join("Contents/Current")).unwrap(),
                PathBuf::from("MacOS/Fogata")
            );
        }
        // Into a folder that is there already: no.
        assert!(extract(&mut packed.as_slice(), &out, u64::MAX).is_err());
        // Past its limit: no.
        let small = dir.join("small");
        assert!(matches!(
            extract(&mut packed.as_slice(), &small, 1000),
            Err(PackError::TooLarge)
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_that_climb_out_or_alias_are_refused() {
        let dir = folder("names");
        let cases: Vec<Vec<u8>> = vec![
            by_hand(&[(FILE, "../evil", 0o644, b"x")]),
            by_hand(&[(FILE, "/etc/evil", 0o644, b"x")]),
            by_hand(&[(FILE, "a//b", 0o644, b"x")]),
            by_hand(&[(FILE, "./a", 0o644, b"x")]),
            by_hand(&[(FILE, "a\\..\\b", 0o644, b"x")]),
            by_hand(&[(FILE, "C:evil", 0o644, b"x")]),
            by_hand(&[(FILE, "nul.txt", 0o644, b"x")]),
            by_hand(&[(FILE, "com1", 0o644, b"x")]),
            by_hand(&[(FILE, "trailing.", 0o644, b"x")]),
            by_hand(&[(FILE, "new\nline", 0o644, b"x")]),
            // Into a folder the package did not make.
            by_hand(&[(FILE, "missing/a", 0o644, b"x")]),
            // Twice, or twice as a case-blind system sees it.
            by_hand(&[(FILE, "a", 0o644, b"x"), (FILE, "a", 0o644, b"y")]),
            by_hand(&[(FILE, "Readme", 0o644, b"x"), (FILE, "README", 0o644, b"y")]),
            // A link that leads out, and anything put through a link.
            by_hand(&[(LINK, "out", 0o777, b"../../etc")]),
            by_hand(&[(LINK, "abs", 0o777, b"/etc")]),
            by_hand(&[(DIR, "d", 0o755, b""), (LINK, "d/up", 0o777, b"../..")]),
            by_hand(&[
                (LINK, "l", 0o777, b"d"),
                (DIR, "d", 0o755, b""),
                (FILE, "l/x", 0o644, b"x"),
            ]),
            // Up through another link: "y" leads to the top, "x" one above it.
            by_hand(&[
                (DIR, "a", 0o755, b""),
                (LINK, "y", 0o777, b"a/.."),
                (LINK, "x", 0o777, b"y/.."),
            ]),
        ];
        for (i, packed) in cases.iter().enumerate() {
            let out = dir.join(format!("case{i}"));
            let result = extract(&mut packed.as_slice(), &out, u64::MAX);
            assert!(result.is_err(), "case {i} was unpacked");
        }
        assert!(!dir.join("evil").exists());
        // Cut short, a wrong start, something after the end.
        let good = by_hand(&[(FILE, "a", 0o644, b"hello")]);
        assert!(extract(&mut &good[..good.len() - 3], &dir.join("short"), u64::MAX).is_err());
        assert!(extract(&mut &b"FGTPACK2"[..], &dir.join("magic"), u64::MAX).is_err());
        let mut longer = good.clone();
        longer.push(9);
        assert!(extract(&mut longer.as_slice(), &dir.join("after"), u64::MAX).is_err());
        assert!(extract(&mut good.as_slice(), &dir.join("good"), u64::MAX).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_inside_the_package_is_kept() {
        let dir = folder("links");
        let packed = by_hand(&[
            (DIR, "Versions", 0o755, b""),
            (DIR, "Versions/A", 0o755, b""),
            (FILE, "Versions/A/lib", 0o755, b"code"),
            (LINK, "Versions/Current", 0o777, b"A"),
            (LINK, "lib", 0o777, b"Versions/Current/lib"),
        ]);
        let out = dir.join("out");
        extract(&mut packed.as_slice(), &out, u64::MAX).unwrap();
        assert_eq!(fs::read(out.join("lib")).unwrap(), b"code");
        let _ = fs::remove_dir_all(&dir);
    }
}
