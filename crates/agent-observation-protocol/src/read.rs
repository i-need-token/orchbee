use serde::{Deserialize, Deserializer, Serialize, de};

use crate::cursor::AgentListCursor;
use crate::identity::{AgentSafeInteger, AgentSessionId, ObapRequestId};
use crate::launch::{AgentLaunchRequest, AgentLaunchResult};
use crate::model::{
    AgentSessionDetail, AgentSessionExplanation, AgentSessionSummary, deserialize_required_nullable,
};
use crate::names::{AgentNamesRequest, AgentNamesResult};

/// The `limit` default applied when a list request omits the field, matching
/// the canonical input contract.
pub const DEFAULT_LIST_LIMIT: u64 = 50;
pub const MAXIMUM_LIST_LIMIT: u64 = 100;
pub const MAXIMUM_LIST_ITEMS: usize = 100;
pub const MAXIMUM_PENDING_REQUESTS: usize = 128;
pub const MAXIMUM_PRESENTATION_ATTACHMENTS: usize = 32;
pub const MAXIMUM_CAPABILITIES: usize = 10;

fn default_list_limit() -> AgentSafeInteger {
    AgentSafeInteger::new(DEFAULT_LIST_LIMIT)
}

/// The list `cursor` key is absent-or-string: when the key is present the
/// value must decode as a cursor, and an explicit `null` is rejected rather
/// than collapsed into `None` — omission and `null` are distinct wire cases.
fn deserialize_absent_or_cursor<'de, D>(
    deserializer: D,
) -> Result<Option<AgentListCursor>, D::Error>
where
    D: Deserializer<'de>,
{
    struct AbsentOrCursor;

    impl<'de> de::Visitor<'de> for AbsentOrCursor {
        type Value = Option<AgentListCursor>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a list cursor string or an omitted key")
        }

        fn visit_none<Error>(self) -> Result<Self::Value, Error>
        where
            Error: de::Error,
        {
            Err(Error::custom("cursor must be a string or omitted"))
        }

        fn visit_unit<Error>(self) -> Result<Self::Value, Error>
        where
            Error: de::Error,
        {
            Err(Error::custom("cursor must be a string or omitted"))
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            AgentListCursor::deserialize(deserializer).map(Some)
        }
    }

    deserializer.deserialize_option(AbsentOrCursor)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum AgentReadRequest {
    #[serde(rename = "agent.session.list.request")]
    List {
        request_id: ObapRequestId,
        #[serde(default)]
        include_ended: bool,
        #[serde(default = "default_list_limit")]
        limit: AgentSafeInteger,
        /// Absent-or-string on the wire: omission means no continuation, an
        /// explicit `null` violates the schema, and `None` never serializes
        /// the key.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_absent_or_cursor"
        )]
        cursor: Option<AgentListCursor>,
    },
    #[serde(rename = "agent.session.get.request")]
    Get {
        request_id: ObapRequestId,
        session_id: AgentSessionId,
    },
    #[serde(rename = "agent.session.explain.request")]
    Explain {
        request_id: ObapRequestId,
        session_id: AgentSessionId,
    },
}

impl AgentReadRequest {
    #[must_use]
    pub fn request_id(&self) -> &ObapRequestId {
        match self {
            Self::List { request_id, .. }
            | Self::Get { request_id, .. }
            | Self::Explain { request_id, .. } => request_id,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentListRejectionReason {
    CursorInvalid,
    CursorStale,
    ReadDenied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionRejectionReason {
    NotFound,
    ReadDenied,
}

/// The only version-1 unavailability reason; no draining state is reserved
/// before a real owner exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentUnavailableReason {
    RuntimeUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentListOutcome {
    Listed {
        items: Vec<AgentSessionSummary>,
        /// Required-but-nullable on the wire: `null` ends pagination.
        #[serde(deserialize_with = "deserialize_required_nullable")]
        next_cursor: Option<AgentListCursor>,
        revision: AgentSafeInteger,
    },
    Rejected {
        reason: AgentListRejectionReason,
    },
    Unavailable {
        reason: AgentUnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentGetOutcome {
    Found {
        revision: AgentSafeInteger,
        session: Box<AgentSessionDetail>,
    },
    Rejected {
        reason: AgentSessionRejectionReason,
    },
    Unavailable {
        reason: AgentUnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentExplainOutcome {
    Found {
        revision: AgentSafeInteger,
        session: Box<AgentSessionDetail>,
        explanation: AgentSessionExplanation,
    },
    Rejected {
        reason: AgentSessionRejectionReason,
    },
    Unavailable {
        reason: AgentUnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentListOutcomeEnvelope {
    pub outcome: AgentListOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentGetOutcomeEnvelope {
    pub outcome: AgentGetOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentExplainOutcomeEnvelope {
    pub outcome: AgentExplainOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum AgentReadResult {
    #[serde(rename = "agent.session.list.result")]
    List {
        request_id: ObapRequestId,
        result: AgentListOutcomeEnvelope,
    },
    #[serde(rename = "agent.session.get.result")]
    Get {
        request_id: ObapRequestId,
        result: AgentGetOutcomeEnvelope,
    },
    #[serde(rename = "agent.session.explain.result")]
    Explain {
        request_id: ObapRequestId,
        result: AgentExplainOutcomeEnvelope,
    },
}

impl AgentReadResult {
    #[must_use]
    pub fn request_id(&self) -> &ObapRequestId {
        match self {
            Self::List { request_id, .. }
            | Self::Get { request_id, .. }
            | Self::Explain { request_id, .. } => request_id,
        }
    }

    /// Whether the outcome is a small terminal refusal; producers bound these
    /// bodies at 1,024 bytes before serialization.
    #[must_use]
    pub fn is_small_outcome(&self) -> bool {
        match self {
            Self::List { result, .. } => matches!(
                result.outcome,
                AgentListOutcome::Rejected { .. } | AgentListOutcome::Unavailable { .. }
            ),
            Self::Get { result, .. } => matches!(
                result.outcome,
                AgentGetOutcome::Rejected { .. } | AgentGetOutcome::Unavailable { .. }
            ),
            Self::Explain { result, .. } => matches!(
                result.outcome,
                AgentExplainOutcome::Rejected { .. } | AgentExplainOutcome::Unavailable { .. }
            ),
        }
    }
}

/// The closed request union every OBAP frame on this channel carries: the
/// session reads or the names sidecar's own read/manage family. A manage is
/// never typed as a session read — it enters through `Names`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentRequest {
    Read(AgentReadRequest),
    Names(AgentNamesRequest),
    Launch(AgentLaunchRequest),
}

impl AgentRequest {
    #[must_use]
    pub fn request_id(&self) -> &ObapRequestId {
        match self {
            Self::Read(request) => request.request_id(),
            Self::Names(request) => request.request_id(),
            Self::Launch(request) => request.request_id(),
        }
    }
}

impl From<AgentReadRequest> for AgentRequest {
    fn from(request: AgentReadRequest) -> Self {
        Self::Read(request)
    }
}

impl From<AgentNamesRequest> for AgentRequest {
    fn from(request: AgentNamesRequest) -> Self {
        Self::Names(request)
    }
}

/// The closed result union — each variant carries the same `requestId`
/// correlation shape as its request family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentResult {
    Read(AgentReadResult),
    Names(AgentNamesResult),
    Launch(AgentLaunchResult),
}

impl AgentResult {
    #[must_use]
    pub fn request_id(&self) -> &ObapRequestId {
        match self {
            Self::Read(result) => result.request_id(),
            Self::Names(result) => result.request_id(),
            Self::Launch(result) => result.request_id(),
        }
    }

    /// Whether the outcome is a small terminal answer; producers bound these
    /// bodies at 1,024 bytes before serialization. Only `list`-family
    /// enumerations are bounded-read payloads.
    #[must_use]
    pub fn is_small_outcome(&self) -> bool {
        match self {
            Self::Read(result) => result.is_small_outcome(),
            Self::Names(result) => result.is_small_outcome(),
            Self::Launch(_) => true,
        }
    }
}

impl From<AgentReadResult> for AgentResult {
    fn from(result: AgentReadResult) -> Self {
        Self::Read(result)
    }
}

impl From<AgentNamesResult> for AgentResult {
    fn from(result: AgentNamesResult) -> Self {
        Self::Names(result)
    }
}

impl From<AgentLaunchRequest> for AgentRequest {
    fn from(request: AgentLaunchRequest) -> Self {
        Self::Launch(request)
    }
}
impl From<AgentLaunchResult> for AgentResult {
    fn from(result: AgentLaunchResult) -> Self {
        Self::Launch(result)
    }
}
