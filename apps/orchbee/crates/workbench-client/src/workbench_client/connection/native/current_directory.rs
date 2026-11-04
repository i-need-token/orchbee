//! Where the Server says a Workspace is working: the directory its first terminal, or the terminal
//! a named Panel shows, stands in, else its own directory. Quick Workspace and Worktree preparation
//! read it as the Web and Desktop Clients do; no Viewer is attached and no process is sampled here.
use super::{CarrierFailure, ClientExecutor, Instant, Result, closed};
use crate::local_client::workbench::CurrentDirectoryError;
use orchbee_workbench_protocol::{
    CurrentDirectoryOutcome, CurrentDirectoryPanel, CurrentDirectoryRequest, ProtocolVersion,
    SessionId, WorkbenchDirectoryPath, WorkbenchWorkspaceId,
};

impl ClientExecutor {
    /// The Server's answer for `workspace_id`, about `panel` when one is named, or `None` when it
    /// gives none, such as for a Workspace or Panel closed since the snapshot the caller read.
    pub(super) fn read_current_directory(
        &mut self,
        session_id: &SessionId,
        workspace_id: &WorkbenchWorkspaceId,
        panel: Option<CurrentDirectoryPanel>,
        deadline: Instant,
    ) -> Result<Option<WorkbenchDirectoryPath>> {
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
                    session_id: session_id.clone(),
                    workspace_id: workspace_id.clone(),
                    panel,
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
        Ok(match result.outcome {
            CurrentDirectoryOutcome::Observed { directory } => Some(directory),
            CurrentDirectoryOutcome::Failed { .. } => None,
        })
    }
}
