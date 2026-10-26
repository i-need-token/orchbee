use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

use crate::{
    AttachmentId, MessageRole, ProtocolError, ProtocolErrorCode,
    control::{RequiredNullable, decode_wire, preflight_json, serialize_wire},
    limits::{
        MAXIMUM_CLIPBOARD_TEXT_BYTES, MAXIMUM_TERMINAL_APPLICATION_SIGNAL_BYTES,
        MAXIMUM_TERMINAL_METADATA_TEXT_BYTES,
    },
    terminal_fact::{
        TerminalIdentity, TerminalIdentityWire, canonical_u64, identity_from_wire, identity_to_wire,
    },
};

const TERMINAL_BELL_KIND: u16 = 55;
const TERMINAL_CLIPBOARD_WRITE_REQUEST_KIND: u16 = 56;
const TERMINAL_METADATA_CHANGED_KIND: u16 = 57;
const TERMINAL_APPLICATION_SIGNAL_KIND: u16 = 58;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalBell {
    pub attachment_id: AttachmentId,
    pub sequence: NonZeroU64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalClipboardWriteRequest {
    pub attachment_id: AttachmentId,
    pub sequence: NonZeroU64,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalMetadataChanged {
    pub current_directory: Option<String>,
    pub identity: TerminalIdentity,
    pub revision: NonZeroU64,
    pub title: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalApplicationSignalChannel {
    OrchbeeAgentV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalApplicationSignal {
    pub channel: TerminalApplicationSignalChannel,
    pub identity: TerminalIdentity,
    pub payload: String,
    pub sequence: NonZeroU64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalEffectEvent {
    Bell(TerminalBell),
    ClipboardWriteRequest(TerminalClipboardWriteRequest),
    MetadataChanged(TerminalMetadataChanged),
    ApplicationSignal(TerminalApplicationSignal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedTerminalEffectEvent {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_terminal_effect_event(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<TerminalEffectEvent, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (TERMINAL_BELL_KIND, MessageRole::Event) => decode_wire::<TerminalBellWire>(payload)
            .and_then(terminal_bell_from_wire)
            .map(TerminalEffectEvent::Bell),
        (TERMINAL_CLIPBOARD_WRITE_REQUEST_KIND, MessageRole::Event) => {
            decode_wire::<TerminalClipboardWriteRequestWire>(payload)
                .and_then(terminal_clipboard_write_request_from_wire)
                .map(TerminalEffectEvent::ClipboardWriteRequest)
        }
        (TERMINAL_METADATA_CHANGED_KIND, MessageRole::Event) => {
            decode_wire::<TerminalMetadataChangedWire>(payload)
                .and_then(terminal_metadata_changed_from_wire)
                .map(TerminalEffectEvent::MetadataChanged)
        }
        (TERMINAL_APPLICATION_SIGNAL_KIND, MessageRole::Event) => {
            decode_wire::<TerminalApplicationSignalWire>(payload)
                .and_then(terminal_application_signal_from_wire)
                .map(TerminalEffectEvent::ApplicationSignal)
        }
        // This codec accepts only its declared message family and request/response roles.
        _ => Err(invalid_field(
            "message kind and role do not select a terminal-effect event",
        )),
    }
}

pub fn encode_terminal_effect_event(
    value: &TerminalEffectEvent,
) -> Result<EncodedTerminalEffectEvent, ProtocolError> {
    let (kind, payload) = match value {
        TerminalEffectEvent::Bell(bell) => (
            TERMINAL_BELL_KIND,
            serialize_wire(&terminal_bell_to_wire(*bell))?,
        ),
        TerminalEffectEvent::ClipboardWriteRequest(request) => (
            TERMINAL_CLIPBOARD_WRITE_REQUEST_KIND,
            serialize_wire(&terminal_clipboard_write_request_to_wire(request)?)?,
        ),
        TerminalEffectEvent::MetadataChanged(metadata) => (
            TERMINAL_METADATA_CHANGED_KIND,
            serialize_wire(&terminal_metadata_changed_to_wire(metadata)?)?,
        ),
        TerminalEffectEvent::ApplicationSignal(signal) => (
            TERMINAL_APPLICATION_SIGNAL_KIND,
            serialize_wire(&terminal_application_signal_to_wire(signal)?)?,
        ),
    };
    Ok(EncodedTerminalEffectEvent {
        kind,
        payload,
        role: MessageRole::Event,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalBellWire {
    attachment_id: String,
    sequence: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalClipboardWriteRequestWire {
    attachment_id: String,
    sequence: String,
    text: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalMetadataChangedWire {
    #[serde(default)]
    current_directory: RequiredNullable<String>,
    identity: TerminalIdentityWire,
    revision: String,
    #[serde(default)]
    title: RequiredNullable<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalApplicationSignalWire {
    channel: TerminalApplicationSignalChannelWire,
    identity: TerminalIdentityWire,
    payload: String,
    sequence: String,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum TerminalApplicationSignalChannelWire {
    #[serde(rename = "orchbee.agent.v1")]
    OrchbeeAgentV1,
}

fn terminal_bell_from_wire(wire: TerminalBellWire) -> Result<TerminalBell, ProtocolError> {
    Ok(TerminalBell {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        sequence: positive_canonical_u64(&wire.sequence)?,
    })
}

fn terminal_bell_to_wire(value: TerminalBell) -> TerminalBellWire {
    TerminalBellWire {
        attachment_id: value.attachment_id.to_text(),
        sequence: value.sequence.get().to_string(),
    }
}

fn terminal_clipboard_write_request_from_wire(
    wire: TerminalClipboardWriteRequestWire,
) -> Result<TerminalClipboardWriteRequest, ProtocolError> {
    validate_bounded_text(
        &wire.text,
        MAXIMUM_CLIPBOARD_TEXT_BYTES,
        "clipboard text exceeds its protocol bounds",
    )?;
    Ok(TerminalClipboardWriteRequest {
        attachment_id: parse_attachment_id(&wire.attachment_id)?,
        sequence: positive_canonical_u64(&wire.sequence)?,
        text: wire.text,
    })
}

fn terminal_clipboard_write_request_to_wire(
    value: &TerminalClipboardWriteRequest,
) -> Result<TerminalClipboardWriteRequestWire, ProtocolError> {
    validate_bounded_text(
        &value.text,
        MAXIMUM_CLIPBOARD_TEXT_BYTES,
        "clipboard text exceeds its protocol bounds",
    )?;
    Ok(TerminalClipboardWriteRequestWire {
        attachment_id: value.attachment_id.to_text(),
        sequence: value.sequence.get().to_string(),
        text: value.text.clone(),
    })
}

fn terminal_metadata_changed_from_wire(
    wire: TerminalMetadataChangedWire,
) -> Result<TerminalMetadataChanged, ProtocolError> {
    let current_directory = wire.current_directory.into_option()?;
    let title = wire.title.into_option()?;
    validate_metadata_text(current_directory.as_deref())?;
    validate_metadata_text(title.as_deref())?;
    Ok(TerminalMetadataChanged {
        current_directory,
        identity: identity_from_wire(wire.identity)?,
        revision: positive_canonical_u64(&wire.revision)?,
        title,
    })
}

fn terminal_metadata_changed_to_wire(
    value: &TerminalMetadataChanged,
) -> Result<TerminalMetadataChangedWire, ProtocolError> {
    validate_metadata_text(value.current_directory.as_deref())?;
    validate_metadata_text(value.title.as_deref())?;
    Ok(TerminalMetadataChangedWire {
        current_directory: RequiredNullable::from_option(value.current_directory.clone()),
        identity: identity_to_wire(value.identity),
        revision: value.revision.get().to_string(),
        title: RequiredNullable::from_option(value.title.clone()),
    })
}

fn terminal_application_signal_from_wire(
    wire: TerminalApplicationSignalWire,
) -> Result<TerminalApplicationSignal, ProtocolError> {
    validate_bounded_text(
        &wire.payload,
        MAXIMUM_TERMINAL_APPLICATION_SIGNAL_BYTES,
        "terminal application signal exceeds its protocol bounds",
    )?;
    Ok(TerminalApplicationSignal {
        channel: application_signal_channel_from_wire(wire.channel),
        identity: identity_from_wire(wire.identity)?,
        payload: wire.payload,
        sequence: positive_canonical_u64(&wire.sequence)?,
    })
}

fn terminal_application_signal_to_wire(
    value: &TerminalApplicationSignal,
) -> Result<TerminalApplicationSignalWire, ProtocolError> {
    validate_bounded_text(
        &value.payload,
        MAXIMUM_TERMINAL_APPLICATION_SIGNAL_BYTES,
        "terminal application signal exceeds its protocol bounds",
    )?;
    Ok(TerminalApplicationSignalWire {
        channel: application_signal_channel_to_wire(value.channel),
        identity: identity_to_wire(value.identity),
        payload: value.payload.clone(),
        sequence: value.sequence.get().to_string(),
    })
}

fn validate_metadata_text(value: Option<&str>) -> Result<(), ProtocolError> {
    if let Some(value) = value {
        validate_bounded_text(
            value,
            MAXIMUM_TERMINAL_METADATA_TEXT_BYTES,
            "terminal metadata text exceeds its protocol bounds",
        )?;
    }
    Ok(())
}

fn validate_bounded_text(
    value: &str,
    maximum_bytes: usize,
    detail: &'static str,
) -> Result<(), ProtocolError> {
    if value.len() > maximum_bytes || value.contains('\0') {
        return Err(invalid_field(detail));
    }
    Ok(())
}

const fn application_signal_channel_from_wire(
    value: TerminalApplicationSignalChannelWire,
) -> TerminalApplicationSignalChannel {
    match value {
        TerminalApplicationSignalChannelWire::OrchbeeAgentV1 => {
            TerminalApplicationSignalChannel::OrchbeeAgentV1
        }
    }
}

const fn application_signal_channel_to_wire(
    value: TerminalApplicationSignalChannel,
) -> TerminalApplicationSignalChannelWire {
    match value {
        TerminalApplicationSignalChannel::OrchbeeAgentV1 => {
            TerminalApplicationSignalChannelWire::OrchbeeAgentV1
        }
    }
}

fn parse_attachment_id(value: &str) -> Result<AttachmentId, ProtocolError> {
    AttachmentId::parse(value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidIdentifier,
            "attachment identity is not canonical nonzero 128-bit hexadecimal text",
        )
    })
}

fn positive_canonical_u64(value: &str) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(canonical_u64(value)?)
        .ok_or_else(|| invalid_field("unsigned 64-bit text must be greater than zero"))
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
