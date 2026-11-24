//! A file or folder the tests make unchangeable for the user that runs them, to see what
//! Fogata does with one it cannot change: a folder nothing can be added to or removed from,
//! a file that cannot be written. Shared by the tests of fogata-net and fogata-core, each
//! of which includes this file (`#[path]`).
//!
//! Permissions are what a person's computer refuses with, and come first. A user they do not
//! bind (root, as in a container or a build machine) is refused by the immutable attribute
//! instead, on Linux, where the file system keeps it and the user may set it. Where neither
//! binds, [`unchangeable`] says so, and the test has nothing to see: it says why and stops.

#![cfg(unix)]
#![allow(dead_code)]

use std::{
    fs,
    io::{self, Write as _},
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
};

/// While kept, the file or folder at `path` cannot be changed; dropped, it can again, as a
/// folder of the user's own (0700) or a private file (0600).
#[must_use = "dropped, the file or folder can be changed again"]
pub struct Unchangeable {
    path: PathBuf,
    immutable: bool,
}

/// Makes the file or folder at `path` unchangeable (see the module). `None` where nothing
/// this user can do here makes it so; it is left as it was.
pub fn unchangeable(path: &Path) -> Option<Unchangeable> {
    let folder = path.is_dir();
    let (locked, open) = if folder {
        (0o500, 0o700)
    } else {
        (0o400, 0o600)
    };
    fs::set_permissions(path, fs::Permissions::from_mode(locked)).unwrap();
    if refused(path, folder) {
        return Some(Unchangeable {
            path: path.to_path_buf(),
            immutable: false,
        });
    }
    fs::set_permissions(path, fs::Permissions::from_mode(open)).unwrap();
    if immutable(path, true).is_ok() {
        if refused(path, folder) {
            return Some(Unchangeable {
                path: path.to_path_buf(),
                immutable: true,
            });
        }
        let _ = immutable(path, false);
    }
    None
}

/// What a test says, before it stops, where [`unchangeable`] gave nothing.
pub fn cannot_be_made_unchangeable(path: &Path) {
    eprintln!(
        "{} cannot be made unchangeable for this user here (permissions do not bind it, \
         nor can the immutable attribute be set); there is nothing to see: skipped",
        path.display()
    );
}

impl Drop for Unchangeable {
    fn drop(&mut self) {
        // Not to be left behind, even by a test that failed (nothing unwraps here).
        if self.immutable {
            let _ = immutable(&self.path, false);
        }
        let open = if self.path.is_dir() { 0o700 } else { 0o600 };
        let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(open));
    }
}

/// Whether a change to `path` is refused now: a file made in the folder, or a byte added to
/// the file (taken back if it went).
fn refused(path: &Path, folder: bool) -> bool {
    if folder {
        let probe = path.join(format!(".unchangeable-{}", std::process::id()));
        match fs::File::create(&probe) {
            Ok(_) => {
                fs::remove_file(&probe).unwrap();
                false
            }
            Err(_) => true,
        }
    } else {
        let len = fs::metadata(path).unwrap().len();
        match fs::OpenOptions::new().append(true).open(path) {
            Ok(mut file) => {
                if file.write_all(b"x").is_ok() {
                    file.set_len(len).unwrap();
                    false
                } else {
                    true
                }
            }
            Err(_) => true,
        }
    }
}

/// Sets (`on`) or clears the immutable attribute of `path` (`chattr +i`), which binds root.
#[cfg(target_os = "linux")]
fn immutable(path: &Path, on: bool) -> io::Result<()> {
    use rustix::fs::{IFlags, ioctl_getflags, ioctl_setflags};
    let file = fs::File::open(path)?;
    let mut flags = ioctl_getflags(&file)?;
    flags.set(IFlags::IMMUTABLE, on);
    ioctl_setflags(&file, flags)?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn immutable(_: &Path, _: bool) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}
