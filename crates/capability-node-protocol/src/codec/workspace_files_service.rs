//! Carries the Workspace files service start as kind 45 and its answer as kind 46. It neither
//! starts a process nor reads a file.

use std::num::NonZeroU64;

use super::PayloadReader;
use crate::{
    OperationId, ProtocolError, WorkerGeneration, WorkerId, WorkspaceFilesService,
    WorkspaceFilesServiceOutcome, WorkspaceFilesServiceRequest, WorkspaceFilesServiceResult,
    WorkspaceFilesTicket,
};

pub(super) const REQUEST_KIND: u8 = 45;
pub(super) const RESULT_KIND: u8 = 46;

fn encode_body(bytes: &mut Vec<u8>, request: WorkspaceFilesServiceRequest) {
    bytes.extend(request.operation_id.as_bytes());
    bytes.extend(request.worker_id.as_bytes());
    bytes.extend(request.worker_generation.get().to_be_bytes());
    bytes.push(match request.service {
        WorkspaceFilesService::Operations => 1,
        WorkspaceFilesService::Watch => 2,
        WorkspaceFilesService::Search => 3,
    });
    bytes.extend(request.ticket.as_bytes());
}

fn decode_body(
    reader: &mut PayloadReader<'_>,
) -> Result<WorkspaceFilesServiceRequest, ProtocolError> {
    Ok(WorkspaceFilesServiceRequest {
        operation_id: OperationId::from_bytes(reader.read_array()?)?,
        worker_id: WorkerId::from_bytes(reader.read_array()?)?,
        worker_generation: WorkerGeneration::new(
            NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
        ),
        service: match reader.read_u8()? {
            1 => WorkspaceFilesService::Operations,
            2 => WorkspaceFilesService::Watch,
            3 => WorkspaceFilesService::Search,
            _ => return Err(ProtocolError::InvalidPayload),
        },
        ticket: WorkspaceFilesTicket::from_bytes(reader.read_array()?)?,
    })
}

pub(super) fn encode_request(request: WorkspaceFilesServiceRequest) -> Vec<u8> {
    let mut bytes = vec![REQUEST_KIND];
    encode_body(&mut bytes, request);
    bytes
}

pub(super) fn decode_request(
    payload: &[u8],
) -> Result<WorkspaceFilesServiceRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(payload, REQUEST_KIND)?;
    let request = decode_body(&mut reader)?;
    reader.finish()?;
    Ok(request)
}

pub(super) fn encode_result(result: WorkspaceFilesServiceResult) -> Vec<u8> {
    let mut bytes = vec![RESULT_KIND];
    encode_body(&mut bytes, result.request);
    bytes.push(match result.outcome {
        WorkspaceFilesServiceOutcome::Started => 1,
        WorkspaceFilesServiceOutcome::NotStarted => 2,
        WorkspaceFilesServiceOutcome::WorkerUnavailable => 3,
    });
    bytes
}

pub(super) fn decode_result(payload: &[u8]) -> Result<WorkspaceFilesServiceResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(payload, RESULT_KIND)?;
    let request = decode_body(&mut reader)?;
    let outcome = match reader.read_u8()? {
        1 => WorkspaceFilesServiceOutcome::Started,
        2 => WorkspaceFilesServiceOutcome::NotStarted,
        3 => WorkspaceFilesServiceOutcome::WorkerUnavailable,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(WorkspaceFilesServiceResult { request, outcome })
}
