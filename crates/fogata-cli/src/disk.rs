//! Free space on a disk, as the application asks it (fogata-ffi's `disk.rs`): `statvfs` on
//! Linux and macOS. Elsewhere it is not told, and a full disk stops a restore or a move
//! instead.

use std::path::Path;

/// The bytes this user may still write on the disk holding `dir`; `None` when the system
/// does not say.
pub fn free_space(dir: &Path) -> Option<u64> {
    // The folder may not exist yet: the nearest one that does is on the same disk.
    let dir = dir.ancestors().find(|d| d.exists())?;
    available(dir)
}

#[cfg(unix)]
fn available(dir: &Path) -> Option<u64> {
    let stats = rustix::fs::statvfs(dir).ok()?;
    // Blocks free to unprivileged users, of the fragment size the counts are in.
    Some(stats.f_bavail.saturating_mul(stats.f_frsize))
}

#[cfg(not(unix))]
fn available(_: &Path) -> Option<u64> {
    None
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn the_system_tells_the_free_space_of_a_disk() {
        let here = std::env::temp_dir();
        let free = free_space(&here).expect("the system says it");
        assert!(free > 0);
        assert!(free_space(&here.join("not-made-yet/nor-this")).is_some());
    }
}
