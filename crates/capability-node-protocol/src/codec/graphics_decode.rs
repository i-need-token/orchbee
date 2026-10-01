//! Large PNG request carrier; the response reuses the correlated viewer record carrier.
use crate::{
    CapabilityNodeMessage, MAXIMUM_GRAPHICS_PNG_BYTES, OperationId, ProtocolError,
    ReadMessageError, TerminalGraphicsDecodeRequest, WorkerGeneration, WorkerId,
};
use std::{io::Read, num::NonZeroU64};
const PREFIX: &[u8; 8] = b"OBNG\0\x01\0\0";
const BODY_PREFIX_BYTES: usize = 64;

pub(super) fn encode(value: &TerminalGraphicsDecodeRequest) -> Result<Vec<u8>, ProtocolError> {
    value.validate()?;
    let length = BODY_PREFIX_BYTES + value.png.len();
    let mut out = Vec::with_capacity(12 + length);
    out.extend_from_slice(PREFIX);
    out.extend_from_slice(
        &u32::try_from(length)
            .map_err(|_| ProtocolError::InvalidPayload)?
            .to_be_bytes(),
    );
    out.extend_from_slice(&value.request_id.as_bytes());
    out.extend_from_slice(&value.worker_id.as_bytes());
    out.extend_from_slice(&value.worker_generation.get().to_be_bytes());
    out.extend_from_slice(&value.attachment_id.as_bytes());
    out.extend_from_slice(&value.width.to_be_bytes());
    out.extend_from_slice(&value.height.to_be_bytes());
    out.extend_from_slice(&value.png);
    Ok(out)
}
fn length(header: &[u8]) -> Result<usize, ProtocolError> {
    if header.len() != 12 || &header[..8] != PREFIX {
        return Err(ProtocolError::InvalidPayload);
    }
    let length = usize::try_from(u32::from_be_bytes(
        header[8..12]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    ))
    .map_err(|_| ProtocolError::InvalidPayload)?;
    if length < BODY_PREFIX_BYTES + 33 {
        return Err(ProtocolError::InvalidPayload);
    }
    if length > BODY_PREFIX_BYTES + MAXIMUM_GRAPHICS_PNG_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: length,
            maximum: BODY_PREFIX_BYTES + MAXIMUM_GRAPHICS_PNG_BYTES,
        });
    }
    Ok(length)
}
fn payload(body: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let invalid = || ProtocolError::InvalidPayload;
    let value = TerminalGraphicsDecodeRequest {
        request_id: OperationId::from_bytes(body[0..16].try_into().map_err(|_| invalid())?)?,
        worker_id: WorkerId::from_bytes(body[16..32].try_into().map_err(|_| invalid())?)?,
        worker_generation: WorkerGeneration::new(
            NonZeroU64::new(u64::from_be_bytes(
                body[32..40].try_into().map_err(|_| invalid())?,
            ))
            .ok_or_else(invalid)?,
        ),
        attachment_id: OperationId::from_bytes(body[40..56].try_into().map_err(|_| invalid())?)?,
        width: u32::from_be_bytes(body[56..60].try_into().map_err(|_| invalid())?),
        height: u32::from_be_bytes(body[60..64].try_into().map_err(|_| invalid())?),
        png: body[BODY_PREFIX_BYTES..].to_vec(),
    };
    value.validate()?;
    Ok(CapabilityNodeMessage::TerminalGraphicsDecodeRequest(value))
}
pub(super) fn decode(bytes: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let size = length(bytes.get(..12).ok_or(ProtocolError::InvalidPayload)?)?;
    if bytes.len() != 12 + size {
        return Err(ProtocolError::InvalidPayload);
    }
    payload(&bytes[12..])
}
pub(super) fn read(
    reader: &mut impl Read,
    prefix: [u8; 8],
) -> Result<CapabilityNodeMessage, ReadMessageError> {
    let mut header = [0; 12];
    header[..8].copy_from_slice(&prefix);
    reader.read_exact(&mut header[8..])?;
    let size = length(&header)?;
    let mut body = vec![0; size];
    reader.read_exact(&mut body)?;
    payload(&body).map_err(Into::into)
}
