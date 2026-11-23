//! The `handover` capability's requests that move a terminal between two Hosts, from the quiesce
//! to the claim's undo, and their answers. A Worker encodes the requests and decodes the answers;
//! a handover-record chunk is binary, a 32-byte header naming the record and the chunk's place,
//! then the chunk, and every other payload is control JSON.

use serde::{Deserialize, Serialize};

use super::{HandoverControl, HandoverProcess, ProcessWire, invalid_field, process_to_wire};
use crate::{
    CreationId, MAXIMUM_HANDOVER_RECORD_CHUNK_BYTES, PayloadEncoding, ProtocolError, TerminalId,
    control::{SafeInteger, decode_wire, serialize_wire},
    terminal_fact::{TerminalIdentityWire, identity_from_wire},
};

pub(super) const QUIESCE_TERMINAL_KIND: u16 = 95;
pub(super) const READ_HANDOVER_RECORD_KIND: u16 = 96;
pub(super) const HANDOVER_RECORD_CHUNK_KIND: u16 = 97;
pub(super) const RESUME_TERMINAL_KIND: u16 = 98;
pub(super) const DROP_TERMINAL_KIND: u16 = 99;
pub(super) const IMPORT_TERMINAL_KIND: u16 = 100;
pub(super) const WRITE_HANDOVER_RECORD_KIND: u16 = 101;
pub(super) const HANDOVER_RECORD_WRITTEN_KIND: u16 = 102;
pub(super) const RESTORE_TERMINAL_KIND: u16 = 103;
pub(super) const DISCARD_IMPORT_KIND: u16 = 105;

/// A record is at most 128 MiB.
pub const MAXIMUM_HANDOVER_RECORD_SIZE: u32 = 128 * 1024 * 1024;
const RECORD_HEADER_BYTES: usize = 32;

/// `QuiesceTerminal`: the terminal to export, the Host its mark names, and the mark's deadline,
/// from 1 to 30 seconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuiesceTerminal {
    pub terminal_id: TerminalId,
    pub target: HandoverProcess,
    pub deadline_seconds: u8,
}

/// `QuiesceTerminal`'s answer: the exported record's size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalQuiesced {
    pub terminal_id: TerminalId,
    pub record_bytes: u32,
}

/// `ReadHandoverRecord`: the next chunk of a terminal's exported record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadHandoverRecord {
    pub terminal_id: TerminalId,
    pub offset: u32,
}

/// One chunk of a record: read from the exporting Host, `id` names its terminal; written to the
/// importing Host, it names the import's creation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandoverRecordChunk {
    pub id: [u8; 16],
    pub offset: u32,
    pub chunk: Vec<u8>,
}

/// What `ResumeTerminal` found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResumeOutcome {
    Resumed,
    /// Another Host claimed the terminal, so this Host holds it as moved.
    ClaimedElsewhere,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalResumed {
    pub terminal_id: TerminalId,
    pub outcome: ResumeOutcome,
}

/// `ImportTerminal`: room for a record of `record_bytes` under `creation_id`, which the import's
/// deadline, from 1 to 30 seconds, discards if no claim follows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImportTerminal {
    pub creation_id: CreationId,
    pub record_bytes: u32,
    pub deadline_seconds: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandoverRecordWritten {
    pub creation_id: CreationId,
    pub received_bytes: u32,
}

/// What `DiscardImport` names: an import not yet restored, or a restored terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscardTarget {
    Import(CreationId),
    Terminal(TerminalId),
}

/// What `DiscardImport` found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscardOutcome {
    Discarded,
    /// The terminal was claimed, so it stays.
    Claimed,
}

/// A move request's kind, payload encoding, and payload.
pub(super) type EncodedRequest = (u16, PayloadEncoding, Vec<u8>);

/// Encodes a move request; `None` for a variant that is not one.
pub(super) fn encode_request(
    value: &HandoverControl,
) -> Option<Result<EncodedRequest, ProtocolError>> {
    let json = |kind: u16, payload: Result<Vec<u8>, ProtocolError>| {
        Some(payload.map(|payload| (kind, PayloadEncoding::ControlJson, payload)))
    };
    match value {
        HandoverControl::QuiesceTerminal(quiesce) => {
            if let Err(error) = deadline(quiesce.deadline_seconds) {
                return Some(Err(error));
            }
            json(
                QUIESCE_TERMINAL_KIND,
                serialize_wire(&QuiesceWire {
                    deadline_seconds: SafeInteger::new(u64::from(quiesce.deadline_seconds)),
                    target: process_to_wire(quiesce.target),
                    terminal_id: quiesce.terminal_id.to_text(),
                }),
            )
        }
        HandoverControl::ReadHandoverRecord(read) => {
            if read.offset >= MAXIMUM_HANDOVER_RECORD_SIZE {
                return Some(Err(invalid_field("a record offset lies past 128 MiB")));
            }
            json(
                READ_HANDOVER_RECORD_KIND,
                serialize_wire(&ReadWire {
                    offset: SafeInteger::new(u64::from(read.offset)),
                    terminal_id: read.terminal_id.to_text(),
                }),
            )
        }
        HandoverControl::ResumeTerminal(terminal) => json(
            RESUME_TERMINAL_KIND,
            serialize_wire(&TargetWire {
                terminal_id: terminal.to_text(),
            }),
        ),
        HandoverControl::DropTerminal(terminal) => json(
            DROP_TERMINAL_KIND,
            serialize_wire(&TargetWire {
                terminal_id: terminal.to_text(),
            }),
        ),
        HandoverControl::ImportTerminal(import) => {
            if let Err(error) = deadline(import.deadline_seconds).and(size(import.record_bytes)) {
                return Some(Err(error));
            }
            json(
                IMPORT_TERMINAL_KIND,
                serialize_wire(&ImportWire {
                    creation_id: import.creation_id.to_text(),
                    deadline_seconds: SafeInteger::new(u64::from(import.deadline_seconds)),
                    record_bytes: SafeInteger::new(u64::from(import.record_bytes)),
                }),
            )
        }
        HandoverControl::WriteHandoverRecord(chunk) => Some(encode_chunk(chunk).map(|payload| {
            (
                WRITE_HANDOVER_RECORD_KIND,
                PayloadEncoding::HandoverRecord,
                payload,
            )
        })),
        HandoverControl::RestoreTerminal(creation) => json(
            RESTORE_TERMINAL_KIND,
            serialize_wire(&CreationWire {
                creation_id: creation.to_text(),
            }),
        ),
        HandoverControl::DiscardImport(target) => json(
            DISCARD_IMPORT_KIND,
            serialize_wire(&match target {
                DiscardTarget::Import(creation) => DiscardWire {
                    creation_id: Some(creation.to_text()),
                    terminal_id: None,
                },
                DiscardTarget::Terminal(terminal) => DiscardWire {
                    creation_id: None,
                    terminal_id: Some(terminal.to_text()),
                },
            }),
        ),
        _ => None,
    }
}

/// Decodes the answer of kind `kind` to a move request; `None` for a kind that answers none.
pub(super) fn decode_answer(
    kind: u16,
    payload: &[u8],
) -> Option<Result<HandoverControl, ProtocolError>> {
    Some(match kind {
        QUIESCE_TERMINAL_KIND => decode_wire::<QuiescedWire>(payload).and_then(|wire| {
            let record_bytes = u32::try_from(wire.record_bytes.get())
                .map_err(|_error| invalid_field("a record size is past 128 MiB"))?;
            size(record_bytes)?;
            Ok(HandoverControl::TerminalQuiesced(TerminalQuiesced {
                terminal_id: terminal(&wire.terminal_id)?,
                record_bytes,
            }))
        }),
        HANDOVER_RECORD_CHUNK_KIND => {
            decode_chunk(payload).map(HandoverControl::HandoverRecordChunk)
        }
        RESUME_TERMINAL_KIND => decode_wire::<ResumedWire>(payload).and_then(|wire| {
            let outcome = match wire.outcome.as_str() {
                "resumed" => ResumeOutcome::Resumed,
                "claimed-elsewhere" => ResumeOutcome::ClaimedElsewhere,
                _ => return Err(invalid_field("a resume answers an unknown outcome")),
            };
            Ok(HandoverControl::TerminalResumed(TerminalResumed {
                terminal_id: terminal(&wire.terminal_id)?,
                outcome,
            }))
        }),
        DROP_TERMINAL_KIND => decode_wire::<TargetWire>(payload)
            .and_then(|wire| terminal(&wire.terminal_id))
            .map(HandoverControl::TerminalDropped),
        IMPORT_TERMINAL_KIND => decode_wire::<CreationWire>(payload)
            .and_then(|wire| creation(&wire.creation_id))
            .map(HandoverControl::TerminalImported),
        HANDOVER_RECORD_WRITTEN_KIND => decode_wire::<WrittenWire>(payload).and_then(|wire| {
            let received_bytes = u32::try_from(wire.received_bytes.get())
                .map_err(|_error| invalid_field("a written size is past 128 MiB"))?;
            size(received_bytes)?;
            Ok(HandoverControl::HandoverRecordWritten(
                HandoverRecordWritten {
                    creation_id: creation(&wire.creation_id)?,
                    received_bytes,
                },
            ))
        }),
        RESTORE_TERMINAL_KIND => decode_wire::<TerminalIdentityWire>(payload)
            .and_then(identity_from_wire)
            .map(HandoverControl::TerminalRestored),
        DISCARD_IMPORT_KIND => {
            decode_wire::<DiscardedWire>(payload).and_then(|wire| match wire.outcome.as_str() {
                "discarded" => Ok(HandoverControl::ImportDiscarded(DiscardOutcome::Discarded)),
                "claimed" => Ok(HandoverControl::ImportDiscarded(DiscardOutcome::Claimed)),
                _ => Err(invalid_field("a discard answers an unknown outcome")),
            })
        }
        _ => return None,
    })
}

/// The answer kind each request expects: its own, except `ReadHandoverRecord` and
/// `WriteHandoverRecord`.
pub(super) const fn answer_kind(request: u16) -> u16 {
    match request {
        READ_HANDOVER_RECORD_KIND => HANDOVER_RECORD_CHUNK_KIND,
        WRITE_HANDOVER_RECORD_KIND => HANDOVER_RECORD_WRITTEN_KIND,
        other => other,
    }
}

fn encode_chunk(chunk: &HandoverRecordChunk) -> Result<Vec<u8>, ProtocolError> {
    let length = u32::try_from(chunk.chunk.len())
        .ok()
        .filter(|length| {
            *length > 0
                && usize::try_from(*length)
                    .is_ok_and(|length| length <= MAXIMUM_HANDOVER_RECORD_CHUNK_BYTES)
        })
        .ok_or_else(|| invalid_field("a record chunk holds 1 byte to 4 MiB"))?;
    if u64::from(chunk.offset) + u64::from(length) > u64::from(MAXIMUM_HANDOVER_RECORD_SIZE) {
        return Err(invalid_field("a record chunk ends past 128 MiB"));
    }
    let mut payload = Vec::with_capacity(RECORD_HEADER_BYTES + chunk.chunk.len());
    payload.extend_from_slice(&chunk.id);
    payload.extend_from_slice(&u64::from(chunk.offset).to_le_bytes());
    payload.extend_from_slice(&length.to_le_bytes());
    payload.extend_from_slice(&0_u32.to_le_bytes());
    payload.extend_from_slice(&chunk.chunk);
    Ok(payload)
}

fn decode_chunk(payload: &[u8]) -> Result<HandoverRecordChunk, ProtocolError> {
    let invalid = || invalid_field("a record chunk's header is out of contract");
    let (header, chunk) = payload
        .split_at_checked(RECORD_HEADER_BYTES)
        .ok_or_else(invalid)?;
    let field = |range: std::ops::Range<usize>| header.get(range).ok_or_else(invalid);
    let id: [u8; 16] = field(0..16)?.try_into().map_err(|_error| invalid())?;
    let offset = u64::from_le_bytes(field(16..24)?.try_into().map_err(|_error| invalid())?);
    let length = u32::from_le_bytes(field(24..28)?.try_into().map_err(|_error| invalid())?);
    let reserved = u32::from_le_bytes(field(28..32)?.try_into().map_err(|_error| invalid())?);
    let fits = reserved == 0
        && length != 0
        && usize::try_from(length).is_ok_and(|length| {
            length == chunk.len() && length <= MAXIMUM_HANDOVER_RECORD_CHUNK_BYTES
        })
        && offset + u64::from(length) <= u64::from(MAXIMUM_HANDOVER_RECORD_SIZE);
    if !fits {
        return Err(invalid());
    }
    Ok(HandoverRecordChunk {
        id,
        offset: u32::try_from(offset).map_err(|_error| invalid())?,
        chunk: chunk.to_vec(),
    })
}

fn deadline(seconds: u8) -> Result<(), ProtocolError> {
    if (1..=super::MAXIMUM_DEADLINE_SECONDS).contains(&seconds) {
        Ok(())
    } else {
        Err(invalid_field("a move's deadline is from 1 to 30 seconds"))
    }
}

fn size(bytes: u32) -> Result<(), ProtocolError> {
    if bytes == 0 || bytes > MAXIMUM_HANDOVER_RECORD_SIZE {
        return Err(invalid_field("a record holds 1 byte to 128 MiB"));
    }
    Ok(())
}

fn terminal(value: &str) -> Result<TerminalId, ProtocolError> {
    TerminalId::parse(value).map_err(|_error| invalid_field("a move answer names no terminal"))
}

fn creation(value: &str) -> Result<CreationId, ProtocolError> {
    CreationId::parse(value).map_err(|_error| invalid_field("a move answer names no creation"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QuiesceWire {
    deadline_seconds: SafeInteger,
    target: ProcessWire,
    terminal_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadWire {
    offset: SafeInteger,
    terminal_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TargetWire {
    terminal_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportWire {
    creation_id: String,
    deadline_seconds: SafeInteger,
    record_bytes: SafeInteger,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreationWire {
    creation_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscardWire {
    #[serde(skip_serializing_if = "Option::is_none")]
    creation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    terminal_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QuiescedWire {
    record_bytes: SafeInteger,
    terminal_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResumedWire {
    outcome: String,
    terminal_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WrittenWire {
    creation_id: String,
    received_bytes: SafeInteger,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DiscardedWire {
    outcome: String,
}
