//! Free space on a disk, asked of the system itself: `statvfs` on Linux and macOS,
//! `GetDiskFreeSpaceExW` on Windows (no program is started, which the macOS sandbox may
//! refuse).

use std::path::Path;

/// The bytes this user may still write on the disk holding `dir`; `None` when the system
/// does not say.
pub(crate) fn free_space(dir: &Path) -> Option<u64> {
    // The folder may not exist yet: the nearest one that does is on the same disk.
    let dir = dir.ancestors().find(|d| d.exists())?;
    available(dir)
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn available(dir: &Path) -> Option<u64> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let path = CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    // SAFETY: `path` is a NUL-terminated string and `stats` points at writable memory of the
    // right type; statvfs only writes into it, and it is read only when the call succeeded.
    let stats = unsafe {
        if libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) != 0 {
            return None;
        }
        stats.assume_init()
    };
    // Blocks free to unprivileged users, of the fragment size the counts are in.
    #[allow(clippy::unnecessary_cast)]
    let bytes = (stats.f_bavail as u64).saturating_mul(stats.f_frsize as u64);
    Some(bytes)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn available(dir: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut free_to_caller = 0u64;
    // SAFETY: `wide` is a NUL-terminated UTF-16 path; the one out pointer given points at a
    // u64 that lives across the call, and the others may be null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_to_caller,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(free_to_caller)
}

#[cfg(not(any(unix, windows)))]
fn available(_: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_tells_the_free_space_of_a_disk() {
        let here = std::env::temp_dir();
        let free = free_space(&here).expect("the system says it");
        assert!(free > 0);
        // A folder not made yet is on the disk of the nearest one that is.
        let later = free_space(&here.join("not-made-yet/nor-this")).unwrap();
        assert!(later.abs_diff(free) < 1 << 30);
    }
}
