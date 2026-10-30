//! The Session lock owns report ordering and samples expiry time after lock admission.
use super::LocalTerminalService;
use orchbee_workbench_protocol::{
    PanelMetadataFailure as Failure, PanelMetadataOutcome as Outcome, PanelMetadataRequest,
    PanelMetadataResult, ProtocolVersion, WorkbenchResponse, serialize_workbench_response,
};

impl LocalTerminalService {
    pub(crate) fn execute_panel_metadata(
        &self,
        request: PanelMetadataRequest,
    ) -> PanelMetadataResult {
        let unavailable = PanelMetadataResult {
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
                coordinator.panel_metadata(request, std::time::Instant::now())
            })
            .unwrap_or(unavailable);
        if serialize_workbench_response(&WorkbenchResponse::PanelMetadata(result.clone())).is_err()
        {
            result.outcome = Outcome::Failed {
                reason: Failure::ResultTooLarge,
            };
        }
        result
    }
}
