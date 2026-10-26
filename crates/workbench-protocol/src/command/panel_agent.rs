//! Explicit terminal Agent reports and reads; process proof is never a public request field.
use crate::{
    ContractError, PresentationBinding, ProtocolVersion, SessionId, TerminalPresentationId,
    WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
};
use orchbee_agent_observation_protocol::{AgentReportSequence, normalize_agent_report_label};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;
mod explanation;
pub(super) mod manifest;
pub use manifest::{
    AgentDetectionManifest, AgentManifestSource, AgentManifestWarning, AgentManifestWarningKind,
};
mod observation;
pub use explanation::{
    AgentDetectionEvaluation, AgentDetectionEvidence, AgentDetectionRule, AgentDetectionSample,
    AgentScreenExplanation, TerminalAgentExplanation,
};
mod state_snapshot;
mod titles;
pub use observation::{
    MAXIMUM_TERMINAL_AGENT_CHANGES, TerminalAgentChange, TerminalAgentObservation,
};
pub use state_snapshot::{
    MAXIMUM_TERMINAL_AGENT_STATE_ROWS, TerminalAgentStateRow, TerminalAgentStateSnapshot,
};
pub use titles::{
    MAXIMUM_TERMINAL_TITLE_BYTES, MAXIMUM_TERMINAL_TITLE_ROWS, TerminalTitleObservation,
    TerminalTitleRow,
};

pub const MAXIMUM_PANEL_AGENT_READ_ROWS: usize = 4;
pub const MAXIMUM_ENCODED_PANEL_AGENT_RESULT_BYTES: usize = 512 * 1024;
fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelAgentState {
    Idle,
    Working,
    Blocked,
    /// Explicit completion of the current turn; never inferred from idle or acknowledgement.
    Done,
    /// An explicit failure, independent of whether a Client has viewed it.
    Error,
    #[default]
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PanelAgentDisposition {
    Applied,
    Ignored,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PanelAgentReport {
    State {
        source: String,
        agent: String,
        state: PanelAgentState,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sequence: Option<AgentReportSequence>,
    },
    Clear {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sequence: Option<AgentReportSequence>,
    },
    Release {
        source: String,
        agent: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sequence: Option<AgentReportSequence>,
    },
}
impl PanelAgentReport {
    pub fn validate(&self) -> Result<(), ContractError> {
        let agent = match self {
            Self::State { agent, .. } | Self::Release { agent, .. } => agent,
            Self::Clear { .. } => return Ok(()),
        };
        normalize_agent_report_label(agent)
            .map(|_| ())
            .ok_or(ContractError::InvalidDocument)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PanelAgentAction {
    Titles {
        bindings: Vec<orchbee_agent_observation_protocol::AgentLaunchBinding>,
    },
    ReadPresentations {
        presentation_ids: Vec<TerminalPresentationId>,
    },
    Explain {
        binding: orchbee_agent_observation_protocol::AgentLaunchBinding,
    },
    States {
        presentation_ids: Vec<TerminalPresentationId>,
    },
    Observe {
        binding: orchbee_agent_observation_protocol::AgentLaunchBinding,
        #[serde(deserialize_with = "nullable")]
        after_sequence: Option<WorkbenchRevision>,
    },
    Read {
        panel_ids: Vec<WorkbenchPanelId>,
    },
    ReportPresentation {
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
        expected_binding: PresentationBinding,
        report: Box<PanelAgentReport>,
    },
    Report {
        panel_id: WorkbenchPanelId,
        expected_binding: PresentationBinding,
        report: Box<PanelAgentReport>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelAgentRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub action: PanelAgentAction,
}
impl PanelAgentRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.action {
            PanelAgentAction::Titles { bindings } => {
                if titles::valid_bindings(bindings) {
                    Ok(())
                } else {
                    Err(ContractError::InvalidDocument)
                }
            }
            PanelAgentAction::ReadPresentations { presentation_ids } => {
                if presentation_ids.len() > MAXIMUM_PANEL_AGENT_READ_ROWS
                    || presentation_ids.iter().collect::<BTreeSet<_>>().len()
                        != presentation_ids.len()
                {
                    return Err(ContractError::InvalidDocument);
                }
                Ok(())
            }
            PanelAgentAction::States { presentation_ids } => {
                if presentation_ids.len() <= MAXIMUM_TERMINAL_AGENT_STATE_ROWS
                    && presentation_ids.iter().collect::<BTreeSet<_>>().len()
                        == presentation_ids.len()
                {
                    Ok(())
                } else {
                    Err(ContractError::InvalidDocument)
                }
            }
            PanelAgentAction::Observe { binding, .. } | PanelAgentAction::Explain { binding } => {
                if orchbee_agent_observation_protocol::AgentHostEpoch::parse(
                    binding.generation.as_str(),
                )
                .is_ok()
                {
                    Ok(())
                } else {
                    Err(ContractError::InvalidDocument)
                }
            }
            PanelAgentAction::Read { panel_ids }
                if panel_ids.len() <= MAXIMUM_PANEL_AGENT_READ_ROWS
                    && panel_ids.iter().collect::<BTreeSet<_>>().len() == panel_ids.len() =>
            {
                Ok(())
            }
            PanelAgentAction::Read { .. } => Err(ContractError::InvalidDocument),
            PanelAgentAction::Report { report, .. }
            | PanelAgentAction::ReportPresentation { report, .. } => report.validate(),
        }
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelAgentView {
    #[serde(deserialize_with = "nullable")]
    pub agent: Option<String>,
    pub state: PanelAgentState,
    #[serde(deserialize_with = "nullable")]
    pub authority_source: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub message: Option<String>,
}
impl PanelAgentView {
    pub(super) fn valid(&self) -> bool {
        self.agent
            .as_ref()
            .is_none_or(|agent| !agent.is_empty() && agent.trim() == agent)
            && (self.authority_source.is_none() || self.agent.is_some())
            && (self.message.is_none() || self.authority_source.is_some())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelAgentRow {
    pub panel_id: WorkbenchPanelId,
    #[serde(deserialize_with = "nullable")]
    pub binding: Option<PresentationBinding>,
    #[serde(deserialize_with = "nullable")]
    pub report: Option<PanelAgentView>,
}
impl PanelAgentRow {
    fn valid(&self) -> bool {
        self.binding.is_some() == self.report.is_some()
            && self.report.as_ref().is_none_or(PanelAgentView::valid)
    }
}
/// An exact primary or child member; the nested row describes this member, not its Panel primary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationAgentRow {
    pub presentation_id: TerminalPresentationId,
    pub panel: PanelAgentRow,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PanelAgentFailure {
    Denied,
    Unavailable,
    #[serde(rename = "session-mismatch")]
    SessionMismatch,
    InvalidRequest,
    NotFound,
    Unbound,
    BindingChanged,
    SourceLimit,
    HistoryLimit,
    ResourceLimit,
    RevisionExhausted,
    ClockOverflow,
    ResultTooLarge,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PanelAgentOutcome {
    Titles {
        workbench_revision: WorkbenchRevision,
        presentations: Vec<TerminalTitleRow>,
    },
    PresentationsRead {
        workbench_revision: WorkbenchRevision,
        report_revision: WorkbenchRevision,
        presentations: Vec<PresentationAgentRow>,
    },
    Explained {
        workbench_revision: WorkbenchRevision,
        report_revision: WorkbenchRevision,
        explanation: Box<TerminalAgentExplanation>,
    },
    States {
        workbench_revision: WorkbenchRevision,
        report_revision: WorkbenchRevision,
        presentations: Vec<TerminalAgentStateRow>,
    },
    Observed {
        workbench_revision: WorkbenchRevision,
        report_revision: WorkbenchRevision,
        observation: Box<TerminalAgentObservation>,
    },
    Read {
        workbench_revision: WorkbenchRevision,
        report_revision: WorkbenchRevision,
        panels: Vec<PanelAgentRow>,
    },
    PresentationReported {
        workbench_revision: WorkbenchRevision,
        report_revision: WorkbenchRevision,
        panel_id: WorkbenchPanelId,
        presentation_id: TerminalPresentationId,
        binding: PresentationBinding,
        disposition: PanelAgentDisposition,
        changed: bool,
        view: Box<PanelAgentView>,
    },
    Reported {
        workbench_revision: WorkbenchRevision,
        report_revision: WorkbenchRevision,
        panel_id: WorkbenchPanelId,
        binding: PresentationBinding,
        disposition: PanelAgentDisposition,
        changed: bool,
        view: Box<PanelAgentView>,
    },
    Failed {
        reason: PanelAgentFailure,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelAgentResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub outcome: PanelAgentOutcome,
}
impl PanelAgentResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        let valid = match &self.outcome {
            PanelAgentOutcome::Titles { presentations, .. } => {
                presentations.len() <= MAXIMUM_TERMINAL_TITLE_ROWS
                    && presentations.iter().all(TerminalTitleRow::valid)
                    && presentations
                        .iter()
                        .map(|r| &r.binding.presentation_id)
                        .collect::<BTreeSet<_>>()
                        .len()
                        == presentations.len()
            }
            PanelAgentOutcome::PresentationsRead { presentations, .. } => {
                presentations.len() <= MAXIMUM_PANEL_AGENT_READ_ROWS
                    && presentations
                        .iter()
                        .map(|r| &r.presentation_id)
                        .collect::<BTreeSet<_>>()
                        .len()
                        == presentations.len()
                    && presentations.iter().all(|r| r.panel.valid())
            }
            PanelAgentOutcome::Explained { explanation, .. } => explanation.valid(),
            PanelAgentOutcome::States { presentations, .. } => {
                presentations.len() <= MAXIMUM_TERMINAL_AGENT_STATE_ROWS
                    && presentations
                        .iter()
                        .map(|r| &r.presentation_id)
                        .collect::<BTreeSet<_>>()
                        .len()
                        == presentations.len()
                    && presentations.iter().all(TerminalAgentStateRow::valid)
            }
            PanelAgentOutcome::Observed { observation, .. } => observation.valid(),
            PanelAgentOutcome::Read { panels, .. } => {
                panels.len() <= MAXIMUM_PANEL_AGENT_READ_ROWS
                    && panels
                        .iter()
                        .map(|p| &p.panel_id)
                        .collect::<BTreeSet<_>>()
                        .len()
                        == panels.len()
                    && panels.iter().all(PanelAgentRow::valid)
            }
            PanelAgentOutcome::Reported { view, .. }
            | PanelAgentOutcome::PresentationReported { view, .. } => view.valid(),
            PanelAgentOutcome::Failed { .. } => true,
        };
        if valid {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
    pub fn answers(&self, request: &PanelAgentRequest) -> bool {
        if self.request_id != request.request_id
            || self.session_id != request.session_id
            || self.validate().is_err()
        {
            return false;
        }
        {
            let shared_failure = matches!(&self.outcome, PanelAgentOutcome::Failed { .. });
            match &request.action {
                PanelAgentAction::Titles { bindings } => {
                    matches!(&self.outcome, PanelAgentOutcome::Titles { presentations, .. } if bindings.iter().eq(presentations.iter().map(|r| &r.binding)))
                        || shared_failure
                }
                PanelAgentAction::Explain { binding } => {
                    matches!(&self.outcome, PanelAgentOutcome::Explained { explanation, .. } if &explanation.binding == binding)
                        || shared_failure
                }
                PanelAgentAction::States { presentation_ids } => {
                    matches!(&self.outcome, PanelAgentOutcome::States { presentations, .. } if presentation_ids
                .iter()
                .eq(presentations.iter().map(|r| &r.presentation_id)))
                        || shared_failure
                }
                PanelAgentAction::Observe {
                    binding,
                    after_sequence,
                } => {
                    matches!(&self.outcome, PanelAgentOutcome::Observed { observation, .. } if &observation.binding == binding && observation.covers(*after_sequence))
                        || shared_failure
                }
                PanelAgentAction::ReadPresentations { presentation_ids } => {
                    matches!(&self.outcome, PanelAgentOutcome::PresentationsRead { presentations, .. } if presentation_ids
                .iter()
                .eq(presentations.iter().map(|row| &row.presentation_id)))
                        || shared_failure
                }
                PanelAgentAction::Read { panel_ids } => {
                    matches!(&self.outcome, PanelAgentOutcome::Read { panels, .. } if panel_ids.iter().eq(panels.iter().map(|p| &p.panel_id)))
                        || shared_failure
                }
                PanelAgentAction::ReportPresentation {
                    panel_id,
                    presentation_id,
                    expected_binding,
                    ..
                } => {
                    matches!(&self.outcome, PanelAgentOutcome::PresentationReported {
                            panel_id: actual,
                            presentation_id: actual_presentation,
                            binding,
                            ..
                        } if panel_id == actual
                            && presentation_id == actual_presentation
                            && expected_binding == binding)
                        || shared_failure
                }
                PanelAgentAction::Report {
                    panel_id,
                    expected_binding,
                    ..
                } => {
                    matches!(&self.outcome, PanelAgentOutcome::Reported {
                    panel_id: actual,
                    binding,
                    ..
                } if panel_id == actual && expected_binding == binding)
                        || shared_failure
                }
            }
        }
    }
}
