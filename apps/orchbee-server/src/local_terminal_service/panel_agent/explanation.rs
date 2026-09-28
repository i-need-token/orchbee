//! Fence one actual Host diagnostic against the report context before publishing it.
use super::super::{LocalTerminalService, placement};
use crate::site_coordinator::ProcessObservationTarget;
use orchbee_server::TerminalAgentExplainContext;
use orchbee_workbench_protocol::{
    PanelAgentFailure as Failure, PanelAgentOutcome as Outcome, PanelAgentRequest,
    PanelAgentResult, ProtocolVersion, TerminalAgentExplanation, WorkbenchResponse,
    serialize_workbench_response,
};
use std::time::Instant;

impl LocalTerminalService {
    fn explanation_context(
        &self,
        request: &PanelAgentRequest,
    ) -> Result<(TerminalAgentExplainContext, ProcessObservationTarget), Failure> {
        let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
        let handle = workers.request_handle();
        let current =
            placement(handle.as_ref().filter(|h| h.is_active())).ok_or(Failure::Unavailable)?;
        let mut site = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
        let prepared = site.prepare_agent_explanation(request, Instant::now())?;
        if prepared.1.target.worker != current {
            return Err(Failure::BindingChanged);
        }
        Ok(prepared)
    }
    pub(super) fn explain_panel_agent(&self, request: &PanelAgentRequest) -> PanelAgentResult {
        let prepare = || -> Result<Outcome, Failure> {
            let (context, target) = self.explanation_context(request)?;
            let explanation = if let Some(skipped) = context.skipped() {
                skipped
            } else {
                let captured = self.request_agent_explanation(target.clone(), context.provider)?;
                let (after, after_target) = self.explanation_context(request)?;
                if after != context || after_target != target {
                    return Err(Failure::Unavailable);
                }
                TerminalAgentExplanation {
                    binding: context.binding.clone(),
                    view: context.view.clone(),
                    explanation: captured.explanation,
                    sample: Some(captured.sample),
                }
            };
            Ok(Outcome::Explained {
                workbench_revision: context.workbench_revision,
                report_revision: context.report_revision,
                explanation: Box::new(explanation),
            })
        };
        let mut result = PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            site_id: request.site_id.clone(),
            outcome: prepare().unwrap_or_else(|reason| Outcome::Failed { reason }),
        };
        if serialize_workbench_response(&WorkbenchResponse::PanelAgent(result.clone())).is_err() {
            result.outcome = Outcome::Failed {
                reason: Failure::ResultTooLarge,
            };
        }
        result
    }
}
