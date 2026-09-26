use std::{fmt, num::NonZeroU64};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{
    ContractError, ProtocolVersion, SiteId, WorkbenchRequestId, WorkbenchRevision, WorkbenchTabId,
    WorkbenchTerminalContextId,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WorkbenchTabCloseAttemptGeneration(NonZeroU64);

impl WorkbenchTabCloseAttemptGeneration {
    pub fn new(value: u64) -> Result<Self, ContractError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(ContractError::InvalidDocument)
    }

    pub fn parse(value: &str) -> Result<Self, ContractError> {
        if value.is_empty()
            || value.len() > 20
            || value.starts_with('0')
            || !value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(ContractError::InvalidDocument);
        }
        Self::new(
            value
                .parse()
                .map_err(|_error| ContractError::InvalidDocument)?,
        )
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0.get()
    }

    #[must_use]
    pub fn checked_next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

impl fmt::Display for WorkbenchTabCloseAttemptGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Serialize for WorkbenchTabCloseAttemptGeneration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for WorkbenchTabCloseAttemptGeneration {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchTabClosePhase {
    Queued,
    WaitingGrace,
    WaitingOutcome,
    Disposing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchTabCloseUnknownPhase {
    Terminate,
    Force,
    Dispose,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchTabCloseBlockedReason {
    WorkerUnavailable,
    HostRefused,
    CleanupIncomplete,
    InventoryIncomplete,
    IdentityConflict,
    ContinuationBudgetExhausted,
    ResumeGenerationExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchTabCloseUnknownReason {
    TransportLost,
    Deadline,
    InvalidResponse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchTabClosePublicationReason {
    TargetCleanupPending,
    Preparing,
    PreparationFailed,
    AuthorityRetired,
    WorkspaceRetained,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchTabCloseUnavailableReason {
    ServerBusy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchTabCloseResumeRejectionReason {
    OperationUnavailable,
    OperationMismatch,
    RequestIdentityConflict,
    GenerationMismatch,
    NotResumable,
    GenerationExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorkbenchTabCloseStatus {
    Closing {
        phase: WorkbenchTabClosePhase,
    },
    CleanupBlocked {
        reason: WorkbenchTabCloseBlockedReason,
        resumable: bool,
    },
    OutcomeUnknown {
        phase: WorkbenchTabCloseUnknownPhase,
        reason: WorkbenchTabCloseUnknownReason,
    },
    DisposedAwaitingPublication {
        reason: WorkbenchTabClosePublicationReason,
        resumable: bool,
    },
    Closed {
        revision: WorkbenchRevision,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchTabCloseRead {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub terminal_context_id: WorkbenchTerminalContextId,
    pub tab_id: WorkbenchTabId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkbenchTabCloseReadOutcome {
    NotClosing {},
    Denied {},
    Unavailable {
        reason: WorkbenchTabCloseUnavailableReason,
    },
    Available {
        close_request_id: WorkbenchRequestId,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        attempt_generation: WorkbenchTabCloseAttemptGeneration,
        status: WorkbenchTabCloseStatus,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchTabCloseReadResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub outcome: WorkbenchTabCloseReadOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchTabCloseResume {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub terminal_context_id: WorkbenchTerminalContextId,
    pub tab_id: WorkbenchTabId,
    pub close_request_id: WorkbenchRequestId,
    pub expected_attempt_generation: WorkbenchTabCloseAttemptGeneration,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkbenchTabCloseResumeOutcome {
    Resumed {
        attempt_generation: WorkbenchTabCloseAttemptGeneration,
    },
    Stale {
        attempt_generation: WorkbenchTabCloseAttemptGeneration,
    },
    Denied {},
    Unavailable {
        reason: WorkbenchTabCloseUnavailableReason,
    },
    Rejected {
        reason: WorkbenchTabCloseResumeRejectionReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchTabCloseResumeResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub terminal_context_id: WorkbenchTerminalContextId,
    pub tab_id: WorkbenchTabId,
    pub close_request_id: WorkbenchRequestId,
    pub outcome: WorkbenchTabCloseResumeOutcome,
}
