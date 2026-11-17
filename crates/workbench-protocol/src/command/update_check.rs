//! An admitted Client asks the Session Server to run its scheduled automatic-update job now, or
//! reads where that job stands. The job is the schedule's own: the same helper, the same thread and
//! the same state, so a requested run never overlaps a scheduled one. `start` queues a run unless
//! one is queued or running and answers at once; the run's end is read with `read`, which never
//! waits. Clients that see a Session without the `update-check` capability never send it.
use crate::{ProtocolVersion, SessionId, WorkbenchRequestId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateCheckAction {
    /// Queues a run of the update job unless one is queued or running, and joins that one.
    Start,
    /// Reads where the update job stands.
    Read,
}

impl UpdateCheckAction {
    pub const fn mutating(self) -> bool {
        matches!(self, Self::Start)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchUpdateCheckRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub action: UpdateCheckAction,
}

/// How the last finished run of the update job ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateCheckFinish {
    /// The store already holds the newest version.
    UpToDate,
    /// A newer version was installed and made the store's active one.
    Installed,
    /// The run did not complete: no helper started, it failed, or it never answered.
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateCheckRejection {
    SessionMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UpdateCheckOutcome {
    /// A run is queued or running.
    Checking {},
    /// No run is queued or running; the last one, requested or scheduled, ended this way.
    Finished {
        result: UpdateCheckFinish,
    },
    /// No run has finished since the Server started, and none is queued or running.
    Idle {},
    /// This Server runs no automatic updates, so it has no job to run.
    Unavailable {},
    Rejected {
        reason: UpdateCheckRejection,
    },
    /// This Client connection carries no authority over the update job.
    Denied {},
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchUpdateCheckResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub outcome: UpdateCheckOutcome,
}
