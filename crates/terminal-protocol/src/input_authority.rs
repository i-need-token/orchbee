//! Reads and writes the shared binary attachment and positive capability-revision prefix used
//! by input payloads. The prefix preserves an authority claim; only the Host can compare it
//! with current attachment capability state.

use std::num::NonZeroU64;

use crate::{AttachmentId, ProtocolError, ProtocolErrorCode};

pub(crate) const INPUT_AUTHORITY_HEADER_BYTES: usize = 24;

#[derive(Clone, Copy)]
pub(crate) struct InputAuthority {
    pub(crate) attachment_id: AttachmentId,
    pub(crate) capability_revision: NonZeroU64,
}

pub(crate) fn decode_input_authority(payload: &[u8]) -> Result<InputAuthority, ProtocolError> {
    let attachment_bytes = read_array::<16>(payload, 0)?;
    let attachment_id = AttachmentId::from_bytes(attachment_bytes).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidIdentifier,
            "input attachment identity is the zero sentinel",
        )
    })?;
    let revision_bytes = read_array::<8>(payload, 16)?;
    let capability_revision = NonZeroU64::new(u64::from_le_bytes(revision_bytes))
        .ok_or_else(|| invalid_field("capability revision must be greater than zero"))?;
    Ok(InputAuthority {
        attachment_id,
        capability_revision,
    })
}

pub(crate) fn append_input_authority(payload: &mut Vec<u8>, authority: InputAuthority) {
    payload.extend_from_slice(authority.attachment_id.as_bytes());
    payload.extend_from_slice(&authority.capability_revision.get().to_le_bytes());
}

fn read_array<const LENGTH: usize>(
    payload: &[u8],
    offset: usize,
) -> Result<[u8; LENGTH], ProtocolError> {
    payload
        .get(offset..offset.saturating_add(LENGTH))
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(truncated)
}

const fn truncated() -> ProtocolError {
    ProtocolError::new(
        ProtocolErrorCode::Truncated,
        "input payload ended before its fixed authority header",
    )
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
