//! Source-owned, bounded Agent view specifications shared by Server and native Client.
use crate::{ContractError, ProtocolVersion, SessionId, WorkbenchRequestId, WorkbenchRevision};
use serde::{Deserialize, Serialize};

pub mod evaluation;
mod validation;
pub use validation::parse_agent_view_spec;

pub const MAXIMUM_AGENT_VIEW_SPEC_BYTES: usize = 64 * 1024;
pub const MAXIMUM_AGENT_VIEW_BYTES: usize = MAXIMUM_AGENT_VIEW_SPEC_BYTES + 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentViewSpec {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<AgentViewFilter>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<AgentViewSort>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentViewFilter {
    All {
        filters: Vec<Self>,
    },
    Any {
        filters: Vec<Self>,
    },
    Not {
        filter: Box<Self>,
    },
    Eq {
        field: AgentViewField,
        value: AgentViewValue,
    },
    In {
        field: AgentViewField,
        values: Vec<AgentViewValue>,
    },
    Exists {
        field: AgentViewField,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentViewField {
    Builtin(AgentViewBuiltinField),
    Token(AgentViewToken),
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentViewToken {
    pub token: String,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentViewBuiltinField {
    Status,
    WorkspaceId,
    TabId,
    PaneId,
    Agent,
    Seen,
    StateChangeSeq,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentViewValue {
    String(String),
    Bool(bool),
    Number(u64),
    Context(AgentViewContextValue),
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentViewContextValue {
    pub context: AgentViewContextOperand,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentViewContextOperand {
    CurrentWorkspaceId,
    CurrentTabId,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentViewSort {
    pub field: AgentViewSortField,
    #[serde(default)]
    pub order: AgentViewSortOrder,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentViewSortField {
    Builtin(AgentViewBuiltinSortField),
    Token(AgentViewToken),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentViewBuiltinSortField {
    WorkspaceOrder,
    TabOrder,
    PaneOrder,
    Attention,
    Status,
    Agent,
    Seen,
    StateChangeSeq,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentViewSortOrder {
    #[default]
    Asc,
    Desc,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AgentViewAction {
    Read {},
    Set {
        spec: Box<AgentViewSpec>,
    },
    Clear {
        #[serde(default)]
        source: Option<String>,
    },
}
impl AgentViewAction {
    pub fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::Set { spec } => spec.validate(),
            Self::Clear {
                source: Some(source),
            } => validation::source(source).map(|_| ()),
            Self::Read {} | Self::Clear { source: None } => Ok(()),
        }
    }
    #[must_use]
    pub fn mutating(&self) -> bool {
        !matches!(self, Self::Read {})
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentViewRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub action: AgentViewAction,
}
impl AgentViewRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.action.validate()
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentViewSnapshot {
    pub revision: WorkbenchRevision,
    #[serde(deserialize_with = "nullable")]
    pub spec: Option<AgentViewSpec>,
}
fn nullable<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<AgentViewSpec>, D::Error> {
    Option::deserialize(d)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentViewFailure {
    Denied,
    Unavailable,
    #[serde(rename = "sessionMismatch")]
    SessionMismatch,
    InvalidRequest,
    PluginNotFound,
    PluginDisabled,
    SourceUnavailable,
    RevisionExhausted,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AgentViewOutcome {
    Available { snapshot: AgentViewSnapshot },
    Failed { reason: AgentViewFailure },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentViewResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub outcome: AgentViewOutcome,
}
impl AgentViewResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        if let AgentViewOutcome::Available { snapshot } = &self.outcome
            && let Some(spec) = &snapshot.spec
        {
            spec.validate()?;
            if spec.normalized()? != *spec {
                return Err(ContractError::InvalidDocument);
            }
        }
        Ok(())
    }
    #[must_use]
    pub fn answers(&self, request: &AgentViewRequest) -> bool {
        self.request_id == request.request_id
            && self.session_id == request.session_id
            && self.validate().is_ok()
            && match &self.outcome {
                AgentViewOutcome::Available { snapshot } => match &request.action {
                    AgentViewAction::Set { spec } => spec
                        .normalized()
                        .is_ok_and(|expected| snapshot.spec.as_ref() == Some(&expected)),
                    AgentViewAction::Clear { source } => {
                        snapshot.spec.as_ref().is_none_or(|spec| {
                            source.as_ref().is_some_and(|s| spec.source != s.trim())
                        })
                    }
                    AgentViewAction::Read {} => true,
                },
                AgentViewOutcome::Failed { .. } => true,
            }
    }
}
