use std::num::NonZeroU64;

use crate::{HostEpoch, HostTerminalId, OperationId, WorkerGeneration, WorkerId};

// Mirrors the owning Terminal Protocol OBTP header plus maximum OBTF payload.
pub const MAXIMUM_VIEWER_RECORD_BYTES: usize = 64 + 36 * 1024 * 1024;

/// `read-rows` bounds mirrored from the owning OBCS contract: the row count
/// and OBRW-payload byte budget the action admits.
pub const MAXIMUM_READ_ROWS: u16 = 4096;
pub const MINIMUM_READ_ROWS_BYTES: u32 = 112;
pub const MAXIMUM_READ_ROWS_BYTES: u32 = 8 * 1024 * 1024;

/// Text preserves original read action tags; render uses distinct action tags.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewerRowDetail {
    Text,
    Render,
}

/// Projection delivery the Server requests the Worker to set on a viewer
/// attachment. Binary `demand`: 1 `Active`, 2 `Paused`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewerProjectionDemand {
    Active,
    Paused,
}

impl ViewerProjectionDemand {
    pub const fn from_tag(tag: u8) -> Result<Self, crate::ProtocolError> {
        match tag {
            1 => Ok(Self::Active),
            2 => Ok(Self::Paused),
            _ => Err(crate::ProtocolError::InvalidPayload),
        }
    }

    pub const fn tag(self) -> u8 {
        match self {
            Self::Active => 1,
            Self::Paused => 2,
        }
    }
}

/// Retained-history delivery the Server requests the Worker to set on a viewer
/// attachment. Binary `demand`: 1 `FollowingBottom`, 2 `ViewingHistory`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewerHistoryDemand {
    FollowingBottom,
    ViewingHistory,
}

impl ViewerHistoryDemand {
    pub const fn from_tag(tag: u8) -> Result<Self, crate::ProtocolError> {
        match tag {
            1 => Ok(Self::FollowingBottom),
            2 => Ok(Self::ViewingHistory),
            _ => Err(crate::ProtocolError::InvalidPayload),
        }
    }

    pub const fn tag(self) -> u8 {
        match self {
            Self::FollowingBottom => 1,
            Self::ViewingHistory => 2,
        }
    }
}

/// The row window origin a `read-viewport-rows` action resolves against the
/// Host's retained rows. Binary `origin` tag: 1 `Tail` then `u32`
/// `rows_before`, 2 `Committed` then `i32` `delta`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewportRowOrigin {
    Tail { rows_before: u32 },
    Committed { delta: i32 },
}

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

/// Viewport-relative terminal cell selected by a mouse gesture. Columns and
/// rows are validated against the Terminal Protocol's `0..=4095` bounds when
/// the private carrier is decoded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewerSelectionViewportPoint {
    pub column: u16,
    pub row: u16,
}

/// Pointer position in the rendered terminal surface. Both axes are limited
/// to `-65535..=65535` at the carrier boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewerSelectionGesturePosition {
    pub x: i32,
    pub y: i32,
}

/// Geometry needed by Host to resolve an update against its committed view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewerSelectionGestureGeometry {
    pub cell_width: u16,
    pub columns: u16,
    pub padding_left: u16,
    pub screen_height: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewerSelectionGestureAction {
    Start {
        click_count: u8,
        point: ViewerSelectionViewportPoint,
        position: ViewerSelectionGesturePosition,
        select_output: bool,
    },
    Update {
        geometry: ViewerSelectionGestureGeometry,
        point: ViewerSelectionViewportPoint,
        position: ViewerSelectionGesturePosition,
        rectangle: bool,
    },
    End {
        point: ViewerSelectionViewportPoint,
    },
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewerSelectionExportFormat {
    Plain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewerSelectionExportCancelReason {
    ClipboardFailed,
    Disposed,
    Replaced,
    User,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalViewerAction {
    Attach {
        host_epoch: HostEpoch,
        terminal_id: HostTerminalId,
    },
    Poll,
    ReadProcessInfo,
    /// Read-only inventory of the exact Host member; never acquires Close ownership.
    ReadLifecycle {
        target: crate::HostTerminalIdentity,
    },
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
        detail: ViewerRowDetail,
        start_row: u64,
        row_count: u16,
        revision: u64,
        generation: NonZeroU64,
        request_sequence: NonZeroU64,
        maximum_bytes: u32,
    },
    SetProjectionDemand {
        demand: ViewerProjectionDemand,
    },
    SetHistoryDemand {
        demand: ViewerHistoryDemand,
    },
    // Distinct from `ReadRows`: the served window's revision is Host-captured
    // viewport identity, never request equality. `maximum_bytes` still bounds
    // the OBRW payload, not the record.
    ReadViewportRows {
        detail: ViewerRowDetail,
        origin: ViewportRowOrigin,
        row_count: u16,
        revision: u64,
        generation: NonZeroU64,
        request_sequence: NonZeroU64,
        maximum_bytes: u32,
    },
    RowsApplied {
        request_sequence: NonZeroU64,
        generation: NonZeroU64,
        revision: u64,
    },
    HistoryObserved {
        sequence: NonZeroU64,
        generation: NonZeroU64,
        revision: u64,
    },
    SelectionGesture {
        generation: NonZeroU64,
        gesture: ViewerSelectionGestureAction,
    },
    SelectionExport {
        export_sequence: NonZeroU64,
        format: ViewerSelectionExportFormat,
    },
    SelectionExportChunkApplied {
        export_sequence: NonZeroU64,
        chunk_sequence: u32,
    },
    SelectionExportCancel {
        export_sequence: NonZeroU64,
        reason: ViewerSelectionExportCancelReason,
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
    LifecycleObserved {
        target: crate::HostTerminalIdentity,
        revision: u64,
        settled: bool,
    },
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
