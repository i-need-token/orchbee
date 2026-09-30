//! OBCN request 23; OBNT v1 reply carries complete bounded titles without a screen.
use super::{PayloadReader, decode_host_identity, encode_host_identity};
use crate::{
    CapabilityNodeMessage, MAXIMUM_TERMINAL_TITLE_BYTES, MAXIMUM_TERMINAL_TITLE_ROWS, OperationId,
    ProtocolError, ReadMessageError, TerminalTitleObservation, TerminalTitleRow,
    TerminalTitlesRequest, TerminalTitlesResult, WorkerGeneration, WorkerId,
};
use std::{io::Read, num::NonZeroU64};
const MAXIMUM_RESULT_BYTES: usize =
    17 + MAXIMUM_TERMINAL_TITLE_ROWS * (62 + MAXIMUM_TERMINAL_TITLE_BYTES);

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
    let worker_generation =
        WorkerGeneration::new(NonZeroU64::new(r.read_u64()?).ok_or(ProtocolError::InvalidPayload)?);
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
        encode_host_identity(&mut body, row.identity);
        match &row.observation {
            TerminalTitleObservation::Unavailable => body.push(0),
            TerminalTitleObservation::Observed { revision, title } => {
                body.push(1);
                body.extend(revision.get().to_be_bytes());
                if let Some(title) = title {
                    body.push(1);
                    body.extend((title.len() as u32).to_be_bytes());
                    body.extend(title.as_bytes());
                } else {
                    body.push(0);
                }
            }
        }
    }
    let mut frame = Vec::with_capacity(12 + body.len());
    frame.extend(b"OBNT\0\x01\0\0");
    frame.extend((body.len() as u32).to_be_bytes());
    frame.extend(&body);
    Ok(frame)
}
fn length(header: &[u8]) -> Result<usize, ProtocolError> {
    if header.get(..8) != Some(b"OBNT\0\x01\0\0") {
        return Err(ProtocolError::InvalidPayload);
    }
    let n = u32::from_be_bytes(
        header
            .get(8..12)
            .ok_or(ProtocolError::InvalidPayload)?
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    ) as usize;
    if !(17..=MAXIMUM_RESULT_BYTES).contains(&n) {
        return Err(ProtocolError::InvalidPayload);
    }
    Ok(n)
}
pub(super) fn decode_result(frame: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let n = length(frame.get(..12).ok_or(ProtocolError::InvalidPayload)?)?;
    if frame.len() != 12 + n {
        return Err(ProtocolError::InvalidPayload);
    }
    payload(&frame[12..])
}
pub(super) fn read_result(
    reader: &mut impl Read,
    header: [u8; 8],
) -> Result<CapabilityNodeMessage, ReadMessageError> {
    let mut full = [0; 12];
    full[..8].copy_from_slice(&header);
    reader.read_exact(&mut full[8..])?;
    let mut body = vec![0; length(&full)?];
    reader.read_exact(&mut body)?;
    payload(&body).map_err(Into::into)
}
fn payload(body: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let mut r = PayloadReader {
        bytes: body,
        offset: 0,
    };
    let request_id = OperationId::from_bytes(r.read_array()?)?;
    let count = usize::from(r.read_u8()?);
    if count > MAXIMUM_TERMINAL_TITLE_ROWS {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut rows = Vec::with_capacity(count);
    for _ in 0..count {
        let identity = decode_host_identity(&mut r)?;
        let observation = match r.read_u8()? {
            0 => TerminalTitleObservation::Unavailable,
            1 => {
                let revision =
                    NonZeroU64::new(r.read_u64()?).ok_or(ProtocolError::InvalidPayload)?;
                let title = match r.read_u8()? {
                    0 => None,
                    1 => {
                        let n = r.read_u32()? as usize;
                        if n > MAXIMUM_TERMINAL_TITLE_BYTES {
                            return Err(ProtocolError::InvalidPayload);
                        }
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
        rows.push(TerminalTitleRow {
            identity,
            observation,
        });
    }
    r.finish()?;
    let value = TerminalTitlesResult { request_id, rows };
    value.validate()?;
    Ok(CapabilityNodeMessage::TerminalTitlesResult(value))
}
