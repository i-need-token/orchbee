use std::num::NonZeroU64;

use crate::{
    AttachmentId, MessageRole, ProtocolError, ProtocolErrorCode,
    limits::MAXIMUM_SELECTION_EXPORT_CHUNK_BYTES,
};

const SELECTION_EXPORT_CHUNK_KIND: u16 = 65;
const FINAL_FLAG: u8 = 1;
pub const SELECTION_EXPORT_CHUNK_HEADER_BYTES: usize = 32;
pub const MAXIMUM_SELECTION_EXPORT_CHUNK_PAYLOAD_BYTES: usize =
    SELECTION_EXPORT_CHUNK_HEADER_BYTES + MAXIMUM_SELECTION_EXPORT_CHUNK_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionExportChunk {
    pub attachment_id: AttachmentId,
    pub bytes: Vec<u8>,
    pub chunk_sequence: u32,
    pub export_sequence: NonZeroU64,
    pub final_chunk: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionExportChunkRoute {
    pub attachment_id: AttachmentId,
    pub body_length: usize,
    pub chunk_sequence: u32,
    pub export_sequence: NonZeroU64,
    pub final_chunk: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedSelectionExportChunk {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_selection_export_chunk(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<SelectionExportChunk, ProtocolError> {
    let route = decode_selection_export_chunk_route(kind, role, payload)?;
    let bytes = payload
        .get(SELECTION_EXPORT_CHUNK_HEADER_BYTES..)
        .ok_or_else(truncated)?
        .to_vec();
    Ok(SelectionExportChunk {
        attachment_id: route.attachment_id,
        bytes,
        chunk_sequence: route.chunk_sequence,
        export_sequence: route.export_sequence,
        final_chunk: route.final_chunk,
    })
}

pub fn decode_selection_export_chunk_route(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<SelectionExportChunkRoute, ProtocolError> {
    if kind != SELECTION_EXPORT_CHUNK_KIND || role != MessageRole::Event {
        return Err(invalid_field(
            "message kind and role do not select a selection-export chunk payload",
        ));
    }
    if payload.len() < SELECTION_EXPORT_CHUNK_HEADER_BYTES {
        return Err(truncated());
    }
    if payload.len() > MAXIMUM_SELECTION_EXPORT_CHUNK_PAYLOAD_BYTES {
        return Err(ProtocolError::new(
            ProtocolErrorCode::LimitExceeded,
            "selection-export chunk payload exceeds its maximum byte length",
        ));
    }

    let attachment_bytes = read_array::<16>(payload, 0)?;
    let export_sequence = u64::from_le_bytes(read_array::<8>(payload, 16)?);
    let chunk_sequence = u32::from_le_bytes(read_array::<4>(payload, 24)?);
    let flags = *payload.get(28).ok_or_else(truncated)?;
    let reserved = payload.get(29..32).ok_or_else(truncated)?;
    if flags & !FINAL_FLAG != 0 || reserved.iter().any(|byte| *byte != 0) {
        return Err(ProtocolError::new(
            ProtocolErrorCode::UnknownRequiredFlag,
            "selection-export chunk contains an unknown flag or nonzero reserved byte",
        ));
    }
    let export_sequence = NonZeroU64::new(export_sequence)
        .ok_or_else(|| invalid_field("selection-export sequence must be positive"))?;
    let attachment_id = AttachmentId::from_bytes(attachment_bytes).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidIdentifier,
            "selection-export chunk attachment identity is invalid",
        )
    })?;
    let body_length = payload
        .len()
        .checked_sub(SELECTION_EXPORT_CHUNK_HEADER_BYTES)
        .ok_or_else(truncated)?;

    Ok(SelectionExportChunkRoute {
        attachment_id,
        body_length,
        chunk_sequence,
        export_sequence,
        final_chunk: flags & FINAL_FLAG != 0,
    })
}

pub fn encode_selection_export_chunk(
    value: &SelectionExportChunk,
) -> Result<EncodedSelectionExportChunk, ProtocolError> {
    if value.bytes.len() > MAXIMUM_SELECTION_EXPORT_CHUNK_BYTES {
        return Err(invalid_field(
            "selection-export chunk body exceeds its maximum byte length",
        ));
    }
    let payload_length = SELECTION_EXPORT_CHUNK_HEADER_BYTES
        .checked_add(value.bytes.len())
        .ok_or_else(|| {
            ProtocolError::new(
                ProtocolErrorCode::ArithmeticOverflow,
                "selection-export chunk payload length overflows the target address space",
            )
        })?;
    if payload_length > MAXIMUM_SELECTION_EXPORT_CHUNK_PAYLOAD_BYTES {
        return Err(ProtocolError::new(
            ProtocolErrorCode::LimitExceeded,
            "selection-export chunk payload exceeds its maximum byte length",
        ));
    }

    let mut payload = Vec::with_capacity(payload_length);
    payload.extend_from_slice(value.attachment_id.as_bytes());
    payload.extend_from_slice(&value.export_sequence.get().to_le_bytes());
    payload.extend_from_slice(&value.chunk_sequence.to_le_bytes());
    payload.push(u8::from(value.final_chunk));
    payload.extend_from_slice(&[0; 3]);
    payload.extend_from_slice(&value.bytes);

    Ok(EncodedSelectionExportChunk {
        kind: SELECTION_EXPORT_CHUNK_KIND,
        payload,
        role: MessageRole::Event,
    })
}

fn read_array<const LENGTH: usize>(
    payload: &[u8],
    offset: usize,
) -> Result<[u8; LENGTH], ProtocolError> {
    let end = offset.checked_add(LENGTH).ok_or_else(|| {
        ProtocolError::new(
            ProtocolErrorCode::ArithmeticOverflow,
            "selection-export chunk field offset overflows the target address space",
        )
    })?;
    payload
        .get(offset..end)
        .ok_or_else(truncated)?
        .try_into()
        .map_err(|_error| truncated())
}

const fn truncated() -> ProtocolError {
    ProtocolError::new(
        ProtocolErrorCode::Truncated,
        "selection-export chunk requires its complete 32-byte header",
    )
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
