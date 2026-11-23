//! Carries a move turn as kind 43 and its answer as kind 44. It neither picks nor moves a
//! terminal.

use std::num::NonZeroU64;

use super::{PayloadReader, decode_host_identity, encode_host_identity};
use crate::{
    OperationId, ProtocolError, TerminalId, TerminalMoveTurnOutcome, TerminalMoveTurnRequest,
    TerminalMoveTurnResult, WorkerGeneration, WorkerId,
};

pub(super) const REQUEST_KIND: u8 = 43;
pub(super) const RESULT_KIND: u8 = 44;

fn encode_body(bytes: &mut Vec<u8>, request: TerminalMoveTurnRequest) {
    bytes.extend(request.request_id.as_bytes());
    bytes.extend(request.worker_id.as_bytes());
    bytes.extend(request.worker_generation.get().to_be_bytes());
}

fn decode_body(reader: &mut PayloadReader<'_>) -> Result<TerminalMoveTurnRequest, ProtocolError> {
    Ok(TerminalMoveTurnRequest {
        request_id: OperationId::from_bytes(reader.read_array()?)?,
        worker_id: WorkerId::from_bytes(reader.read_array()?)?,
        worker_generation: WorkerGeneration::new(
            NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
        ),
    })
}

pub(super) fn encode_request(request: TerminalMoveTurnRequest) -> Vec<u8> {
    let mut bytes = vec![REQUEST_KIND];
    encode_body(&mut bytes, request);
    bytes
}

pub(super) fn decode_request(payload: &[u8]) -> Result<TerminalMoveTurnRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(payload, REQUEST_KIND)?;
    let request = decode_body(&mut reader)?;
    reader.finish()?;
    Ok(request)
}

pub(super) fn encode_result(result: TerminalMoveTurnResult) -> Vec<u8> {
    let mut bytes = vec![RESULT_KIND];
    encode_body(&mut bytes, result.request);
    match result.outcome {
        TerminalMoveTurnOutcome::Moved {
            terminal_id,
            from,
            to,
        } => {
            bytes.push(1);
            bytes.extend(terminal_id.as_bytes());
            encode_host_identity(&mut bytes, from);
            encode_host_identity(&mut bytes, to);
        }
        TerminalMoveTurnOutcome::Waiting => bytes.push(2),
        TerminalMoveTurnOutcome::Done => bytes.push(3),
    }
    bytes
}

pub(super) fn decode_result(payload: &[u8]) -> Result<TerminalMoveTurnResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(payload, RESULT_KIND)?;
    let request = decode_body(&mut reader)?;
    let outcome = match reader.read_u8()? {
        1 => {
            let terminal_id = TerminalId::from_bytes(reader.read_array()?)?;
            let from = decode_host_identity(&mut reader)?;
            let to = decode_host_identity(&mut reader)?;
            // A move keeps the creation and changes the Host.
            if from.creation_id() != to.creation_id() || from.host_epoch() == to.host_epoch() {
                return Err(ProtocolError::InvalidPayload);
            }
            TerminalMoveTurnOutcome::Moved {
                terminal_id,
                from,
                to,
            }
        }
        2 => TerminalMoveTurnOutcome::Waiting,
        3 => TerminalMoveTurnOutcome::Done,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(TerminalMoveTurnResult { request, outcome })
}
