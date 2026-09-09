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
    DirectoryUnavailable,
    WorkerUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalCwdValidationResult {
    pub request: TerminalCwdValidationRequest,
    pub outcome: TerminalCwdValidationOutcome,
}
