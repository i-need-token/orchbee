use std::io::Read;

use super::PayloadReader;
use crate::{
    CapabilityNodeMessage, MAXIMUM_VIEWER_RECORD_BYTES, OperationId, ProtocolError,
    ReadMessageError, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
    TerminalViewerResult, ViewerRejection, ViewerSnapshotReason, WorkerGeneration, WorkerId,
};

pub(super) fn encode_request(value: TerminalViewerRequest) -> Vec<u8> {
    let mut bytes = vec![5];
    bytes.extend_from_slice(&value.request_id.as_bytes());
    bytes.extend_from_slice(&value.worker_id.as_bytes());
    bytes.extend_from_slice(&value.worker_generation.get().to_be_bytes());
    bytes.extend_from_slice(&value.attachment_id.as_bytes());
    match value.action {
        TerminalViewerAction::Attach {
            host_epoch,
            terminal_id,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&host_epoch.as_bytes());
            bytes.extend_from_slice(&terminal_id.as_bytes());
        }
        TerminalViewerAction::Poll => bytes.push(2),
        TerminalViewerAction::FrameApplied {
            sequence,
            generation,
            revision,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&sequence.get().to_be_bytes());
            bytes.extend_from_slice(&generation.get().to_be_bytes());
            bytes.extend_from_slice(&revision.to_be_bytes());
        }
        TerminalViewerAction::NeedSnapshot { sequence, reason } => {
            bytes.push(4);
            bytes.extend_from_slice(&sequence.get().to_be_bytes());
            bytes.push(match reason {
                ViewerSnapshotReason::ActivityStale => 1,
                ViewerSnapshotReason::BaseMismatch => 2,
                ViewerSnapshotReason::DecoderRejected => 3,
                ViewerSnapshotReason::DimensionsMismatch => 4,
                ViewerSnapshotReason::GenerationMismatch => 5,
                ViewerSnapshotReason::IdentityMismatch => 6,
                ViewerSnapshotReason::SequenceGap => 7,
                ViewerSnapshotReason::StoreReset => 8,
            });
        }
        TerminalViewerAction::Detach => bytes.push(5),
        TerminalViewerAction::ReadRows {
            start_row,
            row_count,
            revision,
            generation,
            request_sequence,
            maximum_bytes,
        } => {
            bytes.push(6);
            bytes.extend_from_slice(&start_row.to_be_bytes());
            bytes.extend_from_slice(&row_count.to_be_bytes());
            bytes.extend_from_slice(&revision.to_be_bytes());
            bytes.extend_from_slice(&generation.get().to_be_bytes());
            bytes.extend_from_slice(&request_sequence.get().to_be_bytes());
            bytes.extend_from_slice(&maximum_bytes.to_be_bytes());
        }
    }
    bytes
}

pub(super) fn decode_request(bytes: &[u8]) -> Result<TerminalViewerRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 5)?;
    let request_id = OperationId::from_bytes(reader.read_array()?)?;
    let worker_id = WorkerId::from_bytes(reader.read_array()?)?;
    let worker_generation = WorkerGeneration::new(positive(reader.read_u64()?)?);
    let attachment_id = OperationId::from_bytes(reader.read_array()?)?;
    let action = match reader.read_u8()? {
        1 => TerminalViewerAction::Attach {
            host_epoch: crate::HostEpoch::from_bytes(reader.read_array()?)?,
            terminal_id: crate::HostTerminalId::from_bytes(reader.read_array()?)?,
        },
        2 => TerminalViewerAction::Poll,
        3 => TerminalViewerAction::FrameApplied {
            sequence: positive(reader.read_u64()?)?,
            generation: positive(reader.read_u64()?)?,
            revision: reader.read_u64()?,
        },
        4 => TerminalViewerAction::NeedSnapshot {
            sequence: positive(reader.read_u64()?)?,
            reason: match reader.read_u8()? {
                1 => ViewerSnapshotReason::ActivityStale,
                2 => ViewerSnapshotReason::BaseMismatch,
                3 => ViewerSnapshotReason::DecoderRejected,
                4 => ViewerSnapshotReason::DimensionsMismatch,
                5 => ViewerSnapshotReason::GenerationMismatch,
                6 => ViewerSnapshotReason::IdentityMismatch,
                7 => ViewerSnapshotReason::SequenceGap,
                8 => ViewerSnapshotReason::StoreReset,
                _ => return Err(ProtocolError::InvalidPayload),
            },
        },
        5 => TerminalViewerAction::Detach,
        6 => {
            let start_row = reader.read_u64()?;
            let row_count = reader.read_u16()?;
            let revision = reader.read_u64()?;
            let generation = positive(reader.read_u64()?)?;
            let request_sequence = positive(reader.read_u64()?)?;
            let maximum_bytes = reader.read_u32()?;
            if row_count == 0
                || row_count > crate::MAXIMUM_READ_ROWS
                || !(crate::MINIMUM_READ_ROWS_BYTES..=crate::MAXIMUM_READ_ROWS_BYTES)
                    .contains(&maximum_bytes)
            {
                return Err(ProtocolError::InvalidPayload);
            }
            TerminalViewerAction::ReadRows {
                start_row,
                row_count,
                revision,
                generation,
                request_sequence,
                maximum_bytes,
            }
        }
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(TerminalViewerRequest {
        request_id,
        worker_id,
        worker_generation,
        attachment_id,
        action,
    })
}

pub(super) fn encode_result(value: &TerminalViewerResult) -> Vec<u8> {
    let mut bytes = vec![6];
    bytes.extend_from_slice(&value.request_id.as_bytes());
    bytes.extend_from_slice(&value.attachment_id.as_bytes());
    match value.outcome {
        TerminalViewerOutcome::Attached => bytes.push(1),
        TerminalViewerOutcome::Empty => bytes.push(2),
        TerminalViewerOutcome::FeedbackAccepted => bytes.push(3),
        TerminalViewerOutcome::Detached => bytes.push(4),
        TerminalViewerOutcome::Rejected(reason) => {
            bytes.push(5);
            bytes.push(match reason {
                ViewerRejection::WrongWorker => 1,
                ViewerRejection::StaleWorkerGeneration => 2,
                ViewerRejection::StaleHostEpoch => 3,
                ViewerRejection::CapacityExceeded => 4,
                ViewerRejection::AttachmentUnavailable => 5,
                ViewerRejection::AttachmentAlreadyExists => 6,
                ViewerRejection::FeedbackMismatch => 7,
                ViewerRejection::HostRejected => 8,
            });
        }
        TerminalViewerOutcome::RowsRejected(reason) => {
            bytes.push(6);
            bytes.push(rows_rejected_code(reason));
        }
        TerminalViewerOutcome::Record(_) => unreachable!("record uses OBNR framing"),
    }
    bytes
}

fn rows_rejected_code(reason: crate::ViewerRowsRejectedReason) -> u8 {
    match reason {
        crate::ViewerRowsRejectedReason::StaleRevision => 1,
        crate::ViewerRowsRejectedReason::StaleGeneration => 2,
        crate::ViewerRowsRejectedReason::HistoryEvicted => 3,
        crate::ViewerRowsRejectedReason::MissingResource => 4,
        crate::ViewerRowsRejectedReason::InvalidField => 5,
        crate::ViewerRowsRejectedReason::InvalidAcknowledgement => 6,
        crate::ViewerRowsRejectedReason::LimitExceeded => 7,
        crate::ViewerRowsRejectedReason::ResourceExhausted => 8,
        crate::ViewerRowsRejectedReason::InvalidPhase => 9,
    }
}

fn rows_rejected_reason(code: u8) -> Result<crate::ViewerRowsRejectedReason, ProtocolError> {
    Ok(match code {
        1 => crate::ViewerRowsRejectedReason::StaleRevision,
        2 => crate::ViewerRowsRejectedReason::StaleGeneration,
        3 => crate::ViewerRowsRejectedReason::HistoryEvicted,
        4 => crate::ViewerRowsRejectedReason::MissingResource,
        5 => crate::ViewerRowsRejectedReason::InvalidField,
        6 => crate::ViewerRowsRejectedReason::InvalidAcknowledgement,
        7 => crate::ViewerRowsRejectedReason::LimitExceeded,
        8 => crate::ViewerRowsRejectedReason::ResourceExhausted,
        9 => crate::ViewerRowsRejectedReason::InvalidPhase,
        _ => return Err(ProtocolError::InvalidPayload),
    })
}

pub(super) fn decode_result(bytes: &[u8]) -> Result<TerminalViewerResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 6)?;
    let request_id = OperationId::from_bytes(reader.read_array()?)?;
    let attachment_id = OperationId::from_bytes(reader.read_array()?)?;
    let outcome = match reader.read_u8()? {
        1 => TerminalViewerOutcome::Attached,
        2 => TerminalViewerOutcome::Empty,
        3 => TerminalViewerOutcome::FeedbackAccepted,
        4 => TerminalViewerOutcome::Detached,
        5 => TerminalViewerOutcome::Rejected(match reader.read_u8()? {
            1 => ViewerRejection::WrongWorker,
            2 => ViewerRejection::StaleWorkerGeneration,
            3 => ViewerRejection::StaleHostEpoch,
            4 => ViewerRejection::CapacityExceeded,
            5 => ViewerRejection::AttachmentUnavailable,
            6 => ViewerRejection::AttachmentAlreadyExists,
            7 => ViewerRejection::FeedbackMismatch,
            8 => ViewerRejection::HostRejected,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        6 => TerminalViewerOutcome::RowsRejected(rows_rejected_reason(reader.read_u8()?)?),
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(TerminalViewerResult {
        request_id,
        attachment_id,
        outcome,
    })
}

pub(super) fn encode_record(message: &CapabilityNodeMessage) -> Result<Vec<u8>, ProtocolError> {
    let CapabilityNodeMessage::TerminalViewerResult(value) = message else {
        return Err(ProtocolError::InvalidPayload);
    };
    let TerminalViewerOutcome::Record(record) = &value.outcome else {
        return Err(ProtocolError::InvalidPayload);
    };
    let length = record
        .len()
        .checked_add(32)
        .ok_or(ProtocolError::InvalidPayload)?;
    validate_record_length(length)?;
    let mut bytes = Vec::with_capacity(12 + length);
    bytes.extend_from_slice(b"OBNR\0\x01\0\0");
    bytes.extend_from_slice(
        &u32::try_from(length)
            .map_err(|_| ProtocolError::InvalidPayload)?
            .to_be_bytes(),
    );
    bytes.extend_from_slice(&value.request_id.as_bytes());
    bytes.extend_from_slice(&value.attachment_id.as_bytes());
    bytes.extend_from_slice(record);
    Ok(bytes)
}

pub(super) fn decode_record(bytes: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let header = bytes.get(..12).ok_or(ProtocolError::InvalidPayload)?;
    let length = record_length(header)?;
    if bytes.len() != 12 + length {
        return Err(ProtocolError::InvalidPayload);
    }
    record_payload(&bytes[12..])
}

pub(super) fn read_record(
    reader: &mut impl Read,
    header: [u8; 8],
) -> Result<CapabilityNodeMessage, ReadMessageError> {
    let mut full_header = [0; 12];
    full_header[..8].copy_from_slice(&header);
    reader.read_exact(&mut full_header[8..])?;
    let length = record_length(&full_header)?;
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    record_payload(&bytes).map_err(Into::into)
}

fn record_payload(bytes: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let request_id = OperationId::from_bytes(
        bytes[..16]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    )?;
    let attachment_id = OperationId::from_bytes(
        bytes[16..32]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    )?;
    Ok(CapabilityNodeMessage::TerminalViewerResult(
        TerminalViewerResult {
            request_id,
            attachment_id,
            outcome: TerminalViewerOutcome::Record(bytes[32..].to_vec()),
        },
    ))
}

fn record_length(header: &[u8]) -> Result<usize, ProtocolError> {
    if &header[..4] != b"OBNR" {
        return Err(ProtocolError::InvalidMagic);
    }
    let version = u16::from_be_bytes([header[4], header[5]]);
    if version != 1 {
        return Err(ProtocolError::IncompatibleVersion { received: version });
    }
    if header[6..8] != [0, 0] {
        return Err(ProtocolError::InvalidPayload);
    }
    let length = usize::try_from(u32::from_be_bytes(
        header[8..12]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    ))
    .map_err(|_| ProtocolError::InvalidPayload)?;
    validate_record_length(length)?;
    Ok(length)
}

fn validate_record_length(length: usize) -> Result<(), ProtocolError> {
    if length < 32 + 64 {
        return Err(ProtocolError::InvalidPayload);
    }
    if length > 32 + MAXIMUM_VIEWER_RECORD_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: length,
            maximum: 32 + MAXIMUM_VIEWER_RECORD_BYTES,
        });
    }
    Ok(())
}

fn positive(value: u64) -> Result<std::num::NonZeroU64, ProtocolError> {
    std::num::NonZeroU64::new(value).ok_or(ProtocolError::InvalidPayload)
}
