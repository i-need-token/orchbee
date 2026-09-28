//! Explicit Agent reports resolve retained generations while holding the Site commit gate.
use super::LocalTerminalService;
use orchbee_workbench_protocol::{
    PanelAgentFailure, PanelAgentOutcome, PanelAgentRequest, PanelAgentResult, ProtocolVersion,
};
impl LocalTerminalService {
    pub(crate) fn execute_panel_agent(&self, request: PanelAgentRequest) -> PanelAgentResult {
        let unavailable = PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            site_id: request.site_id.clone(),
            outcome: PanelAgentOutcome::Failed {
                reason: PanelAgentFailure::Unavailable,
            },
        };
        #[cfg(windows)]
        {
            self.coordinator
                .lock()
                .ok()
                .and_then(|mut coordinator| {
                    coordinator.panel_agent(request, std::time::Instant::now())
                })
                .unwrap_or(unavailable)
        }
        #[cfg(not(windows))]
        {
            let _ = request;
            unavailable
        }
    }
}
