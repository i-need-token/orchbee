use super::{PayloadReader, decode_geometry, encode_geometry};
use crate::{
    ControlInput, ControlRejection, OperationId, ProtocolError, TerminalControlAction as Action,
    TerminalControlOutcome as Outcome, TerminalControlRequest, TerminalControlResult,
    WorkerGeneration, WorkerId,
};
use std::num::NonZeroU64;
pub(super) fn encode_request(request: &TerminalControlRequest) -> Vec<u8> {
    let mut bytes = vec![7];
    bytes.extend(request.request_id.as_bytes());
    bytes.extend(request.worker_id.as_bytes());
    bytes.extend(request.worker_generation.get().to_be_bytes());
    bytes.extend(request.viewer_id.as_bytes());
    bytes.extend(request.controller_id.as_bytes());
    match &request.action {
        Action::Acquire => bytes.push(1),
        Action::Input {
            sequence,
            bytes: input,
        } => {
            bytes.push(2);
            bytes.extend(sequence.get().to_be_bytes());
            bytes.extend(
                u16::try_from(input.as_bytes().len())
                    .expect("bounded control input")
                    .to_be_bytes(),
            );
            bytes.extend(input.as_bytes());
        }
        Action::Resize { sequence, geometry } => {
            bytes.push(3);
            bytes.extend(sequence.get().to_be_bytes());
            encode_geometry(&mut bytes, *geometry);
        }
        Action::Release => bytes.push(4),
    }
    bytes
}
pub(super) fn decode_request(bytes: &[u8]) -> Result<TerminalControlRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 7)?;
    let request_id = OperationId::from_bytes(reader.read_array()?)?;
    let worker_id = WorkerId::from_bytes(reader.read_array()?)?;
    let worker_generation = WorkerGeneration::new(positive(reader.read_u64()?)?);
    let viewer_id = OperationId::from_bytes(reader.read_array()?)?;
    let controller_id = OperationId::from_bytes(reader.read_array()?)?;
    let action = match reader.read_u8()? {
        1 => Action::Acquire,
        2 => {
            let sequence = positive(reader.read_u64()?)?;
            let length = usize::from(reader.read_u16()?);
            Action::Input {
                sequence,
                bytes: ControlInput::new(reader.read_slice(length)?.to_vec())?,
            }
        }
        3 => Action::Resize {
            sequence: positive(reader.read_u64()?)?,
            geometry: decode_geometry(&mut reader)?,
        },
        4 => Action::Release,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(TerminalControlRequest {
        request_id,
        worker_id,
        worker_generation,
        viewer_id,
        controller_id,
        action,
    })
}
pub(super) fn encode_result(result: TerminalControlResult) -> Vec<u8> {
    let mut bytes = vec![8];
    bytes.extend(result.request_id.as_bytes());
    bytes.extend(result.viewer_id.as_bytes());
    bytes.extend(result.controller_id.as_bytes());
    match result.outcome {
        Outcome::Acquired => bytes.push(1),
        Outcome::Released => bytes.push(2),
        Outcome::InputAccepted => bytes.push(3),
        Outcome::Resized(geometry) => {
            bytes.push(4);
            encode_geometry(&mut bytes, geometry);
        }
        Outcome::Rejected(reason) => {
            bytes.push(5);
            bytes.push(match reason {
                ControlRejection::WrongWorker => 1,
                ControlRejection::StaleWorkerGeneration => 2,
                ControlRejection::ViewerUnavailable => 3,
                ControlRejection::ControllerBusy => 4,
                ControlRejection::NotController => 5,
                ControlRejection::SequenceMismatch => 6,
                ControlRejection::HostRejected => 7,
            });
        }
    }
    bytes
}
pub(super) fn decode_result(bytes: &[u8]) -> Result<TerminalControlResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 8)?;
    let request_id = OperationId::from_bytes(reader.read_array()?)?;
    let viewer_id = OperationId::from_bytes(reader.read_array()?)?;
    let controller_id = OperationId::from_bytes(reader.read_array()?)?;
    let outcome = match reader.read_u8()? {
        1 => Outcome::Acquired,
        2 => Outcome::Released,
        3 => Outcome::InputAccepted,
        4 => Outcome::Resized(decode_geometry(&mut reader)?),
        5 => Outcome::Rejected(match reader.read_u8()? {
            1 => ControlRejection::WrongWorker,
            2 => ControlRejection::StaleWorkerGeneration,
            3 => ControlRejection::ViewerUnavailable,
            4 => ControlRejection::ControllerBusy,
            5 => ControlRejection::NotController,
            6 => ControlRejection::SequenceMismatch,
            7 => ControlRejection::HostRejected,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(TerminalControlResult {
        request_id,
        viewer_id,
        controller_id,
        outcome,
    })
}
fn positive(value: u64) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(value).ok_or(ProtocolError::InvalidPayload)
}
