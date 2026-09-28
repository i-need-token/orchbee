use std::io::{Read, Write};

use crate::{
    ClientMessage, ProtocolError, ReadMessageError, ServerMessage, WriteMessageError,
    wire::{
        decode_client_payload, decode_server_payload, encode_client_payload, encode_server_payload,
    },
};

pub const FRAME_MAGIC: [u8; 4] = *b"OBCS";
pub const FRAME_VERSION: u16 = 1;
pub const FRAME_HEADER_BYTES: usize = 8;
pub const MAXIMUM_FRAME_PAYLOAD_BYTES: usize = 63 * 1024;

pub fn encode_client_frame(message: &ClientMessage) -> Result<Vec<u8>, ProtocolError> {
    if let ClientMessage::TerminalControl(request) = message
        && let crate::TerminalControlAction::Paste { .. } = request.action()
    {
        return crate::paste_record::encode(request);
    }
    if let ClientMessage::TerminalSend(request) = message {
        return crate::send_record::encode(request);
    }
    encode_frame(&encode_client_payload(message)?)
}

pub fn decode_client_frame(frame: &[u8]) -> Result<ClientMessage, ProtocolError> {
    if frame.starts_with(b"OBCQ") {
        return crate::paste_record::decode(frame);
    }
    if frame.starts_with(b"OBSN") {
        return crate::send_record::decode(frame);
    }
    decode_frame(frame).and_then(decode_client_payload)
}

pub fn encode_server_frame(message: &ServerMessage) -> Result<Vec<u8>, ProtocolError> {
    if let ServerMessage::TerminalViewerResult(result) = message
        && let crate::TerminalViewerOutcome::Record(record) = result.outcome()
    {
        return crate::viewer_record::encode(result, record);
    }
    encode_frame(&encode_server_payload(message)?)
}

pub fn decode_server_frame(frame: &[u8]) -> Result<ServerMessage, ProtocolError> {
    if frame.starts_with(b"OBSR") {
        return crate::viewer_record::decode(frame);
    }
    decode_frame(frame).and_then(decode_server_payload)
}

pub fn read_client_message(reader: &mut impl Read) -> Result<ClientMessage, ReadMessageError> {
    let mut header = [0; FRAME_HEADER_BYTES];
    reader.read_exact(&mut header)?;
    if header.starts_with(b"OBCQ") {
        return crate::paste_record::read(reader, header);
    }
    if header.starts_with(b"OBSN") {
        return crate::send_record::read(reader, header);
    }
    let payload = read_payload_after_header(reader, header)?;
    decode_client_payload(&payload).map_err(Into::into)
}

pub fn read_server_message(reader: &mut impl Read) -> Result<ServerMessage, ReadMessageError> {
    let mut header = [0; FRAME_HEADER_BYTES];
    reader.read_exact(&mut header)?;
    if header.starts_with(b"OBSR") {
        return crate::viewer_record::read(reader, header);
    }
    let payload = read_payload_after_header(reader, header)?;
    decode_server_payload(&payload).map_err(Into::into)
}

pub fn write_client_message(
    writer: &mut impl Write,
    message: &ClientMessage,
) -> Result<(), WriteMessageError> {
    let frame = encode_client_frame(message)?;
    writer.write_all(&frame).map_err(Into::into)
}

pub fn write_server_message(
    writer: &mut impl Write,
    message: &ServerMessage,
) -> Result<(), WriteMessageError> {
    let frame = encode_server_frame(message)?;
    writer.write_all(&frame).map_err(Into::into)
}

fn encode_frame(payload: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    enforce_payload_limit(payload.len())?;
    let payload_length =
        u16::try_from(payload.len()).map_err(|_error| ProtocolError::PayloadTooLarge {
            actual: payload.len(),
            maximum: MAXIMUM_FRAME_PAYLOAD_BYTES,
        })?;
    let mut frame = Vec::with_capacity(FRAME_HEADER_BYTES + payload.len());
    frame.extend_from_slice(&FRAME_MAGIC);
    frame.extend_from_slice(&FRAME_VERSION.to_be_bytes());
    frame.extend_from_slice(&payload_length.to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

fn decode_frame(frame: &[u8]) -> Result<&[u8], ProtocolError> {
    let header: [u8; FRAME_HEADER_BYTES] = frame
        .get(..FRAME_HEADER_BYTES)
        .ok_or(ProtocolError::FrameLengthMismatch {
            expected: FRAME_HEADER_BYTES,
            actual: frame.len(),
        })?
        .try_into()
        .map_err(|_error| ProtocolError::FrameLengthMismatch {
            expected: FRAME_HEADER_BYTES,
            actual: frame.len(),
        })?;
    let payload_length = validate_header(&header)?;
    let expected_length = FRAME_HEADER_BYTES + payload_length;
    if frame.len() != expected_length {
        return Err(ProtocolError::FrameLengthMismatch {
            expected: expected_length,
            actual: frame.len(),
        });
    }
    frame
        .get(FRAME_HEADER_BYTES..)
        .ok_or(ProtocolError::FrameLengthMismatch {
            expected: expected_length,
            actual: frame.len(),
        })
}

fn read_payload_after_header(
    reader: &mut impl Read,
    header: [u8; FRAME_HEADER_BYTES],
) -> Result<Vec<u8>, ReadMessageError> {
    let payload_length = validate_header(&header)?;
    let mut payload = vec![0_u8; payload_length];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

fn validate_header(header: &[u8; FRAME_HEADER_BYTES]) -> Result<usize, ProtocolError> {
    if header.get(..FRAME_MAGIC.len()) != Some(FRAME_MAGIC.as_slice()) {
        return Err(ProtocolError::InvalidMagic);
    }
    let frame_version = u16::from_be_bytes([header[4], header[5]]);
    if frame_version != FRAME_VERSION {
        return Err(ProtocolError::IncompatibleFrameVersion {
            received: frame_version,
        });
    }
    let payload_length = usize::from(u16::from_be_bytes([header[6], header[7]]));
    enforce_payload_limit(payload_length)?;
    Ok(payload_length)
}

fn enforce_payload_limit(payload_length: usize) -> Result<(), ProtocolError> {
    if payload_length > MAXIMUM_FRAME_PAYLOAD_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: payload_length,
            maximum: MAXIMUM_FRAME_PAYLOAD_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
