//! Bounded canonical change pages; cursor authentication stays Server-owned.

use serde::{Deserialize, Deserializer, Serialize};

use crate::cursor::{AgentCursorError, AgentListCursor};
use crate::identity::{AgentProviderId, AgentSafeInteger, AgentSessionId, ObapRequestId};
use crate::model::{
    AgentLifecycle, AgentObservation, AgentSemanticStatus, deserialize_required_nullable,
};
use crate::read::AgentUnavailableReason;

pub const MAXIMUM_CHANGE_ITEMS: usize = 100;

/// Same bounded encoding as a list token, with a distinct operation and MAC
/// domain. Shape parsing alone never proves authenticity or continuity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentChangesCursor(AgentListCursor);

impl AgentChangesCursor {
    pub fn parse(value: &str) -> Result<Self, AgentCursorError> {
        AgentListCursor::parse(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

fn absent_or_cursor<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<AgentChangesCursor>, D::Error> {
    AgentChangesCursor::deserialize(deserializer).map(Some)
}

fn default_limit() -> AgentSafeInteger {
    AgentSafeInteger::new(50)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum AgentChangesRequest {
    #[serde(rename = "agent.session.changes.request")]
    Read {
        request_id: ObapRequestId,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "absent_or_cursor"
        )]
        cursor: Option<AgentChangesCursor>,
        #[serde(default = "default_limit")]
        limit: AgentSafeInteger,
    },
}

impl AgentChangesRequest {
    #[must_use]
    pub fn request_id(&self) -> &ObapRequestId {
        let Self::Read { request_id, .. } = self;
        request_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentChangeTransition {
    SessionObserved,
    TurnStarted,
    TurnActivityChanged,
    LimitedActivityObserved,
    AttentionRequested,
    AttentionResolved,
    TurnFinished,
    SessionIdleObserved,
    ObservationBecameStale,
    ObservationReconnected,
    PresentationAttached,
    PresentationVisibilityChanged,
    PresentationDetached,
    SessionEnded,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentChangeSession {
    pub lifecycle: AgentLifecycle,
    pub observation: AgentObservation,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    pub parent_session_id: Option<AgentSessionId>,
    pub pending_request_count: AgentSafeInteger,
    pub provider_id: AgentProviderId,
    pub session_id: AgentSessionId,
    pub status: AgentSemanticStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentChange {
    pub revision: AgentSafeInteger,
    pub session: AgentChangeSession,
    pub transition: AgentChangeTransition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentChangesRejectionReason {
    ReadDenied,
    CursorInvalid,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentChangesOutcome {
    Changes {
        changes: Vec<AgentChange>,
        cursor: AgentChangesCursor,
        observed_through_revision: AgentSafeInteger,
        runtime_revision: AgentSafeInteger,
        has_more: bool,
    },
    Current {
        cursor: AgentChangesCursor,
        revision: AgentSafeInteger,
    },
    ResetRequired {
        cursor: AgentChangesCursor,
        revision: AgentSafeInteger,
    },
    Rejected {
        reason: AgentChangesRejectionReason,
    },
    Unavailable {
        reason: AgentUnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentChangesOutcomeEnvelope {
    pub outcome: AgentChangesOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum AgentChangesResult {
    #[serde(rename = "agent.session.changes.result")]
    Read {
        request_id: ObapRequestId,
        result: AgentChangesOutcomeEnvelope,
    },
}

impl AgentChangesResult {
    #[must_use]
    pub fn request_id(&self) -> &ObapRequestId {
        let Self::Read { request_id, .. } = self;
        request_id
    }

    #[must_use]
    pub fn is_small_outcome(&self) -> bool {
        let Self::Read { result, .. } = self;
        !matches!(result.outcome, AgentChangesOutcome::Changes { .. })
    }
}
