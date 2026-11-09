//! Defines one literal directory-validation request tied to a Worker generation and its closed
//! correlated outcome. It neither probes the filesystem nor grants terminal creation or
//! directory-use authority.

use crate::{AbsoluteCwd, OperationId, WorkerGeneration, WorkerId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalCwdValidationRequest {
    pub operation_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub cwd: AbsoluteCwd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalCwdValidationOutcome {
    DirectoryAvailable,
    /// Nothing exists at the path, or a component of it is not a directory.
    DirectoryMissing,
    /// Something other than a directory exists at the path.
    NotADirectory,
    /// The path's metadata could not be read, as when access is denied.
    DirectoryInaccessible,
    WorkerUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalCwdValidationResult {
    pub request: TerminalCwdValidationRequest,
    pub outcome: TerminalCwdValidationOutcome,
}
