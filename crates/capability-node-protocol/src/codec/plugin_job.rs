//! Encodes bounded plugin and configured-command job operands and captured-output receipts in
//! OBPJ records. It preserves job and Worker identities without selecting commands, spawning
//! processes, or retaining job history.

use super::{CapabilityNodeMessage, PayloadReader};
use crate::{
    OperationId, ProtocolError, ReadMessageError, WorkerGeneration, WorkerId,
    plugin_job::{
        ConfiguredCommandIntent, MAXIMUM_PLUGIN_LOG_PAGE, MAXIMUM_PLUGIN_OUTPUT_BYTES,
        PluginCommandFailure, PluginCommandIntent, PluginCommandLog, PluginJobAction,
        PluginJobOutcome, PluginJobRefusal, PluginJobRequest, PluginJobResult,
    },
};
use std::{io::Read, num::NonZeroU64};
const MAX_BODY: usize = 2 * 1024 * 1024;

pub(super) fn encode(message: &CapabilityNodeMessage) -> Result<Vec<u8>, ProtocolError> {
    let mut body = Vec::new();
    match message {
        CapabilityNodeMessage::PluginJobRequest(value) => {
            value.action.validate()?;
            body.push(1);
            identity(
                &mut body,
                value.operation_id,
                value.worker_id,
                value.worker_generation,
            );
            match &value.action {
                PluginJobAction::StartConfigured(intent) => {
                    body.push(5);
                    string(&mut body, &intent.command_id);
                    string(&mut body, &intent.directory);
                    string(&mut body, &intent.command);
                    body.push(intent.environment.len() as u8);
                    for (key, val) in &intent.environment {
                        string(&mut body, key);
                        string(&mut body, val);
                    }
                }
                PluginJobAction::Start(intent) => {
                    body.push(1);
                    string(&mut body, &intent.plugin_id);
                    string(&mut body, &intent.command_id);
                    string(&mut body, &intent.directory);
                    strings(&mut body, &intent.argv);
                    body.push(intent.environment.len() as u8);
                    for (key, val) in &intent.environment {
                        string(&mut body, key);
                        string(&mut body, val);
                    }
                }
                PluginJobAction::List {
                    plugin_id,
                    before,
                    limit,
                } => {
                    body.push(2);
                    optional_string(&mut body, plugin_id);
                    optional_u64(&mut body, *before);
                    body.push(*limit);
                }
                PluginJobAction::Get { job_id } => {
                    body.push(3);
                    body.extend(job_id.as_bytes());
                }
                PluginJobAction::Cancel { job_id } => {
                    body.push(4);
                    body.extend(job_id.as_bytes());
                }
            }
        }
        CapabilityNodeMessage::PluginJobResult(value) => {
            value.validate()?;
            body.push(2);
            identity(
                &mut body,
                value.operation_id,
                value.worker_id,
                value.worker_generation,
            );
            match &value.outcome {
                PluginJobOutcome::ConfiguredStarted { job_id, existing } => {
                    body.extend([5, u8::from(*existing)]);
                    body.extend(job_id.as_bytes());
                }
                PluginJobOutcome::Started { log, existing } => {
                    body.extend([1, u8::from(*existing)]);
                    encode_log(&mut body, log);
                }
                PluginJobOutcome::Logs { logs, next_before } => {
                    body.extend([2, logs.len() as u8]);
                    for log in logs {
                        encode_log(&mut body, log);
                    }
                    optional_u64(&mut body, *next_before);
                }
                PluginJobOutcome::Cancellation { already_complete } => {
                    body.extend([3, u8::from(*already_complete)])
                }
                PluginJobOutcome::Refused(reason) => body.extend([
                    4,
                    match reason {
                        PluginJobRefusal::RejectedFencing => 1,
                        PluginJobRefusal::InvalidIntent => 2,
                        PluginJobRefusal::Busy => 3,
                        PluginJobRefusal::Conflict => 4,
                        PluginJobRefusal::Unavailable => 5,
                        PluginJobRefusal::PhysicalUnknown => 6,
                        PluginJobRefusal::NotFound => 7,
                        PluginJobRefusal::Unsupported => 8,
                    },
                ]),
            }
        }
        // This codec selects one OBCN message family; other families use their own codecs.
        _ => return Err(ProtocolError::InvalidPayload),
    }
    if body.len() > MAX_BODY {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut frame = b"OBPJ\0\x01\0\0".to_vec();
    frame.extend((body.len() as u32).to_be_bytes());
    frame.extend(body);
    Ok(frame)
}
fn identity(b: &mut Vec<u8>, op: OperationId, worker: WorkerId, generation: WorkerGeneration) {
    b.extend(op.as_bytes());
    b.extend(worker.as_bytes());
    b.extend(generation.get().to_be_bytes());
}
fn bytes(b: &mut Vec<u8>, v: &[u8]) {
    b.extend((v.len() as u32).to_be_bytes());
    b.extend(v);
}
fn string(b: &mut Vec<u8>, v: &str) {
    bytes(b, v.as_bytes());
}
fn strings(b: &mut Vec<u8>, v: &[String]) {
    b.extend((v.len() as u16).to_be_bytes());
    for value in v {
        string(b, value);
    }
}
fn optional_string(b: &mut Vec<u8>, v: &Option<String>) {
    b.push(u8::from(v.is_some()));
    if let Some(v) = v {
        string(b, v);
    }
}
fn optional_u64(b: &mut Vec<u8>, v: Option<u64>) {
    b.push(u8::from(v.is_some()));
    if let Some(v) = v {
        b.extend(v.to_be_bytes());
    }
}
fn encode_log(b: &mut Vec<u8>, log: &PluginCommandLog) {
    b.extend(log.job_id.as_bytes());
    b.extend(log.sequence.to_be_bytes());
    string(b, &log.plugin_id);
    string(b, &log.command_id);
    strings(b, &log.command);
    b.extend(log.started_ms.to_be_bytes());
    optional_u64(b, log.finished_ms);
    b.push(u8::from(log.exit_code.is_some()));
    if let Some(v) = log.exit_code {
        b.extend(v.to_be_bytes());
    }
    b.push(match log.failure {
        None => 0,
        Some(PluginCommandFailure::ExitFailure) => 1,
        Some(PluginCommandFailure::SpawnUnavailable) => 2,
        Some(PluginCommandFailure::SpawnFailed) => 3,
        Some(PluginCommandFailure::Io) => 4,
        Some(PluginCommandFailure::Cancelled) => 5,
        Some(PluginCommandFailure::PhysicalUnknown) => 6,
    });
    bytes(b, &log.stdout);
    bytes(b, &log.stderr);
    b.extend([
        u8::from(log.stdout_truncated),
        u8::from(log.stderr_truncated),
    ]);
}
fn length(header: &[u8]) -> Result<usize, ProtocolError> {
    if header.len() != 12 || &header[..5] != b"OBPJ\0" || header[5] != 1 || header[6..8] != [0, 0] {
        return Err(ProtocolError::InvalidPayload);
    }
    let length = u32::from_be_bytes(
        header[8..12]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    ) as usize;
    if length == 0 || length > MAX_BODY {
        return Err(ProtocolError::InvalidPayload);
    }
    Ok(length)
}
pub(super) fn decode(frame: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let size = length(frame.get(..12).ok_or(ProtocolError::InvalidPayload)?)?;
    if frame.len() != 12 + size {
        return Err(ProtocolError::InvalidPayload);
    }
    decode_body(&frame[12..])
}
pub(super) fn read(
    reader: &mut impl Read,
    prefix: [u8; 8],
) -> Result<CapabilityNodeMessage, ReadMessageError> {
    let mut header = [0; 12];
    header[..8].copy_from_slice(&prefix);
    reader.read_exact(&mut header[8..])?;
    let size = length(&header)?;
    let mut body = vec![0; size];
    reader.read_exact(&mut body)?;
    decode_body(&body).map_err(Into::into)
}
fn take_bytes(r: &mut PayloadReader<'_>, cap: usize) -> Result<Vec<u8>, ProtocolError> {
    let n = r.read_u32()? as usize;
    if n > cap {
        return Err(ProtocolError::InvalidPayload);
    }
    Ok(r.read_slice(n)?.to_vec())
}
fn take_string(r: &mut PayloadReader<'_>, cap: usize) -> Result<String, ProtocolError> {
    String::from_utf8(take_bytes(r, cap)?).map_err(|_| ProtocolError::InvalidPayload)
}
fn boolean(r: &mut PayloadReader<'_>) -> Result<bool, ProtocolError> {
    match r.read_u8()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ProtocolError::InvalidPayload),
    }
}
fn take_optional_u64(r: &mut PayloadReader<'_>) -> Result<Option<u64>, ProtocolError> {
    if boolean(r)? {
        Ok(Some(r.read_u64()?))
    } else {
        Ok(None)
    }
}
fn take_strings(r: &mut PayloadReader<'_>) -> Result<Vec<String>, ProtocolError> {
    let n = r.read_u16()?;
    if n > 128 {
        return Err(ProtocolError::InvalidPayload);
    }
    (0..n).map(|_| take_string(r, 4096)).collect()
}
fn decode_body(body: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    let kind = *body.first().ok_or(ProtocolError::InvalidPayload)?;
    let mut r = PayloadReader::after_kind(body, kind)?;
    let operation_id = OperationId::from_bytes(r.read_array()?)?;
    let worker_id = WorkerId::from_bytes(r.read_array()?)?;
    let worker_generation =
        WorkerGeneration::new(NonZeroU64::new(r.read_u64()?).ok_or(ProtocolError::InvalidPayload)?);
    let message = match kind {
        1 => {
            let action = match r.read_u8()? {
                5 => {
                    let command_id = take_string(&mut r, 120)?;
                    let directory = take_string(&mut r, 4096)?;
                    let command = take_string(&mut r, 4096)?;
                    let count = r.read_u8()?;
                    if count > 32 {
                        return Err(ProtocolError::InvalidPayload);
                    }
                    let mut environment = Vec::new();
                    for _ in 0..count {
                        environment
                            .push((take_string(&mut r, 120)?, take_string(&mut r, 16 * 1024)?));
                    }
                    PluginJobAction::StartConfigured(ConfiguredCommandIntent {
                        command_id,
                        directory,
                        command,
                        environment,
                    })
                }
                1 => {
                    let plugin_id = take_string(&mut r, 241)?;
                    let command_id = take_string(&mut r, 241)?;
                    let directory = take_string(&mut r, 4096)?;
                    let argv = take_strings(&mut r)?;
                    let count = r.read_u8()?;
                    if count > 32 {
                        return Err(ProtocolError::InvalidPayload);
                    }
                    let mut environment = Vec::new();
                    for _ in 0..count {
                        environment
                            .push((take_string(&mut r, 120)?, take_string(&mut r, 16 * 1024)?));
                    }
                    PluginJobAction::Start(PluginCommandIntent {
                        plugin_id,
                        command_id,
                        directory,
                        argv,
                        environment,
                    })
                }
                2 => {
                    let plugin_id = if boolean(&mut r)? {
                        Some(take_string(&mut r, 241)?)
                    } else {
                        None
                    };
                    let before = take_optional_u64(&mut r)?;
                    let limit = r.read_u8()?;
                    PluginJobAction::List {
                        plugin_id,
                        before,
                        limit,
                    }
                }
                3 => PluginJobAction::Get {
                    job_id: OperationId::from_bytes(r.read_array()?)?,
                },
                4 => PluginJobAction::Cancel {
                    job_id: OperationId::from_bytes(r.read_array()?)?,
                },
                _ => return Err(ProtocolError::InvalidPayload),
            };
            action.validate()?;
            CapabilityNodeMessage::PluginJobRequest(PluginJobRequest {
                operation_id,
                worker_id,
                worker_generation,
                action,
            })
        }
        2 => {
            let outcome = match r.read_u8()? {
                5 => PluginJobOutcome::ConfiguredStarted {
                    existing: boolean(&mut r)?,
                    job_id: OperationId::from_bytes(r.read_array()?)?,
                },
                1 => {
                    let existing = boolean(&mut r)?;
                    PluginJobOutcome::Started {
                        log: Box::new(decode_log(&mut r)?),
                        existing,
                    }
                }
                2 => {
                    let n = r.read_u8()?;
                    if usize::from(n) > MAXIMUM_PLUGIN_LOG_PAGE {
                        return Err(ProtocolError::InvalidPayload);
                    }
                    let mut logs = Vec::new();
                    for _ in 0..n {
                        logs.push(decode_log(&mut r)?);
                    }
                    PluginJobOutcome::Logs {
                        logs,
                        next_before: take_optional_u64(&mut r)?,
                    }
                }
                3 => PluginJobOutcome::Cancellation {
                    already_complete: boolean(&mut r)?,
                },
                4 => PluginJobOutcome::Refused(match r.read_u8()? {
                    1 => PluginJobRefusal::RejectedFencing,
                    2 => PluginJobRefusal::InvalidIntent,
                    3 => PluginJobRefusal::Busy,
                    4 => PluginJobRefusal::Conflict,
                    5 => PluginJobRefusal::Unavailable,
                    6 => PluginJobRefusal::PhysicalUnknown,
                    7 => PluginJobRefusal::NotFound,
                    8 => PluginJobRefusal::Unsupported,
                    _ => return Err(ProtocolError::InvalidPayload),
                }),
                _ => return Err(ProtocolError::InvalidPayload),
            };
            let result = PluginJobResult {
                operation_id,
                worker_id,
                worker_generation,
                outcome,
            };
            result.validate()?;
            CapabilityNodeMessage::PluginJobResult(result)
        }
        _ => return Err(ProtocolError::InvalidPayload),
    };
    r.finish()?;
    Ok(message)
}
fn decode_log(r: &mut PayloadReader<'_>) -> Result<PluginCommandLog, ProtocolError> {
    let job_id = OperationId::from_bytes(r.read_array()?)?;
    let sequence = r.read_u64()?;
    let plugin_id = take_string(r, 241)?;
    let command_id = take_string(r, 241)?;
    let command = take_strings(r)?;
    let started_ms = r.read_u64()?;
    let finished_ms = take_optional_u64(r)?;
    let exit_code = if boolean(r)? {
        Some(i32::from_be_bytes(r.read_array()?))
    } else {
        None
    };
    let failure = match r.read_u8()? {
        0 => None,
        1 => Some(PluginCommandFailure::ExitFailure),
        2 => Some(PluginCommandFailure::SpawnUnavailable),
        3 => Some(PluginCommandFailure::SpawnFailed),
        4 => Some(PluginCommandFailure::Io),
        5 => Some(PluginCommandFailure::Cancelled),
        6 => Some(PluginCommandFailure::PhysicalUnknown),
        _ => return Err(ProtocolError::InvalidPayload),
    };
    let stdout = take_bytes(r, MAXIMUM_PLUGIN_OUTPUT_BYTES)?;
    let stderr = take_bytes(r, MAXIMUM_PLUGIN_OUTPUT_BYTES)?;
    let stdout_truncated = boolean(r)?;
    let stderr_truncated = boolean(r)?;
    let log = PluginCommandLog {
        job_id,
        sequence,
        plugin_id,
        command_id,
        command,
        started_ms,
        finished_ms,
        exit_code,
        failure,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    };
    log.validate()?;
    Ok(log)
}
