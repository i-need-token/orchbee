use super::{PayloadReader, decode_host_identity, encode_host_identity};
use crate::{
    HostTerminalIdentity, OperationId, ProtocolError, SendBytes, SendCleanupStatus, SendComboBase,
    SendKey, SendModifiers, SendPasteContent, SendProgress, SendRecord, SendRecords, SendRejection,
    SendText, SendTextInput, SendUnavailable, TerminalSendOutcome, TerminalSendRequest,
    TerminalSendResult, WorkerGeneration, WorkerId,
};
use std::io::Read;
use std::num::NonZeroU64;

/// The send request never rides the standard OBCN frame: a `bytes` record
/// alone may carry a megabyte of opaque payload, so the whole request is one
/// dedicated binary record — `OBNS` magic, record-encoding version 1, a
/// reserved zero u16, and a big-endian u32 body length.
pub(crate) const SEND_RECORD_MAGIC: [u8; 4] = *b"OBNS";
const SEND_RECORD_HEADER: usize = 12;
const SEND_RECORD_VERSION: u16 = 1;
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
const KEY_RECORD: u8 = 2;
const COMBO_RECORD: u8 = 3;
const BYTES_RECORD: u8 = 4;
const PASTE_RECORD: u8 = 5;
const STROKE_RECORD: u8 = 6;

const COMBO_BASE_NAMED: u8 = 1;
const COMBO_BASE_INSERT: u8 = 2;
const COMBO_BASE_FUNCTION: u8 = 3;
const COMBO_BASE_SPACE: u8 = 4;
const COMBO_BASE_LETTER: u8 = 5;
const COMBO_BASE_DIGIT: u8 = 6;

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
        SendRecord::Key { key } => {
            bytes.push(KEY_RECORD);
            bytes.push(key.code());
        }
        SendRecord::Combo { modifiers, base } => {
            bytes.push(COMBO_RECORD);
            bytes.push(
                u8::from(modifiers.control)
                    | (u8::from(modifiers.alt) << 1)
                    | (u8::from(modifiers.shift) << 2),
            );
            encode_combo_base(bytes, *base);
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

fn encode_combo_base(bytes: &mut Vec<u8>, base: SendComboBase) {
    match base {
        SendComboBase::Named(key) => {
            bytes.push(COMBO_BASE_NAMED);
            bytes.push(key.code());
        }
        SendComboBase::Insert => bytes.extend([COMBO_BASE_INSERT, 0]),
        SendComboBase::Function(number) => bytes.extend([COMBO_BASE_FUNCTION, number]),
        SendComboBase::Space => bytes.extend([COMBO_BASE_SPACE, 0]),
        SendComboBase::Letter(letter) => bytes.extend([COMBO_BASE_LETTER, letter]),
        SendComboBase::Digit(digit) => bytes.extend([COMBO_BASE_DIGIT, digit]),
    }
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
        KEY_RECORD => Ok(SendRecord::Key {
            key: SendKey::from_code(reader.read_u8()?)?,
        }),
        COMBO_RECORD => {
            let flags = reader.read_u8()?;
            if flags & !0b111 != 0 {
                return Err(ProtocolError::InvalidPayload);
            }
            let modifiers =
                SendModifiers::new(flags & 1 != 0, flags & 0b10 != 0, flags & 0b100 != 0)?;
            Ok(SendRecord::Combo {
                modifiers,
                base: decode_combo_base(reader)?,
            })
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

fn decode_combo_base(reader: &mut PayloadReader<'_>) -> Result<SendComboBase, ProtocolError> {
    match reader.read_u8()? {
        COMBO_BASE_NAMED => SendComboBase::new_named(SendKey::from_code(reader.read_u8()?)?),
        COMBO_BASE_INSERT => {
            reader.read_u8().and_then(|reserved| {
                (reserved == 0)
                    .then_some(())
                    .ok_or(ProtocolError::InvalidPayload)
            })?;
            Ok(SendComboBase::Insert)
        }
        COMBO_BASE_FUNCTION => SendComboBase::new_function(reader.read_u8()?),
        COMBO_BASE_SPACE => {
            reader.read_u8().and_then(|reserved| {
                (reserved == 0)
                    .then_some(())
                    .ok_or(ProtocolError::InvalidPayload)
            })?;
            Ok(SendComboBase::Space)
        }
        COMBO_BASE_LETTER => SendComboBase::new_letter(reader.read_u8()?),
        COMBO_BASE_DIGIT => SendComboBase::new_digit(reader.read_u8()?),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

/// Encode the whole request as one `OBNS` record frame. The record frame's
/// own version stays 1: it versions this record encoding, not the protocol
/// — the protocol version bump lives in the connection's frame header.
pub(crate) fn encode_request_record(
    request: &TerminalSendRequest,
) -> Result<Vec<u8>, ProtocolError> {
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
    frame.extend_from_slice(&0_u16.to_be_bytes());
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
    let length = decode_record_header(&header).map_err(crate::ReadMessageError::from)?;
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body)?;
    decode_request_body(&body).map_err(Into::into)
}

/// Decode one complete in-memory `OBNS` frame.
pub(crate) fn decode_request_record(frame: &[u8]) -> Result<TerminalSendRequest, ProtocolError> {
    let header: &[u8; SEND_RECORD_HEADER] = frame
        .get(..SEND_RECORD_HEADER)
        .ok_or(ProtocolError::InvalidPayload)?
        .try_into()
        .map_err(|_error| ProtocolError::InvalidPayload)?;
    let length = decode_record_header(header)?;
    if frame.len() != SEND_RECORD_HEADER + length {
        return Err(ProtocolError::InvalidPayload);
    }
    decode_request_body(&frame[SEND_RECORD_HEADER..])
}

fn decode_record_header(header: &[u8; SEND_RECORD_HEADER]) -> Result<usize, ProtocolError> {
    if header[..4] != SEND_RECORD_MAGIC {
        return Err(ProtocolError::InvalidMagic);
    }
    if u16::from_be_bytes([header[4], header[5]]) != SEND_RECORD_VERSION {
        return Err(ProtocolError::IncompatibleVersion { received: 1 });
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
    if length <= SEND_RECORD_BODY_PREFIX || length > MAXIMUM_SEND_RECORD_BODY {
        return Err(ProtocolError::PayloadTooLarge {
            actual: length,
            maximum: MAXIMUM_SEND_RECORD_BODY,
        });
    }
    Ok(length)
}

fn decode_request_body(body: &[u8]) -> Result<TerminalSendRequest, ProtocolError> {
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
    Ok(TerminalSendRequest {
        request_id,
        worker_id,
        worker_generation,
        target,
        attachment_id,
        records: SendRecords::new(records)?,
    })
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
            bytes.push(reason.code());
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
            reason: SendRejection::from_code(reader.read_u8()?)?,
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
