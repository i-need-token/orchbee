//! The OBAP names sidecar family: selector resolution plus the bounded
//! read/manage operations over the Server's per-Site session alias table.
//! These are their own closed request/result family — a manage is never
//! typed as a session read. Everything here is a typed wire DTO; fold
//! semantics live with the Server's `AgentState`.

use serde::{Deserialize, Serialize};

use crate::identity::{AgentName, AgentSessionId, ObapRequestId};
use crate::model::{AgentLifecycle, deserialize_required_nullable};
use crate::read::AgentUnavailableReason;

/// The names table's binding bound — the same count the canonical runtime
/// retains sessions under; the Server enforces it on new bindings only.
pub const MAXIMUM_NAME_BINDINGS: usize = 512;

/// How a targeted Agent command names its session. `Auto` is the CLI
/// positional token — typed as `AgentSessionId` because every valid name is
/// also a valid opaque identity; the Server resolves it id-existence-first,
/// then the names map. `Session`/`Name` are the explicit `--session`/`--name`
/// escapes that never reinterpret.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum AgentSessionSelector {
    #[serde(rename = "session")]
    Session { session_id: AgentSessionId },
    #[serde(rename = "name")]
    Name { name: AgentName },
    #[serde(rename = "auto")]
    Auto { target: AgentSessionId },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum AgentNamesRequest {
    /// Resolve one selector to its committed session identity.
    #[serde(rename = "agent.session.resolve.request")]
    Resolve {
        request_id: ObapRequestId,
        target: AgentSessionSelector,
    },
    /// Enumerate every live binding in the names table.
    #[serde(rename = "agent.session.names.list.request")]
    NamesList { request_id: ObapRequestId },
    /// Bind `name` to the resolved target — assigning over the session's
    /// existing alias is the atomic rename.
    #[serde(rename = "agent.session.name.assign.request")]
    NameAssign {
        request_id: ObapRequestId,
        target: AgentSessionSelector,
        name: AgentName,
    },
    /// Drop the resolved target's alias; never stops or deletes the session.
    #[serde(rename = "agent.session.name.release.request")]
    NameRelease {
        request_id: ObapRequestId,
        target: AgentSessionSelector,
    },
}

impl AgentNamesRequest {
    #[must_use]
    pub fn request_id(&self) -> &ObapRequestId {
        match self {
            Self::Resolve { request_id, .. }
            | Self::NamesList { request_id }
            | Self::NameAssign { request_id, .. }
            | Self::NameRelease { request_id, .. } => request_id,
        }
    }
}

/// Which resolution rule produced a session identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentResolvedBy {
    SessionId,
    Name,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentResolveRejectionReason {
    NotFound,
    ReadDenied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentNamesListRejectionReason {
    ReadDenied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentNameAssignRejectionReason {
    NameInUse,
    SessionNotFound,
    SessionEnded,
    NameNotFound,
    CapacityExceeded,
    ManageDenied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentNameReleaseRejectionReason {
    NameNotFound,
    SessionNotFound,
    ManageDenied,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentResolveOutcome {
    Resolved {
        session_id: AgentSessionId,
        resolved_by: AgentResolvedBy,
    },
    Rejected {
        reason: AgentResolveRejectionReason,
    },
    Unavailable {
        reason: AgentUnavailableReason,
    },
}

/// One names-table binding: the alias, the bound session identity, and the
/// lifecycle the fold observed for it. Only live sessions hold bindings, so
/// `lifecycle` is `live` in every emitted row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentNameBinding {
    pub name: AgentName,
    pub session_id: AgentSessionId,
    pub lifecycle: AgentLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentNamesListOutcome {
    /// The complete table — ≤512 rows in byte-sorted name order. This is a
    /// bounded-read payload, not a small outcome.
    Listed { bindings: Vec<AgentNameBinding> },
    Rejected {
        reason: AgentNamesListRejectionReason,
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
pub enum AgentNameAssignOutcome {
    /// The binding committed; `previousName` is the alias the atomic rename
    /// released — required-but-nullable, `null` for a first naming.
    Assigned {
        session_id: AgentSessionId,
        name: AgentName,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        previous_name: Option<AgentName>,
    },
    /// The pair was already bound — state idempotence, never a replay claim.
    Unchanged {
        session_id: AgentSessionId,
        name: AgentName,
    },
    /// `conflictSessionId` carries the incumbent session on `name-in-use`
    /// and `null` for every other reason.
    Rejected {
        reason: AgentNameAssignRejectionReason,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        conflict_session_id: Option<AgentSessionId>,
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
pub enum AgentNameReleaseOutcome {
    Released {
        session_id: AgentSessionId,
        name: AgentName,
    },
    /// The session exists and holds no alias — nothing was released.
    Unchanged { session_id: AgentSessionId },
    Rejected {
        reason: AgentNameReleaseRejectionReason,
    },
    Unavailable {
        reason: AgentUnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentResolveOutcomeEnvelope {
    pub outcome: AgentResolveOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentNamesListOutcomeEnvelope {
    pub outcome: AgentNamesListOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentNameAssignOutcomeEnvelope {
    pub outcome: AgentNameAssignOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentNameReleaseOutcomeEnvelope {
    pub outcome: AgentNameReleaseOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum AgentNamesResult {
    #[serde(rename = "agent.session.resolve.result")]
    Resolve {
        request_id: ObapRequestId,
        result: AgentResolveOutcomeEnvelope,
    },
    #[serde(rename = "agent.session.names.list.result")]
    NamesList {
        request_id: ObapRequestId,
        result: AgentNamesListOutcomeEnvelope,
    },
    #[serde(rename = "agent.session.name.assign.result")]
    NameAssign {
        request_id: ObapRequestId,
        result: AgentNameAssignOutcomeEnvelope,
    },
    #[serde(rename = "agent.session.name.release.result")]
    NameRelease {
        request_id: ObapRequestId,
        result: AgentNameReleaseOutcomeEnvelope,
    },
}

impl AgentNamesResult {
    #[must_use]
    pub fn request_id(&self) -> &ObapRequestId {
        match self {
            Self::Resolve { request_id, .. }
            | Self::NamesList { request_id, .. }
            | Self::NameAssign { request_id, .. }
            | Self::NameRelease { request_id, .. } => request_id,
        }
    }

    /// Whether the outcome is a small terminal answer; producers bound these
    /// bodies at 1,024 bytes before serialization. Only `listed` is a
    /// bounded-read payload.
    #[must_use]
    pub fn is_small_outcome(&self) -> bool {
        match self {
            Self::NamesList { result, .. } => !matches!(
                result.outcome,
                AgentNamesListOutcome::Listed { .. }
            ),
            Self::Resolve { .. } | Self::NameAssign { .. } | Self::NameRelease { .. } => true,
        }
    }
}
