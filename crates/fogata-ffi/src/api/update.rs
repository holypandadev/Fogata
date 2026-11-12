//! New versions of Fogata from friends and members (fogata-core's `updates`): where they
//! stand, asking again, installing one. The screens learn of changes from the
//! `UpdateChanged` event.

use anyhow::Result;
use fogata_core::updates::{FailKind, UpdateState, UpdateStatus};

use super::app::run;

/// Where updates stand.
#[derive(Clone, Debug)]
pub struct UiUpdate {
    /// This copy's build and version.
    pub this_build: i64,
    pub this_version: String,
    pub state: UiUpdateState,
    /// Bytes of the package here so far, of `total`, while it downloads.
    pub done: i64,
    pub total: i64,
    /// Why it failed, in English, when it did.
    pub failure: Option<String>,
    pub failure_kind: Option<UiUpdateFailure>,
    /// The newest release known here.
    pub release_build: Option<i64>,
    pub release_version: Option<String>,
    pub release_notes: Option<String>,
    /// This copy must update to keep talking to copies of the newest release.
    pub required: bool,
    /// The device (its key in hex) the package comes from, while it downloads.
    pub from_device: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiUpdateState {
    UpToDate,
    Checking,
    Downloading,
    Verifying,
    /// A newer version is here, checked: [`install_update`] installs it.
    Ready,
    Installing,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiUpdateFailure {
    /// The newest release has no package for this system.
    NoPackage,
    /// The package cannot be written or read on this computer.
    Disk,
    /// This copy cannot install itself (not where installs go).
    CannotInstall,
    /// The install did not go through.
    Install,
    /// An install was started, and this copy is still the older version.
    DidNotTake,
}

fn view(status: UpdateStatus) -> UiUpdate {
    let (state, done, total, failure) = match status.state {
        UpdateState::UpToDate => (UiUpdateState::UpToDate, 0, 0, None),
        UpdateState::Checking => (UiUpdateState::Checking, 0, 0, None),
        UpdateState::Downloading { done, total } => {
            (UiUpdateState::Downloading, done as i64, total as i64, None)
        }
        UpdateState::Verifying => (UiUpdateState::Verifying, 0, 0, None),
        UpdateState::Ready => (UiUpdateState::Ready, 0, 0, None),
        UpdateState::Installing => (UiUpdateState::Installing, 0, 0, None),
        UpdateState::Failed { kind, message } => {
            let kind = match kind {
                FailKind::NoPackage => UiUpdateFailure::NoPackage,
                FailKind::Disk => UiUpdateFailure::Disk,
                FailKind::CannotInstall => UiUpdateFailure::CannotInstall,
                FailKind::Install => UiUpdateFailure::Install,
                FailKind::DidNotTake => UiUpdateFailure::DidNotTake,
            };
            (UiUpdateState::Failed, 0, 0, Some((kind, message)))
        }
    };
    UiUpdate {
        this_build: status.this_build as i64,
        this_version: status.this_version,
        state,
        done,
        total,
        failure_kind: failure.as_ref().map(|(kind, _)| *kind),
        failure: failure.map(|(_, message)| crate::bridge::told(message)),
        release_build: status.release.as_ref().map(|r| r.build as i64),
        release_version: status.release.as_ref().map(|r| r.version.clone()),
        release_notes: status.release.map(|r| r.notes),
        required: status.required,
        from_device: status.from.map(|d| hex::encode(d.as_bytes())),
    }
}

/// Where updates stand; `None` while they do not run.
pub async fn update_status() -> Result<Option<UiUpdate>> {
    run(|core| async move { Ok(core.update_status().map(view)) }).await
}

/// Asks every friend and member connected for a newer version now.
pub async fn check_for_update() -> Result<()> {
    run(|core| async move {
        core.check_for_update();
        Ok(())
    })
    .await
}

/// Installs the version that is ready. When it returns without an error, the application
/// quits at once, as from the tray: the new version is starting, and waits for this one to
/// let go of the data folder.
pub async fn install_update() -> Result<()> {
    run(|core| async move { core.install_update().await }).await
}
