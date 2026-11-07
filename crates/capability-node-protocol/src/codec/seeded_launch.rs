//! Frames a launch that carries earlier output in its dedicated `OBNL` record: the launch's
//! ordinary OBCN frame followed by the output bytes. The 40 KiB control bound stays unchanged;
//! only this record admits the larger output, whose grammar the Host checks.

use super::{
    CapabilityNodeMessage, FRAME_HEADER_BYTES, decode_header, decode_payload, encode_message,
};
use crate::{
    MAXIMUM_OUTPUT_SEED_BYTES, ProtocolError, ReadMessageError,
    terminal_launch::AuthorizedTerminalLaunch,
};
use std::io::Read;

const PREFIX: &[u8; 8] = b"OBNL\0\x01\0\0";
const HEADER_BYTES: usize = 12;
const MAXIMUM_BODY_BYTES: usize =
    FRAME_HEADER_BYTES + super::MAXIMUM_FRAME_PAYLOAD_BYTES + MAXIMUM_OUTPUT_SEED_BYTES;

pub(super) fn encode(value: &AuthorizedTerminalLaunch) -> Result<Vec<u8>, ProtocolError> {
    let seed = value.output_seed().ok_or(ProtocolError::InvalidPayload)?;
    let launch = encode_message(&CapabilityNodeMessage::AuthorizedTerminalLaunch(
        value.clone().with_output_seed(None)?,
    ))?;
    let length = launch.len() + seed.len();
    let mut out = Vec::with_capacity(HEADER_BYTES + length);
    out.extend_from_slice(PREFIX);
    out.extend_from_slice(
        &u32::try_from(length)
            .map_err(|_| ProtocolError::InvalidPayload)?
            .to_be_bytes(),
    );
    out.extend_from_slice(&launch);
    out.extend_from_slice(seed);
    Ok(out)
}

fn length(header: &[u8]) -> Result<usize, ProtocolError> {
    if header.len() != HEADER_BYTES || &header[..8] != PREFIX {
        return Err(ProtocolError::InvalidPayload);
    }
    let length = usize::try_from(u32::from_be_bytes(
        header[8..12]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    ))
    .map_err(|_| ProtocolError::InvalidPayload)?;
    if length > MAXIMUM_BODY_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: length,
            maximum: MAXIMUM_BODY_BYTES,
        });
    }
    if length <= FRAME_HEADER_BYTES {
        return Err(ProtocolError::InvalidPayload);
    }
    Ok(length)
}

fn body(bytes: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let header: &[u8; FRAME_HEADER_BYTES] = bytes
        .get(..FRAME_HEADER_BYTES)
        .ok_or(ProtocolError::InvalidPayload)?
        .try_into()
        .map_err(|_| ProtocolError::InvalidPayload)?;
    let end = FRAME_HEADER_BYTES + decode_header(header)?;
    let payload = bytes
        .get(FRAME_HEADER_BYTES..end)
        .ok_or(ProtocolError::InvalidPayload)?;
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = decode_payload(payload)? else {
        return Err(ProtocolError::InvalidPayload);
    };
    let launch = launch.with_output_seed(Some(bytes[end..].to_vec()))?;
    Ok(CapabilityNodeMessage::AuthorizedTerminalLaunch(launch))
}

pub(super) fn decode(bytes: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let size = length(
        bytes
            .get(..HEADER_BYTES)
            .ok_or(ProtocolError::InvalidPayload)?,
    )?;
    if bytes.len() != HEADER_BYTES + size {
        return Err(ProtocolError::InvalidPayload);
    }
    body(&bytes[HEADER_BYTES..])
}

pub(super) fn read(
    reader: &mut impl Read,
    prefix: [u8; FRAME_HEADER_BYTES],
) -> Result<CapabilityNodeMessage, ReadMessageError> {
    let mut header = [0; HEADER_BYTES];
    header[..FRAME_HEADER_BYTES].copy_from_slice(&prefix);
    reader.read_exact(&mut header[FRAME_HEADER_BYTES..])?;
    let size = length(&header)?;
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes)?;
    body(&bytes).map_err(Into::into)
}
