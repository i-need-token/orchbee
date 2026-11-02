//! Maps viewer controls and responses to binary messages and bounded opaque record carriers
//! while validating action operands. It preserves physical routing and correlation without
//! interpreting inner terminal records or retaining a viewer.

use std::io::Read;

use super::PayloadReader;
use crate::{
    CapabilityNodeMessage, MAXIMUM_VIEWER_RECORD_BYTES, OperationId, ProtocolError,
    ReadMessageError, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
    TerminalViewerResult, ViewerHistoryDemand, ViewerProjectionDemand, ViewerRejection,
    ViewerRowDetail, ViewerSelectionCommand, ViewerSelectionExportCancelReason,
    ViewerSelectionExportFormat, ViewerSelectionGestureAction, ViewerSelectionGestureGeometry,
    ViewerSelectionGesturePosition, ViewerSelectionPoint, ViewerSelectionViewportPoint,
    ViewerSnapshotReason, ViewportRowOrigin, WorkerGeneration, WorkerId,
};

const MAXIMUM_SELECTION_AXIS: i32 = 0xffff;
const MAXIMUM_SELECTION_DIMENSION: u16 = 4096;

pub(super) const RECORD_READY_KIND: u8 = 33;

pub(super) fn encode_record_ready(value: crate::TerminalViewerRecordReady) -> Vec<u8> {
    let mut bytes = vec![RECORD_READY_KIND];
    bytes.extend_from_slice(&value.attachment_id.as_bytes());
    bytes
}

pub(super) fn decode_record_ready(
    bytes: &[u8],
) -> Result<crate::TerminalViewerRecordReady, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, RECORD_READY_KIND)?;
    let attachment_id = OperationId::from_bytes(reader.read_array()?)?;
    reader.finish()?;
    Ok(crate::TerminalViewerRecordReady { attachment_id })
}

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
        TerminalViewerAction::ReadProcessInfo => bytes.push(12),
        TerminalViewerAction::ReadLifecycle { target } => {
            bytes.push(15);
            super::encode_host_identity(&mut bytes, target);
        }
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
            detail,
            start_row,
            row_count,
            revision,
            generation,
            request_sequence,
            maximum_bytes,
        } => {
            bytes.push(match detail {
                ViewerRowDetail::Text => 6,
                ViewerRowDetail::Render => 13,
            });
            bytes.extend_from_slice(&start_row.to_be_bytes());
            bytes.extend_from_slice(&row_count.to_be_bytes());
            bytes.extend_from_slice(&revision.to_be_bytes());
            bytes.extend_from_slice(&generation.get().to_be_bytes());
            bytes.extend_from_slice(&request_sequence.get().to_be_bytes());
            bytes.extend_from_slice(&maximum_bytes.to_be_bytes());
        }
        TerminalViewerAction::SetProjectionDemand { demand } => {
            bytes.push(7);
            bytes.push(demand.tag());
        }
        TerminalViewerAction::SetHistoryDemand { demand } => {
            bytes.push(8);
            bytes.push(demand.tag());
        }
        TerminalViewerAction::ReadViewportRows {
            detail,
            origin,
            row_count,
            revision,
            generation,
            request_sequence,
            maximum_bytes,
        } => {
            bytes.push(match detail {
                ViewerRowDetail::Text => 9,
                ViewerRowDetail::Render => 14,
            });
            match origin {
                ViewportRowOrigin::Tail { rows_before } => {
                    bytes.push(1);
                    bytes.extend_from_slice(&rows_before.to_be_bytes());
                }
                ViewportRowOrigin::Committed { delta } => {
                    bytes.push(2);
                    bytes.extend_from_slice(&delta.to_be_bytes());
                }
                ViewportRowOrigin::Absolute { row } => {
                    bytes.push(3);
                    bytes.extend_from_slice(&row.to_be_bytes());
                }
            }
            bytes.extend_from_slice(&row_count.to_be_bytes());
            bytes.extend_from_slice(&revision.to_be_bytes());
            bytes.extend_from_slice(&generation.get().to_be_bytes());
            bytes.extend_from_slice(&request_sequence.get().to_be_bytes());
            bytes.extend_from_slice(&maximum_bytes.to_be_bytes());
        }
        TerminalViewerAction::RowsApplied {
            request_sequence,
            generation,
            revision,
        } => {
            bytes.push(10);
            bytes.extend_from_slice(&request_sequence.get().to_be_bytes());
            bytes.extend_from_slice(&generation.get().to_be_bytes());
            bytes.extend_from_slice(&revision.to_be_bytes());
        }
        TerminalViewerAction::HistoryObserved {
            sequence,
            generation,
            revision,
        } => {
            bytes.push(11);
            bytes.extend_from_slice(&sequence.get().to_be_bytes());
            bytes.extend_from_slice(&generation.get().to_be_bytes());
            bytes.extend_from_slice(&revision.to_be_bytes());
        }
        TerminalViewerAction::SelectionGesture {
            generation,
            gesture,
        } => {
            bytes.push(16);
            bytes.extend_from_slice(&generation.get().to_be_bytes());
            match gesture {
                ViewerSelectionGestureAction::Start {
                    click_count,
                    point,
                    position,
                    select_output,
                } => {
                    bytes.push(1);
                    bytes.push(click_count);
                    encode_selection_point(&mut bytes, point);
                    encode_selection_position(&mut bytes, position);
                    bytes.push(u8::from(select_output));
                }
                ViewerSelectionGestureAction::Update {
                    geometry,
                    point,
                    position,
                    rectangle,
                } => {
                    bytes.push(2);
                    bytes.extend_from_slice(&geometry.cell_width.to_be_bytes());
                    bytes.extend_from_slice(&geometry.columns.to_be_bytes());
                    bytes.extend_from_slice(&geometry.padding_left.to_be_bytes());
                    bytes.extend_from_slice(&geometry.screen_height.to_be_bytes());
                    encode_selection_point(&mut bytes, point);
                    encode_selection_position(&mut bytes, position);
                    bytes.push(u8::from(rectangle));
                }
                ViewerSelectionGestureAction::End { point } => {
                    bytes.push(3);
                    encode_selection_point(&mut bytes, point);
                }
                ViewerSelectionGestureAction::Cancel => bytes.push(4),
            }
        }
        TerminalViewerAction::SelectionCommand {
            generation,
            revision,
            command,
        } => {
            bytes.push(20);
            bytes.extend_from_slice(&generation.get().to_be_bytes());
            bytes.extend_from_slice(&revision.to_be_bytes());
            match command {
                ViewerSelectionCommand::Clear => bytes.push(1),
                ViewerSelectionCommand::SelectAll => bytes.push(2),
                ViewerSelectionCommand::SetRange {
                    end,
                    rectangle,
                    start,
                } => {
                    bytes.push(3);
                    encode_selection_absolute_point(&mut bytes, start);
                    encode_selection_absolute_point(&mut bytes, end);
                    bytes.push(u8::from(rectangle));
                }
                ViewerSelectionCommand::SelectOutput { point } => {
                    bytes.push(4);
                    encode_selection_absolute_point(&mut bytes, point);
                }
            }
        }
        TerminalViewerAction::SelectionExport {
            export_sequence,
            format,
        } => {
            bytes.push(17);
            bytes.extend_from_slice(&export_sequence.get().to_be_bytes());
            bytes.push(match format {
                ViewerSelectionExportFormat::Plain => 1,
            });
        }
        TerminalViewerAction::SelectionExportChunkApplied {
            export_sequence,
            chunk_sequence,
        } => {
            bytes.push(18);
            bytes.extend_from_slice(&export_sequence.get().to_be_bytes());
            bytes.extend_from_slice(&chunk_sequence.to_be_bytes());
        }
        TerminalViewerAction::SelectionExportCancel {
            export_sequence,
            reason,
        } => {
            bytes.push(19);
            bytes.extend_from_slice(&export_sequence.get().to_be_bytes());
            bytes.push(match reason {
                ViewerSelectionExportCancelReason::ClipboardFailed => 1,
                ViewerSelectionExportCancelReason::Disposed => 2,
                ViewerSelectionExportCancelReason::Replaced => 3,
                ViewerSelectionExportCancelReason::User => 4,
            });
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
        12 => TerminalViewerAction::ReadProcessInfo,
        15 => TerminalViewerAction::ReadLifecycle {
            target: super::decode_host_identity(&mut reader)?,
        },
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
        tag @ (6 | 13) => {
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
                detail: if tag == 6 {
                    ViewerRowDetail::Text
                } else {
                    ViewerRowDetail::Render
                },
                start_row,
                row_count,
                revision,
                generation,
                request_sequence,
                maximum_bytes,
            }
        }
        7 => TerminalViewerAction::SetProjectionDemand {
            demand: ViewerProjectionDemand::from_tag(reader.read_u8()?)?,
        },
        8 => TerminalViewerAction::SetHistoryDemand {
            demand: ViewerHistoryDemand::from_tag(reader.read_u8()?)?,
        },
        tag @ (9 | 14) => {
            let origin = match reader.read_u8()? {
                1 => ViewportRowOrigin::Tail {
                    rows_before: reader.read_u32()?,
                },
                2 => ViewportRowOrigin::Committed {
                    delta: reader.read_i32()?,
                },
                3 => ViewportRowOrigin::Absolute {
                    row: reader.read_u64()?,
                },
                _ => return Err(ProtocolError::InvalidPayload),
            };
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
            TerminalViewerAction::ReadViewportRows {
                detail: if tag == 9 {
                    ViewerRowDetail::Text
                } else {
                    ViewerRowDetail::Render
                },
                origin,
                row_count,
                revision,
                generation,
                request_sequence,
                maximum_bytes,
            }
        }
        10 => TerminalViewerAction::RowsApplied {
            request_sequence: positive(reader.read_u64()?)?,
            generation: positive(reader.read_u64()?)?,
            revision: reader.read_u64()?,
        },
        11 => TerminalViewerAction::HistoryObserved {
            sequence: positive(reader.read_u64()?)?,
            generation: positive(reader.read_u64()?)?,
            revision: reader.read_u64()?,
        },
        16 => {
            let generation = positive(reader.read_u64()?)?;
            let gesture = match reader.read_u8()? {
                1 => {
                    let click_count = reader.read_u8()?;
                    if !(1..=3).contains(&click_count) {
                        return Err(ProtocolError::InvalidPayload);
                    }
                    ViewerSelectionGestureAction::Start {
                        click_count,
                        point: decode_selection_point(&mut reader)?,
                        position: decode_selection_position(&mut reader)?,
                        select_output: decode_bool(reader.read_u8()?)?,
                    }
                }
                2 => ViewerSelectionGestureAction::Update {
                    geometry: decode_selection_geometry(&mut reader)?,
                    point: decode_selection_point(&mut reader)?,
                    position: decode_selection_position(&mut reader)?,
                    rectangle: decode_bool(reader.read_u8()?)?,
                },
                3 => ViewerSelectionGestureAction::End {
                    point: decode_selection_point(&mut reader)?,
                },
                4 => ViewerSelectionGestureAction::Cancel,
                _ => return Err(ProtocolError::InvalidPayload),
            };
            TerminalViewerAction::SelectionGesture {
                generation,
                gesture,
            }
        }
        17 => TerminalViewerAction::SelectionExport {
            export_sequence: positive(reader.read_u64()?)?,
            format: match reader.read_u8()? {
                1 => ViewerSelectionExportFormat::Plain,
                _ => return Err(ProtocolError::InvalidPayload),
            },
        },
        18 => TerminalViewerAction::SelectionExportChunkApplied {
            export_sequence: positive(reader.read_u64()?)?,
            chunk_sequence: reader.read_u32()?,
        },
        19 => TerminalViewerAction::SelectionExportCancel {
            export_sequence: positive(reader.read_u64()?)?,
            reason: match reader.read_u8()? {
                1 => ViewerSelectionExportCancelReason::ClipboardFailed,
                2 => ViewerSelectionExportCancelReason::Disposed,
                3 => ViewerSelectionExportCancelReason::Replaced,
                4 => ViewerSelectionExportCancelReason::User,
                _ => return Err(ProtocolError::InvalidPayload),
            },
        },
        20 => {
            let generation = positive(reader.read_u64()?)?;
            let revision = reader.read_u64()?;
            let command = match reader.read_u8()? {
                1 => ViewerSelectionCommand::Clear,
                2 => ViewerSelectionCommand::SelectAll,
                3 => {
                    let start = decode_selection_absolute_point(&mut reader)?;
                    let end = decode_selection_absolute_point(&mut reader)?;
                    ViewerSelectionCommand::SetRange {
                        end,
                        rectangle: decode_bool(reader.read_u8()?)?,
                        start,
                    }
                }
                4 => ViewerSelectionCommand::SelectOutput {
                    point: decode_selection_absolute_point(&mut reader)?,
                },
                _ => return Err(ProtocolError::InvalidPayload),
            };
            TerminalViewerAction::SelectionCommand {
                generation,
                revision,
                command,
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

fn encode_selection_point(bytes: &mut Vec<u8>, point: ViewerSelectionViewportPoint) {
    bytes.extend_from_slice(&point.column.to_be_bytes());
    bytes.extend_from_slice(&point.row.to_be_bytes());
}

fn encode_selection_absolute_point(bytes: &mut Vec<u8>, point: ViewerSelectionPoint) {
    bytes.extend_from_slice(&point.column.to_be_bytes());
    bytes.extend_from_slice(&point.row.to_be_bytes());
}

fn encode_selection_position(bytes: &mut Vec<u8>, position: ViewerSelectionGesturePosition) {
    bytes.extend_from_slice(&position.x.to_be_bytes());
    bytes.extend_from_slice(&position.y.to_be_bytes());
}

fn decode_selection_point(
    reader: &mut PayloadReader<'_>,
) -> Result<ViewerSelectionViewportPoint, ProtocolError> {
    let point = ViewerSelectionViewportPoint {
        column: reader.read_u16()?,
        row: reader.read_u16()?,
    };
    if point.column >= MAXIMUM_SELECTION_DIMENSION || point.row >= MAXIMUM_SELECTION_DIMENSION {
        return Err(ProtocolError::InvalidPayload);
    }
    Ok(point)
}

fn decode_selection_absolute_point(
    reader: &mut PayloadReader<'_>,
) -> Result<ViewerSelectionPoint, ProtocolError> {
    let point = ViewerSelectionPoint {
        column: reader.read_u16()?,
        row: reader.read_u64()?,
    };
    if point.column >= MAXIMUM_SELECTION_DIMENSION {
        return Err(ProtocolError::InvalidPayload);
    }
    Ok(point)
}

fn decode_selection_position(
    reader: &mut PayloadReader<'_>,
) -> Result<ViewerSelectionGesturePosition, ProtocolError> {
    let position = ViewerSelectionGesturePosition {
        x: reader.read_i32()?,
        y: reader.read_i32()?,
    };
    if !(-MAXIMUM_SELECTION_AXIS..=MAXIMUM_SELECTION_AXIS).contains(&position.x)
        || !(-MAXIMUM_SELECTION_AXIS..=MAXIMUM_SELECTION_AXIS).contains(&position.y)
    {
        return Err(ProtocolError::InvalidPayload);
    }
    Ok(position)
}

fn decode_selection_geometry(
    reader: &mut PayloadReader<'_>,
) -> Result<ViewerSelectionGestureGeometry, ProtocolError> {
    let geometry = ViewerSelectionGestureGeometry {
        cell_width: reader.read_u16()?,
        columns: reader.read_u16()?,
        padding_left: reader.read_u16()?,
        screen_height: reader.read_u16()?,
    };
    if geometry.cell_width == 0
        || geometry.cell_width > MAXIMUM_SELECTION_DIMENSION
        || geometry.columns == 0
        || geometry.columns > MAXIMUM_SELECTION_DIMENSION
        || geometry.screen_height == 0
    {
        return Err(ProtocolError::InvalidPayload);
    }
    Ok(geometry)
}

const fn decode_bool(value: u8) -> Result<bool, ProtocolError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

pub(super) fn encode_result(value: &TerminalViewerResult) -> Vec<u8> {
    let mut bytes = vec![6];
    bytes.extend_from_slice(&value.request_id.as_bytes());
    bytes.extend_from_slice(&value.attachment_id.as_bytes());
    match value.outcome {
        TerminalViewerOutcome::Attached => bytes.push(1),
        TerminalViewerOutcome::Empty => bytes.push(2),
        TerminalViewerOutcome::FeedbackAccepted => bytes.push(3),
        TerminalViewerOutcome::LifecycleObserved {
            target,
            revision,
            settled,
        } => {
            bytes.push(7);
            super::encode_host_identity(&mut bytes, target);
            bytes.extend_from_slice(&revision.to_be_bytes());
            bytes.push(u8::from(settled));
        }
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
        7 => TerminalViewerOutcome::LifecycleObserved {
            target: super::decode_host_identity(&mut reader)?,
            revision: reader.read_u64()?,
            settled: match reader.read_u8()? {
                0 => false,
                1 => true,
                _ => return Err(ProtocolError::InvalidPayload),
            },
        },
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
