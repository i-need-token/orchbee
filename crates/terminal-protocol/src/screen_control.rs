//! Application screen mutations use control JSON, never a synthetic PTY stream.
use crate::{
    AttachmentId, MessageRole, ProtocolError, ProtocolErrorCode, TerminalId,
    control::{decode_wire, preflight_json, serialize_wire},
    terminal_fact::canonical_u64,
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClearTerminal {
    pub attachment_id: AttachmentId,
    pub preserve_prompt: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScreenControlOperation {
    Clear,
    Reset,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalControlAccepted {
    pub attachment_id: AttachmentId,
    pub terminal_id: TerminalId,
    pub generation: NonZeroU64,
    pub revision: u64,
    pub operation: ScreenControlOperation,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScreenControl {
    ClearTerminal(ClearTerminal),
    ResetTerminal(AttachmentId),
    TerminalControlAccepted(TerminalControlAccepted),
}
pub struct EncodedScreenControl {
    pub kind: u16,
    pub role: MessageRole,
    pub payload: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClearWire {
    attachment_id: String,
    preserve_prompt: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResetWire {
    attachment_id: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AcceptedWire {
    attachment_id: String,
    generation: String,
    operation: ScreenControlOperation,
    revision: String,
    terminal_id: String,
}
pub fn decode_screen_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<ScreenControl, ProtocolError> {
    preflight_json(payload)?;
    Ok(match (kind, role) {
        (74, MessageRole::Request) => {
            let wire: ClearWire = decode_wire(payload)?;
            ScreenControl::ClearTerminal(ClearTerminal {
                attachment_id: attachment(&wire.attachment_id)?,
                preserve_prompt: wire.preserve_prompt,
            })
        }
        (75, MessageRole::Request) => {
            let wire: ResetWire = decode_wire(payload)?;
            ScreenControl::ResetTerminal(attachment(&wire.attachment_id)?)
        }
        (76, MessageRole::Response) => {
            let wire: AcceptedWire = decode_wire(payload)?;
            ScreenControl::TerminalControlAccepted(TerminalControlAccepted {
                attachment_id: attachment(&wire.attachment_id)?,
                terminal_id: TerminalId::parse(&wire.terminal_id)
                    .map_err(|_| invalid("invalid screen-control terminal identity"))?,
                generation: positive(&wire.generation)?,
                revision: canonical_u64(&wire.revision)?,
                operation: wire.operation,
            })
        }
        // This codec accepts only its declared message family and request/response roles.
        _ => return Err(invalid("kind/role do not select screen control")),
    })
}
pub fn encode_screen_control(value: ScreenControl) -> Result<EncodedScreenControl, ProtocolError> {
    let (kind, role, payload) = match value {
        ScreenControl::ClearTerminal(value) => (
            74,
            MessageRole::Request,
            serialize_wire(&ClearWire {
                attachment_id: value.attachment_id.to_text(),
                preserve_prompt: value.preserve_prompt,
            })?,
        ),
        ScreenControl::ResetTerminal(id) => (
            75,
            MessageRole::Request,
            serialize_wire(&ResetWire {
                attachment_id: id.to_text(),
            })?,
        ),
        ScreenControl::TerminalControlAccepted(value) => (
            76,
            MessageRole::Response,
            serialize_wire(&AcceptedWire {
                attachment_id: value.attachment_id.to_text(),
                generation: value.generation.to_string(),
                operation: value.operation,
                revision: value.revision.to_string(),
                terminal_id: value.terminal_id.to_text(),
            })?,
        ),
    };
    Ok(EncodedScreenControl {
        kind,
        role,
        payload,
    })
}
fn attachment(value: &str) -> Result<AttachmentId, ProtocolError> {
    AttachmentId::parse(value).map_err(|_| invalid("invalid screen-control attachment identity"))
}
fn positive(value: &str) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(canonical_u64(value)?)
        .ok_or_else(|| invalid("screen-control generation must be positive"))
}
const fn invalid(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
