//! Presentation-only physical title reads: exact raw observations by binding, and the Server's
//! published projections by Presentation. Never a report sequence, acknowledgement or Workbench
//! revision change.
use super::nullable;
use crate::{TerminalPresentationId, WorkbenchRevision};
use orchbee_agent_observation_protocol::{AgentHostEpoch, AgentLaunchBinding};
use serde::{Deserialize, Serialize};
mod projection;
pub use projection::{project_terminal_title, strip_terminal_title};

pub const MAXIMUM_TERMINAL_TITLE_ROWS: usize = 4;
pub const MAXIMUM_TERMINAL_TITLE_BYTES: usize = 64 * 1024;
/// The most Presentations one `presentation-titles` read names.
pub const MAXIMUM_PRESENTATION_TITLE_ROWS: usize = 64;
/// The most UTF-8 bytes of a published title, its truncation ellipsis included.
pub const MAXIMUM_PRESENTATION_TITLE_BYTES: usize = 1024;

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

/// The title the Server publishes for one Presentation's terminal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PresentationTitle {
    /// The Server holds no title for the terminal now, as when its Worker is restarting or
    /// disconnected. A Client keeps the name it last showed for the same binding.
    Unavailable {},
    /// The terminal's title through [`project_terminal_title`]: nonempty, at most
    /// [`MAXIMUM_PRESENTATION_TITLE_BYTES`] and free of control characters. Null when the program
    /// cleared its title or the projection left nothing; a Client then names the terminal by its
    /// program. A Client applies its program naming to the text but never strips it again.
    Observed {
        #[serde(deserialize_with = "nullable")]
        title: Option<String>,
    },
}

/// A requested Presentation's current terminal binding and published title. An unknown or unbound
/// Presentation has a null binding and an unavailable title.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationTitleRow {
    pub presentation_id: TerminalPresentationId,
    #[serde(deserialize_with = "nullable")]
    pub binding: Option<AgentLaunchBinding>,
    pub title: PresentationTitle,
}
pub(super) fn valid_presentation_ids(ids: &[TerminalPresentationId]) -> bool {
    ids.len() <= MAXIMUM_PRESENTATION_TITLE_ROWS
        && ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id))
}
impl PresentationTitleRow {
    pub(super) fn valid(&self) -> bool {
        let binding = match &self.binding {
            Some(binding) => {
                binding.presentation_id.as_str() == self.presentation_id.as_str()
                    && AgentHostEpoch::parse(binding.generation.as_str()).is_ok()
            }
            None => self.title == PresentationTitle::Unavailable {},
        };
        binding
            && match &self.title {
                PresentationTitle::Observed { title: Some(title) } => {
                    projection::publishable(title)
                }
                PresentationTitle::Observed { title: None } | PresentationTitle::Unavailable {} => {
                    true
                }
            }
    }
}

#[cfg(test)]
mod tests;
