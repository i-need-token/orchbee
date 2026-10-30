//! Encodes Agent integration job actions and outcomes with closed tags. Operation requests and
//! reports stay opaque length-prefixed documents; their contract, the job thread, and
//! cancellation settlement remain with the Worker.

use std::num::NonZeroU64;

use super::PayloadReader;
use crate::{
    OperationId, ProtocolError, WorkerGeneration, WorkerId,
    agent_integration_job::{
        AgentIntegrationJobAction, AgentIntegrationJobCancelOutcome, AgentIntegrationJobOutcome,
        AgentIntegrationJobPollOutcome, AgentIntegrationJobRequest, AgentIntegrationJobResult,
        AgentIntegrationJobStartOutcome,
    },
};

pub(super) const REQUEST_KIND: u8 = 31;
pub(super) const RESULT_KIND: u8 = 32;
const ACTION_START: u8 = 1;
const ACTION_POLL: u8 = 2;
const ACTION_CANCEL: u8 = 3;

const START_ACCEPTED: u8 = 1;
const START_BUSY: u8 = 2;
const START_INVALID_REQUEST: u8 = 3;
const START_SPAWN_THREAD_FAILED: u8 = 4;
const START_DEAD: u8 = 5;

const POLL_PENDING: u8 = 1;
const POLL_COMPLETED: u8 = 2;
const POLL_ABANDONED: u8 = 3;

const CANCEL_REGISTERED: u8 = 1;
const CANCEL_ALREADY_TERMINAL: u8 = 2;

const UNKNOWN: u8 = 10;
const REJECTED_FENCING: u8 = 12;

pub(super) fn encode_request(request: &AgentIntegrationJobRequest) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(64);
    bytes.push(REQUEST_KIND);
    bytes.extend_from_slice(&request.operation_id.as_bytes());
    bytes.extend_from_slice(&request.worker_id.as_bytes());
    bytes.extend_from_slice(&request.worker_generation.get().to_be_bytes());
    match &request.action {
        AgentIntegrationJobAction::Start { request } => {
            bytes.push(ACTION_START);
            bytes.extend_from_slice(&(request.len() as u16).to_be_bytes());
            bytes.extend_from_slice(request);
        }
        AgentIntegrationJobAction::Poll { job_id } => {
            bytes.push(ACTION_POLL);
            bytes.extend_from_slice(&job_id.as_bytes());
        }
        AgentIntegrationJobAction::Cancel { job_id } => {
            bytes.push(ACTION_CANCEL);
            bytes.extend_from_slice(&job_id.as_bytes());
        }
    }
    bytes
}

pub(super) fn decode_request(bytes: &[u8]) -> Result<AgentIntegrationJobRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, REQUEST_KIND)?;
    let operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let worker_id = WorkerId::from_bytes(reader.read_array()?)?;
    let worker_generation = WorkerGeneration::new(
        NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
    );
    let action = match reader.read_u8()? {
        ACTION_START => {
            let length = usize::from(reader.read_u16()?);
            AgentIntegrationJobAction::start(reader.read_slice(length)?.to_vec())?
        }
        ACTION_POLL => AgentIntegrationJobAction::Poll {
            job_id: OperationId::from_bytes(reader.read_array()?)?,
        },
        ACTION_CANCEL => AgentIntegrationJobAction::Cancel {
            job_id: OperationId::from_bytes(reader.read_array()?)?,
        },
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(AgentIntegrationJobRequest {
        operation_id,
        worker_id,
        worker_generation,
        action,
    })
}

pub(super) fn encode_result(result: &AgentIntegrationJobResult) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(64);
    bytes.push(RESULT_KIND);
    bytes.extend_from_slice(&result.operation_id.as_bytes());
    bytes.push(match result.outcome {
        AgentIntegrationJobOutcome::Start(_) => ACTION_START,
        AgentIntegrationJobOutcome::Poll(_) => ACTION_POLL,
        AgentIntegrationJobOutcome::Cancel(_) => ACTION_CANCEL,
    });
    bytes.extend_from_slice(&result.job_id.as_bytes());
    match &result.outcome {
        AgentIntegrationJobOutcome::Start(outcome) => bytes.push(match outcome {
            AgentIntegrationJobStartOutcome::Accepted => START_ACCEPTED,
            AgentIntegrationJobStartOutcome::Busy => START_BUSY,
            AgentIntegrationJobStartOutcome::InvalidRequest => START_INVALID_REQUEST,
            AgentIntegrationJobStartOutcome::SpawnThreadFailed => START_SPAWN_THREAD_FAILED,
            AgentIntegrationJobStartOutcome::Dead => START_DEAD,
            AgentIntegrationJobStartOutcome::RejectedFencing => REJECTED_FENCING,
        }),
        AgentIntegrationJobOutcome::Poll(outcome) => match outcome {
            AgentIntegrationJobPollOutcome::Pending => bytes.push(POLL_PENDING),
            AgentIntegrationJobPollOutcome::Completed { report } => {
                bytes.push(POLL_COMPLETED);
                bytes.extend_from_slice(&(report.len() as u16).to_be_bytes());
                bytes.extend_from_slice(report);
            }
            AgentIntegrationJobPollOutcome::Abandoned => bytes.push(POLL_ABANDONED),
            AgentIntegrationJobPollOutcome::Unknown => bytes.push(UNKNOWN),
            AgentIntegrationJobPollOutcome::RejectedFencing => bytes.push(REJECTED_FENCING),
        },
        AgentIntegrationJobOutcome::Cancel(outcome) => bytes.push(match outcome {
            AgentIntegrationJobCancelOutcome::Registered => CANCEL_REGISTERED,
            AgentIntegrationJobCancelOutcome::AlreadyTerminal => CANCEL_ALREADY_TERMINAL,
            AgentIntegrationJobCancelOutcome::Unknown => UNKNOWN,
            AgentIntegrationJobCancelOutcome::RejectedFencing => REJECTED_FENCING,
        }),
    }
    bytes
}

pub(super) fn decode_result(bytes: &[u8]) -> Result<AgentIntegrationJobResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, RESULT_KIND)?;
    let operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let action = reader.read_u8()?;
    let job_id = OperationId::from_bytes(reader.read_array()?)?;
    let outcome = match action {
        ACTION_START => AgentIntegrationJobOutcome::Start(match reader.read_u8()? {
            START_ACCEPTED => AgentIntegrationJobStartOutcome::Accepted,
            START_BUSY => AgentIntegrationJobStartOutcome::Busy,
            START_INVALID_REQUEST => AgentIntegrationJobStartOutcome::InvalidRequest,
            START_SPAWN_THREAD_FAILED => AgentIntegrationJobStartOutcome::SpawnThreadFailed,
            START_DEAD => AgentIntegrationJobStartOutcome::Dead,
            REJECTED_FENCING => AgentIntegrationJobStartOutcome::RejectedFencing,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        ACTION_POLL => AgentIntegrationJobOutcome::Poll(match reader.read_u8()? {
            POLL_PENDING => AgentIntegrationJobPollOutcome::Pending,
            POLL_COMPLETED => {
                let length = usize::from(reader.read_u16()?);
                AgentIntegrationJobPollOutcome::completed(reader.read_slice(length)?.to_vec())?
            }
            POLL_ABANDONED => AgentIntegrationJobPollOutcome::Abandoned,
            UNKNOWN => AgentIntegrationJobPollOutcome::Unknown,
            REJECTED_FENCING => AgentIntegrationJobPollOutcome::RejectedFencing,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        ACTION_CANCEL => AgentIntegrationJobOutcome::Cancel(match reader.read_u8()? {
            CANCEL_REGISTERED => AgentIntegrationJobCancelOutcome::Registered,
            CANCEL_ALREADY_TERMINAL => AgentIntegrationJobCancelOutcome::AlreadyTerminal,
            UNKNOWN => AgentIntegrationJobCancelOutcome::Unknown,
            REJECTED_FENCING => AgentIntegrationJobCancelOutcome::RejectedFencing,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(AgentIntegrationJobResult {
        operation_id,
        job_id,
        outcome,
    })
}
