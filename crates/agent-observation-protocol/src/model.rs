use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::identity::{
    AgentAttachmentGeneration, AgentAttentionRequestId, AgentEventId, AgentExecutionLocationId,
    AgentPresentationId, AgentProviderId, AgentSafeInteger, AgentSessionId, AgentTurnId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentCapability {
    SessionLifecycle,
    TurnLifecycle,
    Questions,
    Approvals,
    Authentication,
    Activity,
    Subagents,
    Progress,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentActivity {
    Thinking,
    Tool,
    Editing,
    Command,
    Subagent,
    Compacting,
    Retrying,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentPresentationVisibility {
    Visible,
    Hidden,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentObservationFidelity {
    Rich,
    Basic,
    Heuristic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentObservationFreshness {
    Fresh,
    Stale,
    Disconnected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentObservationSource {
    Native,
    Hook,
    Terminal,
    Heuristic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentTurnPhase {
    Queued,
    Working,
    WaitingExternal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentAttentionKind {
    Question,
    Approval,
    Authentication,
    Unclassified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentTurnOutcomeKind {
    Succeeded,
    Failed,
    Canceled,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentLifecycle {
    Live,
    Ended,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentStatusBasis {
    AttentionRequest,
    TurnOutcome,
    ActiveTurn,
    AffirmativeIdle,
    SessionEnded,
    LimitedActivityHint,
    UnobservedTurnState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentFreshnessMeaning {
    CurrentObservation,
    LastObservationMayBeOutdated,
    NoTrustedCurrentRoute,
}

/// The `actionPath` literal: every attention request and explanation in v1
/// navigates only — no Agent surface admits content injection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NavigateOnly;

impl Serialize for NavigateOnly {
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        serializer.serialize_str("navigate-only")
    }
}

impl<'de> Deserialize<'de> for NavigateOnly {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == "navigate-only" {
            Ok(Self)
        } else {
            Err(de::Error::custom("actionPath must be 'navigate-only'"))
        }
    }
}

/// The `contentAvailable` literal: v1 Agent reads never carry provider
/// content, so the flag is the constant `false`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContentUnavailable;

impl Serialize for ContentUnavailable {
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        serializer.serialize_bool(false)
    }
}

impl<'de> Deserialize<'de> for ContentUnavailable {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        if bool::deserialize(deserializer)? {
            Err(de::Error::custom(
                "contentAvailable must be false in version 1",
            ))
        } else {
            Ok(Self)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentExecutionLocation {
    pub execution_location_id: AgentExecutionLocationId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentPresentationAttachment {
    pub generation: AgentAttachmentGeneration,
    pub navigable: bool,
    pub presentation_id: AgentPresentationId,
    pub visibility: AgentPresentationVisibility,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentObservation {
    pub fidelity: AgentObservationFidelity,
    pub freshness: AgentObservationFreshness,
    pub generation: AgentSafeInteger,
    pub sequence: AgentSafeInteger,
    pub source: AgentObservationSource,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentTurnState {
    Unknown {},
    Idle {},
    Active {
        /// Required-but-nullable on the wire.
        #[serde(deserialize_with = "deserialize_required_nullable")]
        activity: Option<AgentActivity>,
        phase: AgentTurnPhase,
        turn_id: AgentTurnId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentAttentionRequest {
    pub action_path: NavigateOnly,
    pub kind: AgentAttentionKind,
    pub request_id: AgentAttentionRequestId,
    /// Required-but-nullable on the wire.
    #[serde(deserialize_with = "deserialize_required_nullable")]
    pub turn_id: Option<AgentTurnId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentTurnOutcome {
    pub event_id: AgentEventId,
    pub kind: AgentTurnOutcomeKind,
    pub turn_id: AgentTurnId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentSemanticStatus {
    NeedsAttention {
        count: AgentPositiveInteger,
    },
    Failed {},
    Succeeded {},
    Canceled {},
    Interrupted {},
    Working {
        /// Required-but-nullable on the wire.
        #[serde(deserialize_with = "deserialize_required_nullable")]
        activity: Option<AgentActivity>,
        phase: AgentTurnPhase,
    },
    Idle {},
    Unknown {
        /// Required-but-nullable on the wire.
        #[serde(deserialize_with = "deserialize_required_nullable")]
        activity_hint: Option<AgentActivity>,
    },
    Ended {},
}

/// A positive safe integer (`1..=2^53-1`) for `needs-attention` counts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AgentPositiveInteger(AgentSafeInteger);

impl AgentPositiveInteger {
    pub fn new(value: u64) -> Option<Self> {
        if value == 0 {
            return None;
        }
        Some(Self(AgentSafeInteger::new(value)))
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0.value()
    }
}

impl Serialize for AgentPositiveInteger {
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AgentPositiveInteger {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        let value = AgentSafeInteger::deserialize(deserializer)?;
        Self::new(value.value())
            .ok_or_else(|| de::Error::custom("needs-attention count must be a positive integer"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSessionSnapshot {
    /// Required-but-nullable on the wire.
    #[serde(deserialize_with = "deserialize_required_nullable")]
    pub activity_hint: Option<AgentActivity>,
    pub capabilities: Vec<AgentCapability>,
    pub execution_location: AgentExecutionLocation,
    /// Required-but-nullable on the wire.
    #[serde(deserialize_with = "deserialize_required_nullable")]
    pub last_outcome: Option<AgentTurnOutcome>,
    pub lifecycle: AgentLifecycle,
    pub observation: AgentObservation,
    /// Required-but-nullable on the wire.
    #[serde(deserialize_with = "deserialize_required_nullable")]
    pub parent_session_id: Option<AgentSessionId>,
    pub pending_requests: Vec<AgentAttentionRequest>,
    pub presentation_attachments: Vec<AgentPresentationAttachment>,
    pub provider_id: AgentProviderId,
    pub session_id: AgentSessionId,
    pub turn: AgentTurnState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSessionSummary {
    pub execution_location: AgentExecutionLocation,
    pub lifecycle: AgentLifecycle,
    pub observation: AgentObservation,
    /// Required-but-nullable on the wire.
    #[serde(deserialize_with = "deserialize_required_nullable")]
    pub parent_session_id: Option<AgentSessionId>,
    pub pending_request_count: AgentSafeInteger,
    pub presentation_attachments: Vec<AgentPresentationAttachment>,
    pub provider_id: AgentProviderId,
    pub session_id: AgentSessionId,
    pub status: AgentSemanticStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSessionDetail {
    pub snapshot: AgentSessionSnapshot,
    pub status: AgentSemanticStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSessionExplanation {
    pub action_path: NavigateOnly,
    pub capability_ceiling: Vec<AgentCapability>,
    pub content_available: ContentUnavailable,
    pub fidelity: AgentObservationFidelity,
    pub freshness: AgentObservationFreshness,
    pub freshness_meaning: AgentFreshnessMeaning,
    pub generation: AgentSafeInteger,
    pub sequence: AgentSafeInteger,
    pub source: AgentObservationSource,
    pub status: AgentSemanticStatus,
    pub status_basis: AgentStatusBasis,
}

/// Fields the canonical schema declares `.nullable()`: the key must be
/// present and may carry `null`, but a missing key is a contract violation.
pub(crate) fn deserialize_required_nullable<'de, D, Value>(
    deserializer: D,
) -> Result<Option<Value>, D::Error>
where
    D: Deserializer<'de>,
    Value: Deserialize<'de>,
{
    Option::<Value>::deserialize(deserializer)
}

impl fmt::Display for AgentCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SessionLifecycle => "sessionLifecycle",
            Self::TurnLifecycle => "turnLifecycle",
            Self::Questions => "questions",
            Self::Approvals => "approvals",
            Self::Authentication => "authentication",
            Self::Activity => "activity",
            Self::Subagents => "subagents",
            Self::Progress => "progress",
        })
    }
}
