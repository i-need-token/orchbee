use std::num::NonZeroU64;

use super::PayloadReader;
use crate::{
    OperationId, ProtocolError, WorkerGeneration, WorkerId,
    worktree_job::{
        WorktreeJobAction, WorktreeJobActionKind, WorktreeJobCancelOutcome, WorktreeJobOutcome,
        WorktreeJobPollOutcome, WorktreeJobRequest, WorktreeJobResult, WorktreeJobStage,
        WorktreeJobStartOutcome,
    },
};

const REQUEST_KIND: u8 = 15;
const RESULT_KIND: u8 = 16;
const ACTION_START: u8 = 1;
const ACTION_POLL: u8 = 2;
const ACTION_CANCEL: u8 = 3;
const ACTION_START_SOURCE: u8 = 4;

const START_ACCEPTED: u8 = 1;
const START_BUSY: u8 = 2;
const START_INVALID_DIRECTORY: u8 = 3;
const START_SPAWN_THREAD_FAILED: u8 = 4;
const START_DEAD: u8 = 5;
const START_UNSUPPORTED: u8 = 6;

const POLL_PENDING: u8 = 1;
const POLL_COMPLETED: u8 = 2;
const POLL_GIT_FAILED: u8 = 3;
const POLL_SPAWN_UNAVAILABLE: u8 = 4;
const POLL_SPAWN_FAILED: u8 = 5;
const POLL_TOO_LARGE: u8 = 6;
const POLL_TIMEOUT: u8 = 7;
const POLL_CANCELLED: u8 = 8;
const POLL_OWNER_IO: u8 = 9;
const POLL_UNKNOWN: u8 = 10;
const POLL_PHYSICAL_UNKNOWN: u8 = 11;
const POLL_SOURCE_COMPLETED: u8 = 13;

const CANCEL_REGISTERED: u8 = 1;
const CANCEL_ALREADY_TERMINAL: u8 = 2;

const REJECTED_FENCING: u8 = 12;

pub(super) fn encode_request(request: &WorktreeJobRequest) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(64);
    bytes.push(REQUEST_KIND);
    bytes.extend_from_slice(&request.operation_id.as_bytes());
    bytes.extend_from_slice(&request.worker_id.as_bytes());
    bytes.extend_from_slice(&request.worker_generation.get().to_be_bytes());
    match &request.action {
        WorktreeJobAction::StartSource {
            directory,
            trust_repository,
        } => {
            bytes.push(ACTION_START_SOURCE);
            bytes.push(u8::from(*trust_repository));
            bytes.extend_from_slice(&(directory.len() as u16).to_be_bytes());
            bytes.extend_from_slice(directory);
        }
        WorktreeJobAction::Start { directory } => {
            bytes.push(ACTION_START);
            bytes.extend_from_slice(&(directory.len() as u16).to_be_bytes());
            bytes.extend_from_slice(directory);
        }
        WorktreeJobAction::Poll { job_id } => {
            bytes.push(ACTION_POLL);
            bytes.extend_from_slice(&job_id.as_bytes());
        }
        WorktreeJobAction::Cancel { job_id } => {
            bytes.push(ACTION_CANCEL);
            bytes.extend_from_slice(&job_id.as_bytes());
        }
    }
    bytes
}

pub(super) fn decode_request(bytes: &[u8]) -> Result<WorktreeJobRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, REQUEST_KIND)?;
    let operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let worker_id = WorkerId::from_bytes(reader.read_array()?)?;
    let worker_generation = WorkerGeneration::new(
        NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
    );
    let action = match reader.read_u8()? {
        ACTION_START_SOURCE => {
            let trust = match reader.read_u8()? {
                0 => false,
                1 => true,
                _ => return Err(ProtocolError::InvalidPayload),
            };
            let length = usize::from(reader.read_u16()?);
            WorktreeJobAction::start_source(reader.read_slice(length)?.to_vec(), trust)?
        }
        ACTION_START => {
            let length = usize::from(reader.read_u16()?);
            WorktreeJobAction::start(reader.read_slice(length)?.to_vec())?
        }
        ACTION_POLL => WorktreeJobAction::Poll {
            job_id: OperationId::from_bytes(reader.read_array()?)?,
        },
        ACTION_CANCEL => WorktreeJobAction::Cancel {
            job_id: OperationId::from_bytes(reader.read_array()?)?,
        },
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(WorktreeJobRequest {
        operation_id,
        worker_id,
        worker_generation,
        action,
    })
}

pub(super) fn encode_result(result: &WorktreeJobResult) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(64);
    bytes.push(RESULT_KIND);
    bytes.extend_from_slice(&result.operation_id.as_bytes());
    bytes.push(match result.outcome.action_kind() {
        WorktreeJobActionKind::Start => ACTION_START,
        WorktreeJobActionKind::Poll => ACTION_POLL,
        WorktreeJobActionKind::Cancel => ACTION_CANCEL,
    });
    bytes.extend_from_slice(&result.job_id.as_bytes());
    match &result.outcome {
        WorktreeJobOutcome::Start(outcome) => bytes.push(match outcome {
            WorktreeJobStartOutcome::Accepted => START_ACCEPTED,
            WorktreeJobStartOutcome::Busy => START_BUSY,
            WorktreeJobStartOutcome::InvalidDirectory => START_INVALID_DIRECTORY,
            WorktreeJobStartOutcome::SpawnThreadFailed => START_SPAWN_THREAD_FAILED,
            WorktreeJobStartOutcome::Dead => START_DEAD,
            WorktreeJobStartOutcome::Unsupported => START_UNSUPPORTED,
            WorktreeJobStartOutcome::RejectedFencing => REJECTED_FENCING,
        }),
        WorktreeJobOutcome::Poll(outcome) => encode_poll_outcome(&mut bytes, outcome),
        WorktreeJobOutcome::Cancel(outcome) => bytes.push(match outcome {
            WorktreeJobCancelOutcome::Registered => CANCEL_REGISTERED,
            WorktreeJobCancelOutcome::AlreadyTerminal => CANCEL_ALREADY_TERMINAL,
            WorktreeJobCancelOutcome::Unknown => POLL_UNKNOWN,
            WorktreeJobCancelOutcome::PhysicalUnknown => POLL_PHYSICAL_UNKNOWN,
            WorktreeJobCancelOutcome::RejectedFencing => REJECTED_FENCING,
        }),
    }
    bytes
}

fn encode_poll_outcome(bytes: &mut Vec<u8>, outcome: &WorktreeJobPollOutcome) {
    match outcome {
        WorktreeJobPollOutcome::Pending => bytes.push(POLL_PENDING),
        WorktreeJobPollOutcome::SourceCompleted {
            common_dir,
            source_checkout,
            listing,
            stderr,
        } => {
            bytes.push(POLL_SOURCE_COMPLETED);
            for field in [common_dir, source_checkout, listing, stderr] {
                bytes.extend_from_slice(&(field.len() as u16).to_be_bytes());
                bytes.extend_from_slice(field);
            }
        }
        WorktreeJobPollOutcome::Completed {
            common_dir,
            listing,
            stderr,
        } => {
            bytes.push(POLL_COMPLETED);
            for field in [common_dir, listing, stderr] {
                bytes.extend_from_slice(&(field.len() as u16).to_be_bytes());
                bytes.extend_from_slice(field);
            }
        }
        WorktreeJobPollOutcome::GitFailed {
            stage,
            status,
            stderr,
        } => {
            bytes.push(POLL_GIT_FAILED);
            bytes.push(stage.code());
            bytes.push(u8::from(status.is_some()));
            if let Some(status) = status {
                bytes.extend_from_slice(&status.to_be_bytes());
            }
            bytes.extend_from_slice(&(stderr.len() as u16).to_be_bytes());
            bytes.extend_from_slice(stderr);
        }
        WorktreeJobPollOutcome::SpawnUnavailable { stage } => {
            bytes.push(POLL_SPAWN_UNAVAILABLE);
            bytes.push(stage.code());
        }
        WorktreeJobPollOutcome::SpawnFailed { stage } => {
            bytes.push(POLL_SPAWN_FAILED);
            bytes.push(stage.code());
        }
        WorktreeJobPollOutcome::TooLarge { stage } => {
            bytes.push(POLL_TOO_LARGE);
            bytes.push(stage.map_or(0, WorktreeJobStage::code));
        }
        WorktreeJobPollOutcome::Timeout => bytes.push(POLL_TIMEOUT),
        WorktreeJobPollOutcome::Cancelled => bytes.push(POLL_CANCELLED),
        WorktreeJobPollOutcome::OwnerIo { stage } => {
            bytes.push(POLL_OWNER_IO);
            bytes.push(stage.map_or(0, WorktreeJobStage::code));
        }
        WorktreeJobPollOutcome::Unknown => bytes.push(POLL_UNKNOWN),
        WorktreeJobPollOutcome::PhysicalUnknown => bytes.push(POLL_PHYSICAL_UNKNOWN),
        WorktreeJobPollOutcome::RejectedFencing => bytes.push(REJECTED_FENCING),
    }
}

pub(super) fn decode_result(bytes: &[u8]) -> Result<WorktreeJobResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, RESULT_KIND)?;
    let operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let action = reader.read_u8()?;
    let job_id = OperationId::from_bytes(reader.read_array()?)?;
    let outcome = match action {
        ACTION_START => WorktreeJobOutcome::Start(match reader.read_u8()? {
            START_ACCEPTED => WorktreeJobStartOutcome::Accepted,
            START_BUSY => WorktreeJobStartOutcome::Busy,
            START_INVALID_DIRECTORY => WorktreeJobStartOutcome::InvalidDirectory,
            START_SPAWN_THREAD_FAILED => WorktreeJobStartOutcome::SpawnThreadFailed,
            START_DEAD => WorktreeJobStartOutcome::Dead,
            START_UNSUPPORTED => WorktreeJobStartOutcome::Unsupported,
            REJECTED_FENCING => WorktreeJobStartOutcome::RejectedFencing,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        ACTION_POLL => WorktreeJobOutcome::Poll(decode_poll_outcome(&mut reader)?),
        ACTION_CANCEL => WorktreeJobOutcome::Cancel(match reader.read_u8()? {
            CANCEL_REGISTERED => WorktreeJobCancelOutcome::Registered,
            CANCEL_ALREADY_TERMINAL => WorktreeJobCancelOutcome::AlreadyTerminal,
            POLL_UNKNOWN => WorktreeJobCancelOutcome::Unknown,
            POLL_PHYSICAL_UNKNOWN => WorktreeJobCancelOutcome::PhysicalUnknown,
            REJECTED_FENCING => WorktreeJobCancelOutcome::RejectedFencing,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(WorktreeJobResult {
        operation_id,
        job_id,
        outcome,
    })
}

fn decode_poll_outcome(
    reader: &mut PayloadReader<'_>,
) -> Result<WorktreeJobPollOutcome, ProtocolError> {
    let outcome = match reader.read_u8()? {
        POLL_PENDING => WorktreeJobPollOutcome::Pending,
        POLL_SOURCE_COMPLETED => {
            let common_dir = read_field(reader)?.to_vec();
            let source_checkout = read_field(reader)?.to_vec();
            let listing = read_field(reader)?.to_vec();
            let stderr = read_field(reader)?.to_vec();
            WorktreeJobPollOutcome::source_completed(common_dir, source_checkout, listing, stderr)?
        }
        POLL_COMPLETED => {
            let common_dir = read_field(reader)?.to_vec();
            let listing = read_field(reader)?.to_vec();
            let stderr = read_field(reader)?.to_vec();
            WorktreeJobPollOutcome::completed(common_dir, listing, stderr)?
        }
        POLL_GIT_FAILED => {
            let stage = WorktreeJobStage::from_code(reader.read_u8()?)?;
            let status = match reader.read_u8()? {
                0 => None,
                1 => Some(reader.read_i32()?),
                _ => return Err(ProtocolError::InvalidPayload),
            };
            let stderr = read_field(reader)?.to_vec();
            WorktreeJobPollOutcome::git_failed(stage, status, stderr)?
        }
        POLL_SPAWN_UNAVAILABLE => WorktreeJobPollOutcome::SpawnUnavailable {
            stage: WorktreeJobStage::from_code(reader.read_u8()?)?,
        },
        POLL_SPAWN_FAILED => WorktreeJobPollOutcome::SpawnFailed {
            stage: WorktreeJobStage::from_code(reader.read_u8()?)?,
        },
        POLL_TOO_LARGE => WorktreeJobPollOutcome::TooLarge {
            stage: decode_optional_stage(reader)?,
        },
        POLL_TIMEOUT => WorktreeJobPollOutcome::Timeout,
        POLL_CANCELLED => WorktreeJobPollOutcome::Cancelled,
        POLL_OWNER_IO => WorktreeJobPollOutcome::OwnerIo {
            stage: decode_optional_stage(reader)?,
        },
        POLL_UNKNOWN => WorktreeJobPollOutcome::Unknown,
        POLL_PHYSICAL_UNKNOWN => WorktreeJobPollOutcome::PhysicalUnknown,
        REJECTED_FENCING => WorktreeJobPollOutcome::RejectedFencing,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    Ok(outcome)
}

fn decode_optional_stage(
    reader: &mut PayloadReader<'_>,
) -> Result<Option<WorktreeJobStage>, ProtocolError> {
    match reader.read_u8()? {
        0 => Ok(None),
        code => WorktreeJobStage::from_code(code).map(Some),
    }
}

fn read_field<'a>(reader: &mut PayloadReader<'a>) -> Result<&'a [u8], ProtocolError> {
    let length = usize::from(reader.read_u16()?);
    reader.read_slice(length)
}
