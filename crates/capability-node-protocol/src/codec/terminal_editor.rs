//! Carries closed editor-preference requests and outcomes using the bounded literal
//! launch-selection encoding. It resolves no editor preference and creates no source document
//! or editor process.

use super::{PayloadReader, decode_launch_selection, encode_launch_selection};
use crate::{
    OperationId, ProtocolError, TerminalEditorCommandOutcome as Outcome,
    TerminalEditorCommandRefusal as Refusal, TerminalEditorCommandRequest as Request,
    TerminalEditorCommandResult as Result, TerminalEditorCommandSource as Source, WorkerGeneration,
    WorkerId,
};
use std::num::NonZeroU64;

fn encode_fields(bytes: &mut Vec<u8>, request: Request) {
    bytes.extend(request.request_id.as_bytes());
    bytes.extend(request.worker_id.as_bytes());
    bytes.extend(request.worker_generation.get().to_be_bytes());
}

fn decode_fields(reader: &mut PayloadReader<'_>) -> std::result::Result<Request, ProtocolError> {
    Ok(Request {
        request_id: OperationId::from_bytes(reader.read_array()?)?,
        worker_id: WorkerId::from_bytes(reader.read_array()?)?,
        worker_generation: WorkerGeneration::new(
            NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
        ),
    })
}

pub(super) fn encode_request(request: Request) -> Vec<u8> {
    let mut bytes = vec![24];
    encode_fields(&mut bytes, request);
    bytes
}

pub(super) fn decode_request(bytes: &[u8]) -> std::result::Result<Request, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 24)?;
    let request = decode_fields(&mut reader)?;
    reader.finish()?;
    Ok(request)
}

pub(super) fn encode_result(result: &Result) -> Vec<u8> {
    let mut bytes = vec![25];
    encode_fields(&mut bytes, result.request);
    match &result.outcome {
        Outcome::Resolved { source, selection } => {
            bytes.push(1);
            bytes.push(match source {
                Source::Visual => 1,
                Source::Editor => 2,
                Source::Default => 3,
            });
            encode_launch_selection(&mut bytes, selection);
        }
        Outcome::Refused(reason) => {
            bytes.push(2);
            bytes.push(match reason {
                Refusal::WrongWorker => 1,
                Refusal::StaleWorkerGeneration => 2,
                Refusal::UnsupportedPlatform => 3,
                Refusal::InvalidCommand => 4,
                Refusal::ProgramUnavailable => 5,
            });
        }
    }
    bytes
}

pub(super) fn decode_result(bytes: &[u8]) -> std::result::Result<Result, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, 25)?;
    let request = decode_fields(&mut reader)?;
    let outcome = match reader.read_u8()? {
        1 => {
            let source = match reader.read_u8()? {
                1 => Source::Visual,
                2 => Source::Editor,
                3 => Source::Default,
                _ => return Err(ProtocolError::InvalidPayload),
            };
            Outcome::Resolved {
                source,
                selection: decode_launch_selection(&mut reader)?,
            }
        }
        2 => Outcome::Refused(match reader.read_u8()? {
            1 => Refusal::WrongWorker,
            2 => Refusal::StaleWorkerGeneration,
            3 => Refusal::UnsupportedPlatform,
            4 => Refusal::InvalidCommand,
            5 => Refusal::ProgramUnavailable,
            _ => return Err(ProtocolError::InvalidPayload),
        }),
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    Ok(Result { request, outcome })
}
