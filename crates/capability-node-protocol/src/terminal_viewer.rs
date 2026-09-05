use std::num::NonZeroU64;

use crate::{HostEpoch, HostTerminalId, OperationId, WorkerGeneration, WorkerId};

// Mirrors the owning Terminal Protocol OBTP header plus maximum OBTF payload.
pub const MAXIMUM_VIEWER_RECORD_BYTES: usize = 64 + 36 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewerSnapshotReason {
    ActivityStale,
    BaseMismatch,
    DecoderRejected,
    DimensionsMismatch,
    GenerationMismatch,
    IdentityMismatch,
    SequenceGap,
    StoreReset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalViewerAction {
    Attach {
        host_epoch: HostEpoch,
        terminal_id: HostTerminalId,
    },
    Poll,
    FrameApplied {
        sequence: NonZeroU64,
        generation: NonZeroU64,
        revision: u64,
    },
    NeedSnapshot {
        sequence: NonZeroU64,
        reason: ViewerSnapshotReason,
    },
    Detach,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalViewerRequest {
    pub request_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub attachment_id: OperationId,
    pub action: TerminalViewerAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewerRejection {
    WrongWorker,
    StaleWorkerGeneration,
    StaleHostEpoch,
    CapacityExceeded,
    AttachmentUnavailable,
    AttachmentAlreadyExists,
    FeedbackMismatch,
    HostRejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalViewerOutcome {
    Attached,
    Empty,
    Record(Vec<u8>),
    FeedbackAccepted,
    Detached,
    Rejected(ViewerRejection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalViewerResult {
    pub request_id: OperationId,
    pub attachment_id: OperationId,
    pub outcome: TerminalViewerOutcome,
}
