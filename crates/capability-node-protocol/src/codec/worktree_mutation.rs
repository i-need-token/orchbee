//! Serializes checked Git mutation operands and retained-job receipts, preserving typed
//! effect, failure, and cancellation outcomes. It performs no repository lookup, Git effect,
//! or recovery replay.

use super::PayloadReader;
use crate::{
    AbsoluteCwd, GitReference, OperationId, ProtocolError, WorkerGeneration, WorkerId,
    WorktreeCheckout, WorktreeCheckoutSelector, WorktreeMutation, WorktreeMutationCancelOutcome,
    WorktreeMutationCompletion, WorktreeMutationEffect, WorktreeMutationError,
    WorktreeMutationFailure, WorktreeMutationIntent, WorktreeMutationJobAction,
    WorktreeMutationOutcome, WorktreeMutationPollOutcome, WorktreeMutationRequest,
    WorktreeMutationResult, WorktreeMutationStage, WorktreeMutationStartOutcome,
};
use std::num::NonZeroU64;

fn field(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend(
        u16::try_from(value.len())
            .expect("checked field bound")
            .to_be_bytes(),
    );
    bytes.extend(value);
}

fn path(bytes: &mut Vec<u8>, value: &AbsoluteCwd) {
    field(bytes, value.as_str().as_bytes());
}

fn reference(bytes: &mut Vec<u8>, value: &GitReference) {
    field(bytes, value.as_str().as_bytes());
}

fn read_field<'a>(reader: &mut PayloadReader<'a>) -> Result<&'a [u8], ProtocolError> {
    let length = usize::from(reader.read_u16()?);
    reader.read_slice(length)
}

fn read_text<'a>(reader: &mut PayloadReader<'a>) -> Result<&'a str, ProtocolError> {
    std::str::from_utf8(read_field(reader)?).map_err(|_| ProtocolError::InvalidPayload)
}

fn read_path(reader: &mut PayloadReader<'_>) -> Result<AbsoluteCwd, ProtocolError> {
    AbsoluteCwd::new(read_text(reader)?)
}

fn read_reference(reader: &mut PayloadReader<'_>) -> Result<GitReference, ProtocolError> {
    GitReference::new(read_text(reader)?)
}

fn read_bool(reader: &mut PayloadReader<'_>) -> Result<bool, ProtocolError> {
    match reader.read_u8()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

pub(super) fn encode_request(request: &WorktreeMutationRequest) -> Vec<u8> {
    let mut bytes = vec![21];
    bytes.extend(request.operation_id.as_bytes());
    bytes.extend(request.worker_id.as_bytes());
    bytes.extend(request.worker_generation.get().to_be_bytes());
    match &request.action {
        WorktreeMutationJobAction::Start(intent) => {
            bytes.push(1);
            bytes.push(u8::from(intent.trust_repository()));
            bytes.push(match intent.mutation() {
                WorktreeMutation::Create { .. } => 1,
                WorktreeMutation::Open(_) => 2,
                WorktreeMutation::InspectRemove { .. } => 3,
                WorktreeMutation::Remove { .. } => 4,
                WorktreeMutation::InspectSource => 5,
            });
            path(&mut bytes, intent.source());
            match intent.mutation() {
                WorktreeMutation::InspectSource => {}
                WorktreeMutation::Create {
                    path: target,
                    branch,
                    base,
                } => {
                    path(&mut bytes, target);
                    reference(&mut bytes, branch);
                    reference(&mut bytes, base);
                }
                WorktreeMutation::Open(WorktreeCheckoutSelector::Path(target)) => {
                    bytes.push(1);
                    path(&mut bytes, target);
                }
                WorktreeMutation::Open(WorktreeCheckoutSelector::Branch(branch)) => {
                    bytes.push(2);
                    reference(&mut bytes, branch);
                }
                WorktreeMutation::InspectRemove { path: target } => path(&mut bytes, target),
                WorktreeMutation::Remove {
                    path: target,
                    expected_common_directory,
                    force,
                } => {
                    path(&mut bytes, target);
                    path(&mut bytes, expected_common_directory);
                    bytes.push(u8::from(*force));
                }
            }
        }
        WorktreeMutationJobAction::Poll { job_id } => {
            bytes.push(2);
            bytes.extend(job_id.as_bytes());
        }
        WorktreeMutationJobAction::Cancel { job_id } => {
            bytes.push(3);
            bytes.extend(job_id.as_bytes());
        }
    }
    bytes
}

pub(super) fn decode_request(bytes: &[u8]) -> Result<WorktreeMutationRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 21)?;
    let operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let worker_id = WorkerId::from_bytes(reader.read_array()?)?;
    let worker_generation = WorkerGeneration::new(
        NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
    );
    let action = match reader.read_u8()? {
        1 => WorktreeMutationJobAction::Start(read_intent(&mut reader)?),
        2 => WorktreeMutationJobAction::Poll {
            job_id: OperationId::from_bytes(reader.read_array()?)?,
        },
        3 => WorktreeMutationJobAction::Cancel {
            job_id: OperationId::from_bytes(reader.read_array()?)?,
        },
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(WorktreeMutationRequest {
        operation_id,
        worker_id,
        worker_generation,
        action,
    })
}

fn read_intent(reader: &mut PayloadReader<'_>) -> Result<WorktreeMutationIntent, ProtocolError> {
    let trust = read_bool(reader)?;
    let kind = reader.read_u8()?;
    let source = read_path(reader)?;
    let mutation = match kind {
        1 => WorktreeMutation::Create {
            path: read_path(reader)?,
            branch: read_reference(reader)?,
            base: read_reference(reader)?,
        },
        2 => WorktreeMutation::Open(match reader.read_u8()? {
            1 => WorktreeCheckoutSelector::Path(read_path(reader)?),
            2 => WorktreeCheckoutSelector::Branch(read_reference(reader)?),
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        3 => WorktreeMutation::InspectRemove {
            path: read_path(reader)?,
        },
        4 => WorktreeMutation::Remove {
            path: read_path(reader)?,
            expected_common_directory: read_path(reader)?,
            force: read_bool(reader)?,
        },
        5 => WorktreeMutation::InspectSource,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    WorktreeMutationIntent::new(source, trust, mutation)
}

pub(super) fn encode_result(result: &WorktreeMutationResult) -> Vec<u8> {
    let mut bytes = vec![22];
    bytes.extend(result.operation_id.as_bytes());
    bytes.extend(result.job_id.as_bytes());
    match &result.outcome {
        WorktreeMutationOutcome::Start(outcome) => bytes.extend([
            1,
            match outcome {
                WorktreeMutationStartOutcome::Accepted => 1,
                WorktreeMutationStartOutcome::Existing => 2,
                WorktreeMutationStartOutcome::Busy => 3,
                WorktreeMutationStartOutcome::Conflict => 4,
                WorktreeMutationStartOutcome::Full => 5,
                WorktreeMutationStartOutcome::SpawnFailed => 6,
                WorktreeMutationStartOutcome::PhysicalUnknown => 7,
                WorktreeMutationStartOutcome::Unsupported => 8,
                WorktreeMutationStartOutcome::RejectedFencing => 255,
            },
        ]),
        WorktreeMutationOutcome::Cancel(outcome) => bytes.extend([
            3,
            match outcome {
                WorktreeMutationCancelOutcome::Registered => 1,
                WorktreeMutationCancelOutcome::AlreadyTerminal => 2,
                WorktreeMutationCancelOutcome::Unknown => 3,
                WorktreeMutationCancelOutcome::PhysicalUnknown => 4,
                WorktreeMutationCancelOutcome::RejectedFencing => 255,
            },
        ]),
        WorktreeMutationOutcome::Poll(outcome) => {
            bytes.extend([
                2,
                match outcome {
                    WorktreeMutationPollOutcome::Pending => 1,
                    WorktreeMutationPollOutcome::Completed(_) => 2,
                    WorktreeMutationPollOutcome::Unknown => 3,
                    WorktreeMutationPollOutcome::PhysicalUnknown => 4,
                    WorktreeMutationPollOutcome::RejectedFencing => 255,
                },
            ]);
            if let WorktreeMutationPollOutcome::Completed(completion) = outcome {
                encode_completion(&mut bytes, completion);
            }
        }
    }
    bytes
}

fn encode_completion(bytes: &mut Vec<u8>, completion: &WorktreeMutationCompletion) {
    match completion {
        WorktreeMutationCompletion::Checkout(checkout) => {
            bytes.push(1);
            path(bytes, checkout.repository_root());
            path(bytes, checkout.common_directory());
            path(bytes, checkout.path());
            bytes.push(u8::from(checkout.branch().is_some()));
            if let Some(branch) = checkout.branch() {
                reference(bytes, branch);
            }
            bytes.push(u8::from(checkout.dirty()));
        }
        WorktreeMutationCompletion::Removed { path: target } => {
            bytes.push(2);
            path(bytes, target);
        }
        WorktreeMutationCompletion::Failed(failure) => {
            bytes.push(3);
            bytes.push(match failure.stage {
                WorktreeMutationStage::Probe => 1,
                WorktreeMutationStage::Select => 2,
                WorktreeMutationStage::CheckBranch => 3,
                WorktreeMutationStage::DirtyCheck => 4,
                WorktreeMutationStage::Mutate => 5,
                WorktreeMutationStage::Verify => 6,
            });
            bytes.push(match failure.effect {
                WorktreeMutationEffect::None => 0,
                WorktreeMutationEffect::MayHaveChanged => 1,
            });
            bytes.push(match failure.error {
                WorktreeMutationError::GitFailed(_) => 1,
                WorktreeMutationError::SpawnUnavailable => 2,
                WorktreeMutationError::SpawnFailed => 3,
                WorktreeMutationError::InvalidRepository => 4,
                WorktreeMutationError::CheckoutNotFound => 5,
                WorktreeMutationError::MainCheckout => 6,
                WorktreeMutationError::Dirty => 7,
                WorktreeMutationError::InvalidReference => 8,
                WorktreeMutationError::TooLarge => 9,
                WorktreeMutationError::Timeout => 10,
                WorktreeMutationError::Cancelled => 11,
                WorktreeMutationError::Io => 12,
                WorktreeMutationError::RepositoryChanged => 13,
                WorktreeMutationError::AmbiguousCheckout => 14,
            });
            if let WorktreeMutationError::GitFailed(status) = failure.error {
                bytes.push(u8::from(status.is_some()));
                if let Some(status) = status {
                    bytes.extend(status.to_be_bytes());
                }
            }
            field(bytes, failure.diagnostics());
        }
    }
}

pub(super) fn decode_result(bytes: &[u8]) -> Result<WorktreeMutationResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 22)?;
    let operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let job_id = OperationId::from_bytes(reader.read_array()?)?;
    let action = reader.read_u8()?;
    let outcome = reader.read_u8()?;
    let outcome = match action {
        1 => WorktreeMutationOutcome::Start(match outcome {
            1 => WorktreeMutationStartOutcome::Accepted,
            2 => WorktreeMutationStartOutcome::Existing,
            3 => WorktreeMutationStartOutcome::Busy,
            4 => WorktreeMutationStartOutcome::Conflict,
            5 => WorktreeMutationStartOutcome::Full,
            6 => WorktreeMutationStartOutcome::SpawnFailed,
            7 => WorktreeMutationStartOutcome::PhysicalUnknown,
            8 => WorktreeMutationStartOutcome::Unsupported,
            255 => WorktreeMutationStartOutcome::RejectedFencing,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        2 => WorktreeMutationOutcome::Poll(match outcome {
            1 => WorktreeMutationPollOutcome::Pending,
            2 => WorktreeMutationPollOutcome::Completed(read_completion(&mut reader)?),
            3 => WorktreeMutationPollOutcome::Unknown,
            4 => WorktreeMutationPollOutcome::PhysicalUnknown,
            255 => WorktreeMutationPollOutcome::RejectedFencing,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        3 => WorktreeMutationOutcome::Cancel(match outcome {
            1 => WorktreeMutationCancelOutcome::Registered,
            2 => WorktreeMutationCancelOutcome::AlreadyTerminal,
            3 => WorktreeMutationCancelOutcome::Unknown,
            4 => WorktreeMutationCancelOutcome::PhysicalUnknown,
            255 => WorktreeMutationCancelOutcome::RejectedFencing,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(WorktreeMutationResult {
        operation_id,
        job_id,
        outcome,
    })
}

fn read_completion(
    reader: &mut PayloadReader<'_>,
) -> Result<WorktreeMutationCompletion, ProtocolError> {
    Ok(match reader.read_u8()? {
        1 => {
            let root = read_path(reader)?;
            let common = read_path(reader)?;
            let target = read_path(reader)?;
            let branch = if read_bool(reader)? {
                Some(read_reference(reader)?)
            } else {
                None
            };
            WorktreeMutationCompletion::Checkout(WorktreeCheckout::new(
                root,
                common,
                target,
                branch,
                read_bool(reader)?,
            )?)
        }
        2 => WorktreeMutationCompletion::Removed {
            path: read_path(reader)?,
        },
        3 => WorktreeMutationCompletion::Failed(read_failure(reader)?),
        _ => return Err(ProtocolError::InvalidPayload),
    })
}

fn read_failure(reader: &mut PayloadReader<'_>) -> Result<WorktreeMutationFailure, ProtocolError> {
    let stage = match reader.read_u8()? {
        1 => WorktreeMutationStage::Probe,
        2 => WorktreeMutationStage::Select,
        3 => WorktreeMutationStage::CheckBranch,
        4 => WorktreeMutationStage::DirtyCheck,
        5 => WorktreeMutationStage::Mutate,
        6 => WorktreeMutationStage::Verify,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    let effect = match reader.read_u8()? {
        0 => WorktreeMutationEffect::None,
        1 => WorktreeMutationEffect::MayHaveChanged,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    let error = match reader.read_u8()? {
        1 => WorktreeMutationError::GitFailed(if read_bool(reader)? {
            Some(reader.read_i32()?)
        } else {
            None
        }),
        2 => WorktreeMutationError::SpawnUnavailable,
        3 => WorktreeMutationError::SpawnFailed,
        4 => WorktreeMutationError::InvalidRepository,
        5 => WorktreeMutationError::CheckoutNotFound,
        6 => WorktreeMutationError::MainCheckout,
        7 => WorktreeMutationError::Dirty,
        8 => WorktreeMutationError::InvalidReference,
        9 => WorktreeMutationError::TooLarge,
        10 => WorktreeMutationError::Timeout,
        11 => WorktreeMutationError::Cancelled,
        12 => WorktreeMutationError::Io,
        13 => WorktreeMutationError::RepositoryChanged,
        14 => WorktreeMutationError::AmbiguousCheckout,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    WorktreeMutationFailure::new(stage, effect, error, read_field(reader)?.to_vec())
}
