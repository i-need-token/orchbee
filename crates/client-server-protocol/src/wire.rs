//! Translates direction-specific closed JSON shapes into validated Client-Server values and
//! deterministic payloads, enforcing version, nullable-field, and numeric refinements. Binary
//! record framing and live authorization remain outside this JSON boundary.

use std::str;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{
    ClientMessage, CreateOutcome, Geometry, PROTOCOL_VERSION, ProtocolError, RequestId,
    ServerMessage, SessionId, StartMode, StartOutcome, TerminalCreateRequest, TerminalCreateResult,
    TerminalId, TerminalStartRequest, TerminalStartResult, WorkerConstraint, WorkingDirectory,
    number::Binary64SafeInteger,
};

const UTF8_BOM: &[u8] = &[0xef, 0xbb, 0xbf];

#[derive(Clone, Copy)]
struct ProtocolVersion;

impl Serialize for ProtocolVersion {
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        serializer.serialize_u8(PROTOCOL_VERSION)
    }
}

impl<'de> Deserialize<'de> for ProtocolVersion {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        let value = Binary64SafeInteger::deserialize(deserializer)?;
        if value.value() == u64::from(PROTOCOL_VERSION) {
            Ok(Self)
        } else {
            Err(de::Error::custom(
                "unsupported Client-Server protocol version",
            ))
        }
    }
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum DecodedStartMode {
    Immediate { geometry: Geometry },
    Deferred {},
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum DecodedCreateAcceptedState {
    AwaitingStart {},
    Started { geometry: Geometry },
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum DecodedCreateOutcome {
    Accepted {
        state: DecodedCreateAcceptedState,
    },
    Rejected {
        reason: crate::CreateRejectedReason,
    },
    Unavailable {
        reason: crate::CreateUnavailableReason,
    },
    Failed {
        reason: crate::CreateFailureReason,
    },
    OutcomeUnknown {},
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum DecodedStartOutcome {
    Accepted {
        geometry: Geometry,
        transition: crate::StartTransition,
    },
    Rejected {
        reason: crate::StartRejectedReason,
    },
    Unavailable {
        reason: crate::StartUnavailableReason,
    },
    Failed {
        reason: crate::StartFailureReason,
    },
    OutcomeUnknown {},
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase", deny_unknown_fields)]
enum DecodedClientMessage {
    #[serde(rename = "terminal.control")]
    Control {
        #[serde(rename = "protocolVersion")]
        _protocol_version: ProtocolVersion,
        request_id: RequestId,
        action: crate::TerminalControlAction,
    },
    #[serde(rename = "terminal.viewer")]
    Viewer {
        #[serde(rename = "protocolVersion")]
        _protocol_version: ProtocolVersion,
        request_id: RequestId,
        action: crate::TerminalViewerAction,
    },
    #[serde(rename = "terminal.create")]
    Create {
        #[serde(rename = "protocolVersion")]
        _protocol_version: ProtocolVersion,
        request_id: RequestId,
        #[serde(deserialize_with = "deserialize_nullable")]
        worker_constraint: Option<WorkerConstraint>,
        #[serde(deserialize_with = "deserialize_nullable")]
        cwd: Option<WorkingDirectory>,
        start_mode: DecodedStartMode,
        #[serde(default)]
        launch: Option<crate::LaunchSelection>,
        #[serde(default)]
        environment: crate::LaunchEnvironment,
        #[serde(default, deserialize_with = "deserialize_plugin_environment")]
        plugin_environment: Option<crate::PluginTerminalEnvironment>,
    },
    #[serde(rename = "terminal.start")]
    Start {
        #[serde(rename = "protocolVersion")]
        _protocol_version: ProtocolVersion,
        request_id: RequestId,
        terminal_create_request_id: RequestId,
        geometry: Geometry,
    },

    #[serde(rename = "terminal.launch.read")]
    LaunchRead {
        #[serde(rename = "protocolVersion")]
        _protocol_version: ProtocolVersion,
        request_id: RequestId,
        terminal_id: TerminalId,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase")]
enum EncodedClientMessage<'a> {
    #[serde(rename = "terminal.control")]
    Control {
        protocol_version: ProtocolVersion,
        request_id: &'a RequestId,
        action: &'a crate::TerminalControlAction,
    },
    #[serde(rename = "terminal.viewer")]
    Viewer {
        protocol_version: ProtocolVersion,
        request_id: &'a RequestId,
        action: &'a crate::TerminalViewerAction,
    },
    #[serde(rename = "terminal.create")]
    Create {
        protocol_version: ProtocolVersion,
        request_id: &'a RequestId,
        worker_constraint: &'a Option<WorkerConstraint>,
        cwd: &'a Option<WorkingDirectory>,
        start_mode: &'a StartMode,
        #[serde(skip_serializing_if = "Option::is_none")]
        launch: &'a Option<crate::LaunchSelection>,
        #[serde(skip_serializing_if = "crate::LaunchEnvironment::is_empty")]
        environment: &'a crate::LaunchEnvironment,
        #[serde(skip_serializing_if = "Option::is_none")]
        plugin_environment: Option<&'a crate::PluginTerminalEnvironment>,
    },
    #[serde(rename = "terminal.start")]
    Start {
        protocol_version: ProtocolVersion,
        request_id: &'a RequestId,
        terminal_create_request_id: &'a RequestId,
        geometry: Geometry,
    },

    #[serde(rename = "terminal.launch.read")]
    LaunchRead {
        protocol_version: ProtocolVersion,
        request_id: &'a RequestId,
        terminal_id: &'a TerminalId,
    },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase", deny_unknown_fields)]
enum DecodedServerMessage {
    #[serde(rename = "terminal.control.result")]
    Control {
        #[serde(rename = "protocolVersion")]
        _protocol_version: ProtocolVersion,
        request_id: RequestId,
        outcome: crate::TerminalControlOutcome,
    },
    #[serde(rename = "terminal.viewer.result")]
    Viewer {
        #[serde(rename = "protocolVersion")]
        _protocol_version: ProtocolVersion,
        request_id: RequestId,
        outcome: crate::TerminalViewerOutcome,
    },
    #[serde(rename = "terminal.create.result")]
    Create {
        #[serde(rename = "protocolVersion")]
        _protocol_version: ProtocolVersion,
        request_id: RequestId,
        outcome: DecodedCreateOutcome,
    },
    #[serde(rename = "terminal.start.result")]
    Start {
        #[serde(rename = "protocolVersion")]
        _protocol_version: ProtocolVersion,
        request_id: RequestId,
        outcome: DecodedStartOutcome,
    },
    #[serde(rename = "terminal.send.result")]
    Send {
        #[serde(rename = "protocolVersion")]
        _protocol_version: ProtocolVersion,
        request_id: RequestId,
        #[serde(deserialize_with = "deserialize_nullable")]
        target: Option<crate::SendTarget>,
        outcome: crate::TerminalSendOutcome,
    },
    #[serde(rename = "terminal.launch.read.result")]
    LaunchRead {
        #[serde(rename = "protocolVersion")]
        _protocol_version: ProtocolVersion,
        request_id: RequestId,
        outcome: DecodedTerminalLaunchReadOutcome,
    },
}

/// The launch fact decodes into a checked intermediate shape: `createOutcome`
/// is a `DecodedCreateOutcome` like every create-result boundary, and the
/// conditional-null invariants are enforced when the fact is built.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DecodedTerminalLaunchFact {
    state: crate::TerminalLaunchState,
    #[serde(deserialize_with = "crate::number::positive_decimal_nullable::deserialize")]
    worker_generation: Option<std::num::NonZeroU64>,
    #[serde(deserialize_with = "deserialize_nullable")]
    create_outcome: Option<DecodedCreateOutcome>,
    #[serde(deserialize_with = "deserialize_nullable")]
    geometry: Option<Geometry>,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum DecodedTerminalLaunchReadOutcome {
    Found {
        #[serde(rename = "sessionId")]
        session_id: SessionId,
        #[serde(deserialize_with = "crate::number::decimal::deserialize")]
        workbench_revision: u64,
        terminal: Box<crate::TerminalLaunchRoute>,
        launch: Box<DecodedTerminalLaunchFact>,
        /// The additive provenance member: present only on profile-launched
        /// terminals, absent (not null) otherwise — `default` keeps readers
        /// tolerant of senders that predate it.
        #[serde(default)]
        launch_profile: Option<crate::LaunchProfileProvenance>,
        #[serde(default, deserialize_with = "deserialize_launch_selection")]
        launch_selection: Option<crate::LaunchSelection>,
    },
    NotFound,
    Unavailable {
        reason: crate::TerminalLaunchReadUnavailableReason,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase")]
enum EncodedServerMessage<'a> {
    #[serde(rename = "terminal.control.result")]
    Control {
        protocol_version: ProtocolVersion,
        request_id: &'a RequestId,
        outcome: &'a crate::TerminalControlOutcome,
    },
    #[serde(rename = "terminal.viewer.result")]
    Viewer {
        protocol_version: ProtocolVersion,
        request_id: &'a RequestId,
        outcome: &'a crate::TerminalViewerOutcome,
    },
    #[serde(rename = "terminal.create.result")]
    Create {
        protocol_version: ProtocolVersion,
        request_id: &'a RequestId,
        outcome: CreateOutcome,
    },
    #[serde(rename = "terminal.start.result")]
    Start {
        protocol_version: ProtocolVersion,
        request_id: &'a RequestId,
        outcome: StartOutcome,
    },
    #[serde(rename = "terminal.send.result")]
    Send {
        protocol_version: ProtocolVersion,
        request_id: &'a RequestId,
        target: &'a Option<crate::SendTarget>,
        outcome: &'a crate::TerminalSendOutcome,
    },
    #[serde(rename = "terminal.launch.read.result")]
    LaunchRead {
        protocol_version: ProtocolVersion,
        request_id: &'a RequestId,
        outcome: &'a crate::TerminalLaunchReadOutcome,
    },
}

pub(crate) fn decode_client_payload(payload: &[u8]) -> Result<ClientMessage, ProtocolError> {
    let text = strict_json_text(payload)?;
    let decoded = serde_json::from_str::<DecodedClientMessage>(text)
        .map_err(|_error| ProtocolError::InvalidPayload)?;
    match decoded {
        DecodedClientMessage::Control {
            _protocol_version: _,
            request_id,
            action,
        } => Ok(ClientMessage::TerminalControl(
            crate::TerminalControlRequest::new(request_id, action),
        )),
        DecodedClientMessage::Viewer {
            _protocol_version: _,
            request_id,
            action,
        } => {
            action.validate()?;
            Ok(ClientMessage::TerminalViewer(
                crate::TerminalViewerRequest::new(request_id, action),
            ))
        }
        DecodedClientMessage::Create {
            _protocol_version: _,
            request_id,
            worker_constraint,
            cwd,
            start_mode,
            launch,
            environment,
            plugin_environment,
        } => Ok(ClientMessage::TerminalCreate(
            TerminalCreateRequest::new(
                request_id,
                worker_constraint,
                cwd,
                start_mode.into(),
                launch,
            )
            .with_environment(environment)
            .with_plugin_environment(plugin_environment)?,
        )),
        DecodedClientMessage::Start {
            _protocol_version: _,
            request_id,
            terminal_create_request_id,
            geometry,
        } => TerminalStartRequest::new(request_id, terminal_create_request_id, geometry)
            .map(ClientMessage::TerminalStart)
            .map_err(|_error| ProtocolError::InvalidPayload),
        DecodedClientMessage::LaunchRead {
            _protocol_version: _,
            request_id,
            terminal_id,
        } => Ok(ClientMessage::TerminalLaunchRead(
            crate::TerminalLaunchReadRequest::new(request_id, terminal_id),
        )),
    }
}

pub(crate) fn encode_client_payload(message: &ClientMessage) -> Result<Vec<u8>, ProtocolError> {
    let encoded = match message {
        ClientMessage::TerminalControl(request) => EncodedClientMessage::Control {
            protocol_version: ProtocolVersion,
            request_id: &request.request_id,
            action: &request.action,
        },
        ClientMessage::TerminalViewer(request) => EncodedClientMessage::Viewer {
            protocol_version: ProtocolVersion,
            request_id: &request.request_id,
            action: &request.action,
        },
        ClientMessage::TerminalCreate(request) => EncodedClientMessage::Create {
            protocol_version: ProtocolVersion,
            request_id: &request.request_id,
            worker_constraint: &request.worker_constraint,
            cwd: &request.cwd,
            start_mode: &request.start_mode,
            launch: &request.launch,
            environment: request.environment(),
            plugin_environment: request.plugin_environment(),
        },
        ClientMessage::TerminalStart(request) => EncodedClientMessage::Start {
            protocol_version: ProtocolVersion,
            request_id: &request.request_id,
            terminal_create_request_id: &request.terminal_create_request_id,
            geometry: request.geometry,
        },
        ClientMessage::TerminalSend(_) => {
            unreachable!("send requests ride the dedicated OBSN record frame")
        }
        ClientMessage::TerminalLaunchRead(request) => EncodedClientMessage::LaunchRead {
            protocol_version: ProtocolVersion,
            request_id: &request.request_id,
            terminal_id: &request.terminal_id,
        },
    };
    serde_json::to_vec(&encoded).map_err(|_error| ProtocolError::InvalidPayload)
}

pub(crate) fn decode_server_payload(payload: &[u8]) -> Result<ServerMessage, ProtocolError> {
    let text = strict_json_text(payload)?;
    let decoded = serde_json::from_str::<DecodedServerMessage>(text)
        .map_err(|_error| ProtocolError::InvalidPayload)?;
    Ok(match decoded {
        DecodedServerMessage::Control {
            _protocol_version: _,
            request_id,
            outcome,
        } => ServerMessage::TerminalControlResult(crate::TerminalControlResult::new(
            request_id, outcome,
        )),
        DecodedServerMessage::Viewer {
            _protocol_version: _,
            request_id,
            outcome,
        } => ServerMessage::TerminalViewerResult(crate::TerminalViewerResult::new(
            request_id, outcome,
        )),
        DecodedServerMessage::Create {
            _protocol_version: _,
            request_id,
            outcome,
        } => ServerMessage::TerminalCreateResult(TerminalCreateResult::new(
            request_id,
            outcome.into(),
        )),
        DecodedServerMessage::Start {
            _protocol_version: _,
            request_id,
            outcome,
        } => {
            ServerMessage::TerminalStartResult(TerminalStartResult::new(request_id, outcome.into()))
        }
        DecodedServerMessage::Send {
            _protocol_version: _,
            request_id,
            target,
            outcome,
        } => ServerMessage::TerminalSendResult(
            crate::TerminalSendResult::new(request_id, target, outcome)
                .map_err(|_error| ProtocolError::InvalidPayload)?,
        ),
        DecodedServerMessage::LaunchRead {
            _protocol_version: _,
            request_id,
            outcome,
        } => {
            let outcome = match outcome {
                DecodedTerminalLaunchReadOutcome::Found {
                    session_id,
                    workbench_revision,
                    terminal,
                    launch,
                    launch_profile,
                    launch_selection,
                } => {
                    if launch.state == crate::TerminalLaunchState::NotTracked
                        && launch_selection.is_some()
                    {
                        return Err(ProtocolError::InvalidPayload);
                    }
                    crate::TerminalLaunchReadOutcome::Found {
                        session_id,
                        workbench_revision,
                        terminal,
                        launch: Box::new(crate::TerminalLaunchFact::new(
                            launch.state,
                            launch.worker_generation,
                            launch.create_outcome.map(Into::into),
                            launch.geometry,
                        )?),
                        launch_profile,
                        launch_selection: launch_selection.map(Box::new),
                    }
                }
                DecodedTerminalLaunchReadOutcome::NotFound => {
                    crate::TerminalLaunchReadOutcome::NotFound
                }
                DecodedTerminalLaunchReadOutcome::Unavailable { reason } => {
                    crate::TerminalLaunchReadOutcome::Unavailable { reason }
                }
            };
            ServerMessage::TerminalLaunchReadResult(crate::TerminalLaunchReadResult::new(
                request_id, outcome,
            ))
        }
    })
}

pub(crate) fn encode_server_payload(message: &ServerMessage) -> Result<Vec<u8>, ProtocolError> {
    let encoded = match message {
        ServerMessage::TerminalControlResult(result) => EncodedServerMessage::Control {
            protocol_version: ProtocolVersion,
            request_id: &result.request_id,
            outcome: &result.outcome,
        },
        ServerMessage::TerminalViewerResult(result) => EncodedServerMessage::Viewer {
            protocol_version: ProtocolVersion,
            request_id: &result.request_id,
            outcome: &result.outcome,
        },
        ServerMessage::TerminalCreateResult(result) => EncodedServerMessage::Create {
            protocol_version: ProtocolVersion,
            request_id: &result.request_id,
            outcome: result.outcome,
        },
        ServerMessage::TerminalStartResult(result) => EncodedServerMessage::Start {
            protocol_version: ProtocolVersion,
            request_id: &result.request_id,
            outcome: result.outcome,
        },
        ServerMessage::TerminalSendResult(result) => {
            result.validate()?;
            EncodedServerMessage::Send {
                protocol_version: ProtocolVersion,
                request_id: &result.request_id,
                target: &result.target,
                outcome: &result.outcome,
            }
        }
        ServerMessage::TerminalLaunchReadResult(result) => {
            if let crate::TerminalLaunchReadOutcome::Found {
                launch,
                launch_selection,
                ..
            } = &result.outcome
            {
                launch.validate()?;
                if launch.state() == crate::TerminalLaunchState::NotTracked
                    && launch_selection.is_some()
                {
                    return Err(ProtocolError::InvalidPayload);
                }
            }
            EncodedServerMessage::LaunchRead {
                protocol_version: ProtocolVersion,
                request_id: &result.request_id,
                outcome: &result.outcome,
            }
        }
    };
    serde_json::to_vec(&encoded).map_err(|_error| ProtocolError::InvalidPayload)
}

fn strict_json_text(payload: &[u8]) -> Result<&str, ProtocolError> {
    if payload.starts_with(UTF8_BOM) {
        return Err(ProtocolError::Utf8Bom);
    }
    str::from_utf8(payload).map_err(|_error| ProtocolError::InvalidUtf8)
}

fn deserialize_nullable<'de, DeserializerType, Value>(
    deserializer: DeserializerType,
) -> Result<Option<Value>, DeserializerType::Error>
where
    DeserializerType: Deserializer<'de>,
    Value: Deserialize<'de>,
{
    Option::<Value>::deserialize(deserializer)
}

fn deserialize_plugin_environment<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<crate::PluginTerminalEnvironment>, D::Error> {
    crate::PluginTerminalEnvironment::deserialize(deserializer).map(Some)
}

fn deserialize_launch_selection<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<crate::LaunchSelection>, D::Error> {
    crate::LaunchSelection::deserialize(deserializer).map(Some)
}

impl From<DecodedStartMode> for StartMode {
    fn from(value: DecodedStartMode) -> Self {
        match value {
            DecodedStartMode::Immediate { geometry } => Self::Immediate { geometry },
            DecodedStartMode::Deferred {} => Self::Deferred,
        }
    }
}

impl From<DecodedCreateAcceptedState> for crate::CreateAcceptedState {
    fn from(value: DecodedCreateAcceptedState) -> Self {
        match value {
            DecodedCreateAcceptedState::AwaitingStart {} => Self::AwaitingStart,
            DecodedCreateAcceptedState::Started { geometry } => Self::Started { geometry },
        }
    }
}

impl From<DecodedCreateOutcome> for CreateOutcome {
    fn from(value: DecodedCreateOutcome) -> Self {
        match value {
            DecodedCreateOutcome::Accepted { state } => Self::Accepted {
                state: state.into(),
            },
            DecodedCreateOutcome::Rejected { reason } => Self::Rejected { reason },
            DecodedCreateOutcome::Unavailable { reason } => Self::Unavailable { reason },
            DecodedCreateOutcome::Failed { reason } => Self::Failed { reason },
            DecodedCreateOutcome::OutcomeUnknown {} => Self::OutcomeUnknown,
        }
    }
}

impl From<DecodedStartOutcome> for StartOutcome {
    fn from(value: DecodedStartOutcome) -> Self {
        match value {
            DecodedStartOutcome::Accepted {
                geometry,
                transition,
            } => Self::Accepted {
                geometry,
                transition,
            },
            DecodedStartOutcome::Rejected { reason } => Self::Rejected { reason },
            DecodedStartOutcome::Unavailable { reason } => Self::Unavailable { reason },
            DecodedStartOutcome::Failed { reason } => Self::Failed { reason },
            DecodedStartOutcome::OutcomeUnknown {} => Self::OutcomeUnknown,
        }
    }
}
