//! An admitted Client asks the Session Server to run its scheduled automatic-update job now, or
//! reads where that job stands. The job is the schedule's own: the same helper, the same thread and
//! the same state, so a requested run never overlaps a scheduled one. `start` queues a run unless
//! one is queued or running and answers at once; the run's end is read with `read`, which never
//! waits. Clients that see a Session without the `update-check` capability never send it. A Client
//! that also sees `update-phases` asks with `phases` and reads the running run's download and
//! install, and which phase a failed run ended in; without it every answer keeps the outcomes of
//! `update-check` alone.
use crate::{ContractError, ProtocolVersion, SessionId, WorkbenchRequestId};
use serde::{Deserialize, Serialize};

/// The largest byte count every JSON binding reads exactly.
const MAXIMUM_BYTES: u64 = 9_007_199_254_740_991;
/// The longest release version an answer names, as a handshake's `productVersion`.
const MAXIMUM_VERSION_BYTES: usize = 64;

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
    /// Asks for the running run's `downloading` and `installing` outcomes and a failed run's
    /// phase; sent only to a Server whose handshake answer lists `update-phases`. Omitted when
    /// false, so earlier bytes are unchanged.
    #[serde(default, skip_serializing_if = "crate::model::is_false")]
    pub phases: bool,
}

/// How the last finished run of the update job ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateCheckFinish {
    /// The store already holds the newest version.
    UpToDate,
    /// A newer version was installed and made the store's active one.
    Installed,
    /// The run did not complete: no helper started, it failed, or it never answered. Only an
    /// answer to a request without `phases` names it; one with `phases` names the phase instead.
    Failed,
    /// The run did not complete before any download started: the feed could not be read or
    /// trusted, no helper started, or the run never answered.
    CheckFailed,
    /// An archive's transfer, or the check of its size and SHA-256, failed.
    DownloadFailed,
    /// Every archive the run needed was downloaded, or none was needed, and installing or
    /// activating the new version failed.
    InstallFailed,
}

impl UpdateCheckFinish {
    /// The result as a Client without `phases` reads it: every failure is `failed`.
    #[must_use]
    pub const fn without_phases(self) -> Self {
        match self {
            Self::CheckFailed | Self::DownloadFailed | Self::InstallFailed => Self::Failed,
            other => other,
        }
    }
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
    /// A run is queued or running; with `phases`, one that has not started a download or an
    /// install yet.
    Checking {},
    /// The running run downloads `version`: `receivedBytes` of the `totalBytes` its archives
    /// hold together, one count across the runtime and the window archive. Only with `phases`.
    Downloading {
        version: String,
        received_bytes: u64,
        total_bytes: u64,
    },
    /// The running run has downloaded everything it needs and installs `version`. Only with
    /// `phases`.
    Installing {
        version: String,
    },
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

impl WorkbenchUpdateCheckResult {
    /// A version names a release in 1 to 64 printable ASCII characters, and a download counts at
    /// least one byte in all and no more received than that.
    ///
    /// # Errors
    /// Returns [`ContractError::InvalidDocument`] for an outcome outside those bounds.
    pub fn validate(&self) -> Result<(), ContractError> {
        let valid = match &self.outcome {
            UpdateCheckOutcome::Downloading {
                version,
                received_bytes,
                total_bytes,
            } => {
                valid_version(version)
                    && (1..=MAXIMUM_BYTES).contains(total_bytes)
                    && received_bytes <= total_bytes
            }
            UpdateCheckOutcome::Installing { version } => valid_version(version),
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
}

fn valid_version(version: &str) -> bool {
    (1..=MAXIMUM_VERSION_BYTES).contains(&version.len())
        && version.bytes().all(|byte| byte.is_ascii_graphic())
}
