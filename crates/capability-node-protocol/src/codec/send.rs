//! Encodes bounded OBNS send sequences and their correlated prefix-progress and cleanup
//! outcomes, rejecting malformed records before domain construction. It neither executes
//! records nor treats accepted input as child completion or permission to replay.

use super::{PayloadReader, decode_host_identity, encode_host_identity};
use crate::{
    HostTerminalIdentity, OperationId, ProtocolError, SendBytes, SendCleanupStatus,
    SendPasteContent, SendPrecondition, SendProgress, SendRecord, SendRecords, SendRejection,
    SendText, SendTextInput, SendUnavailable, TerminalSendOutcome, TerminalSendRequest,
    TerminalSendResult, WorkerGeneration, WorkerId,
};
use std::io::Read;
use std::num::NonZeroU64;

/// The send request never rides the standard OBCN frame: a `bytes` record
/// alone may carry a megabyte of opaque payload, so the whole request is one
/// dedicated binary record — `OBNS` magic, record-encoding version 1, a
/// big-endian u16 of precondition flags, and a big-endian u32 body length.
pub(crate) const SEND_RECORD_MAGIC: [u8; 4] = *b"OBNS";
const SEND_RECORD_HEADER: usize = 12;
const SEND_RECORD_VERSION: u16 = 1;
/// Flag bit 0 asks for [`SendPrecondition::IdlePrompt`]; every other bit is reserved zero, so
/// a decoder that predates a flag refuses the record instead of ignoring it.
const IDLE_PROMPT_FLAG: u16 = 1;
/// request_id + worker_id + worker_generation + host identity (3×16) +
/// attachment + record count.
const SEND_RECORD_BODY_PREFIX: usize = 16 + 16 + 8 + 48 + 16 + 1;
/// A semantic stroke uses kind, modifiers, key identity, and a u32 value.
const SEND_RECORD_OVERHEAD: usize = 7;
/// Bounded body a decoder may allocate: the fixed identity prefix plus the
/// record-count overhead plus the aggregate decoded payload bound.
const MAXIMUM_SEND_RECORD_BODY: usize = SEND_RECORD_BODY_PREFIX
    + MAXIMUM_SEND_RECORDS_WIRE * SEND_RECORD_OVERHEAD
    + crate::terminal_send::MAXIMUM_SEND_AGGREGATE_BYTES;
const MAXIMUM_SEND_RECORDS_WIRE: usize = crate::terminal_send::MAXIMUM_SEND_RECORDS;

const SEND_RESULT_KIND: u8 = 14;

const TEXT_RECORD: u8 = 1;
const BYTES_RECORD: u8 = 4;
const PASTE_RECORD: u8 = 5;
const STROKE_RECORD: u8 = 6;

const UNAVAILABLE_INPUT: u8 = 1;
const REJECTED_INPUT: u8 = 2;
const ACCEPTED_INPUT: u8 = 3;
const UNKNOWN_INPUT: u8 = 4;

fn encode_record_kind(bytes: &mut Vec<u8>, record: &SendRecord) -> Result<(), ProtocolError> {
    match record {
        SendRecord::Stroke(stroke) => {
            bytes.push(STROKE_RECORD);
            bytes.extend(stroke.to_wire());
        }
        SendRecord::Text(input) => {
            bytes.push(TEXT_RECORD);
            let text = input.text().as_str();
            bytes.extend(
                u16::try_from(text.len())
                    .map_err(|_| ProtocolError::InvalidPayload)?
                    .to_be_bytes(),
            );
            bytes.extend(text.as_bytes());
            bytes.push(u8::from(input.enter()));
        }
        SendRecord::Bytes { bytes: payload } => {
            bytes.push(BYTES_RECORD);
            bytes.extend(
                u32::try_from(payload.as_bytes().len())
                    .map_err(|_| ProtocolError::InvalidPayload)?
                    .to_be_bytes(),
            );
            bytes.extend_from_slice(payload.as_bytes());
        }
        SendRecord::Paste { content } => {
            bytes.push(PASTE_RECORD);
            bytes.extend(
                u32::try_from(content.as_bytes().len())
                    .map_err(|_| ProtocolError::InvalidPayload)?
                    .to_be_bytes(),
            );
            bytes.extend_from_slice(content.as_bytes());
        }
    }
    Ok(())
}

fn decode_record_kind(reader: &mut PayloadReader<'_>) -> Result<SendRecord, ProtocolError> {
    match reader.read_u8()? {
        TEXT_RECORD => {
            let length = usize::from(reader.read_u16()?);
            let text = std::str::from_utf8(reader.read_slice(length)?)
                .map_err(|_error| ProtocolError::InvalidPayload)?;
            let enter = match reader.read_u8()? {
                0 => false,
                1 => true,
                _ => return Err(ProtocolError::InvalidPayload),
            };
            Ok(SendRecord::Text(SendTextInput::new(
                SendText::new(text.to_owned())?,
                enter,
            )?))
        }
        STROKE_RECORD => Ok(SendRecord::Stroke(crate::SendKeyStroke::from_wire(
            reader
                .read_slice(6)?
                .try_into()
                .map_err(|_| ProtocolError::InvalidPayload)?,
        )?)),
        BYTES_RECORD => {
            let length =
                usize::try_from(reader.read_u32()?).map_err(|_| ProtocolError::InvalidPayload)?;
            Ok(SendRecord::Bytes {
                bytes: SendBytes::new(reader.read_slice(length)?.to_vec())?,
            })
        }
        PASTE_RECORD => {
            let length =
                usize::try_from(reader.read_u32()?).map_err(|_| ProtocolError::InvalidPayload)?;
            Ok(SendRecord::Paste {
                content: SendPasteContent::new(reader.read_slice(length)?.to_vec())?,
            })
        }
        _ => Err(ProtocolError::InvalidPayload),
    }
}

/// Encode the whole request as one `OBNS` record frame. The record frame's
/// own version stays 1: it versions this record encoding, not the protocol
/// — the protocol version bump lives in the connection's frame header.
pub(crate) fn encode_request_record(
    request: &TerminalSendRequest,
) -> Result<Vec<u8>, ProtocolError> {
    request.validate()?;
    let records = request.records.as_slice();
    let mut body = Vec::with_capacity(SEND_RECORD_BODY_PREFIX + 64);
    body.extend(request.request_id.as_bytes());
    body.extend(request.worker_id.as_bytes());
    body.extend_from_slice(&request.worker_generation.get().to_be_bytes());
    encode_host_identity(&mut body, request.target);
    body.extend(request.attachment_id.as_bytes());
    body.push(u8::try_from(records.len()).map_err(|_| ProtocolError::InvalidPayload)?);
    for record in records {
        encode_record_kind(&mut body, record)?;
    }
    if body.len() > MAXIMUM_SEND_RECORD_BODY {
        return Err(ProtocolError::PayloadTooLarge {
            actual: body.len(),
            maximum: MAXIMUM_SEND_RECORD_BODY,
        });
    }
    let mut frame = Vec::with_capacity(SEND_RECORD_HEADER + body.len());
    frame.extend_from_slice(&SEND_RECORD_MAGIC);
    frame.extend_from_slice(&SEND_RECORD_VERSION.to_be_bytes());
    let flags = match request.precondition {
        SendPrecondition::None => 0,
        SendPrecondition::IdlePrompt => IDLE_PROMPT_FLAG,
    };
    frame.extend_from_slice(&flags.to_be_bytes());
    frame.extend_from_slice(
        &u32::try_from(body.len())
            .map_err(|_| ProtocolError::InvalidPayload)?
            .to_be_bytes(),
    );
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// Read one `OBNS` record from a stream given its already-consumed 8-byte
/// prefix. The declared body length is bound-checked before allocation.
pub(crate) fn read_request_record(
    reader: &mut impl Read,
    prefix: [u8; 8],
) -> Result<TerminalSendRequest, crate::ReadMessageError> {
    let mut header = [0_u8; SEND_RECORD_HEADER];
    header[..8].copy_from_slice(&prefix);
    reader.read_exact(&mut header[8..])?;
    let (length, precondition) =
        decode_record_header(&header).map_err(crate::ReadMessageError::from)?;
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body)?;
    decode_request_body(&body, precondition).map_err(Into::into)
}

/// Decode one complete in-memory `OBNS` frame.
pub(crate) fn decode_request_record(frame: &[u8]) -> Result<TerminalSendRequest, ProtocolError> {
    let header: &[u8; SEND_RECORD_HEADER] = frame
        .get(..SEND_RECORD_HEADER)
        .ok_or(ProtocolError::InvalidPayload)?
        .try_into()
        .map_err(|_error| ProtocolError::InvalidPayload)?;
    let (length, precondition) = decode_record_header(header)?;
    if frame.len() != SEND_RECORD_HEADER + length {
        return Err(ProtocolError::InvalidPayload);
    }
    decode_request_body(&frame[SEND_RECORD_HEADER..], precondition)
}

fn decode_record_header(
    header: &[u8; SEND_RECORD_HEADER],
) -> Result<(usize, SendPrecondition), ProtocolError> {
    if header[..4] != SEND_RECORD_MAGIC {
        return Err(ProtocolError::InvalidMagic);
    }
    if u16::from_be_bytes([header[4], header[5]]) != SEND_RECORD_VERSION {
        return Err(ProtocolError::IncompatibleVersion { received: 1 });
    }
    let precondition = match u16::from_be_bytes([header[6], header[7]]) {
        0 => SendPrecondition::None,
        IDLE_PROMPT_FLAG => SendPrecondition::IdlePrompt,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    let length = usize::try_from(u32::from_be_bytes(
        header[8..12]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    ))
    .map_err(|_| ProtocolError::InvalidPayload)?;
    if length <= SEND_RECORD_BODY_PREFIX || length > MAXIMUM_SEND_RECORD_BODY {
        return Err(ProtocolError::PayloadTooLarge {
            actual: length,
            maximum: MAXIMUM_SEND_RECORD_BODY,
        });
    }
    Ok((length, precondition))
}

fn decode_request_body(
    body: &[u8],
    precondition: SendPrecondition,
) -> Result<TerminalSendRequest, ProtocolError> {
    let mut reader = PayloadReader::whole(body);
    let request_id = OperationId::from_bytes(reader.read_array()?)?;
    let worker_id = WorkerId::from_bytes(reader.read_array()?)?;
    let worker_generation = WorkerGeneration::new(
        NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
    );
    let target: HostTerminalIdentity = decode_host_identity(&mut reader)?;
    let attachment_id = OperationId::from_bytes(reader.read_array()?)?;
    let count = usize::from(reader.read_u8()?);
    if count == 0 {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut records = Vec::with_capacity(count.min(MAXIMUM_SEND_RECORDS_WIRE));
    for _ in 0..count {
        records.push(decode_record_kind(&mut reader)?);
    }
    reader.finish()?;
    let request = TerminalSendRequest {
        request_id,
        worker_id,
        worker_generation,
        target,
        attachment_id,
        precondition,
        records: SendRecords::new(records)?,
    };
    request.validate()?;
    Ok(request)
}

fn encode_progress(bytes: &mut Vec<u8>, progress: SendProgress) {
    bytes.push(u8::try_from(progress.records_accepted()).unwrap_or(u8::MAX));
    bytes.push(u8::try_from(progress.first_stopped_record()).unwrap_or(u8::MAX));
    bytes.extend_from_slice(&progress.accepted_encoded_bytes().to_be_bytes());
}

fn decode_progress(reader: &mut PayloadReader<'_>) -> Result<SendProgress, ProtocolError> {
    let records_accepted = u32::from(reader.read_u8()?);
    let first_stopped_record = u32::from(reader.read_u8()?);
    let accepted_encoded_bytes = reader.read_u64()?;
    SendProgress::new(
        records_accepted,
        first_stopped_record,
        accepted_encoded_bytes,
    )
}

pub(super) fn encode_result(result: &TerminalSendResult) -> Result<Vec<u8>, ProtocolError> {
    result.outcome().validate()?;
    let mut bytes = vec![SEND_RESULT_KIND];
    bytes.extend(result.request_id().as_bytes());
    match result.outcome() {
        TerminalSendOutcome::Unavailable {
            reason,
            progress,
            cleanup,
        } => {
            bytes.push(UNAVAILABLE_INPUT);
            bytes.push(reason.code());
            encode_progress(&mut bytes, progress);
            bytes.push(cleanup.code());
        }
        TerminalSendOutcome::Rejected {
            reason,
            progress,
            cleanup,
        } => {
            bytes.push(REJECTED_INPUT);
            reason.encode(&mut bytes);
            encode_progress(&mut bytes, progress);
            bytes.push(cleanup.code());
        }
        TerminalSendOutcome::Accepted { progress, cleanup } => {
            bytes.push(ACCEPTED_INPUT);
            encode_progress(&mut bytes, progress);
            bytes.push(cleanup.code());
        }
        TerminalSendOutcome::OutcomeUnknown { progress, cleanup } => {
            bytes.push(UNKNOWN_INPUT);
            encode_progress(&mut bytes, progress);
            bytes.push(cleanup.code());
        }
    }
    Ok(bytes)
}

pub(super) fn decode_result(bytes: &[u8]) -> Result<TerminalSendResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, SEND_RESULT_KIND)?;
    let request_id = OperationId::from_bytes(reader.read_array()?)?;
    let outcome = match reader.read_u8()? {
        UNAVAILABLE_INPUT => TerminalSendOutcome::Unavailable {
            reason: SendUnavailable::from_code(reader.read_u8()?)?,
            progress: decode_progress(&mut reader)?,
            cleanup: SendCleanupStatus::from_code(reader.read_u8()?)?,
        },
        REJECTED_INPUT => TerminalSendOutcome::Rejected {
            reason: {
                let code = reader.read_u8()?;
                SendRejection::decode(code, || reader.read_u8())?
            },
            progress: decode_progress(&mut reader)?,
            cleanup: SendCleanupStatus::from_code(reader.read_u8()?)?,
        },
        ACCEPTED_INPUT => TerminalSendOutcome::Accepted {
            progress: decode_progress(&mut reader)?,
            cleanup: SendCleanupStatus::from_code(reader.read_u8()?)?,
        },
        UNKNOWN_INPUT => TerminalSendOutcome::OutcomeUnknown {
            progress: decode_progress(&mut reader)?,
            cleanup: SendCleanupStatus::from_code(reader.read_u8()?)?,
        },
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    TerminalSendResult::new(request_id, outcome)
}
