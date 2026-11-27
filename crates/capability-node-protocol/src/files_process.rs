//! Defines the generation-fenced start and end of the one `orchbee-files` process a Worker
//! generation runs for its Server. The Server chooses the one-shot endpoint and the token; the
//! Worker checks its cohort's `orchbee-files`, starts it and answers with its exact identity, which
//! the Server names to end a start whose link never became ready.

use crate::{OperationId, ProtocolError, WorkerGeneration, WorkerId};

/// Bytes of the endpoint address a start names.
pub const MAXIMUM_FILES_ENDPOINT_BYTES: usize = 512;
/// Bytes of the token a start passes to the process.
pub const FILES_TOKEN_BYTES: usize = 32;

/// One `orchbee-files` process: its process ID and its start, the creation time on Windows and the
/// start time on macOS and Linux, as the Worker that started it reads them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilesProcessIdentity {
    pub process_id: u32,
    pub start: u64,
}

/// The address of the endpoint a start's process dials: absolute, 1 to 512 bytes of UTF-8 without
/// NUL. The Server binds it and accepts one connection on it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesEndpoint(String);

impl FilesEndpoint {
    /// Admits `address` when it has the shape above. Whether it names a bound endpoint is the
    /// Server's.
    pub fn new(address: String) -> Result<Self, ProtocolError> {
        let absolute = address.starts_with('/') || address.starts_with(r"\\");
        if !absolute || address.len() > MAXIMUM_FILES_ENDPOINT_BYTES || address.contains('\0') {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(address))
    }

    /// The address as the start passes it to the process.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The secret a start's process presents in its hello, so the Server admits only the process this
/// start created. Its value never appears in a debug rendering.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct FilesToken([u8; FILES_TOKEN_BYTES]);

impl FilesToken {
    /// The token of these bytes; the Server draws them from the system's random source.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; FILES_TOKEN_BYTES]) -> Self {
        Self(bytes)
    }

    /// The token's bytes, which only the start's process and the admitting Server read.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; FILES_TOKEN_BYTES] {
        &self.0
    }
}

impl std::fmt::Debug for FilesToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("FilesToken(..)")
    }
}

/// What a request asks of the Worker's one `orchbee-files` process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FilesProcessAction {
    /// Start the process, which dials `endpoint` and presents `token`.
    Start {
        endpoint: FilesEndpoint,
        token: FilesToken,
    },
    /// End the process `process` names, which this Worker generation started.
    End { process: FilesProcessIdentity },
}

impl FilesProcessAction {
    /// Whether `outcome` answers this action: a start's outcome a start, an end's an end.
    #[must_use]
    pub const fn admits(&self, outcome: &FilesProcessOutcome) -> bool {
        matches!(
            (self, outcome),
            (Self::Start { .. }, FilesProcessOutcome::Start(_))
                | (Self::End { .. }, FilesProcessOutcome::End(_))
        )
    }
}

/// One start or end, fenced to the Worker generation it names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesProcessRequest {
    pub operation_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub action: FilesProcessAction,
}

/// The Worker's answer to a start, given as soon as the process exists or the start failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilesStartOutcome {
    /// The process started, and the Worker holds it until it ends.
    Started(FilesProcessIdentity),
    /// The cohort declares no `orchbee-files`, or holds no such file.
    Missing,
    /// The file differs from the digest the cohort declares, or another user could change it.
    Unverified,
    /// The process could not be created, or the one this Worker held did not end first.
    Failed,
    /// The request named another Worker or generation; nothing was started.
    RejectedFencing,
}

/// The Worker's answer to an end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilesEndOutcome {
    /// The process has ended and the Worker reaped it.
    Ended,
    /// This Worker holds no process of that identity: it ended earlier, or never was this Worker's.
    Unknown,
    /// The process did not end within the Worker's settle budget.
    Failed,
    /// The request named another Worker or generation; nothing was ended.
    RejectedFencing,
}

/// A start's or an end's outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilesProcessOutcome {
    Start(FilesStartOutcome),
    End(FilesEndOutcome),
}

/// The answer to one request, echoing its operation ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilesProcessResult {
    pub operation_id: OperationId,
    pub outcome: FilesProcessOutcome,
}
