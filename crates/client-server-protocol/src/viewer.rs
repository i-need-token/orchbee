use crate::{HostEpoch, RequestId, SiteId, TerminalId, ViewerId, WorkerId};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

pub const MAXIMUM_VIEWER_RECORD_BYTES: usize = 64 + 36 * 1024 * 1024;

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
    Poll {},
    FrameApplied {
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::decimal")]
        revision: u64,
    },
    NeedSnapshot {
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        reason: ViewerSnapshotReason,
    },
    Detach {},
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
