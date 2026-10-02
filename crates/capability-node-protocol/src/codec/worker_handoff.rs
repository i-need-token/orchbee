//! Closed fixed-size handoff records, separate from ordinary Worker enrollment.
use super::PayloadReader;
use crate::{
    OperationId, ProtocolError, WorkerGeneration, WorkerHandoffAction, WorkerHandoffId,
    WorkerHandoffOutcome, WorkerHandoffPeer, WorkerHandoffRefusal, WorkerHandoffRequest,
    WorkerHandoffResult, WorkerHandoffTarget, WorkerId,
};
use std::num::NonZeroU64;

fn encode_peer(bytes: &mut Vec<u8>, peer: WorkerHandoffPeer) {
    bytes.extend(peer.worker_id.as_bytes());
    bytes.extend(peer.worker_generation.get().to_be_bytes());
    bytes.extend(peer.target.handoff_id.as_bytes());
    bytes.extend(peer.target.cohort_sha256);
}

fn decode_peer(reader: &mut PayloadReader<'_>) -> Result<WorkerHandoffPeer, ProtocolError> {
    Ok(WorkerHandoffPeer {
        worker_id: WorkerId::from_bytes(reader.read_array()?)?,
        worker_generation: WorkerGeneration::new(
            NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
        ),
        target: WorkerHandoffTarget {
            handoff_id: WorkerHandoffId::from_bytes(reader.read_array()?)?,
            cohort_sha256: reader.read_array()?,
        },
    })
}

fn encode_request_body(bytes: &mut Vec<u8>, request: WorkerHandoffRequest) {
    bytes.extend(request.operation_id.as_bytes());
    encode_peer(bytes, request.peer());
    bytes.push(match request.action {
        WorkerHandoffAction::Prepare => 1,
        WorkerHandoffAction::Commit => 2,
        WorkerHandoffAction::Abort => 3,
    });
}

fn decode_request_body(
    reader: &mut PayloadReader<'_>,
) -> Result<WorkerHandoffRequest, ProtocolError> {
    let operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let peer = decode_peer(reader)?;
    let action = match reader.read_u8()? {
        1 => WorkerHandoffAction::Prepare,
        2 => WorkerHandoffAction::Commit,
        3 => WorkerHandoffAction::Abort,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    Ok(WorkerHandoffRequest {
        operation_id,
        worker_id: peer.worker_id,
        worker_generation: peer.worker_generation,
        target: peer.target,
        action,
    })
}

pub(super) fn request(value: WorkerHandoffRequest) -> Vec<u8> {
    let mut bytes = vec![26];
    encode_request_body(&mut bytes, value);
    bytes
}

pub(super) fn read_request(bytes: &[u8]) -> Result<WorkerHandoffRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 26)?;
    let value = decode_request_body(&mut reader)?;
    reader.finish()?;
    Ok(value)
}

pub(super) fn result(value: WorkerHandoffResult) -> Vec<u8> {
    let mut bytes = vec![27];
    encode_request_body(&mut bytes, value.request());
    match value.outcome() {
        WorkerHandoffOutcome::Prepared => bytes.push(1),
        WorkerHandoffOutcome::Committed => bytes.push(2),
        WorkerHandoffOutcome::Aborted => bytes.push(3),
        WorkerHandoffOutcome::Refused(reason) => {
            bytes.push(4);
            bytes.push(match reason {
                WorkerHandoffRefusal::WrongWorker => 1,
                WorkerHandoffRefusal::Unmanaged => 2,
                WorkerHandoffRefusal::Busy => 3,
                WorkerHandoffRefusal::TicketMismatch => 4,
                WorkerHandoffRefusal::NotPrepared => 5,
                WorkerHandoffRefusal::ReusedTicket => 6,
                WorkerHandoffRefusal::Capacity => 7,
                WorkerHandoffRefusal::TargetUnavailable => 8,
                WorkerHandoffRefusal::Authentication => 9,
                WorkerHandoffRefusal::AttachmentsBusy => 10,
            });
        }
    }
    bytes
}

pub(super) fn read_result(bytes: &[u8]) -> Result<WorkerHandoffResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 27)?;
    let request = decode_request_body(&mut reader)?;
    let outcome = match reader.read_u8()? {
        1 => WorkerHandoffOutcome::Prepared,
        2 => WorkerHandoffOutcome::Committed,
        3 => WorkerHandoffOutcome::Aborted,
        4 => WorkerHandoffOutcome::Refused(match reader.read_u8()? {
            1 => WorkerHandoffRefusal::WrongWorker,
            2 => WorkerHandoffRefusal::Unmanaged,
            3 => WorkerHandoffRefusal::Busy,
            4 => WorkerHandoffRefusal::TicketMismatch,
            5 => WorkerHandoffRefusal::NotPrepared,
            6 => WorkerHandoffRefusal::ReusedTicket,
            7 => WorkerHandoffRefusal::Capacity,
            8 => WorkerHandoffRefusal::TargetUnavailable,
            9 => WorkerHandoffRefusal::Authentication,
            10 => WorkerHandoffRefusal::AttachmentsBusy,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    WorkerHandoffResult::new(request, outcome)
}

pub(super) fn peer(kind: u8, value: WorkerHandoffPeer) -> Vec<u8> {
    let mut bytes = vec![kind];
    encode_peer(&mut bytes, value);
    bytes
}

pub(super) fn read_peer(kind: u8, bytes: &[u8]) -> Result<WorkerHandoffPeer, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, kind)?;
    let value = decode_peer(&mut reader)?;
    reader.finish()?;
    Ok(value)
}
