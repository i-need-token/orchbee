use std::num::NonZeroU64;

use crate::{HostEpoch, HostTerminalId, OperationId, WorkerGeneration, WorkerId};

// Mirrors the owning Terminal Protocol OBTP header plus maximum OBTF payload.
pub const MAXIMUM_VIEWER_RECORD_BYTES: usize = 64 + 36 * 1024 * 1024;

/// `read-rows` bounds mirrored from the owning OBCS contract: the row count
/// and OBRW-payload byte budget the action admits.
pub const MAXIMUM_READ_ROWS: u16 = 4096;
pub const MINIMUM_READ_ROWS_BYTES: u32 = 112;
pub const MAXIMUM_READ_ROWS_BYTES: u32 = 8 * 1024 * 1024;

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
    // The attachment lives on the request envelope; the action does not
    // repeat it. `maximum_bytes` bounds the OBRW payload, not the record.
    ReadRows {
        start_row: u64,
        row_count: u16,
        revision: u64,
        generation: NonZeroU64,
        request_sequence: NonZeroU64,
        maximum_bytes: u32,
    },
    Detach,
}

/// The closed reason vocabulary a `rows-rejected` result may carry; each
/// variant maps to exactly one Host row-window failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewerRowsRejectedReason {
    StaleRevision,
    StaleGeneration,
    HistoryEvicted,
    MissingResource,
    InvalidField,
    InvalidAcknowledgement,
    LimitExceeded,
    ResourceExhausted,
    InvalidPhase,
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
    RowsRejected(ViewerRowsRejectedReason),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalViewerResult {
    pub request_id: OperationId,
    pub attachment_id: OperationId,
    pub outcome: TerminalViewerOutcome,
}
