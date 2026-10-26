use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

use crate::{
    AttachmentId, MessageRole, ProtocolError, ProtocolErrorCode,
    control::{SafeInteger, decode_wire, preflight_json, serialize_wire},
    limits::MAXIMUM_SELECTION_EXPORT_BYTES,
    terminal_fact::canonical_u64,
};

const SELECTION_EXPORT_KIND: u16 = 63;
const SELECTION_EXPORT_STARTED_KIND: u16 = 64;
const SELECTION_EXPORT_CHUNK_APPLIED_KIND: u16 = 66;
const SELECTION_EXPORT_FINISHED_KIND: u16 = 67;
const SELECTION_EXPORT_CANCEL_KIND: u16 = 68;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionExportFormat {
    Plain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionExport {
    pub attachment_id: AttachmentId,
    pub export_sequence: NonZeroU64,
    pub format: SelectionExportFormat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionExportStarted {
    pub attachment_id: AttachmentId,
    pub byte_length: u32,
    pub export_sequence: NonZeroU64,
    pub format: SelectionExportFormat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionExportChunkApplied {
    pub attachment_id: AttachmentId,
    pub chunk_sequence: u32,
    pub export_sequence: NonZeroU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionExportStatus {
    Cancelled,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionExportFinished {
    pub attachment_id: AttachmentId,
    pub byte_length: u32,
    pub export_sequence: NonZeroU64,
    pub status: SelectionExportStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionExportCancelReason {
    ClipboardFailed,
    Disposed,
    Replaced,
    User,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionExportCancel {
    pub attachment_id: AttachmentId,
    pub export_sequence: NonZeroU64,
    pub reason: SelectionExportCancelReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionExportControl {
    ExportRequest(SelectionExport),
    StartedResponse(SelectionExportStarted),
    ChunkAppliedRequest(SelectionExportChunkApplied),
    ChunkAppliedResponse(SelectionExportChunkApplied),
    FinishedResponse(SelectionExportFinished),
    FinishedEvent(SelectionExportFinished),
    CancelRequest(SelectionExportCancel),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedSelectionExportControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_selection_export_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<SelectionExportControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (SELECTION_EXPORT_KIND, MessageRole::Request) => {
            decode_wire::<SelectionExportWire>(payload)
                .and_then(selection_export_from_wire)
                .map(SelectionExportControl::ExportRequest)
        }
        (SELECTION_EXPORT_STARTED_KIND, MessageRole::Response) => {
            decode_wire::<SelectionExportStartedWire>(payload)
                .and_then(selection_export_started_from_wire)
                .map(SelectionExportControl::StartedResponse)
        }
        (SELECTION_EXPORT_CHUNK_APPLIED_KIND, MessageRole::Request) => {
            decode_wire::<SelectionExportChunkAppliedWire>(payload)
                .and_then(selection_export_chunk_applied_from_wire)
                .map(SelectionExportControl::ChunkAppliedRequest)
        }
        (SELECTION_EXPORT_CHUNK_APPLIED_KIND, MessageRole::Response) => {
            decode_wire::<SelectionExportChunkAppliedWire>(payload)
                .and_then(selection_export_chunk_applied_from_wire)
                .map(SelectionExportControl::ChunkAppliedResponse)
        }
        (SELECTION_EXPORT_FINISHED_KIND, MessageRole::Response) => {
            decode_wire::<SelectionExportFinishedWire>(payload)
                .and_then(selection_export_finished_from_wire)
                .map(SelectionExportControl::FinishedResponse)
        }
        (SELECTION_EXPORT_FINISHED_KIND, MessageRole::Event) => {
            decode_wire::<SelectionExportFinishedWire>(payload)
                .and_then(selection_export_finished_from_wire)
                .map(SelectionExportControl::FinishedEvent)
        }
        (SELECTION_EXPORT_CANCEL_KIND, MessageRole::Request) => {
            decode_wire::<SelectionExportCancelWire>(payload)
                .and_then(selection_export_cancel_from_wire)
                .map(SelectionExportControl::CancelRequest)
        }
        // This codec accepts only its declared message family and request/response roles.
        _ => Err(invalid_field(
            "message kind and role do not select a selection-export control payload",
        )),
    }
}

pub fn encode_selection_export_control(
    value: &SelectionExportControl,
) -> Result<EncodedSelectionExportControl, ProtocolError> {
    let (kind, role, payload) = match value {
        SelectionExportControl::ExportRequest(value) => (
            SELECTION_EXPORT_KIND,
            MessageRole::Request,
            serialize_wire(&selection_export_to_wire(*value))?,
        ),
        SelectionExportControl::StartedResponse(value) => (
            SELECTION_EXPORT_STARTED_KIND,
            MessageRole::Response,
            serialize_wire(&selection_export_started_to_wire(*value)?)?,
        ),
        SelectionExportControl::ChunkAppliedRequest(value) => (
            SELECTION_EXPORT_CHUNK_APPLIED_KIND,
            MessageRole::Request,
            serialize_wire(&selection_export_chunk_applied_to_wire(*value))?,
        ),
        SelectionExportControl::ChunkAppliedResponse(value) => (
            SELECTION_EXPORT_CHUNK_APPLIED_KIND,
            MessageRole::Response,
            serialize_wire(&selection_export_chunk_applied_to_wire(*value))?,
        ),
        SelectionExportControl::FinishedResponse(value) => (
            SELECTION_EXPORT_FINISHED_KIND,
            MessageRole::Response,
            serialize_wire(&selection_export_finished_to_wire(*value)?)?,
        ),
        SelectionExportControl::FinishedEvent(value) => (
            SELECTION_EXPORT_FINISHED_KIND,
            MessageRole::Event,
            serialize_wire(&selection_export_finished_to_wire(*value)?)?,
        ),
        SelectionExportControl::CancelRequest(value) => (
            SELECTION_EXPORT_CANCEL_KIND,
            MessageRole::Request,
            serialize_wire(&selection_export_cancel_to_wire(*value))?,
        ),
    };
    Ok(EncodedSelectionExportControl {
        kind,
        payload,
        role,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionExportWire {
    attachment_id: String,
    export_sequence: String,
    format: SelectionExportFormatWire,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionExportStartedWire {
    attachment_id: String,
    byte_length: SafeInteger,
    export_sequence: String,
    format: SelectionExportFormatWire,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionExportChunkAppliedWire {
    attachment_id: String,
    chunk_sequence: SafeInteger,
    export_sequence: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionExportFinishedWire {
    attachment_id: String,
    byte_length: SafeInteger,
    export_sequence: String,
    status: SelectionExportStatusWire,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionExportCancelWire {
    attachment_id: String,
    export_sequence: String,
    reason: SelectionExportCancelReasonWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionExportFormatWire {
    #[serde(rename = "plain")]
    Plain,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionExportStatusWire {
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "completed")]
    Completed,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionExportCancelReasonWire {
    #[serde(rename = "clipboard-failed")]
    ClipboardFailed,
    #[serde(rename = "disposed")]
    Disposed,
    #[serde(rename = "replaced")]
    Replaced,
    #[serde(rename = "user")]
    User,
}

fn selection_export_from_wire(wire: SelectionExportWire) -> Result<SelectionExport, ProtocolError> {
    Ok(SelectionExport {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        export_sequence: positive_canonical_u64(&wire.export_sequence)?,
        format: selection_export_format_from_wire(wire.format),
    })
}

fn selection_export_to_wire(value: SelectionExport) -> SelectionExportWire {
    SelectionExportWire {
        attachment_id: value.attachment_id.to_text(),
        export_sequence: value.export_sequence.get().to_string(),
        format: selection_export_format_to_wire(value.format),
    }
}

fn selection_export_started_from_wire(
    wire: SelectionExportStartedWire,
) -> Result<SelectionExportStarted, ProtocolError> {
    Ok(SelectionExportStarted {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        byte_length: export_byte_length_from_wire(wire.byte_length)?,
        export_sequence: positive_canonical_u64(&wire.export_sequence)?,
        format: selection_export_format_from_wire(wire.format),
    })
}

fn selection_export_started_to_wire(
    value: SelectionExportStarted,
) -> Result<SelectionExportStartedWire, ProtocolError> {
    validate_export_byte_length(value.byte_length)?;
    Ok(SelectionExportStartedWire {
        attachment_id: value.attachment_id.to_text(),
        byte_length: SafeInteger::new(u64::from(value.byte_length)),
        export_sequence: value.export_sequence.get().to_string(),
        format: selection_export_format_to_wire(value.format),
    })
}

fn selection_export_chunk_applied_from_wire(
    wire: SelectionExportChunkAppliedWire,
) -> Result<SelectionExportChunkApplied, ProtocolError> {
    Ok(SelectionExportChunkApplied {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        chunk_sequence: u32_from_wire(wire.chunk_sequence, "chunk sequence exceeds u32")?,
        export_sequence: positive_canonical_u64(&wire.export_sequence)?,
    })
}

fn selection_export_chunk_applied_to_wire(
    value: SelectionExportChunkApplied,
) -> SelectionExportChunkAppliedWire {
    SelectionExportChunkAppliedWire {
        attachment_id: value.attachment_id.to_text(),
        chunk_sequence: SafeInteger::new(u64::from(value.chunk_sequence)),
        export_sequence: value.export_sequence.get().to_string(),
    }
}

fn selection_export_finished_from_wire(
    wire: SelectionExportFinishedWire,
) -> Result<SelectionExportFinished, ProtocolError> {
    Ok(SelectionExportFinished {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        byte_length: export_byte_length_from_wire(wire.byte_length)?,
        export_sequence: positive_canonical_u64(&wire.export_sequence)?,
        status: selection_export_status_from_wire(wire.status),
    })
}

fn selection_export_finished_to_wire(
    value: SelectionExportFinished,
) -> Result<SelectionExportFinishedWire, ProtocolError> {
    validate_export_byte_length(value.byte_length)?;
    Ok(SelectionExportFinishedWire {
        attachment_id: value.attachment_id.to_text(),
        byte_length: SafeInteger::new(u64::from(value.byte_length)),
        export_sequence: value.export_sequence.get().to_string(),
        status: selection_export_status_to_wire(value.status),
    })
}

fn selection_export_cancel_from_wire(
    wire: SelectionExportCancelWire,
) -> Result<SelectionExportCancel, ProtocolError> {
    Ok(SelectionExportCancel {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        export_sequence: positive_canonical_u64(&wire.export_sequence)?,
        reason: selection_export_cancel_reason_from_wire(wire.reason),
    })
}

fn selection_export_cancel_to_wire(value: SelectionExportCancel) -> SelectionExportCancelWire {
    SelectionExportCancelWire {
        attachment_id: value.attachment_id.to_text(),
        export_sequence: value.export_sequence.get().to_string(),
        reason: selection_export_cancel_reason_to_wire(value.reason),
    }
}

const fn selection_export_format_from_wire(
    value: SelectionExportFormatWire,
) -> SelectionExportFormat {
    match value {
        SelectionExportFormatWire::Plain => SelectionExportFormat::Plain,
    }
}

const fn selection_export_format_to_wire(
    value: SelectionExportFormat,
) -> SelectionExportFormatWire {
    match value {
        SelectionExportFormat::Plain => SelectionExportFormatWire::Plain,
    }
}

const fn selection_export_status_from_wire(
    value: SelectionExportStatusWire,
) -> SelectionExportStatus {
    match value {
        SelectionExportStatusWire::Cancelled => SelectionExportStatus::Cancelled,
        SelectionExportStatusWire::Completed => SelectionExportStatus::Completed,
    }
}

const fn selection_export_status_to_wire(
    value: SelectionExportStatus,
) -> SelectionExportStatusWire {
    match value {
        SelectionExportStatus::Cancelled => SelectionExportStatusWire::Cancelled,
        SelectionExportStatus::Completed => SelectionExportStatusWire::Completed,
    }
}

const fn selection_export_cancel_reason_from_wire(
    value: SelectionExportCancelReasonWire,
) -> SelectionExportCancelReason {
    match value {
        SelectionExportCancelReasonWire::ClipboardFailed => {
            SelectionExportCancelReason::ClipboardFailed
        }
        SelectionExportCancelReasonWire::Disposed => SelectionExportCancelReason::Disposed,
        SelectionExportCancelReasonWire::Replaced => SelectionExportCancelReason::Replaced,
        SelectionExportCancelReasonWire::User => SelectionExportCancelReason::User,
    }
}

const fn selection_export_cancel_reason_to_wire(
    value: SelectionExportCancelReason,
) -> SelectionExportCancelReasonWire {
    match value {
        SelectionExportCancelReason::ClipboardFailed => {
            SelectionExportCancelReasonWire::ClipboardFailed
        }
        SelectionExportCancelReason::Disposed => SelectionExportCancelReasonWire::Disposed,
        SelectionExportCancelReason::Replaced => SelectionExportCancelReasonWire::Replaced,
        SelectionExportCancelReason::User => SelectionExportCancelReasonWire::User,
    }
}

fn export_byte_length_from_wire(value: SafeInteger) -> Result<u32, ProtocolError> {
    let value = u32_from_wire(value, "selection export byte length exceeds u32")?;
    validate_export_byte_length(value)?;
    Ok(value)
}

fn u32_from_wire(value: SafeInteger, detail: &'static str) -> Result<u32, ProtocolError> {
    u32::try_from(value.get()).map_err(|_error| invalid_field(detail))
}

fn validate_export_byte_length(value: u32) -> Result<(), ProtocolError> {
    if usize::try_from(value).map_or(true, |value| value > MAXIMUM_SELECTION_EXPORT_BYTES) {
        return Err(invalid_field(
            "selection export byte length exceeds its protocol bound",
        ));
    }
    Ok(())
}

fn parse_attachment_id(value: &str) -> Result<AttachmentId, ProtocolError> {
    AttachmentId::parse(value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidIdentifier,
            "attachment identity is not canonical nonzero 128-bit hexadecimal text",
        )
    })
}

fn positive_canonical_u64(value: &str) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(canonical_u64(value)?)
        .ok_or_else(|| invalid_field("unsigned 64-bit text must be greater than zero"))
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
