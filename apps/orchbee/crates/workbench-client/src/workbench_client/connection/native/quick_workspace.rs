//! Quick Workspace directory defaults under the ordinary bounded refresh lease.
use super::{CarrierFailure, ClientExecutor, Result, WorkbenchSnapshotOutcome};
use crate::client_context::ClientContextLocation;
use crate::workbench_state::workspace_setup::quick::QuickPreparation;
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

impl ClientExecutor {
    /// A folder chosen in the menu, else where the Server says the focused Workspace is working:
    /// the current directory of its first terminal, else its own directory. Without either, the
    /// directory this Client was launched from.
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
        if let ClientContextLocation::Workspace { workspace_id, .. } =
            preparation.baseline.location()
            && let Some(directory) = self.read_current_directory(
                preparation.baseline.session_id(),
                workspace_id,
                preparation.deadlines.forward_cutoff(),
            )?
        {
            return Ok(Some(directory));
        }
        if let Some(directory) = preparation.baseline.directory() {
            return Ok(Some(directory.path.clone()));
        }
        launch_directory().map_err(|_| CarrierFailure::Transport)
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
