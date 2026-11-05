//! Defines closed terminal start, termination, disposal, and lifecycle observation payloads
//! with validated geometry and process facts. It transports requested transitions and reported
//! outcomes without owning a child process or deciding whether termination is authorized.

use serde::{Deserialize, Serialize};

use crate::{
    MessageRole, ProtocolError, ProtocolErrorCode, TerminalId,
    control::{decode_wire, preflight_json, serialize_wire},
    geometry::{
        TerminalPixelGeometry, TerminalPixelGeometryWire, geometry_from_wire, geometry_to_wire,
    },
    terminal_fact::{
        ExitResult, ExitResultWire, FailurePairWire, ProcessObservation, ProcessObservationWire,
        RuntimeFailure, TerminalIdentity, TerminalIdentityWire, canonical_u64,
        exit_result_from_wire, exit_result_to_wire, identity_from_wire, identity_to_wire,
        process_observation_from_wire, process_observation_to_wire, runtime_failure_from_wire,
        runtime_failure_to_wire, validate_exit_result, validate_process_observation,
    },
};

const TERMINATE_TERMINAL_KIND: u16 = 12;
const DISPOSE_TERMINAL_KIND: u16 = 13;
const TERMINAL_STARTED_KIND: u16 = 14;
const TERMINAL_EXITED_KIND: u16 = 15;
const START_TERMINAL_KIND: u16 = 16;
const TERMINAL_START_ACCEPTED_KIND: u16 = 17;
const FORCE_TERMINATE_TERMINAL_KIND: u16 = 18;
const TERMINAL_FAILED_KIND: u16 = 19;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalTarget {
    pub terminal_id: TerminalId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartTerminal {
    pub geometry: TerminalPixelGeometry,
    pub terminal_id: TerminalId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartTransition {
    AlreadyStarted,
    Started,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalStartAccepted {
    pub geometry: TerminalPixelGeometry,
    pub identity: TerminalIdentity,
    pub transition: StartTransition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalExited {
    pub exit: ExitResult,
    pub final_revision: u64,
    pub identity: TerminalIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalFailed {
    pub failure: RuntimeFailure,
    pub final_revision: u64,
    pub identity: TerminalIdentity,
    pub process: ProcessObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleControl {
    TerminateTerminal(TerminalTarget),
    TerminateTerminalAccepted(TerminalTarget),
    DisposeTerminal(TerminalTarget),
    DisposeTerminalAccepted(TerminalTarget),
    TerminalStarted(TerminalIdentity),
    TerminalExited(TerminalExited),
    StartTerminal(StartTerminal),
    TerminalStartAccepted(TerminalStartAccepted),
    ForceTerminateTerminal(TerminalTarget),
    ForceTerminateTerminalAccepted(TerminalTarget),
    TerminalFailed(TerminalFailed),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedLifecycleControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_lifecycle_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<LifecycleControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (TERMINATE_TERMINAL_KIND, MessageRole::Request) => {
            decode_wire::<TerminalTargetWire>(payload)
                .and_then(terminal_target_from_wire)
                .map(LifecycleControl::TerminateTerminal)
        }
        (TERMINATE_TERMINAL_KIND, MessageRole::Response) => {
            decode_wire::<TerminalTargetWire>(payload)
                .and_then(terminal_target_from_wire)
                .map(LifecycleControl::TerminateTerminalAccepted)
        }
        (DISPOSE_TERMINAL_KIND, MessageRole::Request) => decode_wire::<TerminalTargetWire>(payload)
            .and_then(terminal_target_from_wire)
            .map(LifecycleControl::DisposeTerminal),
        (DISPOSE_TERMINAL_KIND, MessageRole::Response) => {
            decode_wire::<TerminalTargetWire>(payload)
                .and_then(terminal_target_from_wire)
                .map(LifecycleControl::DisposeTerminalAccepted)
        }
        (TERMINAL_STARTED_KIND, MessageRole::Event) => decode_wire::<TerminalIdentityWire>(payload)
            .and_then(identity_from_wire)
            .map(LifecycleControl::TerminalStarted),
        (TERMINAL_EXITED_KIND, MessageRole::Event) => decode_wire::<TerminalExitedWire>(payload)
            .and_then(terminal_exited_from_wire)
            .map(LifecycleControl::TerminalExited),
        (START_TERMINAL_KIND, MessageRole::Request) => decode_wire::<StartTerminalWire>(payload)
            .and_then(start_terminal_from_wire)
            .map(LifecycleControl::StartTerminal),
        (TERMINAL_START_ACCEPTED_KIND, MessageRole::Response) => {
            decode_wire::<TerminalStartAcceptedWire>(payload)
                .and_then(terminal_start_accepted_from_wire)
                .map(LifecycleControl::TerminalStartAccepted)
        }
        (FORCE_TERMINATE_TERMINAL_KIND, MessageRole::Request) => {
            decode_wire::<TerminalTargetWire>(payload)
                .and_then(terminal_target_from_wire)
                .map(LifecycleControl::ForceTerminateTerminal)
        }
        (FORCE_TERMINATE_TERMINAL_KIND, MessageRole::Response) => {
            decode_wire::<TerminalTargetWire>(payload)
                .and_then(terminal_target_from_wire)
                .map(LifecycleControl::ForceTerminateTerminalAccepted)
        }
        (TERMINAL_FAILED_KIND, MessageRole::Event) => decode_wire::<TerminalFailedWire>(payload)
            .and_then(terminal_failed_from_wire)
            .map(LifecycleControl::TerminalFailed),
        // This codec accepts only its declared message family and request/response roles.
        _ => Err(invalid_field(
            "message kind and role do not select a terminal-lifecycle payload",
        )),
    }
}

pub fn encode_lifecycle_control(
    value: &LifecycleControl,
) -> Result<EncodedLifecycleControl, ProtocolError> {
    let (kind, role, payload) = match value {
        LifecycleControl::TerminateTerminal(target) => (
            TERMINATE_TERMINAL_KIND,
            MessageRole::Request,
            serialize_wire(&terminal_target_to_wire(*target))?,
        ),
        LifecycleControl::TerminateTerminalAccepted(target) => (
            TERMINATE_TERMINAL_KIND,
            MessageRole::Response,
            serialize_wire(&terminal_target_to_wire(*target))?,
        ),
        LifecycleControl::DisposeTerminal(target) => (
            DISPOSE_TERMINAL_KIND,
            MessageRole::Request,
            serialize_wire(&terminal_target_to_wire(*target))?,
        ),
        LifecycleControl::DisposeTerminalAccepted(target) => (
            DISPOSE_TERMINAL_KIND,
            MessageRole::Response,
            serialize_wire(&terminal_target_to_wire(*target))?,
        ),
        LifecycleControl::TerminalStarted(identity) => (
            TERMINAL_STARTED_KIND,
            MessageRole::Event,
            serialize_wire(&identity_to_wire(*identity))?,
        ),
        LifecycleControl::TerminalExited(exited) => (
            TERMINAL_EXITED_KIND,
            MessageRole::Event,
            serialize_wire(&terminal_exited_to_wire(*exited)?)?,
        ),
        LifecycleControl::StartTerminal(start) => (
            START_TERMINAL_KIND,
            MessageRole::Request,
            serialize_wire(&start_terminal_to_wire(*start)?)?,
        ),
        LifecycleControl::TerminalStartAccepted(accepted) => (
            TERMINAL_START_ACCEPTED_KIND,
            MessageRole::Response,
            serialize_wire(&terminal_start_accepted_to_wire(*accepted)?)?,
        ),
        LifecycleControl::ForceTerminateTerminal(target) => (
            FORCE_TERMINATE_TERMINAL_KIND,
            MessageRole::Request,
            serialize_wire(&terminal_target_to_wire(*target))?,
        ),
        LifecycleControl::ForceTerminateTerminalAccepted(target) => (
            FORCE_TERMINATE_TERMINAL_KIND,
            MessageRole::Response,
            serialize_wire(&terminal_target_to_wire(*target))?,
        ),
        LifecycleControl::TerminalFailed(failed) => (
            TERMINAL_FAILED_KIND,
            MessageRole::Event,
            serialize_wire(&terminal_failed_to_wire(*failed)?)?,
        ),
    };
    Ok(EncodedLifecycleControl {
        kind,
        payload,
        role,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalTargetWire {
    terminal_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StartTerminalWire {
    geometry: TerminalPixelGeometryWire,
    terminal_id: String,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum StartTransitionWire {
    AlreadyStarted,
    Started,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TerminalStartAcceptedWire {
    geometry: TerminalPixelGeometryWire,
    identity: TerminalIdentityWire,
    transition: StartTransitionWire,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalExitedWire {
    exit: ExitResultWire,
    final_revision: String,
    identity: TerminalIdentityWire,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalFailedWire {
    failure: FailurePairWire,
    final_revision: String,
    identity: TerminalIdentityWire,
    process: ProcessObservationWire,
}

fn terminal_target_from_wire(wire: TerminalTargetWire) -> Result<TerminalTarget, ProtocolError> {
    Ok(TerminalTarget {
        terminal_id: parse_terminal_id(&wire.terminal_id)?,
    })
}

fn terminal_target_to_wire(value: TerminalTarget) -> TerminalTargetWire {
    TerminalTargetWire {
        terminal_id: value.terminal_id.to_text(),
    }
}

fn start_terminal_from_wire(wire: StartTerminalWire) -> Result<StartTerminal, ProtocolError> {
    Ok(StartTerminal {
        geometry: geometry_from_wire(wire.geometry)?,
        terminal_id: parse_terminal_id(&wire.terminal_id)?,
    })
}

fn start_terminal_to_wire(value: StartTerminal) -> Result<StartTerminalWire, ProtocolError> {
    Ok(StartTerminalWire {
        geometry: geometry_to_wire(value.geometry)?,
        terminal_id: value.terminal_id.to_text(),
    })
}

fn terminal_start_accepted_from_wire(
    wire: TerminalStartAcceptedWire,
) -> Result<TerminalStartAccepted, ProtocolError> {
    Ok(TerminalStartAccepted {
        geometry: geometry_from_wire(wire.geometry)?,
        identity: identity_from_wire(wire.identity)?,
        transition: match wire.transition {
            StartTransitionWire::AlreadyStarted => StartTransition::AlreadyStarted,
            StartTransitionWire::Started => StartTransition::Started,
        },
    })
}

fn terminal_start_accepted_to_wire(
    value: TerminalStartAccepted,
) -> Result<TerminalStartAcceptedWire, ProtocolError> {
    Ok(TerminalStartAcceptedWire {
        geometry: geometry_to_wire(value.geometry)?,
        identity: identity_to_wire(value.identity),
        transition: match value.transition {
            StartTransition::AlreadyStarted => StartTransitionWire::AlreadyStarted,
            StartTransition::Started => StartTransitionWire::Started,
        },
    })
}

fn terminal_exited_from_wire(wire: TerminalExitedWire) -> Result<TerminalExited, ProtocolError> {
    Ok(TerminalExited {
        exit: exit_result_from_wire(wire.exit)?,
        final_revision: canonical_u64(&wire.final_revision)?,
        identity: identity_from_wire(wire.identity)?,
    })
}

fn terminal_exited_to_wire(value: TerminalExited) -> Result<TerminalExitedWire, ProtocolError> {
    validate_exit_result(value.exit)?;
    Ok(TerminalExitedWire {
        exit: exit_result_to_wire(value.exit),
        final_revision: value.final_revision.to_string(),
        identity: identity_to_wire(value.identity),
    })
}

fn terminal_failed_from_wire(wire: TerminalFailedWire) -> Result<TerminalFailed, ProtocolError> {
    Ok(TerminalFailed {
        failure: runtime_failure_from_wire(wire.failure)?,
        final_revision: canonical_u64(&wire.final_revision)?,
        identity: identity_from_wire(wire.identity)?,
        process: process_observation_from_wire(wire.process)?,
    })
}

fn terminal_failed_to_wire(value: TerminalFailed) -> Result<TerminalFailedWire, ProtocolError> {
    validate_process_observation(value.process)?;
    Ok(TerminalFailedWire {
        failure: runtime_failure_to_wire(value.failure),
        final_revision: value.final_revision.to_string(),
        identity: identity_to_wire(value.identity),
        process: process_observation_to_wire(value.process),
    })
}

fn parse_terminal_id(value: &str) -> Result<TerminalId, ProtocolError> {
    TerminalId::parse(value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidIdentifier,
            "terminal target identity is not canonical nonzero 128-bit hexadecimal text",
        )
    })
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
