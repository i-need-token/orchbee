//! Quick Workspace directory defaults under the ordinary bounded refresh lease.
use super::{CarrierFailure, ClientExecutor, Result, WorkbenchSnapshotOutcome, closed};
use crate::client_context::ClientContextLocation;
use crate::local_client::workbench::CurrentDirectoryError;
use crate::workbench_state::workspace_setup::quick::QuickPreparation;
use orchbee_workbench_protocol::{
    CurrentDirectoryOutcome, CurrentDirectoryRequest, ProtocolVersion, WorkbenchDirectoryPath,
    WorkbenchWorkspaceId,
};

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
            && let Some(directory) = self.read_current_directory(preparation, workspace_id)?
        {
            return Ok(Some(directory));
        }
        if let Some(directory) = preparation.baseline.directory() {
            return Ok(Some(directory.path.clone()));
        }
        launch_directory().map_err(|_| CarrierFailure::Transport)
    }

    fn read_current_directory(
        &mut self,
        preparation: &QuickPreparation,
        workspace_id: &WorkbenchWorkspaceId,
    ) -> Result<Option<WorkbenchDirectoryPath>> {
        let deadline = preparation.deadlines.forward_cutoff();
        self.ensure_before(deadline)?;
        let request_id = self
            .mailbox
            .allocate_workbench_request_id()
            .map_err(closed)?;
        let result = self
            .client
            .current_directory(
                CurrentDirectoryRequest {
                    protocol_version: ProtocolVersion,
                    request_id,
                    session_id: preparation.baseline.session_id().clone(),
                    workspace_id: workspace_id.clone(),
                },
                deadline,
                None,
            )
            .map_err(|error| match error {
                CurrentDirectoryError::Unavailable | CurrentDirectoryError::Transport => {
                    CarrierFailure::Transport
                }
                CurrentDirectoryError::Encoding | CurrentDirectoryError::Protocol => {
                    CarrierFailure::Protocol
                }
                CurrentDirectoryError::Timeout => CarrierFailure::Deadline,
            })?;
        // A Workspace closed since the snapshot leaves the directories that snapshot recorded.
        Ok(match result.outcome {
            CurrentDirectoryOutcome::Observed { directory } => Some(directory),
            CurrentDirectoryOutcome::Failed { .. } => None,
        })
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
