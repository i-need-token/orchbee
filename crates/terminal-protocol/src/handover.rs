//! Defines the Host handover requests a Worker sends a Windows Terminal Host to attach the
//! Session's Keeper and to recover a program the Keeper kept after its Host ended, with their
//! answers and the typed refusal that adds a `reason`. It transports the requests without
//! deciding when a Keeper attaches or which holding a recovery takes.

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

use crate::{
    CreationId, MessageRole, ProtocolError, ProtocolErrorCode, TerminalId, TerminalProfile,
    control::{SafeInteger, decode_wire, preflight_json, serialize_wire},
    creation::{TerminalProfileWire, terminal_profile_to_wire, validate_terminal_profile},
    geometry::{TerminalPixelGeometry, TerminalPixelGeometryWire, geometry_to_wire},
    terminal_configuration::{
        TerminalConfiguration, TerminalConfigurationWire, configuration_to_wire,
    },
    terminal_fact::{TerminalIdentity, TerminalIdentityWire, identity_from_wire, identity_to_wire},
};

const TYPED_FAILURE_KIND: u16 = 5;
const ATTACH_KEEPER_KIND: u16 = 90;
const CLAIM_TERMINAL_KIND: u16 = 104;
const RECOVER_TERMINAL_KIND: u16 = 106;
const MAXIMUM_DEADLINE_SECONDS: u8 = 30;

/// A process as a handover payload names it: a positive process ID and its creation time as a
/// `FILETIME`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandoverProcess {
    pub pid: NonZeroU32,
    pub creation_time: u64,
}

/// `AttachKeeper`: the Keeper to attach and its endpoint id, 16 bytes sent as 32 lowercase
/// hexadecimal digits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachKeeper {
    pub keeper: HandoverProcess,
    pub endpoint_id: [u8; 16],
}

/// `RecoverTerminal`: the terminal a holding's program ran as under the recovering Host's epoch,
/// the holding and the mark the Keeper assigned it under, and how the empty terminal starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoverTerminal {
    pub creation_id: CreationId,
    pub terminal_id: TerminalId,
    pub holding: [u8; 16],
    pub mark: [u8; 16],
    pub geometry: TerminalPixelGeometry,
    pub terminal_profile: TerminalProfile,
    pub configuration: Option<TerminalConfiguration>,
    /// From 1 to 30.
    pub deadline_seconds: u8,
}

/// A request a Worker sends, or the successful answer it reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandoverControl {
    AttachKeeper(AttachKeeper),
    KeeperAttached(HandoverProcess),
    ClaimTerminal(TerminalId),
    TerminalClaimed(TerminalId),
    RecoverTerminal(Box<RecoverTerminal>),
    TerminalRecovered(TerminalIdentity),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedHandoverControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

/// A `TypedFailure` answering a handover request: its code, whether a retry may pass, and the
/// `reason` a refusal the Worker acts on adds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandoverRefusal {
    pub code: String,
    pub retryable: bool,
    pub reason: Option<String>,
}

pub fn decode_handover_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<HandoverControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (ATTACH_KEEPER_KIND, MessageRole::Response) => decode_wire::<KeeperAttachedWire>(payload)
            .and_then(|wire| process_from_wire(&wire.keeper))
            .map(HandoverControl::KeeperAttached),
        (CLAIM_TERMINAL_KIND, MessageRole::Response) => decode_wire::<TerminalTargetWire>(payload)
            .and_then(|wire| terminal_from_wire(&wire.terminal_id))
            .map(HandoverControl::TerminalClaimed),
        (RECOVER_TERMINAL_KIND, MessageRole::Response) => {
            decode_wire::<TerminalIdentityWire>(payload)
                .and_then(identity_from_wire)
                .map(HandoverControl::TerminalRecovered)
        }
        // A Worker decodes only the answers to its own requests.
        _ => Err(invalid_field(
            "message kind and role do not select a handover answer",
        )),
    }
}

pub fn encode_handover_control(
    value: &HandoverControl,
) -> Result<EncodedHandoverControl, ProtocolError> {
    let (kind, role, payload) = match value {
        HandoverControl::AttachKeeper(attach) => (
            ATTACH_KEEPER_KIND,
            MessageRole::Request,
            serialize_wire(&AttachKeeperWire {
                keeper: process_to_wire(attach.keeper),
                endpoint_id: hex(&attach.endpoint_id),
            })?,
        ),
        HandoverControl::KeeperAttached(keeper) => (
            ATTACH_KEEPER_KIND,
            MessageRole::Response,
            serialize_wire(&KeeperAttachedWire {
                keeper: process_to_wire(*keeper),
            })?,
        ),
        HandoverControl::ClaimTerminal(terminal) => (
            CLAIM_TERMINAL_KIND,
            MessageRole::Request,
            serialize_wire(&TerminalTargetWire {
                terminal_id: terminal.to_text(),
            })?,
        ),
        HandoverControl::TerminalClaimed(terminal) => (
            CLAIM_TERMINAL_KIND,
            MessageRole::Response,
            serialize_wire(&TerminalTargetWire {
                terminal_id: terminal.to_text(),
            })?,
        ),
        HandoverControl::RecoverTerminal(recover) => (
            RECOVER_TERMINAL_KIND,
            MessageRole::Request,
            serialize_wire(&recover_to_wire(recover)?)?,
        ),
        HandoverControl::TerminalRecovered(identity) => (
            RECOVER_TERMINAL_KIND,
            MessageRole::Response,
            serialize_wire(&identity_to_wire(*identity))?,
        ),
    };
    Ok(EncodedHandoverControl {
        kind,
        payload,
        role,
    })
}

/// Decodes the `TypedFailure` answering a handover request, whose `reason` may name any refusal
/// reason, unlike the closed reasons of [`crate::TypedFailure`].
pub fn decode_handover_refusal(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<HandoverRefusal, ProtocolError> {
    preflight_json(payload)?;
    if (kind, role) != (TYPED_FAILURE_KIND, MessageRole::Response) {
        return Err(invalid_field(
            "message kind and role do not select a typed failure",
        ));
    }
    let wire = decode_wire::<HandoverRefusalWire>(payload)?;
    if wire.code.is_empty() || wire.reason.as_ref().is_some_and(String::is_empty) {
        return Err(invalid_field(
            "a typed failure names an empty code or reason",
        ));
    }
    Ok(HandoverRefusal {
        code: wire.code,
        retryable: wire.retryable,
        reason: wire.reason,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProcessWire {
    pid: SafeInteger,
    creation_time: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AttachKeeperWire {
    keeper: ProcessWire,
    endpoint_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KeeperAttachedWire {
    keeper: ProcessWire,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalTargetWire {
    terminal_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoverTerminalWire {
    creation_id: String,
    terminal_id: String,
    holding: String,
    mark: String,
    geometry: TerminalPixelGeometryWire,
    terminal_profile: TerminalProfileWire,
    #[serde(skip_serializing_if = "Option::is_none")]
    configuration: Option<TerminalConfigurationWire>,
    deadline_seconds: SafeInteger,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HandoverRefusalWire {
    code: String,
    #[serde(default, rename = "message")]
    _message: Option<String>,
    retryable: bool,
    #[serde(default)]
    reason: Option<String>,
}

fn recover_to_wire(value: &RecoverTerminal) -> Result<RecoverTerminalWire, ProtocolError> {
    if !(1..=MAXIMUM_DEADLINE_SECONDS).contains(&value.deadline_seconds) {
        return Err(invalid_field(
            "a recovery's deadline is from 1 to 30 seconds",
        ));
    }
    if value.holding == [0; 16] || value.mark == [0; 16] {
        return Err(invalid_field("a recovery names a zero holding or mark"));
    }
    validate_terminal_profile(&value.terminal_profile)?;
    Ok(RecoverTerminalWire {
        creation_id: value.creation_id.to_text(),
        terminal_id: value.terminal_id.to_text(),
        holding: hex(&value.holding),
        mark: hex(&value.mark),
        geometry: geometry_to_wire(value.geometry)?,
        terminal_profile: terminal_profile_to_wire(&value.terminal_profile),
        configuration: value.configuration.as_ref().map(configuration_to_wire),
        deadline_seconds: SafeInteger::new(u64::from(value.deadline_seconds)),
    })
}

fn process_to_wire(value: HandoverProcess) -> ProcessWire {
    ProcessWire {
        pid: SafeInteger::new(u64::from(value.pid.get())),
        creation_time: value.creation_time.to_string(),
    }
}

pub(crate) fn process_from_wire(wire: &ProcessWire) -> Result<HandoverProcess, ProtocolError> {
    let pid = u32::try_from(wire.pid.get())
        .ok()
        .and_then(NonZeroU32::new)
        .ok_or_else(|| invalid_field("a handover process names no positive process ID"))?;
    let canonical = !wire.creation_time.is_empty()
        && wire.creation_time.bytes().all(|byte| byte.is_ascii_digit())
        && (wire.creation_time == "0" || !wire.creation_time.starts_with('0'));
    let creation_time = canonical
        .then(|| wire.creation_time.parse::<u64>().ok())
        .flatten()
        .ok_or_else(|| {
            invalid_field("a handover process's creation time is not canonical decimal text")
        })?;
    Ok(HandoverProcess { pid, creation_time })
}

fn terminal_from_wire(value: &str) -> Result<TerminalId, ProtocolError> {
    TerminalId::parse(value).map_err(|_error| invalid_field("a handover answer names no terminal"))
}

pub(crate) fn hex(bytes: &[u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(32);
    for byte in bytes {
        text.push(char::from(HEX[usize::from(byte >> 4)]));
        text.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    text
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
