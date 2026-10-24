use std::num::NonZeroU64;

use crate::{
    AttachmentId, HostEpoch, MAXIMUM_TERMINAL_FRAME_BYTES, ProtocolError, ProtocolErrorCode,
};

pub const TERMINAL_FRAME_HEADER_BYTES: usize = 160;

/// Routing facts only. Success does not validate cells, presentation fields, or payload tables.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalFrameRoute {
    pub attachment_id: AttachmentId,
    pub host_epoch: HostEpoch,
    pub sequence: NonZeroU64,
    pub generation: NonZeroU64,
    pub revision: u64,
    pub base_revision: Option<u64>,
}

/// Inspect the OBTF 0.1.0 routing prefix without decoding or copying the terminal projection.
pub fn inspect_terminal_frame_route(bytes: &[u8]) -> Result<TerminalFrameRoute, ProtocolError> {
    if bytes.len() < TERMINAL_FRAME_HEADER_BYTES {
        return Err(error(ProtocolErrorCode::Truncated));
    }
    if bytes.len() > MAXIMUM_TERMINAL_FRAME_BYTES {
        return Err(error(ProtocolErrorCode::LimitExceeded));
    }
    if &bytes[..4] != b"OBTF" {
        return Err(error(ProtocolErrorCode::InvalidMagic));
    }
    if bytes[4] != 0
        || bytes[5] != 1
        || bytes[144] != 0
        || u16::from_le_bytes(array(bytes, 12)?) != 160
    {
        return Err(error(ProtocolErrorCode::UnsupportedVersion));
    }
    if usize::try_from(u32::from_le_bytes(array(bytes, 8)?)).ok() != Some(bytes.len()) {
        return Err(error(ProtocolErrorCode::LengthMismatch));
    }
    if bytes[7] & 0x80 != 0 || bytes[14..16] != [0, 0] {
        return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
    }
    let revision = u64::from_le_bytes(array(bytes, 64)?);
    let encoded_base = u64::from_le_bytes(array(bytes, 56)?);
    let base_revision = (encoded_base != u64::MAX).then_some(encoded_base);
    match (bytes[6], base_revision) {
        (1, None) => {}
        (2, Some(base)) if base < revision => {}
        _ => return Err(error(ProtocolErrorCode::InvalidField)),
    }
    Ok(TerminalFrameRoute {
        attachment_id: AttachmentId::from_bytes(array(bytes, 32)?)
            .map_err(|_| error(ProtocolErrorCode::InvalidIdentifier))?,
        host_epoch: HostEpoch::from_bytes(array(bytes, 16)?)
            .map_err(|_| error(ProtocolErrorCode::InvalidIdentifier))?,
        sequence: NonZeroU64::new(u64::from_le_bytes(array(bytes, 48)?))
            .ok_or_else(|| error(ProtocolErrorCode::InvalidField))?,
        generation: NonZeroU64::new(u64::from_le_bytes(array(bytes, 72)?))
            .ok_or_else(|| error(ProtocolErrorCode::InvalidField))?,
        revision,
        base_revision,
    })
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], ProtocolError> {
    bytes
        .get(offset..offset + N)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| error(ProtocolErrorCode::Truncated))
}

const fn error(code: ProtocolErrorCode) -> ProtocolError {
    ProtocolError::new(code, "invalid terminal frame routing prefix")
}
