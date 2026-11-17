//! Carries a Worker's decline as kind 39: the declined request's kind as one byte, then its
//! 16-byte identity.

use super::PayloadReader;
use crate::{DeclinedRequestKind, ProtocolError, WorkerRequestDeclined};

pub(super) const KIND: u8 = 39;

pub(super) fn encode(value: WorkerRequestDeclined) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(18);
    bytes.push(KIND);
    bytes.push(kind_code(value.kind));
    bytes.extend(value.identity);
    bytes
}

pub(super) fn decode(payload: &[u8]) -> Result<WorkerRequestDeclined, ProtocolError> {
    let mut reader = PayloadReader::after_kind(payload, KIND)?;
    let kind = request_kind(reader.read_u8()?)?;
    let identity = reader.read_array()?;
    reader.finish()?;
    Ok(WorkerRequestDeclined { kind, identity })
}

const fn kind_code(kind: DeclinedRequestKind) -> u8 {
    use DeclinedRequestKind as Kind;
    match kind {
        Kind::Launch => 1,
        Kind::Start => 2,
        Kind::Viewer => 3,
        Kind::Control => 4,
        Kind::Lifecycle => 5,
        Kind::CwdValidation => 6,
        Kind::Send => 7,
        Kind::WorktreeJob => 8,
        Kind::AgentDrain => 9,
        Kind::Shutdown => 10,
        Kind::WorktreeMutation => 11,
        Kind::Titles => 12,
        Kind::Handoff => 13,
        Kind::AgentIntegrationJob => 14,
        Kind::AgentProbeJob => 15,
        Kind::HostQuery => 16,
        Kind::TitleChanges => 17,
        Kind::PluginJob => 18,
        Kind::GraphicsDecode => 19,
        Kind::AdoptHost => 20,
    }
}

const fn request_kind(code: u8) -> Result<DeclinedRequestKind, ProtocolError> {
    use DeclinedRequestKind as Kind;
    Ok(match code {
        1 => Kind::Launch,
        2 => Kind::Start,
        3 => Kind::Viewer,
        4 => Kind::Control,
        5 => Kind::Lifecycle,
        6 => Kind::CwdValidation,
        7 => Kind::Send,
        8 => Kind::WorktreeJob,
        9 => Kind::AgentDrain,
        10 => Kind::Shutdown,
        11 => Kind::WorktreeMutation,
        12 => Kind::Titles,
        13 => Kind::Handoff,
        14 => Kind::AgentIntegrationJob,
        15 => Kind::AgentProbeJob,
        16 => Kind::HostQuery,
        17 => Kind::TitleChanges,
        18 => Kind::PluginJob,
        19 => Kind::GraphicsDecode,
        20 => Kind::AdoptHost,
        _ => return Err(ProtocolError::InvalidPayload),
    })
}
