//! Quick Workspace directory defaults under the ordinary bounded refresh lease.
use super::{ClientExecutor, Result, WorkbenchSnapshotOutcome};
use crate::workbench_client::state::workspace_setup::quick::QuickPreparation;
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

impl ClientExecutor {
    /// A folder chosen in the menu, else the focused Workspace's own directory, otherwise the
    /// directory this Client was launched from. A terminal's later working directory never moves
    /// the default.
    pub(super) fn prepare_quick_cwd(
        &mut self,
        preparation: &QuickPreparation,
        outcome: &WorkbenchSnapshotOutcome,
    ) -> Result<Option<WorkbenchDirectoryPath>> {
        let WorkbenchSnapshotOutcome::Available { snapshot } = outcome else {
            return Ok(None);
        };
        if preparation
            .baseline
            .project_snapshot(snapshot)
            .ok()
            .as_ref()
            != Some(&preparation.baseline)
        {
            return Ok(None);
        }
        if let Some(directory) = preparation
            .target
            .as_ref()
            .and_then(|target| target.directory())
        {
            return Ok(Some(directory.clone()));
        }
        if let Some(directory) = preparation.baseline.directory() {
            return Ok(Some(directory.path.clone()));
        }
        launch_directory().map_err(|_| super::CarrierFailure::Transport)
    }
}

#[cfg(windows)]
fn launch_directory() -> std::io::Result<Option<WorkbenchDirectoryPath>> {
    // The Client never changes its process directory, so it remains the launch directory.
    Ok(orchbee_local_runtime::normalize_user_path(
        ".",
        |name| std::env::var_os(name),
        std::env::current_dir,
    )
    .ok()
    .and_then(|path| WorkbenchDirectoryPath::parse(&path).ok()))
}

#[cfg(not(windows))]
fn launch_directory() -> std::io::Result<Option<WorkbenchDirectoryPath>> {
    Err(
        crate::host_capabilities::Unsupported(crate::host_capabilities::Capability::DirectoryInput)
            .into(),
    )
}
