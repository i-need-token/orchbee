use std::num::{NonZeroU16, NonZeroU64};

use super::{PayloadReader, decode_host_identity, encode_host_identity};
use crate::{
    LeaseExpiresAtUnixMilliseconds, LeaseId, LifecycleCloseActivity, LifecycleHostFailure,
    LifecycleInventoryCursor, LifecycleInventoryFailure, LifecycleInventoryPage,
    LifecycleRejection, LifecycleStepOrdinal, LifecycleTargetObservation, LifecycleTerminalPhase,
    LifecycleUnknownReason, OperationId, ProtocolError, TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
    TerminalFence, TerminalId, TerminalLifecycleAction, TerminalLifecycleAuthorization,
    TerminalLifecycleCapabilityVersion, TerminalLifecycleOutcome, TerminalLifecycleRequest,
    TerminalLifecycleResult, TerminalOperationLease, WorkerGeneration, WorkerId,
};

pub(super) fn encode_request(request: TerminalLifecycleRequest) -> Vec<u8> {
    let mut bytes = vec![9];
    encode_request_body(&mut bytes, request);
    bytes
}

pub(super) fn encode_result(result: TerminalLifecycleResult) -> Vec<u8> {
    let mut bytes = vec![10];
    encode_request_body(&mut bytes, result.request());
    match result.outcome() {
        TerminalLifecycleOutcome::HostUnavailableBeforeHandoff => bytes.push(9),
        TerminalLifecycleOutcome::TerminateAccepted => bytes.push(1),
        TerminalLifecycleOutcome::ForceAccepted => bytes.push(2),
        TerminalLifecycleOutcome::Disposed => bytes.push(3),
        TerminalLifecycleOutcome::InventoryPage(page) => {
            bytes.push(4);
            bytes.extend(page.inventory_generation.get().to_be_bytes());
            encode_cursor(&mut bytes, page.next_cursor);
            bytes.extend(page.returned_count.to_be_bytes());
            match page.target {
                None => bytes.push(0),
                Some(target) => {
                    bytes.push(1);
                    bytes.extend(target.revision.to_be_bytes());
                    encode_phase(&mut bytes, target.phase);
                    bytes.push(match target.close_activity {
                        LifecycleCloseActivity::Unknown => 0,
                        LifecycleCloseActivity::Idle => 1,
                        LifecycleCloseActivity::Active => 2,
                    });
                }
            }
        }
        TerminalLifecycleOutcome::Rejected(reason) => bytes.extend([
            5,
            match reason {
                LifecycleRejection::WrongWorker => 1,
                LifecycleRejection::StaleWorkerGeneration => 2,
                LifecycleRejection::UnsupportedCapability => 3,
                LifecycleRejection::LeaseExpired => 4,
                LifecycleRejection::BindingUnavailable => 5,
                LifecycleRejection::BindingFenceMismatch => 6,
                LifecycleRejection::HostIdentityMismatch => 7,
                LifecycleRejection::LifecycleConflict => 8,
            },
        ]),
        TerminalLifecycleOutcome::HostFailure(reason) => bytes.extend([
            6,
            match reason {
                LifecycleHostFailure::HostRefused => 1,
                LifecycleHostFailure::CleanupIncomplete => 2,
                LifecycleHostFailure::ConfirmationRequired => 3,
            },
        ]),
        TerminalLifecycleOutcome::InventoryFailure(reason) => bytes.extend([
            7,
            match reason {
                LifecycleInventoryFailure::StaleCursor => 1,
                LifecycleInventoryFailure::PageRejected => 2,
                LifecycleInventoryFailure::InvalidPage => 3,
                LifecycleInventoryFailure::IdentityConflict => 4,
            },
        ]),
        TerminalLifecycleOutcome::OutcomeUnknown(reason) => bytes.extend([
            8,
            match reason {
                LifecycleUnknownReason::TransportLost => 1,
                LifecycleUnknownReason::Deadline => 2,
                LifecycleUnknownReason::InvalidResponse => 3,
            },
        ]),
    }
    bytes
}

pub(super) fn decode_request(bytes: &[u8]) -> Result<TerminalLifecycleRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 9)?;
    let request = decode_request_body(&mut reader)?;
    reader.finish()?;
    Ok(request)
}

pub(super) fn decode_result(bytes: &[u8]) -> Result<TerminalLifecycleResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 10)?;
    let request = decode_request_body(&mut reader)?;
    let outcome = match reader.read_u8()? {
        9 => TerminalLifecycleOutcome::HostUnavailableBeforeHandoff,
        1 => TerminalLifecycleOutcome::TerminateAccepted,
        2 => TerminalLifecycleOutcome::ForceAccepted,
        3 => TerminalLifecycleOutcome::Disposed,
        4 => {
            let inventory_generation = positive(reader.read_u64()?)?;
            let next_cursor = decode_cursor(&mut reader)?;
            let returned_count = reader.read_u16()?;
            let target = match reader.read_u8()? {
                0 => None,
                1 => Some(LifecycleTargetObservation {
                    revision: reader.read_u64()?,
                    phase: decode_phase(&mut reader)?,
                    close_activity: match reader.read_u8()? {
                        0 => LifecycleCloseActivity::Unknown,
                        1 => LifecycleCloseActivity::Idle,
                        2 => LifecycleCloseActivity::Active,
                        _ => return Err(ProtocolError::InvalidPayload),
                    },
                }),
                _ => return Err(ProtocolError::InvalidPayload),
            };
            TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
                inventory_generation,
                next_cursor,
                returned_count,
                target,
            })
        }
        5 => TerminalLifecycleOutcome::Rejected(match reader.read_u8()? {
            1 => LifecycleRejection::WrongWorker,
            2 => LifecycleRejection::StaleWorkerGeneration,
            3 => LifecycleRejection::UnsupportedCapability,
            4 => LifecycleRejection::LeaseExpired,
            5 => LifecycleRejection::BindingUnavailable,
            6 => LifecycleRejection::BindingFenceMismatch,
            7 => LifecycleRejection::HostIdentityMismatch,
            8 => LifecycleRejection::LifecycleConflict,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        6 => TerminalLifecycleOutcome::HostFailure(match reader.read_u8()? {
            1 => LifecycleHostFailure::HostRefused,
            2 => LifecycleHostFailure::CleanupIncomplete,
            3 => LifecycleHostFailure::ConfirmationRequired,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        7 => TerminalLifecycleOutcome::InventoryFailure(match reader.read_u8()? {
            1 => LifecycleInventoryFailure::StaleCursor,
            2 => LifecycleInventoryFailure::PageRejected,
            3 => LifecycleInventoryFailure::InvalidPage,
            4 => LifecycleInventoryFailure::IdentityConflict,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        8 => TerminalLifecycleOutcome::OutcomeUnknown(match reader.read_u8()? {
            1 => LifecycleUnknownReason::TransportLost,
            2 => LifecycleUnknownReason::Deadline,
            3 => LifecycleUnknownReason::InvalidResponse,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    TerminalLifecycleResult::new(request, outcome)
}

fn encode_request_body(bytes: &mut Vec<u8>, request: TerminalLifecycleRequest) {
    let authorization = request.authorization;
    bytes.extend(authorization.capability_version.get().to_be_bytes());
    bytes.extend(authorization.terminal_id.as_bytes());
    bytes.extend(authorization.worker_id.as_bytes());
    bytes.extend(authorization.worker_generation.get().to_be_bytes());
    bytes.extend(authorization.binding_fence.get().to_be_bytes());
    bytes.extend(authorization.lease.id().as_bytes());
    bytes.extend(authorization.lease.expires_at().get().to_be_bytes());
    bytes.extend(request.close_operation_id.as_bytes());
    bytes.extend(request.request_id.as_bytes());
    bytes.extend(request.attempt_generation.get().to_be_bytes());
    bytes.push(request.step_ordinal.get());
    encode_host_identity(bytes, request.target);
    match request.action {
        TerminalLifecycleAction::Terminate => bytes.push(1),
        TerminalLifecycleAction::TerminateIfIdle => bytes.push(5),
        TerminalLifecycleAction::Force => bytes.push(2),
        TerminalLifecycleAction::Dispose => bytes.push(3),
        TerminalLifecycleAction::InventoryPage { cursor } => {
            bytes.push(4);
            encode_cursor(bytes, cursor);
        }
        TerminalLifecycleAction::InspectInventoryPage { cursor } => {
            bytes.push(6);
            encode_cursor(bytes, cursor);
        }
    }
}

fn decode_request_body(
    reader: &mut PayloadReader<'_>,
) -> Result<TerminalLifecycleRequest, ProtocolError> {
    let capability_version = TerminalLifecycleCapabilityVersion::new(
        NonZeroU16::new(reader.read_u16()?).ok_or(ProtocolError::InvalidPayload)?,
    );
    let terminal_id = TerminalId::from_bytes(reader.read_array()?)?;
    let worker_id = WorkerId::from_bytes(reader.read_array()?)?;
    let worker_generation = WorkerGeneration::new(positive(reader.read_u64()?)?);
    let binding_fence = TerminalFence::new(positive(reader.read_u64()?)?);
    let lease_id = LeaseId::from_bytes(reader.read_array()?)?;
    let lease = TerminalOperationLease::new(
        lease_id,
        LeaseExpiresAtUnixMilliseconds::new(positive(reader.read_u64()?)?),
    );
    let close_operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let request_id = OperationId::from_bytes(reader.read_array()?)?;
    let attempt_generation = positive(reader.read_u64()?)?;
    let step_ordinal = LifecycleStepOrdinal::new(reader.read_u8()?)?;
    let target = decode_host_identity(reader)?;
    let action = match reader.read_u8()? {
        1 => TerminalLifecycleAction::Terminate,
        5 if capability_version == TERMINAL_LIFECYCLE_CAPABILITY_VERSION => {
            TerminalLifecycleAction::TerminateIfIdle
        }
        6 if capability_version == TERMINAL_LIFECYCLE_CAPABILITY_VERSION => {
            TerminalLifecycleAction::InspectInventoryPage {
                cursor: decode_cursor(reader)?,
            }
        }
        2 => TerminalLifecycleAction::Force,
        3 => TerminalLifecycleAction::Dispose,
        4 => TerminalLifecycleAction::InventoryPage {
            cursor: decode_cursor(reader)?,
        },
        _ => return Err(ProtocolError::InvalidPayload),
    };
    Ok(TerminalLifecycleRequest {
        authorization: TerminalLifecycleAuthorization {
            capability_version,
            terminal_id,
            worker_id,
            worker_generation,
            binding_fence,
            lease,
        },
        close_operation_id,
        request_id,
        attempt_generation,
        step_ordinal,
        target,
        action,
    })
}

fn encode_cursor(bytes: &mut Vec<u8>, cursor: Option<LifecycleInventoryCursor>) {
    match cursor {
        None => bytes.push(0),
        Some(cursor) => {
            bytes.push(1);
            bytes.extend(cursor.inventory_generation.get().to_be_bytes());
            bytes.extend(cursor.offset.to_be_bytes());
        }
    }
}

fn decode_cursor(
    reader: &mut PayloadReader<'_>,
) -> Result<Option<LifecycleInventoryCursor>, ProtocolError> {
    match reader.read_u8()? {
        0 => Ok(None),
        1 => Ok(Some(LifecycleInventoryCursor {
            inventory_generation: positive(reader.read_u64()?)?,
            offset: reader.read_u64()?,
        })),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

fn encode_phase(bytes: &mut Vec<u8>, phase: LifecycleTerminalPhase) {
    match phase {
        LifecycleTerminalPhase::AwaitingStart => bytes.push(1),
        LifecycleTerminalPhase::Running {
            terminate_requested,
        } => bytes.extend([2, u8::from(terminate_requested)]),
        LifecycleTerminalPhase::Finishing {
            terminate_requested,
        } => bytes.extend([3, u8::from(terminate_requested)]),
        LifecycleTerminalPhase::ContainingFailure => bytes.push(4),
        LifecycleTerminalPhase::Exited {
            terminate_requested,
        } => bytes.extend([5, u8::from(terminate_requested)]),
        LifecycleTerminalPhase::Failed => bytes.push(6),
    }
}

fn decode_phase(reader: &mut PayloadReader<'_>) -> Result<LifecycleTerminalPhase, ProtocolError> {
    match reader.read_u8()? {
        1 => Ok(LifecycleTerminalPhase::AwaitingStart),
        2 => Ok(LifecycleTerminalPhase::Running {
            terminate_requested: boolean(reader)?,
        }),
        3 => Ok(LifecycleTerminalPhase::Finishing {
            terminate_requested: boolean(reader)?,
        }),
        4 => Ok(LifecycleTerminalPhase::ContainingFailure),
        5 => Ok(LifecycleTerminalPhase::Exited {
            terminate_requested: boolean(reader)?,
        }),
        6 => Ok(LifecycleTerminalPhase::Failed),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

fn boolean(reader: &mut PayloadReader<'_>) -> Result<bool, ProtocolError> {
    match reader.read_u8()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

fn positive(value: u64) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(value).ok_or(ProtocolError::InvalidPayload)
}
