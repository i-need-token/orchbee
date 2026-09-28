//! Prepare an immutable report/physical context for a separately owned Host diagnostic read.
use super::{Failure, Outcome, ReportMembership, TerminalAgentBinding, WorkbenchAuthority};
use orchbee_agent_observation_protocol::{AgentLaunchBinding, AgentReportProvider};
use orchbee_workbench_protocol::{
    AgentScreenExplanation, PanelAgentAction, PanelAgentRequest, PanelAgentView,
    TerminalAgentExplanation, WorkbenchRevision,
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
    pub full_lifecycle_authority: bool,
}
impl TerminalAgentExplainContext {
    pub fn skipped(&self) -> Option<TerminalAgentExplanation> {
        self.full_lifecycle_authority
            .then(|| TerminalAgentExplanation {
                binding: self.binding.clone(),
                view: self.view.clone(),
                sample: None,
                explanation: AgentScreenExplanation {
                    agent: self
                        .view
                        .agent
                        .clone()
                        .expect("full authority has an Agent"),
                    state: self.view.state,
                    manifest_source: None,
                    manifest_version: None,
                    matched_rule: None,
                    visible_idle: false,
                    visible_blocker: false,
                    visible_working: false,
                    screen_detection_skipped: true,
                    screen_detection_skip_reason: Some("full-lifecycle-hook-authority".into()),
                    skip_state_update: false,
                    skipped_update_reason: None,
                    fallback_reason: None,
                    evaluated_rules: vec![],
                },
            })
    }
}
impl WorkbenchAuthority {
    pub fn prepare_terminal_agent_explain(
        &mut self,
        request: &PanelAgentRequest,
        facts: &[TerminalAgentBinding],
        now: Instant,
    ) -> Result<TerminalAgentExplainContext, Failure> {
        let PanelAgentAction::Explain { binding } = &request.action else {
            return Err(Failure::InvalidRequest);
        };
        let mut probe = request.clone();
        probe.action = PanelAgentAction::Observe {
            binding: binding.clone(),
            after_sequence: None,
        };
        let result = self.panel_agent(probe, facts, now, |_| false);
        let (workbench_revision, report_revision, view) = match result.outcome {
            Outcome::Observed {
                workbench_revision,
                report_revision,
                observation,
            } => (workbench_revision, report_revision, observation.view),
            Outcome::Failed { reason } => return Err(reason),
            _ => return Err(Failure::Unavailable),
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
        let (provider, full_lifecycle_authority) = resource.reports.explanation_owner();
        Ok(TerminalAgentExplainContext {
            binding: binding.clone(),
            physical: physical.clone(),
            workbench_revision,
            report_revision,
            view,
            provider: provider.ok_or(Failure::Unavailable)?,
            full_lifecycle_authority,
        })
    }
}
