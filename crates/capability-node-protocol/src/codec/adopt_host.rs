//! Carries one `AdoptHost` page as kind 40 and its answer as kind 41. It checks each value through
//! its constructor; it does not join pages, verify a Host or import a terminal.

use std::num::{NonZeroU32, NonZeroU64};

use super::{PayloadReader, decode_host_identity, encode_host_identity};
use crate::{
    AdoptBinding, AdoptBindingState, AdoptCloseInFlight, AdoptHostFailure, AdoptHostKind,
    AdoptHostOutcome, AdoptHostPage, AdoptHostPageKey, AdoptHostProcess, AdoptHostRequest,
    AdoptHostResult, AdoptHostSlot, AdoptHostTarget, AdoptMovedBinding, HostEpoch,
    LifecycleStepOrdinal, MAXIMUM_ADOPT_HOST_BINDINGS, MAXIMUM_ADOPT_HOST_MOVED,
    MAXIMUM_ADOPT_HOST_PAGE_BINDINGS, MAXIMUM_ADOPT_HOST_SLOTS, OperationId, ProtocolError,
    TerminalFence, TerminalId, WorkerGeneration, WorkerId,
};

pub(super) const REQUEST_KIND: u8 = 40;
pub(super) const RESULT_KIND: u8 = 41;

pub(super) fn encode_request(request: &AdoptHostRequest) -> Vec<u8> {
    let bindings = request.bindings();
    let mut bytes = Vec::with_capacity(77 + bindings.len() * 100);
    bytes.push(REQUEST_KIND);
    encode_identity(&mut bytes, request.key());
    let target = request.target();
    bytes.push(u8::try_from(target.slots().len()).expect("an adoption names at most four slots"));
    for slot in target.slots() {
        bytes.push(slot.slot);
        match slot.kind {
            AdoptHostKind::Host { process } => {
                bytes.push(1);
                encode_process_option(&mut bytes, process);
            }
            AdoptHostKind::Recovery => bytes.push(2),
        }
        bytes.push(u8::from(slot.epoch.is_some()));
        if let Some(epoch) = slot.epoch {
            bytes.extend(epoch.as_bytes());
        }
    }
    encode_process_option(&mut bytes, target.keeper());
    encode_page(&mut bytes, request.key().page);
    bytes.extend(
        u16::try_from(bindings.len())
            .expect("a page holds at most 256 bindings")
            .to_be_bytes(),
    );
    for binding in bindings {
        bytes.extend(binding.terminal_id.as_bytes());
        encode_host_identity(&mut bytes, binding.identity);
        bytes.push(match binding.state {
            AdoptBindingState::AwaitingStart => 1,
            AdoptBindingState::Started => 2,
        });
        bytes.extend(binding.fence.get().to_be_bytes());
        bytes.push(u8::from(binding.close.is_some()));
        if let Some(close) = binding.close {
            bytes.extend(close.close_operation_id.as_bytes());
            bytes.extend(close.attempt_generation.get().to_be_bytes());
            bytes.push(close.step_ordinal.get());
            bytes.push(u8::from(close.step_effect_unknown));
        }
    }
    bytes
}

fn encode_process_option(bytes: &mut Vec<u8>, process: Option<AdoptHostProcess>) {
    bytes.push(u8::from(process.is_some()));
    if let Some(process) = process {
        bytes.extend(process.process_id.get().to_be_bytes());
        bytes.extend(process.creation_time.get().to_be_bytes());
    }
}

fn decode_process_option(
    reader: &mut PayloadReader<'_>,
) -> Result<Option<AdoptHostProcess>, ProtocolError> {
    if !read_flag(reader)? {
        return Ok(None);
    }
    Ok(Some(AdoptHostProcess {
        process_id: NonZeroU32::new(reader.read_u32()?).ok_or(ProtocolError::InvalidPayload)?,
        creation_time: positive(reader.read_u64()?)?,
    }))
}

pub(super) fn decode_request(payload: &[u8]) -> Result<AdoptHostRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(payload, REQUEST_KIND)?;
    let (adoption_id, worker_id, worker_generation) = decode_identity(&mut reader)?;
    let count = usize::from(reader.read_u8()?);
    if count > MAXIMUM_ADOPT_HOST_SLOTS {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut slots = Vec::with_capacity(count);
    for _ in 0..count {
        let slot = reader.read_u8()?;
        let kind = match reader.read_u8()? {
            1 => AdoptHostKind::Host {
                process: decode_process_option(&mut reader)?,
            },
            2 => AdoptHostKind::Recovery,
            _ => return Err(ProtocolError::InvalidPayload),
        };
        let epoch = if read_flag(&mut reader)? {
            Some(HostEpoch::from_bytes(reader.read_array()?)?)
        } else {
            None
        };
        slots.push(AdoptHostSlot { slot, kind, epoch });
    }
    let keeper = decode_process_option(&mut reader)?;
    let target = AdoptHostTarget::new(slots, keeper)?;
    let page = decode_page(&mut reader)?;
    let count = usize::from(reader.read_u16()?);
    if count > MAXIMUM_ADOPT_HOST_PAGE_BINDINGS {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut bindings = Vec::with_capacity(count);
    for _ in 0..count {
        let terminal_id = TerminalId::from_bytes(reader.read_array()?)?;
        let identity = decode_host_identity(&mut reader)?;
        let state = match reader.read_u8()? {
            1 => AdoptBindingState::AwaitingStart,
            2 => AdoptBindingState::Started,
            _ => return Err(ProtocolError::InvalidPayload),
        };
        let fence = TerminalFence::new(positive(reader.read_u64()?)?);
        let close = if read_flag(&mut reader)? {
            Some(AdoptCloseInFlight {
                close_operation_id: OperationId::from_bytes(reader.read_array()?)?,
                attempt_generation: positive(reader.read_u64()?)?,
                step_ordinal: LifecycleStepOrdinal::new(reader.read_u8()?)?,
                step_effect_unknown: read_flag(&mut reader)?,
            })
        } else {
            None
        };
        bindings.push(AdoptBinding {
            terminal_id,
            identity,
            state,
            fence,
            close,
        });
    }
    reader.finish()?;
    AdoptHostRequest::new(
        AdoptHostPageKey {
            adoption_id,
            worker_id,
            worker_generation,
            page,
        },
        target,
        bindings,
    )
}

pub(super) fn encode_result(result: &AdoptHostResult) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(55);
    bytes.push(RESULT_KIND);
    encode_identity(&mut bytes, result.key());
    encode_page(&mut bytes, result.key().page);
    match result.outcome() {
        AdoptHostOutcome::PageAccepted => bytes.push(1),
        AdoptHostOutcome::Adopted {
            imported,
            quarantined,
            missing,
            moved,
        } => {
            bytes.push(2);
            bytes.extend(imported.to_be_bytes());
            bytes.extend(quarantined.to_be_bytes());
            bytes.extend(
                u16::try_from(missing.len())
                    .expect("an adoption reports at most 1,024 missing terminals")
                    .to_be_bytes(),
            );
            for terminal in missing {
                bytes.extend(terminal.as_bytes());
            }
            bytes.push(u8::try_from(moved.len()).expect("an adoption reports at most 16 moved"));
            for binding in moved {
                bytes.extend(binding.terminal_id.as_bytes());
                encode_host_identity(&mut bytes, binding.identity);
            }
        }
        AdoptHostOutcome::Failed(failure) => {
            bytes.push(3);
            bytes.push(match failure {
                AdoptHostFailure::HostGone => 1,
                AdoptHostFailure::HostUnreachable => 2,
                AdoptHostFailure::HostUnverified => 3,
                AdoptHostFailure::HostMismatch => 4,
                AdoptHostFailure::HostRefused => 5,
                AdoptHostFailure::InventoryFailed => 6,
                AdoptHostFailure::RecoveryFailed => 7,
            });
        }
    }
    bytes
}

pub(super) fn decode_result(payload: &[u8]) -> Result<AdoptHostResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(payload, RESULT_KIND)?;
    let (adoption_id, worker_id, worker_generation) = decode_identity(&mut reader)?;
    let page = decode_page(&mut reader)?;
    let outcome = match reader.read_u8()? {
        1 => AdoptHostOutcome::PageAccepted,
        2 => {
            let imported = reader.read_u32()?;
            let quarantined = reader.read_u32()?;
            let count = usize::from(reader.read_u16()?);
            if count > MAXIMUM_ADOPT_HOST_BINDINGS {
                return Err(ProtocolError::InvalidPayload);
            }
            let missing = (0..count)
                .map(|_| TerminalId::from_bytes(reader.read_array()?))
                .collect::<Result<_, _>>()?;
            let moved_count = usize::from(reader.read_u8()?);
            if moved_count > MAXIMUM_ADOPT_HOST_MOVED {
                return Err(ProtocolError::InvalidPayload);
            }
            let moved = (0..moved_count)
                .map(|_| {
                    Ok(AdoptMovedBinding {
                        terminal_id: TerminalId::from_bytes(reader.read_array()?)?,
                        identity: decode_host_identity(&mut reader)?,
                    })
                })
                .collect::<Result<_, ProtocolError>>()?;
            AdoptHostOutcome::Adopted {
                imported,
                quarantined,
                missing,
                moved,
            }
        }
        3 => AdoptHostOutcome::Failed(match reader.read_u8()? {
            1 => AdoptHostFailure::HostGone,
            2 => AdoptHostFailure::HostUnreachable,
            3 => AdoptHostFailure::HostUnverified,
            4 => AdoptHostFailure::HostMismatch,
            5 => AdoptHostFailure::HostRefused,
            6 => AdoptHostFailure::InventoryFailed,
            7 => AdoptHostFailure::RecoveryFailed,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    AdoptHostResult::new(
        AdoptHostPageKey {
            adoption_id,
            worker_id,
            worker_generation,
            page,
        },
        outcome,
    )
}

fn encode_identity(bytes: &mut Vec<u8>, key: AdoptHostPageKey) {
    bytes.extend(key.adoption_id.as_bytes());
    bytes.extend(key.worker_id.as_bytes());
    bytes.extend(key.worker_generation.get().to_be_bytes());
}

fn decode_identity(
    reader: &mut PayloadReader<'_>,
) -> Result<(OperationId, WorkerId, WorkerGeneration), ProtocolError> {
    Ok((
        OperationId::from_bytes(reader.read_array()?)?,
        WorkerId::from_bytes(reader.read_array()?)?,
        WorkerGeneration::new(positive(reader.read_u64()?)?),
    ))
}

fn encode_page(bytes: &mut Vec<u8>, page: AdoptHostPage) {
    bytes.extend(page.index.to_be_bytes());
    bytes.push(u8::from(page.last));
}

fn decode_page(reader: &mut PayloadReader<'_>) -> Result<AdoptHostPage, ProtocolError> {
    Ok(AdoptHostPage {
        index: reader.read_u16()?,
        last: read_flag(reader)?,
    })
}

fn read_flag(reader: &mut PayloadReader<'_>) -> Result<bool, ProtocolError> {
    match reader.read_u8()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

fn positive(value: u64) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(value).ok_or(ProtocolError::InvalidPayload)
}
