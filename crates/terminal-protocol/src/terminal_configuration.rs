use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

use crate::{
    MessageRole, ProtocolError, ProtocolErrorCode, TerminalId,
    control::{SafeInteger, decode_wire, preflight_json, serialize_wire},
    limits::MAXIMUM_SCROLLBACK_BYTES,
    terminal_fact::{bounded_u32, canonical_u64},
};

const SET_TERMINAL_DEFAULTS_KIND: u16 = 70;
const TERMINAL_DEFAULTS_ACCEPTED_KIND: u16 = 71;
const UPDATE_TERMINAL_CONFIGURATION_KIND: u16 = 72;
const TERMINAL_CONFIGURATION_ACCEPTED_KIND: u16 = 73;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalConfiguration {
    pub scrollback_bytes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetTerminalDefaults {
    pub configuration: TerminalConfiguration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalDefaultsAccepted {
    pub configuration: TerminalConfiguration,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateTerminalConfiguration {
    pub configuration: TerminalConfiguration,
    pub terminal_id: TerminalId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalConfigurationAccepted {
    pub configuration: TerminalConfiguration,
    pub configuration_revision: u64,
    pub generation: NonZeroU64,
    pub history_rows_evicted: u64,
    pub revision: u64,
    pub terminal_id: TerminalId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalConfigurationControl {
    SetDefaultsRequest(SetTerminalDefaults),
    DefaultsAcceptedResponse(TerminalDefaultsAccepted),
    UpdateRequest(UpdateTerminalConfiguration),
    ConfigurationAcceptedResponse(TerminalConfigurationAccepted),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedTerminalConfigurationControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_terminal_configuration_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<TerminalConfigurationControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (SET_TERMINAL_DEFAULTS_KIND, MessageRole::Request) => {
            decode_wire::<SetTerminalDefaultsWire>(payload)
                .and_then(set_terminal_defaults_from_wire)
                .map(TerminalConfigurationControl::SetDefaultsRequest)
        }
        (TERMINAL_DEFAULTS_ACCEPTED_KIND, MessageRole::Response) => {
            decode_wire::<TerminalDefaultsAcceptedWire>(payload)
                .and_then(terminal_defaults_accepted_from_wire)
                .map(TerminalConfigurationControl::DefaultsAcceptedResponse)
        }
        (UPDATE_TERMINAL_CONFIGURATION_KIND, MessageRole::Request) => {
            decode_wire::<UpdateTerminalConfigurationWire>(payload)
                .and_then(update_terminal_configuration_from_wire)
                .map(TerminalConfigurationControl::UpdateRequest)
        }
        (TERMINAL_CONFIGURATION_ACCEPTED_KIND, MessageRole::Response) => {
            decode_wire::<TerminalConfigurationAcceptedWire>(payload)
                .and_then(terminal_configuration_accepted_from_wire)
                .map(TerminalConfigurationControl::ConfigurationAcceptedResponse)
        }
        _ => Err(invalid_field(
            "message kind and role do not select a terminal-configuration control payload",
        )),
    }
}

pub fn encode_terminal_configuration_control(
    value: &TerminalConfigurationControl,
) -> Result<EncodedTerminalConfigurationControl, ProtocolError> {
    let (kind, role, payload) = match value {
        TerminalConfigurationControl::SetDefaultsRequest(value) => {
            validate_configuration(&value.configuration)?;
            (
                SET_TERMINAL_DEFAULTS_KIND,
                MessageRole::Request,
                serialize_wire(&SetTerminalDefaultsWire {
                    configuration: configuration_to_wire(&value.configuration),
                })?,
            )
        }
        TerminalConfigurationControl::DefaultsAcceptedResponse(value) => {
            validate_configuration(&value.configuration)?;
            (
                TERMINAL_DEFAULTS_ACCEPTED_KIND,
                MessageRole::Response,
                serialize_wire(&TerminalDefaultsAcceptedWire {
                    configuration: configuration_to_wire(&value.configuration),
                    revision: value.revision.to_string(),
                })?,
            )
        }
        TerminalConfigurationControl::UpdateRequest(value) => {
            validate_configuration(&value.configuration)?;
            (
                UPDATE_TERMINAL_CONFIGURATION_KIND,
                MessageRole::Request,
                serialize_wire(&UpdateTerminalConfigurationWire {
                    configuration: configuration_to_wire(&value.configuration),
                    terminal_id: value.terminal_id.to_text(),
                })?,
            )
        }
        TerminalConfigurationControl::ConfigurationAcceptedResponse(value) => {
            validate_configuration(&value.configuration)?;
            (
                TERMINAL_CONFIGURATION_ACCEPTED_KIND,
                MessageRole::Response,
                serialize_wire(&TerminalConfigurationAcceptedWire {
                    configuration: configuration_to_wire(&value.configuration),
                    configuration_revision: value.configuration_revision.to_string(),
                    generation: value.generation.get().to_string(),
                    history_rows_evicted: value.history_rows_evicted.to_string(),
                    revision: value.revision.to_string(),
                    terminal_id: value.terminal_id.to_text(),
                })?,
            )
        }
    };
    Ok(EncodedTerminalConfigurationControl {
        kind,
        payload,
        role,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TerminalConfigurationWire {
    scrollback_bytes: SafeInteger,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetTerminalDefaultsWire {
    configuration: TerminalConfigurationWire,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalDefaultsAcceptedWire {
    configuration: TerminalConfigurationWire,
    revision: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateTerminalConfigurationWire {
    configuration: TerminalConfigurationWire,
    terminal_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalConfigurationAcceptedWire {
    configuration: TerminalConfigurationWire,
    configuration_revision: String,
    generation: String,
    history_rows_evicted: String,
    revision: String,
    terminal_id: String,
}

fn set_terminal_defaults_from_wire(
    wire: SetTerminalDefaultsWire,
) -> Result<SetTerminalDefaults, ProtocolError> {
    Ok(SetTerminalDefaults {
        configuration: configuration_from_wire(wire.configuration)?,
    })
}

fn terminal_defaults_accepted_from_wire(
    wire: TerminalDefaultsAcceptedWire,
) -> Result<TerminalDefaultsAccepted, ProtocolError> {
    Ok(TerminalDefaultsAccepted {
        configuration: configuration_from_wire(wire.configuration)?,
        revision: canonical_u64(&wire.revision)?,
    })
}

fn update_terminal_configuration_from_wire(
    wire: UpdateTerminalConfigurationWire,
) -> Result<UpdateTerminalConfiguration, ProtocolError> {
    Ok(UpdateTerminalConfiguration {
        configuration: configuration_from_wire(wire.configuration)?,
        terminal_id: parse_terminal_id(&wire.terminal_id)?,
    })
}

fn terminal_configuration_accepted_from_wire(
    wire: TerminalConfigurationAcceptedWire,
) -> Result<TerminalConfigurationAccepted, ProtocolError> {
    Ok(TerminalConfigurationAccepted {
        configuration: configuration_from_wire(wire.configuration)?,
        configuration_revision: canonical_u64(&wire.configuration_revision)?,
        generation: positive_canonical_u64(&wire.generation)?,
        history_rows_evicted: canonical_u64(&wire.history_rows_evicted)?,
        revision: canonical_u64(&wire.revision)?,
        terminal_id: parse_terminal_id(&wire.terminal_id)?,
    })
}

pub(crate) fn configuration_from_wire(
    wire: TerminalConfigurationWire,
) -> Result<TerminalConfiguration, ProtocolError> {
    Ok(TerminalConfiguration {
        scrollback_bytes: bounded_u32(wire.scrollback_bytes, 0, MAXIMUM_SCROLLBACK_BYTES)?,
    })
}

pub(crate) fn configuration_to_wire(value: &TerminalConfiguration) -> TerminalConfigurationWire {
    TerminalConfigurationWire {
        scrollback_bytes: SafeInteger::new(u64::from(value.scrollback_bytes)),
    }
}

pub(crate) fn validate_configuration(value: &TerminalConfiguration) -> Result<(), ProtocolError> {
    if value.scrollback_bytes > MAXIMUM_SCROLLBACK_BYTES {
        return Err(invalid_field(
            "terminal scrollback configuration exceeds its protocol bound",
        ));
    }
    Ok(())
}

fn parse_terminal_id(value: &str) -> Result<TerminalId, ProtocolError> {
    TerminalId::parse(value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidIdentifier,
            "terminal configuration contains an invalid terminal identity",
        )
    })
}

fn positive_canonical_u64(value: &str) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(canonical_u64(value)?)
        .ok_or_else(|| invalid_field("terminal generation must be positive"))
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
