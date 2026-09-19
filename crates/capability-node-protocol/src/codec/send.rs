use super::{PayloadReader, decode_host_identity, encode_host_identity};
use crate::{
    HostTerminalIdentity, OperationId, ProtocolError, SendAcceptedBytes, SendCleanupStatus,
    SendKey, SendRejection, SendText, SendUnavailable, TerminalSendOutcome,
    TerminalSendPayload, TerminalSendRequest, TerminalSendResult, WorkerGeneration, WorkerId,
};
use std::num::NonZeroU64;

const SEND_REQUEST_KIND: u8 = 13;
const SEND_RESULT_KIND: u8 = 14;
const TEXT_PAYLOAD: u8 = 1;
const KEY_PAYLOAD: u8 = 2;
const UNAVAILABLE_INPUT: u8 = 1;
const REJECTED_INPUT: u8 = 2;
const ACCEPTED_INPUT: u8 = 3;
const UNKNOWN_INPUT: u8 = 4;

pub(super) fn encode_request(request: &TerminalSendRequest) -> Result<Vec<u8>, ProtocolError> {
    let mut bytes = vec![SEND_REQUEST_KIND];
    bytes.extend(request.request_id.as_bytes());
    bytes.extend(request.worker_id.as_bytes());
    bytes.extend_from_slice(&request.worker_generation.get().to_be_bytes());
    encode_host_identity(&mut bytes, request.target);
    bytes.extend(request.attachment_id.as_bytes());
    match &request.payload {
        TerminalSendPayload::Text(input) => {
            // The combined text-plus-return bound is a wire invariant;
            // re-check it at the encode boundary regardless of construction.
            if input.record_bytes() > crate::MAXIMUM_SEND_TEXT_BYTES {
                return Err(ProtocolError::InvalidPayload);
            }
            bytes.push(TEXT_PAYLOAD);
            let text = input.text().as_str();
            bytes.extend(
                u16::try_from(text.len())
                    .map_err(|_| ProtocolError::InvalidPayload)?
                    .to_be_bytes(),
            );
            bytes.extend(text.as_bytes());
            bytes.push(u8::from(input.enter()));
        }
        TerminalSendPayload::Key { key } => {
            bytes.push(KEY_PAYLOAD);
            bytes.push(key.code());
        }
    }
    Ok(bytes)
}

pub(super) fn decode_request(bytes: &[u8]) -> Result<TerminalSendRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, SEND_REQUEST_KIND)?;
    let request_id = OperationId::from_bytes(reader.read_array()?)?;
    let worker_id = WorkerId::from_bytes(reader.read_array()?)?;
    let worker_generation = WorkerGeneration::new(
        NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
    );
    let target: HostTerminalIdentity = decode_host_identity(&mut reader)?;
    let attachment_id = OperationId::from_bytes(reader.read_array()?)?;
    let payload = match reader.read_u8()? {
        TEXT_PAYLOAD => {
            let length = usize::from(reader.read_u16()?);
            let text = std::str::from_utf8(reader.read_slice(length)?)
                .map_err(|_error| ProtocolError::InvalidPayload)?;
            let enter = match reader.read_u8()? {
                0 => false,
                1 => true,
                _ => return Err(ProtocolError::InvalidPayload),
            };
            TerminalSendPayload::new_text(SendText::new(text.to_string())?, enter)?
        }
        KEY_PAYLOAD => TerminalSendPayload::Key {
            key: SendKey::from_code(reader.read_u8()?)?,
        },
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(TerminalSendRequest {
        request_id,
        worker_id,
        worker_generation,
        target,
        attachment_id,
        payload,
    })
}

pub(super) fn encode_result(result: &TerminalSendResult) -> Result<Vec<u8>, ProtocolError> {
    result.outcome().validate()?;
    let mut bytes = vec![SEND_RESULT_KIND];
    bytes.extend(result.request_id().as_bytes());
    let outcome = result.outcome();
    match outcome {
        TerminalSendOutcome::Unavailable { reason, cleanup } => {
            bytes.push(UNAVAILABLE_INPUT);
            bytes.push(reason.code());
            bytes.push(cleanup.code());
        }
        TerminalSendOutcome::Rejected { reason, cleanup } => {
            bytes.push(REJECTED_INPUT);
            bytes.push(reason.code());
            bytes.push(cleanup.code());
        }
        TerminalSendOutcome::Accepted {
            accepted_bytes,
            cleanup,
        } => {
            bytes.push(ACCEPTED_INPUT);
            bytes.extend_from_slice(&accepted_bytes.get().to_be_bytes());
            bytes.push(cleanup.code());
        }
        TerminalSendOutcome::OutcomeUnknown { cleanup } => {
            bytes.push(UNKNOWN_INPUT);
            bytes.push(cleanup.code());
        }
    }
    Ok(bytes)
}

pub(super) fn decode_result(bytes: &[u8]) -> Result<TerminalSendResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, SEND_RESULT_KIND)?;
    let request_id = OperationId::from_bytes(reader.read_array()?)?;
    let outcome = match reader.read_u8()? {
        UNAVAILABLE_INPUT => TerminalSendOutcome::Unavailable {
            reason: SendUnavailable::from_code(reader.read_u8()?)?,
            cleanup: SendCleanupStatus::from_code(reader.read_u8()?)?,
        },
        REJECTED_INPUT => TerminalSendOutcome::Rejected {
            reason: SendRejection::from_code(reader.read_u8()?)?,
            cleanup: SendCleanupStatus::from_code(reader.read_u8()?)?,
        },
        ACCEPTED_INPUT => TerminalSendOutcome::Accepted {
            accepted_bytes: SendAcceptedBytes::new(reader.read_u32()?)?,
            cleanup: SendCleanupStatus::from_code(reader.read_u8()?)?,
        },
        UNKNOWN_INPUT => TerminalSendOutcome::OutcomeUnknown {
            cleanup: SendCleanupStatus::from_code(reader.read_u8()?)?,
        },
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    TerminalSendResult::new(request_id, outcome)
}
