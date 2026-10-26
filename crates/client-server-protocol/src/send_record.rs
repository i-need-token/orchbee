//! Carries one send sequence in the bounded OBSN binary layout, validating record tags,
//! identities, lengths, and aggregate content before construction. It preserves opaque input
//! bytes and semantic strokes without encoding terminal modes or executing the sequence.

use crate::record_identities::{identity, identity_bytes};
use crate::{
    ClientMessage, ProtocolError, ReadMessageError, RequestId, SendBytes, SendPasteContent,
    SendRecord, SendRecords, SendText, SendTextInput, SessionId, TerminalId, TerminalSendRequest,
    WorkerId,
};
use std::io::Read;

/// The send request never rides the standard JSON frame: a `bytes` record
/// alone may carry a megabyte of opaque payload, so the whole request is one
/// dedicated binary record — `OBSN` magic, record-encoding version 1, a
/// reserved zero u16, and a big-endian u32 body length. The record version
/// is independent of the protocol version in OBCS JSON message bodies and
/// the OBCS frame version.
pub const SEND_RECORD_MAGIC: [u8; 4] = *b"OBSN";

const HEADER: usize = 12;
const PREFIX: [u8; 8] = *b"OBSN\0\x01\0\0";
/// request_id + session_id + worker_id + terminal_id + record count.
const BODY_PREFIX: usize = 16 * 4 + 1;
/// A semantic stroke uses kind, modifiers, key identity, and a u32 value.
const RECORD_OVERHEAD: usize = 7;
/// Bounded body a decoder may allocate: the fixed identity prefix plus the
/// record-count overhead plus the aggregate decoded payload bound.
const MAXIMUM_BODY: usize = BODY_PREFIX
    + crate::MAXIMUM_SEND_RECORDS * RECORD_OVERHEAD
    + crate::MAXIMUM_SEND_AGGREGATE_BYTES;

const TEXT_RECORD: u8 = 1;
const BYTES_RECORD: u8 = 4;
const PASTE_RECORD: u8 = 5;
const STROKE_RECORD: u8 = 6;

pub(crate) fn encode(request: &TerminalSendRequest) -> Result<Vec<u8>, ProtocolError> {
    let records = request.records().as_slice();
    let mut body = Vec::with_capacity(BODY_PREFIX + 64);
    body.extend_from_slice(&identity_bytes(request.request_id())?);
    body.extend_from_slice(&identity_bytes(request.session_id())?);
    body.extend_from_slice(&identity_bytes(request.worker_id())?);
    body.extend_from_slice(&identity_bytes(request.terminal_id())?);
    body.push(u8::try_from(records.len()).map_err(|_| ProtocolError::InvalidPayload)?);
    for record in records {
        encode_record(&mut body, record)?;
    }
    if body.len() > MAXIMUM_BODY {
        return Err(ProtocolError::PayloadTooLarge {
            actual: body.len(),
            maximum: MAXIMUM_BODY,
        });
    }
    let mut frame = Vec::with_capacity(HEADER + body.len());
    frame.extend_from_slice(&PREFIX);
    frame.extend_from_slice(
        &u32::try_from(body.len())
            .map_err(|_| ProtocolError::InvalidPayload)?
            .to_be_bytes(),
    );
    frame.extend_from_slice(&body);
    Ok(frame)
}

fn encode_record(body: &mut Vec<u8>, record: &SendRecord) -> Result<(), ProtocolError> {
    match record {
        SendRecord::Stroke(stroke) => {
            body.push(STROKE_RECORD);
            body.extend(stroke.to_wire());
        }
        SendRecord::Text(input) => {
            body.push(TEXT_RECORD);
            let text = input.text().as_str();
            body.extend(
                u16::try_from(text.len())
                    .map_err(|_| ProtocolError::InvalidPayload)?
                    .to_be_bytes(),
            );
            body.extend(text.as_bytes());
            body.push(u8::from(input.enter()));
        }
        SendRecord::Bytes { bytes } => {
            body.push(BYTES_RECORD);
            body.extend(
                u32::try_from(bytes.as_bytes().len())
                    .map_err(|_| ProtocolError::InvalidPayload)?
                    .to_be_bytes(),
            );
            body.extend_from_slice(bytes.as_bytes());
        }
        SendRecord::Paste { content } => {
            body.push(PASTE_RECORD);
            body.extend(
                u32::try_from(content.as_bytes().len())
                    .map_err(|_| ProtocolError::InvalidPayload)?
                    .to_be_bytes(),
            );
            body.extend_from_slice(content.as_bytes());
        }
    }
    Ok(())
}

pub(crate) fn decode(frame: &[u8]) -> Result<ClientMessage, ProtocolError> {
    let header = frame.get(..HEADER).ok_or(ProtocolError::InvalidPayload)?;
    let length = body_length(header)?;
    if frame.len() != HEADER + length {
        return Err(ProtocolError::InvalidPayload);
    }
    decode_body(&frame[HEADER..])
}

pub(crate) fn read(
    reader: &mut impl Read,
    prefix: [u8; 8],
) -> Result<ClientMessage, ReadMessageError> {
    let mut header = [0; HEADER];
    header[..8].copy_from_slice(&prefix);
    reader.read_exact(&mut header[8..])?;
    let length = body_length(&header)?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    decode_body(&body).map_err(Into::into)
}

fn body_length(header: &[u8]) -> Result<usize, ProtocolError> {
    if header[..4] != SEND_RECORD_MAGIC {
        return Err(ProtocolError::InvalidMagic);
    }
    let version = u16::from_be_bytes(
        header[4..6]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    );
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
    if length <= BODY_PREFIX {
        return Err(ProtocolError::InvalidPayload);
    }
    if length > MAXIMUM_BODY {
        return Err(ProtocolError::PayloadTooLarge {
            actual: length,
            maximum: MAXIMUM_BODY,
        });
    }
    Ok(length)
}

fn decode_body(body: &[u8]) -> Result<ClientMessage, ProtocolError> {
    let request_id = identity::<RequestId>(body.get(..16))?;
    let session_id = identity::<SessionId>(body.get(16..32))?;
    let worker_id = identity::<WorkerId>(body.get(32..48))?;
    let terminal_id = identity::<TerminalId>(body.get(48..64))?;
    let count = usize::from(*body.get(64).ok_or(ProtocolError::InvalidPayload)?);
    if count == 0 || count > crate::MAXIMUM_SEND_RECORDS {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut records = Vec::with_capacity(count);
    let mut offset = BODY_PREFIX;
    for _ in 0..count {
        offset = decode_record(body, offset, &mut records)?;
    }
    if offset != body.len() {
        return Err(ProtocolError::InvalidPayload);
    }
    Ok(ClientMessage::TerminalSend(TerminalSendRequest::new(
        request_id,
        session_id,
        worker_id,
        terminal_id,
        SendRecords::new(records)?,
    )))
}

fn decode_record(
    body: &[u8],
    offset: usize,
    records: &mut Vec<SendRecord>,
) -> Result<usize, ProtocolError> {
    let kind = *body.get(offset).ok_or(ProtocolError::InvalidPayload)?;
    let offset = offset + 1;
    let take = |offset: usize, length: usize| -> Result<(&[u8], usize), ProtocolError> {
        let end = offset
            .checked_add(length)
            .ok_or(ProtocolError::InvalidPayload)?;
        let slice = body.get(offset..end).ok_or(ProtocolError::InvalidPayload)?;
        Ok((slice, end))
    };
    match kind {
        TEXT_RECORD => {
            let (raw, offset) = take(offset, 2)?;
            let length = usize::from(u16::from_be_bytes(
                raw.try_into().map_err(|_| ProtocolError::InvalidPayload)?,
            ));
            let (raw, offset) = take(offset, length)?;
            let text = std::str::from_utf8(raw).map_err(|_| ProtocolError::InvalidPayload)?;
            let (raw, offset) = take(offset, 1)?;
            let enter = match raw[0] {
                0 => false,
                1 => true,
                _ => return Err(ProtocolError::InvalidPayload),
            };
            records.push(SendRecord::Text(SendTextInput::new(
                SendText::new(text.to_owned())?,
                enter,
            )?));
            Ok(offset)
        }
        STROKE_RECORD => {
            let (raw, offset) = take(offset, 6)?;
            let stroke = crate::SendKeyStroke::from_wire(
                raw.try_into().map_err(|_| ProtocolError::InvalidPayload)?,
            )?;
            records.push(SendRecord::Stroke(stroke));
            Ok(offset)
        }
        BYTES_RECORD | PASTE_RECORD => {
            let (raw, offset) = take(offset, 4)?;
            let length = usize::try_from(u32::from_be_bytes(
                raw.try_into().map_err(|_| ProtocolError::InvalidPayload)?,
            ))
            .map_err(|_| ProtocolError::InvalidPayload)?;
            let (raw, offset) = take(offset, length)?;
            records.push(if kind == BYTES_RECORD {
                SendRecord::Bytes {
                    bytes: SendBytes::new(raw.to_vec())?,
                }
            } else {
                SendRecord::Paste {
                    content: SendPasteContent::new(raw.to_vec())?,
                }
            });
            Ok(offset)
        }
        _ => Err(ProtocolError::InvalidPayload),
    }
}
