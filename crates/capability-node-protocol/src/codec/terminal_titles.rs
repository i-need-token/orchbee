//! Carries physical title requests, complete bounded OBNT title replies, the kind-35 change note,
//! the kind-36 change read and its OBNC page record, without a screen projection. It neither
//! observes OSC events nor interprets titles as Agent state.

use super::{PayloadReader, decode_host_identity, encode_host_identity};
use crate::{
    CapabilityNodeMessage, MAXIMUM_TERMINAL_TITLE_BYTES, MAXIMUM_TERMINAL_TITLE_CHANGE_BYTES,
    MAXIMUM_TERMINAL_TITLE_CHANGES, MAXIMUM_TERMINAL_TITLE_ROWS, OperationId, ProtocolError,
    ReadMessageError, TerminalTitleChangePage, TerminalTitleChangesOutcome,
    TerminalTitleChangesRequest, TerminalTitleChangesResult, TerminalTitleObservation,
    TerminalTitleRow, TerminalTitlesChanged, TerminalTitlesRequest, TerminalTitlesResult,
    WorkerGeneration, WorkerId,
};
use std::{io::Read, num::NonZeroU64};

pub(super) const CHANGED_KIND: u8 = 35;
pub(super) const CHANGES_REQUEST_KIND: u8 = 36;
const RESULT_HEADER: [u8; 8] = *b"OBNT\0\x01\0\0";
const CHANGES_HEADER: [u8; 8] = *b"OBNC\0\x01\0\0";
const RECORD_PREFIX_BYTES: usize = 12;
/// A Host identity, the observation tag, revision, title tag and title length.
const ROW_OVERHEAD_BYTES: usize = 48 + 1 + 8 + 1 + 4;
const MINIMUM_RESULT_BYTES: usize = 17;
const MAXIMUM_RESULT_BYTES: usize = MINIMUM_RESULT_BYTES
    + MAXIMUM_TERMINAL_TITLE_ROWS * (ROW_OVERHEAD_BYTES + MAXIMUM_TERMINAL_TITLE_BYTES);
/// Request ID, Worker ID, generation and cursor.
const CHANGES_REQUEST_BYTES: usize = 16 + 16 + 8 + 8;
/// The echoed request and the outcome tag of a reset or fencing rejection.
const MINIMUM_CHANGES_BYTES: usize = CHANGES_REQUEST_BYTES + 1;
/// The echoed request, outcome tag, `next`, `complete`, row count and the rows.
const MAXIMUM_CHANGES_BYTES: usize = MINIMUM_CHANGES_BYTES
    + 8
    + 1
    + 2
    + MAXIMUM_TERMINAL_TITLE_CHANGES * ROW_OVERHEAD_BYTES
    + MAXIMUM_TERMINAL_TITLE_CHANGE_BYTES;
const PAGE_OUTCOME: u8 = 1;
const RESET_OUTCOME: u8 = 2;
const REJECTED_FENCING_OUTCOME: u8 = 3;

pub(super) fn encode_request(value: &TerminalTitlesRequest) -> Result<Vec<u8>, ProtocolError> {
    value.validate()?;
    let mut bytes = vec![23];
    bytes.extend(value.request_id.as_bytes());
    bytes.extend(value.worker_id.as_bytes());
    bytes.extend(value.worker_generation.get().to_be_bytes());
    bytes.push(value.identities.len() as u8);
    for id in &value.identities {
        encode_host_identity(&mut bytes, *id);
    }
    Ok(bytes)
}
pub(super) fn decode_request(bytes: &[u8]) -> Result<TerminalTitlesRequest, ProtocolError> {
    let mut r = PayloadReader::after_kind(bytes, 23)?;
    let request_id = OperationId::from_bytes(r.read_array()?)?;
    let worker_id = WorkerId::from_bytes(r.read_array()?)?;
    let worker_generation = decode_generation(&mut r)?;
    let count = usize::from(r.read_u8()?);
    if count > MAXIMUM_TERMINAL_TITLE_ROWS {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut identities = Vec::with_capacity(count);
    for _ in 0..count {
        identities.push(decode_host_identity(&mut r)?);
    }
    r.finish()?;
    let value = TerminalTitlesRequest {
        request_id,
        worker_id,
        worker_generation,
        identities,
    };
    value.validate()?;
    Ok(value)
}
pub(super) fn encode_result(value: &TerminalTitlesResult) -> Result<Vec<u8>, ProtocolError> {
    value.validate()?;
    let mut body = Vec::new();
    body.extend(value.request_id.as_bytes());
    body.push(value.rows.len() as u8);
    for row in &value.rows {
        encode_row(&mut body, row);
    }
    Ok(record(RESULT_HEADER, &body))
}
pub(super) fn decode_result(frame: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let body = record_body(
        frame,
        RESULT_HEADER,
        MINIMUM_RESULT_BYTES,
        MAXIMUM_RESULT_BYTES,
    )?;
    result_payload(body)
}
pub(super) fn read_result(
    reader: &mut impl Read,
    header: [u8; 8],
) -> Result<CapabilityNodeMessage, ReadMessageError> {
    let body = read_record_body(
        reader,
        header,
        RESULT_HEADER,
        MINIMUM_RESULT_BYTES,
        MAXIMUM_RESULT_BYTES,
    )?;
    result_payload(&body).map_err(Into::into)
}
fn result_payload(body: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let mut r = PayloadReader::whole(body);
    let request_id = OperationId::from_bytes(r.read_array()?)?;
    let count = usize::from(r.read_u8()?);
    if count > MAXIMUM_TERMINAL_TITLE_ROWS {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut rows = Vec::with_capacity(count);
    for _ in 0..count {
        rows.push(decode_row(&mut r)?);
    }
    r.finish()?;
    let value = TerminalTitlesResult { request_id, rows };
    value.validate()?;
    Ok(CapabilityNodeMessage::TerminalTitlesResult(value))
}

pub(super) fn encode_changed(value: TerminalTitlesChanged) -> Vec<u8> {
    let mut bytes = vec![CHANGED_KIND];
    bytes.extend(value.through.get().to_be_bytes());
    bytes
}
pub(super) fn decode_changed(bytes: &[u8]) -> Result<TerminalTitlesChanged, ProtocolError> {
    let mut r = PayloadReader::after_kind(bytes, CHANGED_KIND)?;
    let through = NonZeroU64::new(r.read_u64()?).ok_or(ProtocolError::InvalidPayload)?;
    r.finish()?;
    Ok(TerminalTitlesChanged { through })
}

pub(super) fn encode_changes_request(value: TerminalTitleChangesRequest) -> Vec<u8> {
    let mut bytes = vec![CHANGES_REQUEST_KIND];
    encode_changes_request_body(&mut bytes, value);
    bytes
}
pub(super) fn decode_changes_request(
    bytes: &[u8],
) -> Result<TerminalTitleChangesRequest, ProtocolError> {
    let mut r = PayloadReader::after_kind(bytes, CHANGES_REQUEST_KIND)?;
    let value = decode_changes_request_body(&mut r)?;
    r.finish()?;
    Ok(value)
}
pub(super) fn encode_changes_result(
    value: &TerminalTitleChangesResult,
) -> Result<Vec<u8>, ProtocolError> {
    value.validate()?;
    let mut body = Vec::new();
    encode_changes_request_body(&mut body, value.request);
    match &value.outcome {
        TerminalTitleChangesOutcome::Page(page) => {
            body.push(PAGE_OUTCOME);
            body.extend(page.next.to_be_bytes());
            body.push(u8::from(page.complete));
            // Validation bounds the rows to 256.
            body.extend((page.rows.len() as u16).to_be_bytes());
            for row in &page.rows {
                encode_row(&mut body, row);
            }
        }
        TerminalTitleChangesOutcome::Reset => body.push(RESET_OUTCOME),
        TerminalTitleChangesOutcome::RejectedFencing => body.push(REJECTED_FENCING_OUTCOME),
    }
    Ok(record(CHANGES_HEADER, &body))
}
pub(super) fn decode_changes_result(frame: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let body = record_body(
        frame,
        CHANGES_HEADER,
        MINIMUM_CHANGES_BYTES,
        MAXIMUM_CHANGES_BYTES,
    )?;
    changes_payload(body)
}
/// Reads the rest of the record whose first eight bytes are `header`, refusing an out-of-bounds
/// length before allocating the body.
pub(super) fn read_changes_result(
    reader: &mut impl Read,
    header: [u8; 8],
) -> Result<CapabilityNodeMessage, ReadMessageError> {
    let body = read_record_body(
        reader,
        header,
        CHANGES_HEADER,
        MINIMUM_CHANGES_BYTES,
        MAXIMUM_CHANGES_BYTES,
    )?;
    changes_payload(&body).map_err(Into::into)
}
fn changes_payload(body: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let mut r = PayloadReader::whole(body);
    let request = decode_changes_request_body(&mut r)?;
    let outcome = match r.read_u8()? {
        PAGE_OUTCOME => {
            let next = r.read_u64()?;
            let complete = match r.read_u8()? {
                0 => false,
                1 => true,
                _ => return Err(ProtocolError::InvalidPayload),
            };
            let count = usize::from(r.read_u16()?);
            if count > MAXIMUM_TERMINAL_TITLE_CHANGES {
                return Err(ProtocolError::InvalidPayload);
            }
            let mut rows = Vec::with_capacity(count);
            for _ in 0..count {
                rows.push(decode_row(&mut r)?);
            }
            TerminalTitleChangesOutcome::Page(TerminalTitleChangePage {
                rows,
                next,
                complete,
            })
        }
        RESET_OUTCOME => TerminalTitleChangesOutcome::Reset,
        REJECTED_FENCING_OUTCOME => TerminalTitleChangesOutcome::RejectedFencing,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    r.finish()?;
    let value = TerminalTitleChangesResult { request, outcome };
    value.validate()?;
    Ok(CapabilityNodeMessage::TerminalTitleChangesResult(value))
}

fn encode_changes_request_body(bytes: &mut Vec<u8>, value: TerminalTitleChangesRequest) {
    bytes.extend(value.request_id.as_bytes());
    bytes.extend(value.worker_id.as_bytes());
    bytes.extend(value.worker_generation.get().to_be_bytes());
    bytes.extend(value.after.to_be_bytes());
}
fn decode_changes_request_body(
    r: &mut PayloadReader<'_>,
) -> Result<TerminalTitleChangesRequest, ProtocolError> {
    Ok(TerminalTitleChangesRequest {
        request_id: OperationId::from_bytes(r.read_array()?)?,
        worker_id: WorkerId::from_bytes(r.read_array()?)?,
        worker_generation: decode_generation(r)?,
        after: r.read_u64()?,
    })
}
fn decode_generation(r: &mut PayloadReader<'_>) -> Result<WorkerGeneration, ProtocolError> {
    NonZeroU64::new(r.read_u64()?)
        .map(WorkerGeneration::new)
        .ok_or(ProtocolError::InvalidPayload)
}

fn encode_row(body: &mut Vec<u8>, row: &TerminalTitleRow) {
    encode_host_identity(body, row.identity);
    match &row.observation {
        TerminalTitleObservation::Unavailable => body.push(0),
        TerminalTitleObservation::Observed { revision, title } => {
            body.push(1);
            body.extend(revision.get().to_be_bytes());
            if let Some(title) = title {
                body.push(1);
                // Validation bounds each title to 64 KiB.
                body.extend((title.len() as u32).to_be_bytes());
                body.extend(title.as_bytes());
            } else {
                body.push(0);
            }
        }
    }
}
fn decode_row(r: &mut PayloadReader<'_>) -> Result<TerminalTitleRow, ProtocolError> {
    let identity = decode_host_identity(r)?;
    let observation = match r.read_u8()? {
        0 => TerminalTitleObservation::Unavailable,
        1 => {
            let revision = NonZeroU64::new(r.read_u64()?).ok_or(ProtocolError::InvalidPayload)?;
            let title = match r.read_u8()? {
                0 => None,
                1 => {
                    let n = usize::try_from(r.read_u32()?)
                        .ok()
                        .filter(|n| *n <= MAXIMUM_TERMINAL_TITLE_BYTES)
                        .ok_or(ProtocolError::InvalidPayload)?;
                    Some(
                        std::str::from_utf8(r.read_slice(n)?)
                            .map_err(|_| ProtocolError::InvalidPayload)?
                            .to_owned(),
                    )
                }
                _ => return Err(ProtocolError::InvalidPayload),
            };
            TerminalTitleObservation::Observed { revision, title }
        }
        _ => return Err(ProtocolError::InvalidPayload),
    };
    Ok(TerminalTitleRow {
        identity,
        observation,
    })
}

fn record(header: [u8; 8], body: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(RECORD_PREFIX_BYTES + body.len());
    frame.extend(header);
    // Both record bounds stay far below 4 GiB.
    frame.extend((body.len() as u32).to_be_bytes());
    frame.extend(body);
    frame
}
fn body_length(
    prefix: &[u8; RECORD_PREFIX_BYTES],
    expected: [u8; 8],
    minimum: usize,
    maximum: usize,
) -> Result<usize, ProtocolError> {
    let (header, length) = prefix.split_at(8);
    if header != expected {
        return Err(ProtocolError::InvalidPayload);
    }
    let length = length
        .try_into()
        .map(u32::from_be_bytes)
        .map_err(|_| ProtocolError::InvalidPayload)?;
    usize::try_from(length)
        .ok()
        .filter(|length| (minimum..=maximum).contains(length))
        .ok_or(ProtocolError::InvalidPayload)
}
fn record_body(
    frame: &[u8],
    expected: [u8; 8],
    minimum: usize,
    maximum: usize,
) -> Result<&[u8], ProtocolError> {
    let prefix = frame
        .get(..RECORD_PREFIX_BYTES)
        .ok_or(ProtocolError::InvalidPayload)?
        .try_into()
        .map_err(|_| ProtocolError::InvalidPayload)?;
    let length = body_length(prefix, expected, minimum, maximum)?;
    match frame.get(RECORD_PREFIX_BYTES..) {
        Some(body) if body.len() == length => Ok(body),
        _ => Err(ProtocolError::InvalidPayload),
    }
}
fn read_record_body(
    reader: &mut impl Read,
    header: [u8; 8],
    expected: [u8; 8],
    minimum: usize,
    maximum: usize,
) -> Result<Vec<u8>, ReadMessageError> {
    let mut prefix = [0; RECORD_PREFIX_BYTES];
    prefix[..8].copy_from_slice(&header);
    reader.read_exact(&mut prefix[8..])?;
    let mut body = vec![0; body_length(&prefix, expected, minimum, maximum)?];
    reader.read_exact(&mut body)?;
    Ok(body)
}
