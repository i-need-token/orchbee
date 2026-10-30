//! The Session lock owns report ordering and samples expiry time after lock admission.
use super::LocalTerminalService;
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchResponse, WorkspaceMetadataFailure as Failure,
    WorkspaceMetadataOutcome as Outcome, WorkspaceMetadataRequest, WorkspaceMetadataResult,
    serialize_workbench_response,
};

impl LocalTerminalService {
    pub(crate) fn execute_workspace_metadata(
        &self,
        request: WorkspaceMetadataRequest,
    ) -> WorkspaceMetadataResult {
        let unavailable = WorkspaceMetadataResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            session_id: request.session_id.clone(),
            outcome: Outcome::Failed {
                reason: Failure::Unavailable,
            },
        };
        let mut result = self
            .coordinator
            .lock()
            .ok()
            .and_then(|mut coordinator| {
                coordinator.workspace_metadata(request, std::time::Instant::now())
            })
            .unwrap_or(unavailable);
        if serialize_workbench_response(&WorkbenchResponse::WorkspaceMetadata(result.clone()))
            .is_err()
        {
            result.outcome = Outcome::Failed {
                reason: Failure::ResultTooLarge,
            };
        }
        result
    }
}
