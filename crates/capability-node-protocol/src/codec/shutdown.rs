//! Carries Worker shutdown budgets and closed stage, refusal, and failure outcomes in the
//! binary protocol. It does not stop admission, dispose Hosts, or own a shutdown deadline.

use super::PayloadReader;
use crate::{
    HostEpoch, OperationId, ProtocolError, ShutdownBudget, ShutdownFailure, ShutdownRefusal,
    ShutdownStage, WorkerGeneration, WorkerId, WorkerShutdownOutcome, WorkerShutdownRequest,
    WorkerShutdownResult,
};
use std::num::NonZeroU64;

fn encode_body(bytes: &mut Vec<u8>, request: WorkerShutdownRequest) {
    bytes.extend(request.operation_id.as_bytes());
    bytes.extend(request.worker_id.as_bytes());
    bytes.extend(request.worker_generation.get().to_be_bytes());
    bytes.extend(request.budget.milliseconds().to_be_bytes());
}

fn decode_body(reader: &mut PayloadReader<'_>) -> Result<WorkerShutdownRequest, ProtocolError> {
    Ok(WorkerShutdownRequest {
        operation_id: OperationId::from_bytes(reader.read_array()?)?,
        worker_id: WorkerId::from_bytes(reader.read_array()?)?,
        worker_generation: WorkerGeneration::new(
            NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
        ),
        budget: ShutdownBudget::new(u32::from_be_bytes(reader.read_array()?))?,
    })
}

pub(super) fn encode_request(request: WorkerShutdownRequest) -> Vec<u8> {
    let mut bytes = vec![19];
    encode_body(&mut bytes, request);
    bytes
}

pub(super) fn decode_request(bytes: &[u8]) -> Result<WorkerShutdownRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 19)?;
    let request = decode_body(&mut reader)?;
    reader.finish()?;
    Ok(request)
}

pub(super) fn encode_result(result: WorkerShutdownResult) -> Vec<u8> {
    let mut bytes = vec![20];
    encode_body(&mut bytes, result.request);
    match result.outcome {
        WorkerShutdownOutcome::NoHost => bytes.push(1),
        WorkerShutdownOutcome::Stopped {
            host_epoch,
            disposed_terminals,
        } => {
            bytes.push(2);
            bytes.extend(host_epoch.as_bytes());
            bytes.extend(disposed_terminals.to_be_bytes());
        }
        WorkerShutdownOutcome::Refused(reason) => {
            bytes.push(3);
            bytes.push(match reason {
                ShutdownRefusal::WrongWorker => 1,
                ShutdownRefusal::Unmanaged => 2,
            });
        }
        WorkerShutdownOutcome::Incomplete {
            host_epoch,
            disposed_terminals,
            stage,
            failure,
        } => {
            bytes.push(4);
            bytes.push(u8::from(host_epoch.is_some()));
            if let Some(epoch) = host_epoch {
                bytes.extend(epoch.as_bytes());
            }
            bytes.extend(disposed_terminals.to_be_bytes());
            bytes.push(match stage {
                ShutdownStage::BackgroundJobs => 1,
                ShutdownStage::Inventory => 2,
                ShutdownStage::Force => 3,
                ShutdownStage::Dispose => 4,
                ShutdownStage::HostExit => 5,
            });
            bytes.push(match failure {
                ShutdownFailure::Deadline => 1,
                ShutdownFailure::Transport => 2,
                ShutdownFailure::Protocol => 3,
                ShutdownFailure::HostRefused => 4,
                ShutdownFailure::ProcessWait => 5,
            });
        }
    }
    bytes
}

pub(super) fn decode_result(bytes: &[u8]) -> Result<WorkerShutdownResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 20)?;
    let request = decode_body(&mut reader)?;
    let outcome = match reader.read_u8()? {
        1 => WorkerShutdownOutcome::NoHost,
        2 => WorkerShutdownOutcome::Stopped {
            host_epoch: HostEpoch::from_bytes(reader.read_array()?)?,
            disposed_terminals: u32::from_be_bytes(reader.read_array()?),
        },
        3 => WorkerShutdownOutcome::Refused(match reader.read_u8()? {
            1 => ShutdownRefusal::WrongWorker,
            2 => ShutdownRefusal::Unmanaged,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        4 => {
            let host_epoch = match reader.read_u8()? {
                0 => None,
                1 => Some(HostEpoch::from_bytes(reader.read_array()?)?),
                _ => return Err(ProtocolError::InvalidPayload),
            };
            let disposed_terminals = u32::from_be_bytes(reader.read_array()?);
            let stage = match reader.read_u8()? {
                1 => ShutdownStage::BackgroundJobs,
                2 => ShutdownStage::Inventory,
                3 => ShutdownStage::Force,
                4 => ShutdownStage::Dispose,
                5 => ShutdownStage::HostExit,
                _ => return Err(ProtocolError::InvalidPayload),
            };
            let failure = match reader.read_u8()? {
                1 => ShutdownFailure::Deadline,
                2 => ShutdownFailure::Transport,
                3 => ShutdownFailure::Protocol,
                4 => ShutdownFailure::HostRefused,
                5 => ShutdownFailure::ProcessWait,
                _ => return Err(ProtocolError::InvalidPayload),
            };
            WorkerShutdownOutcome::Incomplete {
                host_epoch,
                disposed_terminals,
                stage,
                failure,
            }
        }
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(WorkerShutdownResult { request, outcome })
}
