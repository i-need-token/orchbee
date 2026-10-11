//! Presentation-only physical title reads; never a report sequence or acknowledgement.
use super::nullable;
use crate::WorkbenchRevision;
use orchbee_agent_observation_protocol::{AgentHostEpoch, AgentLaunchBinding};
use serde::{Deserialize, Serialize};
pub const MAXIMUM_TERMINAL_TITLE_ROWS: usize = 4;
pub const MAXIMUM_TERMINAL_TITLE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TerminalTitleObservation {
    Unavailable {},
    Observed {
        revision: WorkbenchRevision,
        #[serde(deserialize_with = "nullable")]
        title: Option<String>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalTitleRow {
    pub binding: AgentLaunchBinding,
    pub observation: TerminalTitleObservation,
}
pub(super) fn valid_bindings(bindings: &[AgentLaunchBinding]) -> bool {
    bindings.len() <= MAXIMUM_TERMINAL_TITLE_ROWS
        && bindings.iter().enumerate().all(|(i, b)| {
            AgentHostEpoch::parse(b.generation.as_str()).is_ok()
                && !bindings[..i]
                    .iter()
                    .any(|p| p.presentation_id == b.presentation_id)
        })
}
impl TerminalTitleRow {
    pub(super) fn valid(&self) -> bool {
        AgentHostEpoch::parse(self.binding.generation.as_str()).is_ok()
            && match &self.observation {
                TerminalTitleObservation::Unavailable {} => true,
                TerminalTitleObservation::Observed { revision, title } => {
                    revision.value() != 0
                        && title
                            .as_ref()
                            .is_none_or(|t| t.len() <= MAXIMUM_TERMINAL_TITLE_BYTES)
                }
            }
    }
}

#[cfg(test)]
mod tests;
