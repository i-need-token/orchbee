//! Prepare an immutable report/physical context for a separately owned Host diagnostic read.
use super::{Failure, Outcome, ReportMembership, TerminalAgentBinding, WorkbenchAuthority};
use orchbee_agent_observation_protocol::{AgentLaunchBinding, AgentReportProvider};
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentRequest, PanelAgentView, WorkbenchRevision,
};
use std::time::Instant;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalAgentExplainContext {
    pub binding: AgentLaunchBinding,
    pub physical: TerminalAgentBinding,
    pub workbench_revision: WorkbenchRevision,
    pub report_revision: WorkbenchRevision,
    pub view: PanelAgentView,
    pub provider: AgentReportProvider,
}
impl WorkbenchAuthority {
    pub fn prepare_terminal_agent_explain(
        &mut self,
        request: &PanelAgentRequest,
        facts: &[TerminalAgentBinding],
        now: Instant,
    ) -> Result<TerminalAgentExplainContext, Failure> {
        let PanelAgentAction::Explain { binding, .. } = &request.action else {
            return Err(Failure::InvalidRequest);
        };
        let mut probe = request.clone();
        probe.action = PanelAgentAction::Observe {
            binding: binding.clone(),
            after_sequence: None,
        };
        let result = self.panel_agent(probe, facts, now);
        let (workbench_revision, report_revision, view) = match result.outcome {
            Outcome::Observed {
                workbench_revision,
                report_revision,
                observation,
            } => (workbench_revision, report_revision, observation.view),
            Outcome::Failed { reason } => return Err(reason),
            Outcome::Titles { .. }
            | Outcome::PresentationTitles { .. }
            | Outcome::PresentationsRead { .. }
            | Outcome::Explained { .. }
            | Outcome::Identified { .. }
            | Outcome::States { .. }
            | Outcome::Read { .. }
            | Outcome::PresentationReported { .. }
            | Outcome::Reported { .. } => return Err(Failure::Unavailable),
        };
        let membership = ReportMembership::read(self.snapshot());
        let logical = membership
            .presentations
            .get(binding.presentation_id.as_str())
            .and_then(Option::as_ref)
            .ok_or(Failure::Unbound)?;
        let physical = facts
            .iter()
            .find(|fact| &fact.binding == logical)
            .ok_or(Failure::Unavailable)?;
        let resource = self
            .panel_agent
            .resources
            .get(&super::key(logical))
            .filter(|resource| resource.physical == *physical)
            .ok_or(Failure::Unavailable)?;
        let provider = resource.reports.explanation_owner();
        Ok(TerminalAgentExplainContext {
            binding: binding.clone(),
            physical: physical.clone(),
            workbench_revision,
            report_revision,
            view,
            provider: provider.ok_or(Failure::Unavailable)?,
        })
    }
}
