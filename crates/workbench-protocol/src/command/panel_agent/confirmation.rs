//! What the Server decided about answering an Agent's confirmations in one Presentation's
//! terminal, and the choice a person makes about it. A Client sends these actions only to a
//! Server whose handshake answer lists `agent-confirmation`.
use super::nullable;
use crate::{PresentationBinding, TerminalPresentationId};
use serde::{Deserialize, Serialize};

/// The most Presentations one `confirmations` read names.
pub const MAXIMUM_AGENT_CONFIRMATION_ROWS: usize = 64;
/// The most UTF-8 bytes of a pressed confirmation's text, its truncation ellipsis included.
pub const MAXIMUM_AGENT_CONFIRMATION_TEXT_BYTES: usize = 512;
const MAXIMUM_AGENT_LABEL_BYTES: usize = 64;

/// Whether Orchbee answers the Agent's routine confirmations: not at all, for this Agent run
/// only, or for every Agent of this type through the saved setting.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentConfirmationChoice {
    Off,
    Run,
    AgentType,
}

/// Why the confirmation on screen waits for the person.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentConfirmationWait {
    LeavesWorkspace,
    /// The Workspace is the home folder or holds it, so a file there is not judged inside it.
    HomeWorkspace,
    ProtectedFile,
    NotUnderstood,
    NotVisible,
    MarkedDangerous,
    /// The Agent waits on something Orchbee never answers, such as a trust or login prompt.
    NotAnswered,
    /// A person typed or clicked in the terminal since the confirmation appeared.
    Person,
    Changing,
    /// The same confirmation kept coming back, so Orchbee stopped answering for this Agent run.
    Repeating,
    HostTooOld,
}

/// What a pressed confirmation asked for.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentConfirmationKind {
    Command,
    Create,
    Edit,
}

/// The last confirmation Orchbee answered in this Agent run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentConfirmationPress {
    /// Counts the run's presses from 1, so a Client tells a new press from one it already showed.
    pub serial: u32,
    pub kind: AgentConfirmationKind,
    /// The command or path, shortened with an ellipsis; never empty.
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentConfirmationView {
    /// The Agent's report label, as a Panel Agent view names it.
    pub agent: String,
    pub choice: AgentConfirmationChoice,
    /// The Server offers to turn answering on; only while `choice` is off.
    pub offer: bool,
    #[serde(deserialize_with = "nullable")]
    pub wait: Option<AgentConfirmationWait>,
    #[serde(deserialize_with = "nullable")]
    pub pressed: Option<AgentConfirmationPress>,
    /// Answering stopped for this run because a confirmation kept coming back.
    pub stopped: bool,
}
impl AgentConfirmationView {
    fn valid(&self) -> bool {
        !self.agent.is_empty()
            && self.agent.len() <= MAXIMUM_AGENT_LABEL_BYTES
            && self.agent.trim() == self.agent
            && (!self.offer || self.choice == AgentConfirmationChoice::Off)
            && self.pressed.as_ref().is_none_or(|pressed| {
                pressed.serial != 0
                    && !pressed.text.is_empty()
                    && pressed.text.len() <= MAXIMUM_AGENT_CONFIRMATION_TEXT_BYTES
                    && !pressed.text.chars().any(char::is_control)
            })
    }
}

/// One requested Presentation. `confirmation` is absent when no Agent Orchbee can answer runs in
/// its terminal, and a Client then shows no entry for it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationConfirmationRow {
    pub presentation_id: TerminalPresentationId,
    #[serde(deserialize_with = "nullable")]
    pub binding: Option<PresentationBinding>,
    #[serde(deserialize_with = "nullable")]
    pub confirmation: Option<AgentConfirmationView>,
}
impl PresentationConfirmationRow {
    pub(super) fn valid(&self) -> bool {
        self.confirmation
            .as_ref()
            .is_none_or(|view| self.binding.is_some() && view.valid())
    }
}

pub(super) fn valid_presentation_ids(presentation_ids: &[TerminalPresentationId]) -> bool {
    presentation_ids.len() <= MAXIMUM_AGENT_CONFIRMATION_ROWS
        && presentation_ids
            .iter()
            .enumerate()
            .all(|(index, id)| !presentation_ids[..index].contains(id))
}
