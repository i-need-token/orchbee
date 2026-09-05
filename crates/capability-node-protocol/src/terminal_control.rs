use crate::{OperationId, ProtocolError, TerminalPixelGeometry, WorkerGeneration, WorkerId};
use std::num::NonZeroU64;
pub const MAXIMUM_CONTROL_INPUT_BYTES: usize = 4096;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlInput(Vec<u8>);
impl ControlInput {
    pub fn new(bytes: Vec<u8>) -> Result<Self, ProtocolError> {
        if bytes.is_empty() || bytes.len() > MAXIMUM_CONTROL_INPUT_BYTES {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(bytes))
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalControlAction {
    Acquire,
    Release,
    Input {
        sequence: NonZeroU64,
        bytes: ControlInput,
    },
    Resize {
        sequence: NonZeroU64,
        geometry: TerminalPixelGeometry,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalControlRequest {
    pub request_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub viewer_id: OperationId,
    pub controller_id: OperationId,
    pub action: TerminalControlAction,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlRejection {
    WrongWorker,
    StaleWorkerGeneration,
    ViewerUnavailable,
    ControllerBusy,
    NotController,
    SequenceMismatch,
    HostRejected,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalControlOutcome {
    Acquired,
    Released,
    InputAccepted,
    Resized(TerminalPixelGeometry),
    Rejected(ControlRejection),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalControlResult {
    pub request_id: OperationId,
    pub viewer_id: OperationId,
    pub controller_id: OperationId,
    pub outcome: TerminalControlOutcome,
}
