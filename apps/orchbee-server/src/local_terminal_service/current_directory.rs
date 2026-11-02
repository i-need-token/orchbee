//! Answers where a Workspace is working: the directory its first terminal is in when the read
//! arrives, sampled on the process observer, otherwise the Workspace's own directory.
use super::LocalTerminalService;
use orchbee_workbench_protocol::{
    CurrentDirectoryFailure as Failure, CurrentDirectoryOutcome as Outcome,
    CurrentDirectoryRequest, CurrentDirectoryResult, ProtocolVersion, WorkbenchDirectoryPath,
};

impl LocalTerminalService {
    pub(crate) fn execute_current_directory(
        &self,
        request: CurrentDirectoryRequest,
    ) -> CurrentDirectoryResult {
        let outcome = match self.current_directory(&request) {
            Ok(directory) => Outcome::Observed { directory },
            Err(reason) => Outcome::Failed { reason },
        };
        CurrentDirectoryResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            workspace_id: request.workspace_id,
            outcome,
        }
    }

    fn current_directory(
        &self,
        request: &CurrentDirectoryRequest,
    ) -> Result<WorkbenchDirectoryPath, Failure> {
        // The sample waits on the observer, which takes the coordinator itself, so the lock ends
        // with this block.
        let (target, primary) = {
            let mut coordinator = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
            coordinator.current_directory_sources(&request.session_id, &request.workspace_id)?
        };
        target
            .and_then(|target| self.request_current_directory(target))
            .map(|sampled| without_trailing_separator(&sampled))
            .or(primary)
            .ok_or(Failure::Unavailable)
    }
}

/// A process reports its directory with a trailing separator, which a new Workspace would keep.
fn without_trailing_separator(directory: &WorkbenchDirectoryPath) -> WorkbenchDirectoryPath {
    WorkbenchDirectoryPath::parse(orchbee_server::without_trailing_separator(
        directory.as_str(),
    ))
    .unwrap_or_else(|_| directory.clone())
}
