//! Compact physical state checkpoints for Client acknowledgement; no report messages or history.
use super::{PanelAgentState, WorkbenchRevision, nullable};
use crate::TerminalPresentationId;
use orchbee_agent_observation_protocol::{AgentHostEpoch, AgentLaunchBinding};
use serde::{Deserialize, Serialize};

/// Maximum number of physical state checkpoints in one read.
pub const MAXIMUM_TERMINAL_AGENT_STATE_ROWS: usize = 64;

/// The report owner's current semantic version, qualified by its full physical identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalAgentStateSnapshot {
    pub binding: AgentLaunchBinding,
    pub sequence: WorkbenchRevision,
    #[serde(deserialize_with = "nullable")]
    pub agent: Option<String>,
    pub state: PanelAgentState,
}
impl TerminalAgentStateSnapshot {
    pub(super) fn valid(&self) -> bool {
        AgentHostEpoch::parse(self.binding.generation.as_str()).is_ok()
            && self
                .agent
                .as_ref()
                .is_none_or(|a| !a.is_empty() && a.trim() == a)
    }
}

/// A requested presentation has either one physical checkpoint or no current physical observation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalAgentStateRow {
    pub presentation_id: TerminalPresentationId,
    #[serde(deserialize_with = "nullable")]
    pub snapshot: Option<TerminalAgentStateSnapshot>,
}
impl TerminalAgentStateRow {
    pub(super) fn valid(&self) -> bool {
        self.snapshot.as_ref().is_none_or(|s| {
            s.valid() && s.binding.presentation_id.as_str() == self.presentation_id.as_str()
        })
    }
}

#[cfg(test)]
mod tests;
