//! Encodes Agent probe job actions and outcomes with closed tags. Command names and results are
//! counted lists; the lookup, its deadlines, and cancellation settlement remain with the Worker.

use std::num::NonZeroU64;

use super::PayloadReader;
use crate::{
    OperationId, ProtocolError, WorkerGeneration, WorkerId,
    agent_probe_job::{
        AgentProbeJobAction, AgentProbeJobCancelOutcome, AgentProbeJobOutcome,
        AgentProbeJobPollOutcome, AgentProbeJobRequest, AgentProbeJobResult,
        AgentProbeJobStartOutcome, AgentProbeResult,
    },
};

pub(super) const REQUEST_KIND: u8 = 37;
pub(super) const RESULT_KIND: u8 = 38;
const ACTION_START: u8 = 1;
const ACTION_POLL: u8 = 2;
const ACTION_CANCEL: u8 = 3;

const START_ACCEPTED: u8 = 1;
const START_BUSY: u8 = 2;
const START_UNSUPPORTED: u8 = 3;
const START_SPAWN_THREAD_FAILED: u8 = 4;
const START_DEAD: u8 = 5;

const POLL_PENDING: u8 = 1;
const POLL_COMPLETED: u8 = 2;
const POLL_ABANDONED: u8 = 3;

const CANCEL_REGISTERED: u8 = 1;
const CANCEL_ALREADY_TERMINAL: u8 = 2;

const RESULT_FOUND: u8 = 1;
const RESULT_MISSING: u8 = 2;
const RESULT_UNAVAILABLE: u8 = 3;

const UNKNOWN: u8 = 10;
const REJECTED_FENCING: u8 = 12;

// Counts and lengths fit one byte: the action and outcome constructors bound both at 64.
pub(super) fn encode_request(request: &AgentProbeJobRequest) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(64);
    bytes.push(REQUEST_KIND);
    bytes.extend_from_slice(&request.operation_id.as_bytes());
    bytes.extend_from_slice(&request.worker_id.as_bytes());
    bytes.extend_from_slice(&request.worker_generation.get().to_be_bytes());
    match &request.action {
        AgentProbeJobAction::Start { commands } => {
            bytes.push(ACTION_START);
            bytes.push(commands.len() as u8);
            for command in commands {
                bytes.push(command.len() as u8);
                bytes.extend_from_slice(command.as_bytes());
            }
        }
        AgentProbeJobAction::Poll { job_id } => {
            bytes.push(ACTION_POLL);
            bytes.extend_from_slice(&job_id.as_bytes());
        }
        AgentProbeJobAction::Cancel { job_id } => {
            bytes.push(ACTION_CANCEL);
            bytes.extend_from_slice(&job_id.as_bytes());
        }
    }
    bytes
}

pub(super) fn decode_request(bytes: &[u8]) -> Result<AgentProbeJobRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, REQUEST_KIND)?;
    let operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let worker_id = WorkerId::from_bytes(reader.read_array()?)?;
    let worker_generation = WorkerGeneration::new(
        NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
    );
    let action = match reader.read_u8()? {
        ACTION_START => {
            let count = usize::from(reader.read_u8()?);
            let mut commands = Vec::with_capacity(count);
            for _ in 0..count {
                let length = usize::from(reader.read_u8()?);
                let command = std::str::from_utf8(reader.read_slice(length)?)
                    .map_err(|_| ProtocolError::InvalidPayload)?;
                commands.push(command.to_owned());
            }
            AgentProbeJobAction::start(commands)?
        }
        ACTION_POLL => AgentProbeJobAction::Poll {
            job_id: OperationId::from_bytes(reader.read_array()?)?,
        },
        ACTION_CANCEL => AgentProbeJobAction::Cancel {
            job_id: OperationId::from_bytes(reader.read_array()?)?,
        },
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(AgentProbeJobRequest {
        operation_id,
        worker_id,
        worker_generation,
        action,
    })
}

pub(super) fn encode_result(result: &AgentProbeJobResult) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(64);
    bytes.push(RESULT_KIND);
    bytes.extend_from_slice(&result.operation_id.as_bytes());
    bytes.push(match result.outcome {
        AgentProbeJobOutcome::Start(_) => ACTION_START,
        AgentProbeJobOutcome::Poll(_) => ACTION_POLL,
        AgentProbeJobOutcome::Cancel(_) => ACTION_CANCEL,
    });
    bytes.extend_from_slice(&result.job_id.as_bytes());
    match &result.outcome {
        AgentProbeJobOutcome::Start(outcome) => bytes.push(match outcome {
            AgentProbeJobStartOutcome::Accepted => START_ACCEPTED,
            AgentProbeJobStartOutcome::Busy => START_BUSY,
            AgentProbeJobStartOutcome::Unsupported => START_UNSUPPORTED,
            AgentProbeJobStartOutcome::SpawnThreadFailed => START_SPAWN_THREAD_FAILED,
            AgentProbeJobStartOutcome::Dead => START_DEAD,
            AgentProbeJobStartOutcome::RejectedFencing => REJECTED_FENCING,
        }),
        AgentProbeJobOutcome::Poll(outcome) => match outcome {
            AgentProbeJobPollOutcome::Pending => bytes.push(POLL_PENDING),
            AgentProbeJobPollOutcome::Completed { results } => {
                bytes.push(POLL_COMPLETED);
                bytes.push(results.len() as u8);
                bytes.extend(results.iter().map(|result| match result {
                    AgentProbeResult::Found => RESULT_FOUND,
                    AgentProbeResult::Missing => RESULT_MISSING,
                    AgentProbeResult::Unavailable => RESULT_UNAVAILABLE,
                }));
            }
            AgentProbeJobPollOutcome::Abandoned => bytes.push(POLL_ABANDONED),
            AgentProbeJobPollOutcome::Unknown => bytes.push(UNKNOWN),
            AgentProbeJobPollOutcome::RejectedFencing => bytes.push(REJECTED_FENCING),
        },
        AgentProbeJobOutcome::Cancel(outcome) => bytes.push(match outcome {
            AgentProbeJobCancelOutcome::Registered => CANCEL_REGISTERED,
            AgentProbeJobCancelOutcome::AlreadyTerminal => CANCEL_ALREADY_TERMINAL,
            AgentProbeJobCancelOutcome::Unknown => UNKNOWN,
            AgentProbeJobCancelOutcome::RejectedFencing => REJECTED_FENCING,
        }),
    }
    bytes
}

pub(super) fn decode_result(bytes: &[u8]) -> Result<AgentProbeJobResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, RESULT_KIND)?;
    let operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let action = reader.read_u8()?;
    let job_id = OperationId::from_bytes(reader.read_array()?)?;
    let outcome = match action {
        ACTION_START => AgentProbeJobOutcome::Start(match reader.read_u8()? {
            START_ACCEPTED => AgentProbeJobStartOutcome::Accepted,
            START_BUSY => AgentProbeJobStartOutcome::Busy,
            START_UNSUPPORTED => AgentProbeJobStartOutcome::Unsupported,
            START_SPAWN_THREAD_FAILED => AgentProbeJobStartOutcome::SpawnThreadFailed,
            START_DEAD => AgentProbeJobStartOutcome::Dead,
            REJECTED_FENCING => AgentProbeJobStartOutcome::RejectedFencing,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        ACTION_POLL => AgentProbeJobOutcome::Poll(match reader.read_u8()? {
            POLL_PENDING => AgentProbeJobPollOutcome::Pending,
            POLL_COMPLETED => {
                let count = usize::from(reader.read_u8()?);
                let results = reader
                    .read_slice(count)?
                    .iter()
                    .map(|byte| match *byte {
                        RESULT_FOUND => Ok(AgentProbeResult::Found),
                        RESULT_MISSING => Ok(AgentProbeResult::Missing),
                        RESULT_UNAVAILABLE => Ok(AgentProbeResult::Unavailable),
                        _ => Err(ProtocolError::InvalidPayload),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                AgentProbeJobPollOutcome::completed(results)?
            }
            POLL_ABANDONED => AgentProbeJobPollOutcome::Abandoned,
            UNKNOWN => AgentProbeJobPollOutcome::Unknown,
            REJECTED_FENCING => AgentProbeJobPollOutcome::RejectedFencing,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        ACTION_CANCEL => AgentProbeJobOutcome::Cancel(match reader.read_u8()? {
            CANCEL_REGISTERED => AgentProbeJobCancelOutcome::Registered,
            CANCEL_ALREADY_TERMINAL => AgentProbeJobCancelOutcome::AlreadyTerminal,
            UNKNOWN => AgentProbeJobCancelOutcome::Unknown,
            REJECTED_FENCING => AgentProbeJobCancelOutcome::RejectedFencing,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(AgentProbeJobResult {
        operation_id,
        job_id,
        outcome,
    })
}
