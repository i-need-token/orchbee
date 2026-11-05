//! Read a generation-pinned physical report without borrowing the parent Panel's Agent.
use super::{
    BindingKey, Failure, Outcome, ReportMembership, TerminalAgentBinding, WorkbenchAuthority,
};
use orchbee_agent_observation_protocol::{
    AgentAttachmentGeneration, AgentHostEpoch, AgentHostTerminalId, AgentLaunchBinding,
    AgentPresentationId,
};
use orchbee_workbench_protocol::{
    TerminalAgentObservation, TerminalAgentStateRow, TerminalAgentStateSnapshot,
    TerminalPresentationId, WorkbenchRevision,
};
use std::collections::BTreeMap;

impl WorkbenchAuthority {
    pub(super) fn agent_states(
        &self,
        presentations: &[TerminalPresentationId],
        membership: &ReportMembership,
        facts: &BTreeMap<BindingKey, &TerminalAgentBinding>,
    ) -> Result<Outcome, Failure> {
        let mut rows = Vec::with_capacity(presentations.len());
        for id in presentations {
            let logical = membership
                .presentations
                .get(id.as_str())
                .ok_or(Failure::NotFound)?;
            let physical = logical
                .as_ref()
                .and_then(|b| facts.get(&super::key(b)).copied());
            let snapshot = physical
                .map(|physical| {
                    let resource = self
                        .panel_agent
                        .resources
                        .get(&super::key(&physical.binding))
                        .filter(|r| r.physical == *physical);
                    let view = resource.map_or_else(Default::default, |r| r.reports.view());
                    let hex = |bytes: [u8; 16]| format!("{:032x}", u128::from_be_bytes(bytes));
                    Ok::<_, Failure>(TerminalAgentStateSnapshot {
                        binding: AgentLaunchBinding {
                            presentation_id: AgentPresentationId::parse(id.as_str())
                                .map_err(|_| Failure::InvalidRequest)?,
                            generation: AgentAttachmentGeneration::parse(&hex(physical
                                .identity
                                .creation_id()
                                .as_bytes()))
                            .map_err(|_| Failure::Unavailable)?,
                            host_epoch: AgentHostEpoch::parse(&hex(physical
                                .identity
                                .host_epoch()
                                .as_bytes()))
                            .map_err(|_| Failure::Unavailable)?,
                            host_terminal_id: AgentHostTerminalId::parse(&hex(physical
                                .identity
                                .terminal_id()
                                .as_bytes()))
                            .map_err(|_| Failure::Unavailable)?,
                        },
                        sequence: resource
                            .map_or_else(|| WorkbenchRevision::new(0), |r| r.semantic.sequence()),
                        agent: view.agent,
                        state: view.state,
                        completion: view.completion,
                    })
                })
                .transpose()?;
            rows.push(TerminalAgentStateRow {
                presentation_id: id.clone(),
                snapshot,
            });
        }
        Ok(Outcome::States {
            workbench_revision: self.snapshot().revision,
            report_revision: WorkbenchRevision::new(self.panel_agent.revision),
            presentations: rows,
        })
    }

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
