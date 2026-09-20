use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Geometry, LaunchSelection, RequestId, WorkerId, WorkingDirectory};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkerConstraint {
    Exact { worker_id: WorkerId },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum StartMode {
    Immediate { geometry: Geometry },
    Deferred,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalCreateRequest {
    pub(crate) request_id: RequestId,
    pub(crate) worker_constraint: Option<WorkerConstraint>,
    pub(crate) cwd: Option<WorkingDirectory>,
    pub(crate) start_mode: StartMode,
    pub(crate) launch: Option<LaunchSelection>,
}

impl TerminalCreateRequest {
    #[must_use]
    pub const fn new(
        request_id: RequestId,
        worker_constraint: Option<WorkerConstraint>,
        cwd: Option<WorkingDirectory>,
        start_mode: StartMode,
        launch: Option<LaunchSelection>,
    ) -> Self {
        Self {
            request_id,
            worker_constraint,
            cwd,
            start_mode,
            launch,
        }
    }

    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    #[must_use]
    pub const fn worker_constraint(&self) -> Option<&WorkerConstraint> {
        self.worker_constraint.as_ref()
    }

    #[must_use]
    pub const fn cwd(&self) -> Option<&WorkingDirectory> {
        self.cwd.as_ref()
    }

    #[must_use]
    pub const fn start_mode(&self) -> &StartMode {
        &self.start_mode
    }

    #[must_use]
    pub const fn launch(&self) -> Option<&LaunchSelection> {
        self.launch.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartRequestError {
    RequestIdentityMatchesCreate,
}

impl fmt::Display for StartRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .write_str("start request identity must differ from terminal create request identity")
    }
}

impl std::error::Error for StartRequestError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalStartRequest {
    pub(crate) request_id: RequestId,
    pub(crate) terminal_create_request_id: RequestId,
    pub(crate) geometry: Geometry,
}

impl TerminalStartRequest {
    pub fn new(
        request_id: RequestId,
        terminal_create_request_id: RequestId,
        geometry: Geometry,
    ) -> Result<Self, StartRequestError> {
        if request_id == terminal_create_request_id {
            return Err(StartRequestError::RequestIdentityMatchesCreate);
        }
        Ok(Self {
            request_id,
            terminal_create_request_id,
            geometry,
        })
    }

    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    #[must_use]
    pub const fn terminal_create_request_id(&self) -> &RequestId {
        &self.terminal_create_request_id
    }

    #[must_use]
    pub const fn geometry(&self) -> Geometry {
        self.geometry
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientMessage {
    TerminalOpen(crate::TerminalOpenRequest),
    TerminalControl(crate::TerminalControlRequest),
    TerminalCreate(TerminalCreateRequest),
    TerminalStart(TerminalStartRequest),
    TerminalViewer(crate::TerminalViewerRequest),
    TerminalSend(crate::TerminalSendRequest),
    TerminalLaunchRead(crate::TerminalLaunchReadRequest),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CreateAcceptedState {
    AwaitingStart,
    Started { geometry: Geometry },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CreateRejectedReason {
    NotAuthorized,
    WorkerConstraintUnsatisfied,
    CwdNotAccepted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CreateUnavailableReason {
    NoCompatibleWorker,
    TerminalRuntimeUnavailable,
    ServerBusy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CreateFailureReason {
    TerminalCreateFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CreateOutcome {
    Accepted { state: CreateAcceptedState },
    Rejected { reason: CreateRejectedReason },
    Unavailable { reason: CreateUnavailableReason },
    Failed { reason: CreateFailureReason },
    OutcomeUnknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalCreateResult {
    pub(crate) request_id: RequestId,
    pub(crate) outcome: CreateOutcome,
}

impl TerminalCreateResult {
    #[must_use]
    pub const fn new(request_id: RequestId, outcome: CreateOutcome) -> Self {
        Self {
            request_id,
            outcome,
        }
    }

    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    #[must_use]
    pub const fn outcome(&self) -> CreateOutcome {
        self.outcome
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StartTransition {
    Started,
    AlreadyStarted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StartRejectedReason {
    NotAuthorized,
    TerminalCreateUnavailable,
    TerminalNotAwaitingStart,
    GeometryConflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StartUnavailableReason {
    TerminalRuntimeUnavailable,
    ServerBusy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StartFailureReason {
    TerminalStartFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum StartOutcome {
    Accepted {
        geometry: Geometry,
        transition: StartTransition,
    },
    Rejected {
        reason: StartRejectedReason,
    },
    Unavailable {
        reason: StartUnavailableReason,
    },
    Failed {
        reason: StartFailureReason,
    },
    OutcomeUnknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalStartResult {
    pub(crate) request_id: RequestId,
    pub(crate) outcome: StartOutcome,
}

impl TerminalStartResult {
    #[must_use]
    pub const fn new(request_id: RequestId, outcome: StartOutcome) -> Self {
        Self {
            request_id,
            outcome,
        }
    }

    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    #[must_use]
    pub const fn outcome(&self) -> StartOutcome {
        self.outcome
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerMessage {
    TerminalOpenResult(crate::TerminalOpenResult),
    TerminalControlResult(crate::TerminalControlResult),
    TerminalCreateResult(TerminalCreateResult),
    TerminalStartResult(TerminalStartResult),
    TerminalViewerResult(crate::TerminalViewerResult),
    TerminalSendResult(crate::TerminalSendResult),
    TerminalLaunchReadResult(crate::TerminalLaunchReadResult),
}

#[cfg(test)]
mod tests;
