//! Read a generation-pinned physical report without borrowing the parent Panel's Agent.
use super::{
    BindingKey, Failure, Outcome, ReportMembership, TerminalAgentBinding, WorkbenchAuthority,
};
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::{TerminalAgentObservation, WorkbenchRevision};
use std::collections::BTreeMap;

impl WorkbenchAuthority {
    pub(super) fn observe_agent(
        &self,
        expected: &AgentLaunchBinding,
        after: Option<WorkbenchRevision>,
        membership: &ReportMembership,
        facts: &BTreeMap<BindingKey, &TerminalAgentBinding>,
    ) -> Result<Outcome, Failure> {
        let logical = membership
            .presentations
            .get(expected.presentation_id.as_str())
            .ok_or(Failure::NotFound)?
            .as_ref()
            .ok_or(Failure::Unbound)?;
        let key = super::key(logical);
        let physical = facts.get(&key).ok_or(Failure::Unavailable)?;
        if format!(
            "{:032x}",
            u128::from_be_bytes(physical.identity.creation_id().as_bytes())
        ) != expected.generation.as_str()
            || format!(
                "{:032x}",
                u128::from_be_bytes(physical.identity.host_epoch().as_bytes())
            ) != expected.host_epoch.as_str()
            || format!(
                "{:032x}",
                u128::from_be_bytes(physical.identity.terminal_id().as_bytes())
            ) != expected.host_terminal_id.as_str()
        {
            return Err(Failure::BindingChanged);
        }
        let resource = self
            .panel_agent
            .resources
            .get(&key)
            .filter(|r| r.physical == **physical);
        let empty = super::semantic::SemanticHistory::default();
        let history = resource.map_or(&empty, |r| &r.semantic);
        Ok(Outcome::Observed {
            workbench_revision: self.snapshot().revision,
            report_revision: WorkbenchRevision::new(self.panel_agent.revision),
            observation: Box::new(TerminalAgentObservation {
                binding: expected.clone(),
                sequence: history.sequence(),
                changes: history.since(after)?,
                view: resource.map_or_else(Default::default, |r| r.reports.view()),
            }),
        })
    }
}
