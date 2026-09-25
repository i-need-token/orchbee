use crate::{HostEpoch, ProtocolError, RequestId, SiteId, TerminalId, ViewerId, WorkerId};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

pub const MAXIMUM_VIEWER_RECORD_BYTES: usize = 64 + 36 * 1024 * 1024;

/// Buffer reads default to text; render retains styles and graphics resources.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewerRowDetail {
    #[default]
    Text,
    Render,
}

impl ViewerRowDetail {
    const fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }
}

/// Projection delivery the Client requests for an attached viewer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectionDemand {
    Active,
    Paused,
}

/// Retained-history delivery the Client requests for an attached viewer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HistoryDemand {
    FollowingBottom,
    ViewingHistory,
}

/// The row window origin a `read-viewport-rows` action resolves against the
/// Host's retained rows: a distance back from the live tail, or a signed
/// delta from the Host-committed viewport anchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ViewportRowOrigin {
    Tail { rows_before: u32 },
    Committed { delta: i32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TerminalViewerAction {
    Attach {
        terminal_create_request_id: RequestId,
    },
    AttachBinding {
        site_id: SiteId,
        worker_id: WorkerId,
        terminal_id: TerminalId,
    },
    Poll {
        attachment_id: ViewerId,
    },
    ReadProcessInfo {
        attachment_id: ViewerId,
    },
    FrameApplied {
        attachment_id: ViewerId,
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::decimal")]
        revision: u64,
    },
    NeedSnapshot {
        attachment_id: ViewerId,
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        reason: ViewerSnapshotReason,
    },
    ReadRows {
        attachment_id: ViewerId,
        #[serde(default, skip_serializing_if = "ViewerRowDetail::is_text")]
        detail: ViewerRowDetail,
        #[serde(with = "crate::number::decimal")]
        start_row: u64,
        row_count: u16,
        #[serde(with = "crate::number::decimal")]
        revision: u64,
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::positive_decimal")]
        request_sequence: NonZeroU64,
        maximum_bytes: u32,
    },
    SetProjectionDemand {
        attachment_id: ViewerId,
        demand: ProjectionDemand,
    },
    SetHistoryDemand {
        attachment_id: ViewerId,
        demand: HistoryDemand,
    },
    // `maximum_bytes` bounds the OBRW payload, not the record. The served
    // window's revision is Host-captured identity, never request equality.
    ReadViewportRows {
        attachment_id: ViewerId,
        origin: ViewportRowOrigin,
        row_count: u16,
        #[serde(with = "crate::number::decimal")]
        revision: u64,
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::positive_decimal")]
        request_sequence: NonZeroU64,
        maximum_bytes: u32,
    },
    RowsApplied {
        attachment_id: ViewerId,
        #[serde(with = "crate::number::positive_decimal")]
        request_sequence: NonZeroU64,
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::decimal")]
        revision: u64,
    },
    HistoryObserved {
        attachment_id: ViewerId,
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::decimal")]
        revision: u64,
    },
    Detach {
        attachment_id: ViewerId,
    },
}

/// The closed reason vocabulary a `rows-rejected` outcome may carry; each
/// variant maps to exactly one Host row-window failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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

/// `read-rows` bounds: the row count and byte budget the action contract
/// admits. `maximum_bytes` bounds the OBRW payload, not the carrier record.
pub const MAXIMUM_READ_ROWS: u16 = 4096;
pub const MINIMUM_READ_ROWS_BYTES: u32 = 112;
pub const MAXIMUM_READ_ROWS_BYTES: u32 = 8 * 1024 * 1024;

impl TerminalViewerAction {
    /// Semantic bounds the JSON shape cannot express; callers reject the
    /// message when this fails, exactly like a malformed field.
    pub(crate) fn validate(&self) -> Result<(), ProtocolError> {
        let bounds = match self {
            Self::ReadRows {
                row_count,
                maximum_bytes,
                ..
            }
            | Self::ReadViewportRows {
                row_count,
                maximum_bytes,
                ..
            } => (*row_count, *maximum_bytes),
            _ => return Ok(()),
        };
        if bounds.0 == 0
            || bounds.0 > MAXIMUM_READ_ROWS
            || !(MINIMUM_READ_ROWS_BYTES..=MAXIMUM_READ_ROWS_BYTES).contains(&bounds.1)
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(())
    }

    #[must_use]
    pub const fn attachment_id(&self) -> Option<&ViewerId> {
        match self {
            Self::Attach { .. } | Self::AttachBinding { .. } => None,
            Self::Poll { attachment_id }
            | Self::ReadProcessInfo { attachment_id }
            | Self::FrameApplied { attachment_id, .. }
            | Self::NeedSnapshot { attachment_id, .. }
            | Self::ReadRows { attachment_id, .. }
            | Self::SetProjectionDemand { attachment_id, .. }
            | Self::SetHistoryDemand { attachment_id, .. }
            | Self::ReadViewportRows { attachment_id, .. }
            | Self::RowsApplied { attachment_id, .. }
            | Self::HistoryObserved { attachment_id, .. }
            | Self::Detach { attachment_id } => Some(attachment_id),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalViewerRequest {
    pub(crate) request_id: RequestId,
    pub(crate) action: TerminalViewerAction,
}
impl TerminalViewerRequest {
    #[must_use]
    pub const fn new(request_id: RequestId, action: TerminalViewerAction) -> Self {
        Self { request_id, action }
    }
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }
    #[must_use]
    pub const fn action(&self) -> &TerminalViewerAction {
        &self.action
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewerRejectedReason {
    AlreadyAttached,
    TerminalUnavailable,
    FeedbackMismatch,
    /// Reserved for the four control-acknowledgement actions' explicit Host
    /// typed failures; distinct from the controller-plane reason of the same
    /// name and never applied to older actions.
    HostRejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewerUnavailableReason {
    ServerBusy,
    TerminalRuntimeUnavailable,
    WorkbenchUnavailable,
}

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TerminalViewerOutcome {
    Attached {
        attachment_id: ViewerId,
        host_epoch: HostEpoch,
    },
    Empty {},
    FeedbackAccepted {},
    Detached {},
    Rejected {
        reason: ViewerRejectedReason,
    },
    RowsRejected {
        reason: ViewerRowsRejectedReason,
    },
    Unavailable {
        reason: ViewerUnavailableReason,
    },
    // Records use OBSR binary framing and have no JSON representation.
    #[serde(skip)]
    Record(Vec<u8>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalViewerResult {
    pub(crate) request_id: RequestId,
    pub(crate) outcome: TerminalViewerOutcome,
}
impl TerminalViewerResult {
    #[must_use]
    pub const fn new(request_id: RequestId, outcome: TerminalViewerOutcome) -> Self {
        Self {
            request_id,
            outcome,
        }
    }
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }
    #[must_use]
    pub const fn outcome(&self) -> &TerminalViewerOutcome {
        &self.outcome
    }
    #[must_use]
    pub fn into_outcome(self) -> TerminalViewerOutcome {
        self.outcome
    }
}
