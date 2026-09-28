//! Explicit terminal Agent reports and reads; process proof is never a public request field.
use crate::{
    ContractError, PresentationBinding, ProtocolVersion, SiteId, TerminalPresentationId,
    WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
};
use orchbee_agent_observation_protocol::{
    AgentProviderSessionReference as Reference, AgentProviderSessionReferenceKind as ReferenceKind,
    AgentReportSequence, AgentReportSourcePolicy, agent_report_source_policy,
    normalize_agent_report_label,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;
mod explanation;
mod manifest;
pub use manifest::{
    AgentDetectionManifest, AgentManifestSource, AgentManifestWarning, AgentManifestWarningKind,
};
mod observation;
pub use explanation::{
    AgentDetectionEvaluation, AgentDetectionEvidence, AgentDetectionRule, AgentDetectionSample,
    AgentScreenExplanation, TerminalAgentExplanation,
};
mod state_snapshot;
pub use observation::{
    MAXIMUM_TERMINAL_AGENT_CHANGES, TerminalAgentChange, TerminalAgentObservation,
};
pub use state_snapshot::{
    MAXIMUM_TERMINAL_AGENT_STATE_ROWS, TerminalAgentStateRow, TerminalAgentStateSnapshot,
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
    #[default]
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PanelAgentDisposition {
    Applied,
    Ignored,
    Deferred,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_session_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_session_path: Option<String>,
    },
    Session {
        source: String,
        agent: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sequence: Option<AgentReportSequence>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_session_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_session_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_start_source: Option<String>,
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
            Self::State { agent, .. }
            | Self::Session { agent, .. }
            | Self::Release { agent, .. } => agent,
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
    pub site_id: SiteId,
    pub action: PanelAgentAction,
}
impl PanelAgentRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.action {
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
            PanelAgentAction::Report { report, .. } => report.validate(),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelAgentReference {
    pub kind: ReferenceKind,
    pub value: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelAgentSession {
    pub source: String,
    pub agent: String,
    pub reference: PanelAgentReference,
}
impl PanelAgentSession {
    fn valid(&self) -> bool {
        if normalize_agent_report_label(&self.agent).as_deref() != Some(&self.agent)
            || agent_report_source_policy(&self.source, &self.agent)
                == AgentReportSourcePolicy::Custom
        {
            return false;
        }
        match self.reference.kind {
            ReferenceKind::Id => Reference::id(&self.reference.value).is_some(),
            // Platform absoluteness is proved by the reporting Server before publication.
            ReferenceKind::Path => {
                matches!(self.agent.as_str(), "pi" | "omp")
                    && Reference::path(&self.reference.value, |_| true).is_some()
            }
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
    #[serde(deserialize_with = "nullable")]
    pub session: Option<PanelAgentSession>,
}
impl PanelAgentView {
    fn valid(&self) -> bool {
        self.agent
            .as_ref()
            .is_none_or(|agent| !agent.is_empty() && agent.trim() == agent)
            && (self.authority_source.is_none() || self.agent.is_some())
            && (self.message.is_none() || self.authority_source.is_some())
            && self.session.as_ref().is_none_or(PanelAgentSession::valid)
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PanelAgentFailure {
    Denied,
    Unavailable,
    SiteMismatch,
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
    pub site_id: SiteId,
    pub outcome: PanelAgentOutcome,
}
impl PanelAgentResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        let valid = match &self.outcome {
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
                    && panels.iter().all(|p| {
                        p.binding.is_some() == p.report.is_some()
                            && p.report.as_ref().is_none_or(PanelAgentView::valid)
                    })
            }
            PanelAgentOutcome::Reported { view, .. } => view.valid(),
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
            || self.site_id != request.site_id
            || self.validate().is_err()
        {
            return false;
        }
        match (&request.action, &self.outcome) {
            (
                PanelAgentAction::Explain { binding },
                PanelAgentOutcome::Explained { explanation, .. },
            ) => &explanation.binding == binding,
            (
                PanelAgentAction::States { presentation_ids },
                PanelAgentOutcome::States { presentations, .. },
            ) => presentation_ids
                .iter()
                .eq(presentations.iter().map(|r| &r.presentation_id)),
            (
                PanelAgentAction::Observe {
                    binding,
                    after_sequence,
                },
                PanelAgentOutcome::Observed { observation, .. },
            ) => &observation.binding == binding && observation.covers(*after_sequence),
            (_, PanelAgentOutcome::Failed { .. }) => true,
            (PanelAgentAction::Read { panel_ids }, PanelAgentOutcome::Read { panels, .. }) => {
                panel_ids.iter().eq(panels.iter().map(|p| &p.panel_id))
            }
            (
                PanelAgentAction::Report {
                    panel_id,
                    expected_binding,
                    ..
                },
                PanelAgentOutcome::Reported {
                    panel_id: actual,
                    binding,
                    ..
                },
            ) => panel_id == actual && expected_binding == binding,
            _ => false,
        }
    }
}
