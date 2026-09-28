//! A physical report observation carries current state separately from contiguous semantic changes.
use super::{PanelAgentState, PanelAgentView, WorkbenchRevision, nullable};
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use serde::{Deserialize, Serialize};

pub const MAXIMUM_TERMINAL_AGENT_CHANGES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalAgentChange {
    pub sequence: WorkbenchRevision,
    #[serde(deserialize_with = "nullable")]
    pub agent: Option<String>,
    pub state: PanelAgentState,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalAgentObservation {
    pub binding: AgentLaunchBinding,
    pub sequence: WorkbenchRevision,
    pub changes: Vec<TerminalAgentChange>,
    pub view: PanelAgentView,
}
impl TerminalAgentObservation {
    pub(super) fn valid(&self) -> bool {
        orchbee_agent_observation_protocol::AgentHostEpoch::parse(self.binding.generation.as_str())
            .is_ok()
            && self.view.valid()
            && self.changes.len() <= MAXIMUM_TERMINAL_AGENT_CHANGES
            && self.changes.iter().all(|c| {
                c.sequence.value() > 0
                    && c.agent
                        .as_ref()
                        .is_none_or(|a| !a.is_empty() && a.trim() == a)
            })
            && self
                .changes
                .windows(2)
                .all(|c| c[0].sequence.value().checked_add(1) == Some(c[1].sequence.value()))
            && self.changes.last().is_none_or(|c| {
                c.sequence == self.sequence
                    && c.agent == self.view.agent
                    && c.state == self.view.state
            })
    }
    pub(super) fn covers(&self, after: Option<WorkbenchRevision>) -> bool {
        match after {
            None => self.changes.is_empty(),
            Some(after) if after == self.sequence => self.changes.is_empty(),
            Some(after) => self
                .changes
                .first()
                .is_some_and(|c| after.value().checked_add(1) == Some(c.sequence.value())),
        }
    }
}
