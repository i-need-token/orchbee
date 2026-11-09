//! Dispatches bounded raw and semantic input payloads and validates queue-admission
//! acknowledgements under the registered input message kinds. Encoding a message neither
//! validates live input capability nor promises that the child executed the accepted bytes.

use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

use crate::{
    AttachmentId, MessageRole, ProtocolError, ProtocolErrorCode,
    control::{SafeInteger, decode_wire, preflight_json, serialize_wire},
    input_authority::{
        INPUT_AUTHORITY_HEADER_BYTES, InputAuthority, append_input_authority,
        decode_input_authority,
    },
    limits::MAXIMUM_INPUT_BYTES,
    semantic_input::{
        FocusInput, KeyInput, MouseInput, PasteAutoInput, PasteInput, PromptLineInput, TextInput,
        decode_focus_input, decode_key_input, decode_mouse_input, decode_paste_auto_input,
        decode_paste_input, decode_prompt_line_input, decode_text_input, encode_focus_input,
        encode_key_input, encode_mouse_input, encode_paste_auto_input, encode_paste_input,
        encode_prompt_line_input, encode_text_input,
    },
    terminal_fact::{bounded_u32, canonical_u64},
};

const WRITE_INPUT_KIND: u16 = 30;
const INPUT_ACCEPTED_KIND: u16 = 31;
pub const KEY_INPUT_KIND: u16 = 34;
pub const TEXT_INPUT_KIND: u16 = 35;
pub const MOUSE_INPUT_KIND: u16 = 36;
pub const FOCUS_INPUT_KIND: u16 = 37;
const PASTE_INPUT_KIND: u16 = 38;
const PASTE_AUTO_INPUT_KIND: u16 = 39;
pub const PROMPT_LINE_INPUT_KIND: u16 = 84;
pub const WRITE_INPUT_HEADER_BYTES: usize = INPUT_AUTHORITY_HEADER_BYTES;
pub const MAXIMUM_WRITE_INPUT_PAYLOAD_BYTES: usize = WRITE_INPUT_HEADER_BYTES + MAXIMUM_INPUT_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteInput {
    pub attachment_id: AttachmentId,
    pub bytes: Vec<u8>,
    pub capability_revision: NonZeroU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputAccepted {
    pub accepted_bytes: u32,
    pub attachment_id: AttachmentId,
    pub capability_revision: NonZeroU64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputMessage {
    WriteInput(WriteInput),
    InputAccepted(InputAccepted),
    KeyInput(KeyInput),
    TextInput(TextInput),
    MouseInput(MouseInput),
    FocusInput(FocusInput),
    PasteInput(PasteInput),
    PasteAutoInput(PasteAutoInput),
    PromptLineInput(PromptLineInput),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedInputMessage {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_input_message(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<InputMessage, ProtocolError> {
    match (kind, role) {
        (WRITE_INPUT_KIND, MessageRole::Request) => {
            decode_write_input(payload).map(InputMessage::WriteInput)
        }
        (INPUT_ACCEPTED_KIND, MessageRole::Response) => {
            preflight_json(payload)?;
            decode_wire::<InputAcceptedWire>(payload)
                .and_then(input_accepted_from_wire)
                .map(InputMessage::InputAccepted)
        }
        (KEY_INPUT_KIND, MessageRole::Request) => {
            decode_key_input(payload).map(InputMessage::KeyInput)
        }
        (TEXT_INPUT_KIND, MessageRole::Request) => {
            decode_text_input(payload).map(InputMessage::TextInput)
        }
        (MOUSE_INPUT_KIND, MessageRole::Request) => {
            decode_mouse_input(payload).map(InputMessage::MouseInput)
        }
        (FOCUS_INPUT_KIND, MessageRole::Request) => {
            decode_focus_input(payload).map(InputMessage::FocusInput)
        }
        (PASTE_INPUT_KIND, MessageRole::Request) => {
            decode_paste_input(payload).map(InputMessage::PasteInput)
        }
        (PASTE_AUTO_INPUT_KIND, MessageRole::Request) => {
            decode_paste_auto_input(payload).map(InputMessage::PasteAutoInput)
        }
        (PROMPT_LINE_INPUT_KIND, MessageRole::Request) => {
            decode_prompt_line_input(payload).map(InputMessage::PromptLineInput)
        }
        // This codec accepts only its declared message family and request/response roles.
        _ => Err(invalid_field(
            "message kind and role do not select an input payload",
        )),
    }
}

pub fn encode_input_message(value: &InputMessage) -> Result<EncodedInputMessage, ProtocolError> {
    let (kind, role, payload) = match value {
        InputMessage::WriteInput(write) => (
            WRITE_INPUT_KIND,
            MessageRole::Request,
            encode_write_input(write)?,
        ),
        InputMessage::InputAccepted(accepted) => (
            INPUT_ACCEPTED_KIND,
            MessageRole::Response,
            serialize_wire(&input_accepted_to_wire(*accepted)?)?,
        ),
        InputMessage::KeyInput(key) => {
            (KEY_INPUT_KIND, MessageRole::Request, encode_key_input(key)?)
        }
        InputMessage::TextInput(text) => (
            TEXT_INPUT_KIND,
            MessageRole::Request,
            encode_text_input(text)?,
        ),
        InputMessage::MouseInput(mouse) => (
            MOUSE_INPUT_KIND,
            MessageRole::Request,
            encode_mouse_input(*mouse)?,
        ),
        InputMessage::FocusInput(focus) => (
            FOCUS_INPUT_KIND,
            MessageRole::Request,
            encode_focus_input(*focus),
        ),
        InputMessage::PasteInput(paste) => (
            PASTE_INPUT_KIND,
            MessageRole::Request,
            encode_paste_input(paste)?,
        ),
        InputMessage::PasteAutoInput(paste) => (
            PASTE_AUTO_INPUT_KIND,
            MessageRole::Request,
            encode_paste_auto_input(paste)?,
        ),
        InputMessage::PromptLineInput(line) => (
            PROMPT_LINE_INPUT_KIND,
            MessageRole::Request,
            encode_prompt_line_input(line)?,
        ),
    };
    Ok(EncodedInputMessage {
        kind,
        payload,
        role,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InputAcceptedWire {
    accepted_bytes: SafeInteger,
    attachment_id: String,
    capability_revision: String,
}

fn decode_write_input(payload: &[u8]) -> Result<WriteInput, ProtocolError> {
    if payload.len() <= WRITE_INPUT_HEADER_BYTES {
        return Err(ProtocolError::new(
            ProtocolErrorCode::Truncated,
            "WriteInput requires an authority header and at least one input byte",
        ));
    }
    if payload.len() > MAXIMUM_WRITE_INPUT_PAYLOAD_BYTES {
        return Err(ProtocolError::new(
            ProtocolErrorCode::LimitExceeded,
            "WriteInput payload exceeds its maximum byte length",
        ));
    }

    let authority = decode_input_authority(payload)?;
    let bytes = payload
        .get(WRITE_INPUT_HEADER_BYTES..)
        .ok_or_else(truncated)?
        .to_vec();
    Ok(WriteInput {
        attachment_id: authority.attachment_id,
        bytes,
        capability_revision: authority.capability_revision,
    })
}

fn encode_write_input(value: &WriteInput) -> Result<Vec<u8>, ProtocolError> {
    validate_input_bytes(&value.bytes)?;
    let payload_length = WRITE_INPUT_HEADER_BYTES
        .checked_add(value.bytes.len())
        .ok_or_else(|| {
            ProtocolError::new(
                ProtocolErrorCode::ArithmeticOverflow,
                "WriteInput payload length overflows the target address space",
            )
        })?;
    let mut payload = Vec::with_capacity(payload_length);
    append_input_authority(
        &mut payload,
        InputAuthority {
            attachment_id: value.attachment_id,
            capability_revision: value.capability_revision,
        },
    );
    payload.extend_from_slice(&value.bytes);
    Ok(payload)
}

fn input_accepted_from_wire(wire: InputAcceptedWire) -> Result<InputAccepted, ProtocolError> {
    let maximum = u32::try_from(MAXIMUM_INPUT_BYTES)
        .map_err(|_error| invalid_field("input-byte maximum cannot be represented as u32"))?;
    Ok(InputAccepted {
        accepted_bytes: bounded_u32(wire.accepted_bytes, 0, maximum)?,
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        capability_revision: positive_canonical_u64(&wire.capability_revision)?,
    })
}

fn input_accepted_to_wire(value: InputAccepted) -> Result<InputAcceptedWire, ProtocolError> {
    validate_accepted_bytes(value.accepted_bytes)?;
    Ok(InputAcceptedWire {
        accepted_bytes: SafeInteger::new(u64::from(value.accepted_bytes)),
        attachment_id: value.attachment_id.to_text(),
        capability_revision: value.capability_revision.get().to_string(),
    })
}

fn validate_input_bytes(bytes: &[u8]) -> Result<(), ProtocolError> {
    if bytes.is_empty() || bytes.len() > MAXIMUM_INPUT_BYTES {
        return Err(invalid_field(
            "input must contain one byte through the protocol maximum",
        ));
    }
    Ok(())
}

fn validate_accepted_bytes(value: u32) -> Result<(), ProtocolError> {
    if usize::try_from(value).map_or(true, |value| value > MAXIMUM_INPUT_BYTES) {
        return Err(invalid_field(
            "accepted bytes exceed the protocol input maximum",
        ));
    }
    Ok(())
}

fn positive_canonical_u64(value: &str) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(canonical_u64(value)?)
        .ok_or_else(|| invalid_field("capability revision must be greater than zero"))
}

fn parse_attachment_id(value: &str) -> Result<AttachmentId, ProtocolError> {
    AttachmentId::parse(value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidIdentifier,
            "input attachment identity is invalid",
        )
    })
}

const fn truncated() -> ProtocolError {
    ProtocolError::new(
        ProtocolErrorCode::Truncated,
        "input payload ended before its fixed authority header",
    )
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
