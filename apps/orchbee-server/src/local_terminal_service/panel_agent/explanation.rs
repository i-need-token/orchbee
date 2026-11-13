//! Fence one actual Host diagnostic against the report context before publishing it.
use super::super::{LocalTerminalService, placement};
use crate::session_coordinator::ProcessObservationTarget;
use orchbee_server::TerminalAgentExplainContext;
use orchbee_workbench_protocol::{
    PanelAgentFailure as Failure, PanelAgentOutcome as Outcome, PanelAgentRequest,
    PanelAgentResult, ProtocolVersion, TerminalAgentExplanation, WorkbenchResponse,
    serialize_workbench_response,
};
use std::time::Instant;

fn asks_input_area(request: &PanelAgentRequest) -> bool {
    matches!(
        request.action,
        orchbee_workbench_protocol::PanelAgentAction::Explain {
            input_area: true,
            ..
        }
    )
}

impl LocalTerminalService {
    fn explanation_context(
        &self,
        request: &PanelAgentRequest,
    ) -> Result<
        (
            TerminalAgentExplainContext,
            ProcessObservationTarget,
            orchbee_agent_observation_protocol::AgentProcessIdentity,
        ),
        Failure,
    > {
        let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
        let handle = workers.request_handle();
        let current =
            placement(handle.as_ref().filter(|h| h.is_active())).ok_or(Failure::Unavailable)?;
        let mut session = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
        let prepared = session.prepare_agent_explanation(request, Instant::now())?;
        if prepared.1.target.worker != current {
            return Err(Failure::BindingChanged);
        }
        let identity = session.process_identity(&prepared.0.physical);
        Ok((prepared.0, prepared.1, identity))
    }
    pub(super) fn explain_panel_agent(&self, request: &PanelAgentRequest) -> PanelAgentResult {
        let prepare = || -> Result<Outcome, Failure> {
            let (context, target, identity) = self.explanation_context(request)?;
            let explanation = {
                let captured = self.request_agent_explanation(target.clone(), context.provider)?;
                let (after, after_target, _) = self.explanation_context(request)?;
                if after != context || after_target != target {
                    return Err(Failure::Unavailable);
                }
                TerminalAgentExplanation {
                    binding: context.binding.clone(),
                    view: context.view.clone(),
                    explanation: captured.explanation,
                    sample: captured.sample,
                    identity,
                    // Only a Client that asked may meet the member.
                    input_row: captured.input_row.filter(|_| asks_input_area(request)),
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
            session_id: request.session_id.clone(),
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
