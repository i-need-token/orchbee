//! Validates frame-applied proofs and snapshot requests with exact attachment, generation,
//! revision, and sequence fields. It carries feedback without committing a renderer frame or
//! deciding whether a proof matches Host state.

use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

use crate::{
    AttachmentId, MessageRole, ProtocolError, ProtocolErrorCode,
    control::{decode_wire, preflight_json, serialize_wire},
    terminal_fact::canonical_u64,
};

const FRAME_APPLIED_KIND: u16 = 43;
const NEED_SNAPSHOT_KIND: u16 = 44;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameApplied {
    pub attachment_id: AttachmentId,
    pub generation: NonZeroU64,
    pub revision: u64,
    pub sequence: NonZeroU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotReason {
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
pub struct NeedSnapshot {
    pub attachment_id: AttachmentId,
    pub reason: SnapshotReason,
    pub sequence: NonZeroU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionFeedbackControl {
    FrameAppliedRequest(FrameApplied),
    FrameAppliedResponse(FrameApplied),
    NeedSnapshotRequest(NeedSnapshot),
    NeedSnapshotResponse(NeedSnapshot),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedProjectionFeedbackControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_projection_feedback_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<ProjectionFeedbackControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (FRAME_APPLIED_KIND, MessageRole::Request) => {
            decode_frame_applied(payload).map(ProjectionFeedbackControl::FrameAppliedRequest)
        }
        (FRAME_APPLIED_KIND, MessageRole::Response) => {
            decode_frame_applied(payload).map(ProjectionFeedbackControl::FrameAppliedResponse)
        }
        (NEED_SNAPSHOT_KIND, MessageRole::Request) => {
            decode_need_snapshot(payload).map(ProjectionFeedbackControl::NeedSnapshotRequest)
        }
        (NEED_SNAPSHOT_KIND, MessageRole::Response) => {
            decode_need_snapshot(payload).map(ProjectionFeedbackControl::NeedSnapshotResponse)
        }
        // This codec accepts only its declared message family and request/response roles.
        _ => Err(invalid_field(
            "message kind and role do not select a projection-feedback payload",
        )),
    }
}

pub fn encode_projection_feedback_control(
    value: &ProjectionFeedbackControl,
) -> Result<EncodedProjectionFeedbackControl, ProtocolError> {
    let (kind, role, payload) = match value {
        ProjectionFeedbackControl::FrameAppliedRequest(applied) => (
            FRAME_APPLIED_KIND,
            MessageRole::Request,
            serialize_wire(&frame_applied_to_wire(*applied))?,
        ),
        ProjectionFeedbackControl::FrameAppliedResponse(applied) => (
            FRAME_APPLIED_KIND,
            MessageRole::Response,
            serialize_wire(&frame_applied_to_wire(*applied))?,
        ),
        ProjectionFeedbackControl::NeedSnapshotRequest(request) => (
            NEED_SNAPSHOT_KIND,
            MessageRole::Request,
            serialize_wire(&need_snapshot_to_wire(*request))?,
        ),
        ProjectionFeedbackControl::NeedSnapshotResponse(request) => (
            NEED_SNAPSHOT_KIND,
            MessageRole::Response,
            serialize_wire(&need_snapshot_to_wire(*request))?,
        ),
    };
    Ok(EncodedProjectionFeedbackControl {
        kind,
        payload,
        role,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrameAppliedWire {
    attachment_id: String,
    generation: String,
    revision: String,
    sequence: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NeedSnapshotWire {
    attachment_id: String,
    reason: SnapshotReasonWire,
    sequence: String,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SnapshotReasonWire {
    ActivityStale,
    BaseMismatch,
    DecoderRejected,
    DimensionsMismatch,
    GenerationMismatch,
    IdentityMismatch,
    SequenceGap,
    StoreReset,
}

fn decode_frame_applied(payload: &[u8]) -> Result<FrameApplied, ProtocolError> {
    let wire = decode_wire::<FrameAppliedWire>(payload)?;
    Ok(FrameApplied {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        generation: positive_canonical_u64(&wire.generation)?,
        revision: canonical_u64(&wire.revision)?,
        sequence: positive_canonical_u64(&wire.sequence)?,
    })
}

fn frame_applied_to_wire(value: FrameApplied) -> FrameAppliedWire {
    FrameAppliedWire {
        attachment_id: value.attachment_id.to_text(),
        generation: value.generation.get().to_string(),
        revision: value.revision.to_string(),
        sequence: value.sequence.get().to_string(),
    }
}

fn decode_need_snapshot(payload: &[u8]) -> Result<NeedSnapshot, ProtocolError> {
    let wire = decode_wire::<NeedSnapshotWire>(payload)?;
    Ok(NeedSnapshot {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        reason: snapshot_reason_from_wire(wire.reason),
        sequence: positive_canonical_u64(&wire.sequence)?,
    })
}

fn need_snapshot_to_wire(value: NeedSnapshot) -> NeedSnapshotWire {
    NeedSnapshotWire {
        attachment_id: value.attachment_id.to_text(),
        reason: snapshot_reason_to_wire(value.reason),
        sequence: value.sequence.get().to_string(),
    }
}

const fn snapshot_reason_from_wire(value: SnapshotReasonWire) -> SnapshotReason {
    match value {
        SnapshotReasonWire::ActivityStale => SnapshotReason::ActivityStale,
        SnapshotReasonWire::BaseMismatch => SnapshotReason::BaseMismatch,
        SnapshotReasonWire::DecoderRejected => SnapshotReason::DecoderRejected,
        SnapshotReasonWire::DimensionsMismatch => SnapshotReason::DimensionsMismatch,
        SnapshotReasonWire::GenerationMismatch => SnapshotReason::GenerationMismatch,
        SnapshotReasonWire::IdentityMismatch => SnapshotReason::IdentityMismatch,
        SnapshotReasonWire::SequenceGap => SnapshotReason::SequenceGap,
        SnapshotReasonWire::StoreReset => SnapshotReason::StoreReset,
    }
}

const fn snapshot_reason_to_wire(value: SnapshotReason) -> SnapshotReasonWire {
    match value {
        SnapshotReason::ActivityStale => SnapshotReasonWire::ActivityStale,
        SnapshotReason::BaseMismatch => SnapshotReasonWire::BaseMismatch,
        SnapshotReason::DecoderRejected => SnapshotReasonWire::DecoderRejected,
        SnapshotReason::DimensionsMismatch => SnapshotReasonWire::DimensionsMismatch,
        SnapshotReason::GenerationMismatch => SnapshotReasonWire::GenerationMismatch,
        SnapshotReason::IdentityMismatch => SnapshotReasonWire::IdentityMismatch,
        SnapshotReason::SequenceGap => SnapshotReasonWire::SequenceGap,
        SnapshotReason::StoreReset => SnapshotReasonWire::StoreReset,
    }
}

fn positive_canonical_u64(value: &str) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(canonical_u64(value)?).ok_or_else(|| {
        invalid_field("projection sequence and generation must be greater than zero")
    })
}

fn parse_attachment_id(value: &str) -> Result<AttachmentId, ProtocolError> {
    AttachmentId::parse(value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidIdentifier,
            "projection-feedback attachment identity is invalid",
        )
    })
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
