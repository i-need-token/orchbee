//! Encodes `orchbee-files` starts and ends with closed tags: a start carries its endpoint as a
//! length-prefixed address and its 32-byte token, and a started process or an end names the
//! process by its 32-bit ID and 64-bit start. Spawning, verification and reaping stay with the
//! Worker.

use std::num::NonZeroU64;

use super::PayloadReader;
use crate::{
    OperationId, ProtocolError, WorkerGeneration, WorkerId,
    files_process::{
        FilesEndOutcome, FilesEndpoint, FilesProcessAction, FilesProcessIdentity,
        FilesProcessOutcome, FilesProcessRequest, FilesProcessResult, FilesStartOutcome,
        FilesToken,
    },
};

pub(super) const REQUEST_KIND: u8 = 45;
pub(super) const RESULT_KIND: u8 = 46;
const ACTION_START: u8 = 1;
const ACTION_END: u8 = 2;

const STARTED: u8 = 1;
const MISSING: u8 = 2;
const UNVERIFIED: u8 = 3;
const FAILED: u8 = 4;
const ENDED: u8 = 5;
const UNKNOWN: u8 = 10;
const REJECTED_FENCING: u8 = 12;

pub(super) fn encode_request(request: &FilesProcessRequest) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(96);
    bytes.push(REQUEST_KIND);
    bytes.extend_from_slice(&request.operation_id.as_bytes());
    bytes.extend_from_slice(&request.worker_id.as_bytes());
    bytes.extend_from_slice(&request.worker_generation.get().to_be_bytes());
    match &request.action {
        FilesProcessAction::Start { endpoint, token } => {
            bytes.push(ACTION_START);
            // The constructor bounds the address at 512 bytes.
            let address = endpoint.as_str().as_bytes();
            bytes.extend_from_slice(&(address.len() as u16).to_be_bytes());
            bytes.extend_from_slice(address);
            bytes.extend_from_slice(token.as_bytes());
        }
        FilesProcessAction::End { process } => {
            bytes.push(ACTION_END);
            encode_identity(&mut bytes, *process);
        }
    }
    bytes
}

pub(super) fn decode_request(bytes: &[u8]) -> Result<FilesProcessRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, REQUEST_KIND)?;
    let operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let worker_id = WorkerId::from_bytes(reader.read_array()?)?;
    let worker_generation = WorkerGeneration::new(
        NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
    );
    let action = match reader.read_u8()? {
        ACTION_START => {
            let length = usize::from(reader.read_u16()?);
            let address = std::str::from_utf8(reader.read_slice(length)?)
                .map_err(|_| ProtocolError::InvalidPayload)?;
            FilesProcessAction::Start {
                endpoint: FilesEndpoint::new(address.to_owned())?,
                token: FilesToken::from_bytes(reader.read_array()?),
            }
        }
        ACTION_END => FilesProcessAction::End {
            process: decode_identity(&mut reader)?,
        },
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(FilesProcessRequest {
        operation_id,
        worker_id,
        worker_generation,
        action,
    })
}

pub(super) fn encode_result(result: &FilesProcessResult) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(32);
    bytes.push(RESULT_KIND);
    bytes.extend_from_slice(&result.operation_id.as_bytes());
    match result.outcome {
        FilesProcessOutcome::Start(outcome) => {
            bytes.push(ACTION_START);
            match outcome {
                FilesStartOutcome::Started(process) => {
                    bytes.push(STARTED);
                    encode_identity(&mut bytes, process);
                }
                FilesStartOutcome::Missing => bytes.push(MISSING),
                FilesStartOutcome::Unverified => bytes.push(UNVERIFIED),
                FilesStartOutcome::Failed => bytes.push(FAILED),
                FilesStartOutcome::RejectedFencing => bytes.push(REJECTED_FENCING),
            }
        }
        FilesProcessOutcome::End(outcome) => {
            bytes.push(ACTION_END);
            bytes.push(match outcome {
                FilesEndOutcome::Ended => ENDED,
                FilesEndOutcome::Unknown => UNKNOWN,
                FilesEndOutcome::Failed => FAILED,
                FilesEndOutcome::RejectedFencing => REJECTED_FENCING,
            });
        }
    }
    bytes
}

pub(super) fn decode_result(bytes: &[u8]) -> Result<FilesProcessResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, RESULT_KIND)?;
    let operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let outcome = match reader.read_u8()? {
        ACTION_START => FilesProcessOutcome::Start(match reader.read_u8()? {
            STARTED => FilesStartOutcome::Started(decode_identity(&mut reader)?),
            MISSING => FilesStartOutcome::Missing,
            UNVERIFIED => FilesStartOutcome::Unverified,
            FAILED => FilesStartOutcome::Failed,
            REJECTED_FENCING => FilesStartOutcome::RejectedFencing,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        ACTION_END => FilesProcessOutcome::End(match reader.read_u8()? {
            ENDED => FilesEndOutcome::Ended,
            UNKNOWN => FilesEndOutcome::Unknown,
            FAILED => FilesEndOutcome::Failed,
            REJECTED_FENCING => FilesEndOutcome::RejectedFencing,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(FilesProcessResult {
        operation_id,
        outcome,
    })
}

fn encode_identity(bytes: &mut Vec<u8>, process: FilesProcessIdentity) {
    bytes.extend_from_slice(&process.process_id.to_be_bytes());
    bytes.extend_from_slice(&process.start.to_be_bytes());
}

fn decode_identity(reader: &mut PayloadReader<'_>) -> Result<FilesProcessIdentity, ProtocolError> {
    Ok(FilesProcessIdentity {
        process_id: reader.read_u32()?,
        start: reader.read_u64()?,
    })
}
