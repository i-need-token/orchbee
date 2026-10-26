//! Defines bounded row-window requests and acknowledgement tuples, including explicit detail,
//! purpose, origin, and revision policy. It validates those operands without reading retained
//! history or promoting a pending viewport anchor.

use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

use crate::{
    AttachmentId, MessageRole, ProtocolError, ProtocolErrorCode,
    control::{SafeInteger, SafeSignedInteger, decode_wire, preflight_json, serialize_wire},
    limits::{MAXIMUM_ROW_WINDOW_BYTES, MAXIMUM_ROW_WINDOW_ROWS},
    terminal_fact::{bounded_u16, bounded_u32, canonical_u64},
};

const READ_ROWS_KIND: u16 = 45;
const ROWS_APPLIED_KIND: u16 = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowWindowDetail {
    Render,
    Text,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowWindowPurpose {
    Buffer,
    Search,
    Viewport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowRevisionPolicy {
    CurrentView,
    Exact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowWindowOrigin {
    Absolute { row: u64 },
    Oldest { offset: u32 },
    Committed { delta: i32 },
    Tail { rows_before: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadRows {
    pub attachment_id: AttachmentId,
    pub detail: RowWindowDetail,
    pub generation: NonZeroU64,
    pub maximum_bytes: u32,
    pub origin: RowWindowOrigin,
    pub purpose: RowWindowPurpose,
    pub request_sequence: NonZeroU64,
    pub revision: u64,
    pub revision_policy: RowRevisionPolicy,
    pub row_count: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowsApplied {
    pub attachment_id: AttachmentId,
    pub generation: NonZeroU64,
    pub request_sequence: NonZeroU64,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowAccessControl {
    ReadRows(ReadRows),
    RowsAppliedRequest(RowsApplied),
    RowsAppliedResponse(RowsApplied),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedRowAccessControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_row_access_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<RowAccessControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (READ_ROWS_KIND, MessageRole::Request) => decode_wire::<ReadRowsWire>(payload)
            .and_then(read_rows_from_wire)
            .map(RowAccessControl::ReadRows),
        (ROWS_APPLIED_KIND, MessageRole::Request) => decode_wire::<RowsAppliedWire>(payload)
            .and_then(rows_applied_from_wire)
            .map(RowAccessControl::RowsAppliedRequest),
        (ROWS_APPLIED_KIND, MessageRole::Response) => decode_wire::<RowsAppliedWire>(payload)
            .and_then(rows_applied_from_wire)
            .map(RowAccessControl::RowsAppliedResponse),
        // This codec accepts only its declared message family and request/response roles.
        _ => Err(invalid_field(
            "message kind and role do not select a row-access-control payload",
        )),
    }
}

pub fn encode_row_access_control(
    value: &RowAccessControl,
) -> Result<EncodedRowAccessControl, ProtocolError> {
    let (kind, role, payload) = match value {
        RowAccessControl::ReadRows(read) => (
            READ_ROWS_KIND,
            MessageRole::Request,
            serialize_wire(&read_rows_to_wire(*read)?)?,
        ),
        RowAccessControl::RowsAppliedRequest(applied) => (
            ROWS_APPLIED_KIND,
            MessageRole::Request,
            serialize_wire(&rows_applied_to_wire(*applied))?,
        ),
        RowAccessControl::RowsAppliedResponse(applied) => (
            ROWS_APPLIED_KIND,
            MessageRole::Response,
            serialize_wire(&rows_applied_to_wire(*applied))?,
        ),
    };
    Ok(EncodedRowAccessControl {
        kind,
        payload,
        role,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadRowsWire {
    attachment_id: String,
    detail: RowWindowDetailWire,
    generation: String,
    maximum_bytes: SafeInteger,
    origin: RowWindowOriginWire,
    purpose: RowWindowPurposeWire,
    request_sequence: String,
    revision: String,
    revision_policy: RowRevisionPolicyWire,
    row_count: SafeInteger,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum RowWindowDetailWire {
    Render,
    Text,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum RowWindowPurposeWire {
    Buffer,
    Search,
    Viewport,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RowRevisionPolicyWire {
    CurrentView,
    Exact,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum RowWindowOriginWire {
    Absolute(AbsoluteOriginWire),
    Oldest(OldestOriginWire),
    Committed(CommittedOriginWire),
    Tail(TailOriginWire),
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AbsoluteOriginWire {
    kind: AbsoluteOriginKindWire,
    row: String,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum AbsoluteOriginKindWire {
    #[serde(rename = "absolute")]
    Absolute,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OldestOriginWire {
    kind: OldestOriginKindWire,
    offset: SafeInteger,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum OldestOriginKindWire {
    #[serde(rename = "oldest")]
    Oldest,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CommittedOriginWire {
    delta: SafeSignedInteger,
    kind: CommittedOriginKindWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum CommittedOriginKindWire {
    #[serde(rename = "committed")]
    Committed,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TailOriginWire {
    kind: TailOriginKindWire,
    rows_before: SafeInteger,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum TailOriginKindWire {
    #[serde(rename = "tail")]
    Tail,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RowsAppliedWire {
    attachment_id: String,
    generation: String,
    request_sequence: String,
    revision: String,
}

fn read_rows_from_wire(wire: ReadRowsWire) -> Result<ReadRows, ProtocolError> {
    let value = ReadRows {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        detail: detail_from_wire(wire.detail),
        generation: positive_canonical_u64(&wire.generation)?,
        maximum_bytes: bounded_u32(wire.maximum_bytes, 1, maximum_row_window_bytes_u32()?)?,
        origin: origin_from_wire(wire.origin)?,
        purpose: purpose_from_wire(wire.purpose),
        request_sequence: positive_canonical_u64(&wire.request_sequence)?,
        revision: canonical_u64(&wire.revision)?,
        revision_policy: revision_policy_from_wire(wire.revision_policy),
        row_count: bounded_u16(wire.row_count, 1, MAXIMUM_ROW_WINDOW_ROWS)?,
    };
    validate_read_rows(value)?;
    Ok(value)
}

fn read_rows_to_wire(value: ReadRows) -> Result<ReadRowsWire, ProtocolError> {
    validate_read_rows(value)?;
    Ok(ReadRowsWire {
        attachment_id: value.attachment_id.to_text(),
        detail: detail_to_wire(value.detail),
        generation: value.generation.get().to_string(),
        maximum_bytes: SafeInteger::new(u64::from(value.maximum_bytes)),
        origin: origin_to_wire(value.origin),
        purpose: purpose_to_wire(value.purpose),
        request_sequence: value.request_sequence.get().to_string(),
        revision: value.revision.to_string(),
        revision_policy: revision_policy_to_wire(value.revision_policy),
        row_count: SafeInteger::new(u64::from(value.row_count)),
    })
}

fn rows_applied_from_wire(wire: RowsAppliedWire) -> Result<RowsApplied, ProtocolError> {
    Ok(RowsApplied {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        generation: positive_canonical_u64(&wire.generation)?,
        request_sequence: positive_canonical_u64(&wire.request_sequence)?,
        revision: canonical_u64(&wire.revision)?,
    })
}

fn rows_applied_to_wire(value: RowsApplied) -> RowsAppliedWire {
    RowsAppliedWire {
        attachment_id: value.attachment_id.to_text(),
        generation: value.generation.get().to_string(),
        request_sequence: value.request_sequence.get().to_string(),
        revision: value.revision.to_string(),
    }
}

fn validate_read_rows(value: ReadRows) -> Result<(), ProtocolError> {
    if value.maximum_bytes == 0
        || usize::try_from(value.maximum_bytes).map_or(true, |maximum_bytes| {
            maximum_bytes > MAXIMUM_ROW_WINDOW_BYTES
        })
        || value.row_count == 0
        || usize::from(value.row_count) > MAXIMUM_ROW_WINDOW_ROWS
    {
        return Err(invalid_field("row-read size is outside its field bounds"));
    }
    if let RowWindowOrigin::Committed { delta } = value.origin {
        validate_committed_delta(delta)?;
    }
    if value.revision_policy == RowRevisionPolicy::CurrentView
        && (value.purpose != RowWindowPurpose::Viewport
            || !matches!(
                value.origin,
                RowWindowOrigin::Committed { .. } | RowWindowOrigin::Tail { .. }
            ))
    {
        return Err(invalid_field(
            "current-view row reads require viewport purpose and a relative origin",
        ));
    }
    Ok(())
}

fn origin_from_wire(value: RowWindowOriginWire) -> Result<RowWindowOrigin, ProtocolError> {
    match value {
        RowWindowOriginWire::Absolute(origin) => Ok(RowWindowOrigin::Absolute {
            row: canonical_u64(&origin.row)?,
        }),
        RowWindowOriginWire::Oldest(origin) => Ok(RowWindowOrigin::Oldest {
            offset: bounded_u32(origin.offset, 0, u32::MAX)?,
        }),
        RowWindowOriginWire::Committed(origin) => {
            let delta = i32::try_from(origin.delta.get()).map_err(|_error| {
                invalid_field("committed row delta exceeds signed 32-bit range")
            })?;
            validate_committed_delta(delta)?;
            Ok(RowWindowOrigin::Committed { delta })
        }
        RowWindowOriginWire::Tail(origin) => Ok(RowWindowOrigin::Tail {
            rows_before: bounded_u32(origin.rows_before, 0, u32::MAX)?,
        }),
    }
}

fn origin_to_wire(value: RowWindowOrigin) -> RowWindowOriginWire {
    match value {
        RowWindowOrigin::Absolute { row } => RowWindowOriginWire::Absolute(AbsoluteOriginWire {
            kind: AbsoluteOriginKindWire::Absolute,
            row: row.to_string(),
        }),
        RowWindowOrigin::Oldest { offset } => RowWindowOriginWire::Oldest(OldestOriginWire {
            kind: OldestOriginKindWire::Oldest,
            offset: SafeInteger::new(u64::from(offset)),
        }),
        RowWindowOrigin::Committed { delta } => {
            RowWindowOriginWire::Committed(CommittedOriginWire {
                delta: SafeSignedInteger::new(i64::from(delta)),
                kind: CommittedOriginKindWire::Committed,
            })
        }
        RowWindowOrigin::Tail { rows_before } => RowWindowOriginWire::Tail(TailOriginWire {
            kind: TailOriginKindWire::Tail,
            rows_before: SafeInteger::new(u64::from(rows_before)),
        }),
    }
}

fn validate_committed_delta(delta: i32) -> Result<(), ProtocolError> {
    let maximum = i32::try_from(MAXIMUM_ROW_WINDOW_ROWS)
        .map_err(|_error| invalid_field("row-window row limit exceeds signed 32-bit range"))?;
    if !(-maximum..=maximum).contains(&delta) {
        return Err(invalid_field(
            "committed row delta is outside its field bounds",
        ));
    }
    Ok(())
}

const fn detail_from_wire(value: RowWindowDetailWire) -> RowWindowDetail {
    match value {
        RowWindowDetailWire::Render => RowWindowDetail::Render,
        RowWindowDetailWire::Text => RowWindowDetail::Text,
    }
}

const fn detail_to_wire(value: RowWindowDetail) -> RowWindowDetailWire {
    match value {
        RowWindowDetail::Render => RowWindowDetailWire::Render,
        RowWindowDetail::Text => RowWindowDetailWire::Text,
    }
}

const fn purpose_from_wire(value: RowWindowPurposeWire) -> RowWindowPurpose {
    match value {
        RowWindowPurposeWire::Buffer => RowWindowPurpose::Buffer,
        RowWindowPurposeWire::Search => RowWindowPurpose::Search,
        RowWindowPurposeWire::Viewport => RowWindowPurpose::Viewport,
    }
}

const fn purpose_to_wire(value: RowWindowPurpose) -> RowWindowPurposeWire {
    match value {
        RowWindowPurpose::Buffer => RowWindowPurposeWire::Buffer,
        RowWindowPurpose::Search => RowWindowPurposeWire::Search,
        RowWindowPurpose::Viewport => RowWindowPurposeWire::Viewport,
    }
}

const fn revision_policy_from_wire(value: RowRevisionPolicyWire) -> RowRevisionPolicy {
    match value {
        RowRevisionPolicyWire::CurrentView => RowRevisionPolicy::CurrentView,
        RowRevisionPolicyWire::Exact => RowRevisionPolicy::Exact,
    }
}

const fn revision_policy_to_wire(value: RowRevisionPolicy) -> RowRevisionPolicyWire {
    match value {
        RowRevisionPolicy::CurrentView => RowRevisionPolicyWire::CurrentView,
        RowRevisionPolicy::Exact => RowRevisionPolicyWire::Exact,
    }
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

fn maximum_row_window_bytes_u32() -> Result<u32, ProtocolError> {
    u32::try_from(MAXIMUM_ROW_WINDOW_BYTES)
        .map_err(|_error| invalid_field("row-window byte limit exceeds unsigned 32-bit range"))
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
