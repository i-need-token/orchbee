use crate::{
    MAXIMUM_VIEWER_RECORD_BYTES, ProtocolError, ReadMessageError, RequestId, ServerMessage,
    TerminalViewerOutcome, TerminalViewerResult,
};
use std::io::Read;

const HEADER: usize = 12;
const PREFIX: [u8; 8] = *b"OBSR\0\x01\0\0";

pub(crate) fn encode(
    result: &TerminalViewerResult,
    record: &[u8],
) -> Result<Vec<u8>, ProtocolError> {
    let length = record
        .len()
        .checked_add(16)
        .ok_or(ProtocolError::InvalidPayload)?;
    validate_length(length)?;
    let id = u128::from_str_radix(result.request_id().as_str(), 16)
        .map_err(|_| ProtocolError::InvalidPayload)?;
    let mut bytes = Vec::with_capacity(HEADER + length);
    bytes.extend_from_slice(&PREFIX);
    bytes.extend_from_slice(
        &u32::try_from(length)
            .map_err(|_| ProtocolError::InvalidPayload)?
            .to_be_bytes(),
    );
    bytes.extend_from_slice(&id.to_be_bytes());
    bytes.extend_from_slice(record);
    Ok(bytes)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<ServerMessage, ProtocolError> {
    let header = bytes.get(..HEADER).ok_or(ProtocolError::InvalidPayload)?;
    let length = payload_length(header)?;
    if bytes.len() != HEADER + length {
        return Err(ProtocolError::InvalidPayload);
    }
    decode_payload(bytes[HEADER..].to_vec())
}

pub(crate) fn read(
    reader: &mut impl Read,
    prefix: [u8; 8],
) -> Result<ServerMessage, ReadMessageError> {
    let mut header = [0; HEADER];
    header[..8].copy_from_slice(&prefix);
    reader.read_exact(&mut header[8..])?;
    let length = payload_length(&header)?;
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    decode_payload(bytes).map_err(Into::into)
}

fn payload_length(header: &[u8]) -> Result<usize, ProtocolError> {
    if header[..4] != PREFIX[..4] {
        return Err(ProtocolError::InvalidMagic);
    }
    let version = u16::from_be_bytes([header[4], header[5]]);
    if version != 1 {
        return Err(ProtocolError::IncompatibleFrameVersion { received: version });
    }
    if header[6..8] != [0, 0] {
        return Err(ProtocolError::InvalidPayload);
    }
    let length = usize::try_from(u32::from_be_bytes(
        header[8..12]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    ))
    .map_err(|_| ProtocolError::InvalidPayload)?;
    validate_length(length)?;
    Ok(length)
}
fn validate_length(length: usize) -> Result<(), ProtocolError> {
    if length < 16 + 64 {
        return Err(ProtocolError::InvalidPayload);
    }
    if length > 16 + MAXIMUM_VIEWER_RECORD_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: length,
            maximum: 16 + MAXIMUM_VIEWER_RECORD_BYTES,
        });
    }
    Ok(())
}
fn decode_payload(bytes: Vec<u8>) -> Result<ServerMessage, ProtocolError> {
    let id = u128::from_be_bytes(
        bytes[..16]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    );
    let request_id =
        RequestId::parse(&format!("{id:032x}")).map_err(|_| ProtocolError::InvalidPayload)?;
    Ok(ServerMessage::TerminalViewerResult(
        TerminalViewerResult::new(
            request_id,
            TerminalViewerOutcome::Record(bytes[16..].to_vec()),
        ),
    ))
}
