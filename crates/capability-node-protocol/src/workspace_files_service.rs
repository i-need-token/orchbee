//! The Server's request that its Worker start one Workspace files service process, and the
//! Worker's answer. The request names which service to run and the one-time ticket the process
//! presents when it connects back to the Server; the files and Git work itself, and every byte of
//! it, travels on that connection, never on this carrier.

use crate::{OperationId, ProtocolError, WorkerGeneration, WorkerId};

/// Which program of the Workspace files service the Worker starts.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkspaceFilesService {
    /// Answers file reads, saves, change lists, Git changes and replacements, one at a time.
    Operations,
    /// Follows the watched directories and reports what changed.
    Watch,
    /// Searches file contents and lists file names.
    Search,
}

/// The sixteen random bytes that tie one service connection to the request that started it; never
/// all zero.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkspaceFilesTicket([u8; 16]);

impl WorkspaceFilesTicket {
    /// # Errors
    ///
    /// Returns [`ProtocolError::InvalidPayload`] for sixteen zero bytes.
    pub fn from_bytes(bytes: [u8; 16]) -> Result<Self, ProtocolError> {
        if bytes.iter().all(|byte| *byte == 0) {
            Err(ProtocolError::InvalidPayload)
        } else {
            Ok(Self(bytes))
        }
    }

    #[must_use]
    pub const fn as_bytes(&self) -> [u8; 16] {
        self.0
    }
}

/// Asks the Worker to start `service`, which connects to the Server's endpoint for `ticket`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceFilesServiceRequest {
    pub operation_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub service: WorkspaceFilesService,
    pub ticket: WorkspaceFilesTicket,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceFilesServiceOutcome {
    /// The process started; whether it connects is the Server's to observe.
    Started,
    /// The process could not be started.
    NotStarted,
    /// The request names another Worker or generation.
    WorkerUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceFilesServiceResult {
    /// The request this result answers, echoed whole.
    pub request: WorkspaceFilesServiceRequest,
    pub outcome: WorkspaceFilesServiceOutcome,
}
