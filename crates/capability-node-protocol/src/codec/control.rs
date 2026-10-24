use super::{PayloadReader, decode_geometry, encode_geometry};
use crate::{
    CapabilityNodeMessage, ControlInput, ControlRejection, HostDefaultColors,
    MAXIMUM_PASTE_CONTENT_BYTES, OperationId, PasteAcceptedBytes, PasteContent, ProtocolError,
    ReadMessageError, TerminalControlAction as Action, TerminalControlOutcome as Outcome,
    TerminalControlRequest, TerminalControlResult, WorkerGeneration, WorkerId,
};
use std::io::Read;
use std::num::NonZeroU64;

const PASTE_RECORD_HEADER_BYTES: usize = 12;
const PASTE_RECORD_PREFIX: [u8; 8] = *b"OBNQ\0\x01\0\0";
const PASTE_RECORD_BODY_PREFIX_BYTES: usize = 80;
const PASTE_RECORD_MAXIMUM_BODY_BYTES: usize =
    PASTE_RECORD_BODY_PREFIX_BYTES + MAXIMUM_PASTE_CONTENT_BYTES;

pub(super) fn encode_paste_record(
    request: &TerminalControlRequest,
) -> Result<Vec<u8>, ProtocolError> {
    let Action::Paste { sequence, content } = &request.action else {
        return Err(ProtocolError::InvalidPayload);
    };
    let body_length = PASTE_RECORD_BODY_PREFIX_BYTES + content.as_bytes().len();
    let mut bytes = Vec::with_capacity(PASTE_RECORD_HEADER_BYTES + body_length);
    bytes.extend_from_slice(&PASTE_RECORD_PREFIX);
    bytes.extend_from_slice(
        &u32::try_from(body_length)
            .map_err(|_| ProtocolError::InvalidPayload)?
            .to_be_bytes(),
    );
    bytes.extend(request.request_id.as_bytes());
    bytes.extend(request.worker_id.as_bytes());
    bytes.extend_from_slice(&request.worker_generation.get().to_be_bytes());
    bytes.extend(request.viewer_id.as_bytes());
    bytes.extend(request.controller_id.as_bytes());
    bytes.extend(sequence.get().to_be_bytes());
    bytes.extend(content.as_bytes());
    Ok(bytes)
}

pub(super) fn decode_paste_record(record: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let header = record
        .get(..PASTE_RECORD_HEADER_BYTES)
        .ok_or(ProtocolError::InvalidPayload)?;
    let body_length = paste_record_body_length(header)?;
    if record.len() != PASTE_RECORD_HEADER_BYTES + body_length {
        return Err(ProtocolError::InvalidPayload);
    }
    decode_paste_record_body(&record[PASTE_RECORD_HEADER_BYTES..])
}

pub(super) fn read_paste_record(
    reader: &mut impl Read,
    prefix: [u8; 8],
) -> Result<CapabilityNodeMessage, ReadMessageError> {
    let mut header = [0; PASTE_RECORD_HEADER_BYTES];
    header[..8].copy_from_slice(&prefix);
    reader.read_exact(&mut header[8..])?;
    let body_length = paste_record_body_length(&header)?;
    let mut body = vec![0; body_length];
    reader.read_exact(&mut body)?;
    decode_paste_record_body(&body).map_err(Into::into)
}

fn paste_record_body_length(header: &[u8]) -> Result<usize, ProtocolError> {
    if header[..4] != PASTE_RECORD_PREFIX[..4] {
        return Err(ProtocolError::InvalidMagic);
    }
    let version = u16::from_be_bytes([header[4], header[5]]);
    if version != 1 {
        return Err(ProtocolError::IncompatibleVersion { received: version });
    }
    if header[6..8] != [0, 0] {
        return Err(ProtocolError::InvalidPayload);
    }
    let body_length = usize::try_from(u32::from_be_bytes(
        header[8..12]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    ))
    .map_err(|_| ProtocolError::InvalidPayload)?;
    if body_length <= PASTE_RECORD_BODY_PREFIX_BYTES {
        return Err(ProtocolError::InvalidPayload);
    }
    if body_length > PASTE_RECORD_MAXIMUM_BODY_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: body_length,
            maximum: PASTE_RECORD_MAXIMUM_BODY_BYTES,
        });
    }
    Ok(body_length)
}

fn decode_paste_record_body(body: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let request_id = OperationId::from_bytes(
        body[..16]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    )?;
    let worker_id = WorkerId::from_bytes(
        body[16..32]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    )?;
    let worker_generation = WorkerGeneration::new(positive(u64::from_be_bytes(
        body[32..40]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    ))?);
    let viewer_id = OperationId::from_bytes(
        body[40..56]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    )?;
    let controller_id = OperationId::from_bytes(
        body[56..72]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    )?;
    let sequence = positive(u64::from_be_bytes(
        body[72..80]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    ))?;
    let content = PasteContent::new(body[PASTE_RECORD_BODY_PREFIX_BYTES..].to_vec())?;
    Ok(CapabilityNodeMessage::TerminalControlRequest(
        TerminalControlRequest {
            request_id,
            worker_id,
            worker_generation,
            viewer_id,
            controller_id,
            action: Action::Paste { sequence, content },
        },
    ))
}

pub(super) fn encode_request(request: &TerminalControlRequest) -> Vec<u8> {
    let mut bytes = vec![7];
    bytes.extend(request.request_id.as_bytes());
    bytes.extend(request.worker_id.as_bytes());
    bytes.extend(request.worker_generation.get().to_be_bytes());
    bytes.extend(request.viewer_id.as_bytes());
    bytes.extend(request.controller_id.as_bytes());
    match &request.action {
        Action::Acquire => bytes.push(1),
        Action::Clear => bytes.push(6),
        Action::Takeover {
            previous_controller_id,
        } => {
            bytes.push(5);
            bytes.extend(previous_controller_id.as_bytes());
        }
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
        Action::HostColors { sequence, colors } => {
            bytes.push(7);
            bytes.extend(sequence.get().to_be_bytes());
            bytes.extend(colors.foreground().to_be_bytes());
            bytes.extend(colors.background().to_be_bytes());
        }
        Action::Paste { .. } => unreachable!("paste requests use OBNQ framing"),
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
        6 => Action::Clear,
        5 => Action::Takeover {
            previous_controller_id: OperationId::from_bytes(reader.read_array()?)?,
        },
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
        7 => Action::HostColors {
            sequence: positive(reader.read_u64()?)?,
            colors: HostDefaultColors::new(
                u32::from_be_bytes(reader.read_array()?),
                u32::from_be_bytes(reader.read_array()?),
            )?,
        },
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
        Outcome::Cleared {
            generation,
            revision,
            cleanup_confirmed,
        } => {
            bytes.push(7);
            bytes.extend(generation.get().to_be_bytes());
            bytes.extend(revision.get().to_be_bytes());
            bytes.push(u8::from(cleanup_confirmed));
        }
        Outcome::ClearOutcomeUnknown => bytes.push(8),
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
                ControlRejection::HostClearAlternateScreen => 12,
                ControlRejection::HostInputInvalid => 8,
                ControlRejection::HostInputLimitExceeded => 9,
                ControlRejection::HostInputBackpressure => 10,
                ControlRejection::HostInputUnavailable => 11,
            });
        }
        Outcome::Pasted { accepted_bytes } => {
            bytes.push(6);
            bytes.extend(accepted_bytes.get().to_be_bytes());
        }
        Outcome::HostColorsAccepted => bytes.push(9),
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
        7 => Outcome::Cleared {
            generation: positive(reader.read_u64()?)?,
            revision: positive(reader.read_u64()?)?,
            cleanup_confirmed: match reader.read_u8()? {
                0 => false,
                1 => true,
                _ => return Err(ProtocolError::InvalidPayload),
            },
        },
        8 => Outcome::ClearOutcomeUnknown,
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
            12 => ControlRejection::HostClearAlternateScreen,
            8 => ControlRejection::HostInputInvalid,
            9 => ControlRejection::HostInputLimitExceeded,
            10 => ControlRejection::HostInputBackpressure,
            11 => ControlRejection::HostInputUnavailable,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        6 => Outcome::Pasted {
            accepted_bytes: PasteAcceptedBytes::new(u32::from_be_bytes(reader.read_array()?))?,
        },
        9 => Outcome::HostColorsAccepted,
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
