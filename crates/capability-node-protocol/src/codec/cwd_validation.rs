//! Maps working-directory validation requests and closed outcomes to exact binary fields. It
//! carries the Worker generation and literal path without inspecting the filesystem or
//! deciding whether the Worker is current.

use std::num::NonZeroU64;

use super::PayloadReader;
use crate::{
    AbsoluteCwd, OperationId, ProtocolError, TerminalCwdValidationOutcome,
    TerminalCwdValidationRequest, TerminalCwdValidationResult, WorkerGeneration, WorkerId,
};

const REQUEST_KIND: u8 = 11;
const RESULT_KIND: u8 = 12;

pub(super) fn encode_request(request: &TerminalCwdValidationRequest) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(64);
    bytes.push(REQUEST_KIND);
    encode_request_body(&mut bytes, request);
    bytes
}

pub(super) fn decode_request(bytes: &[u8]) -> Result<TerminalCwdValidationRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, REQUEST_KIND)?;
    let request = decode_request_body(&mut reader)?;
    reader.finish()?;
    Ok(request)
}

pub(super) fn encode_result(result: &TerminalCwdValidationResult) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(64);
    bytes.push(RESULT_KIND);
    encode_request_body(&mut bytes, &result.request);
    bytes.push(match result.outcome {
        TerminalCwdValidationOutcome::DirectoryAvailable => 1,
        TerminalCwdValidationOutcome::DirectoryMissing => 2,
        TerminalCwdValidationOutcome::WorkerUnavailable => 3,
        TerminalCwdValidationOutcome::NotADirectory => 4,
        TerminalCwdValidationOutcome::DirectoryInaccessible => 5,
    });
    bytes
}

pub(super) fn decode_result(bytes: &[u8]) -> Result<TerminalCwdValidationResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, RESULT_KIND)?;
    let request = decode_request_body(&mut reader)?;
    let outcome = match reader.read_u8()? {
        1 => TerminalCwdValidationOutcome::DirectoryAvailable,
        2 => TerminalCwdValidationOutcome::DirectoryMissing,
        3 => TerminalCwdValidationOutcome::WorkerUnavailable,
        4 => TerminalCwdValidationOutcome::NotADirectory,
        5 => TerminalCwdValidationOutcome::DirectoryInaccessible,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(TerminalCwdValidationResult { request, outcome })
}

fn encode_request_body(bytes: &mut Vec<u8>, request: &TerminalCwdValidationRequest) {
    bytes.extend(request.operation_id.as_bytes());
    bytes.extend(request.worker_id.as_bytes());
    bytes.extend(request.worker_generation.get().to_be_bytes());
    bytes.extend(request.cwd.encoded_length().to_be_bytes());
    bytes.extend(request.cwd.as_str().as_bytes());
}

fn decode_request_body(
    reader: &mut PayloadReader<'_>,
) -> Result<TerminalCwdValidationRequest, ProtocolError> {
    let operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let worker_id = WorkerId::from_bytes(reader.read_array()?)?;
    let worker_generation = WorkerGeneration::new(
        NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
    );
    let cwd_length = usize::from(reader.read_u16()?);
    let cwd = std::str::from_utf8(reader.read_slice(cwd_length)?)
        .map_err(|_error| ProtocolError::InvalidCwd)
        .and_then(AbsoluteCwd::new)?;
    Ok(TerminalCwdValidationRequest {
        operation_id,
        worker_id,
        worker_generation,
        cwd,
    })
}
