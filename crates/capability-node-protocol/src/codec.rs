//! Frames bounded Capability Node messages and dispatches their binary payload codecs,
//! preserving exact record boundaries and checked domain construction. It opens no transport
//! and does not validate live Worker generations, leases, or operation authorization.

use std::{
    io::{Read, Write},
    num::{NonZeroU16, NonZeroU64},
};

use crate::{
    error::{ProtocolError, ReadMessageError, WriteMessageError},
    identity::{
        HostCreationId, HostEpoch, HostTerminalId, LeaseId, OperationId, TerminalId, WorkerId,
    },
    terminal_launch::{
        AbsoluteCwd, AuthorizedTerminalLaunch, AuthorizedTerminalStart, HostLaunchAccepted,
        HostLaunchState, HostStartAccepted, HostTerminalIdentity, LeaseExpiresAtUnixMilliseconds,
        StartTransition, TerminalFence, TerminalLaunchCapabilityVersion, TerminalLaunchFailure,
        TerminalLaunchMode, TerminalLaunchOutcome, TerminalLaunchRejection, TerminalLaunchResult,
        TerminalOperationAuthorization, TerminalOperationLease, TerminalPixelGeometry,
        TerminalStartFailure, TerminalStartOutcome, TerminalStartRejection, TerminalStartResult,
        WorkerGeneration,
    },
};

mod agent_drain;
mod control;
mod cwd_validation;
mod graphics_decode;
mod lifecycle;
mod plugin_job;
mod send;
mod shutdown;
mod terminal_editor;
mod terminal_titles;
mod viewer;
mod worker_handoff;
mod worktree_job;
mod worktree_mutation;

const FRAME_MAGIC: [u8; 4] = *b"OBCN";
const FRAME_HEADER_BYTES: usize = 8;
const IMMEDIATE_LAUNCH_KIND: u8 = 1;
const DEFERRED_LAUNCH_KIND: u8 = 2;
const HOST_ACCEPTED_OUTCOME: u8 = 1;
const REJECTED_OUTCOME: u8 = 2;
const FAILED_OUTCOME: u8 = 3;
const OUTCOME_UNKNOWN: u8 = 4;
const AWAITING_START_STATE: u8 = 1;
const STARTED_STATE: u8 = 2;
const AUTHORIZED_TERMINAL_LAUNCH_KIND: u8 = 1;
const TERMINAL_LAUNCH_RESULT_KIND: u8 = 2;
const AUTHORIZED_TERMINAL_START_KIND: u8 = 3;
const TERMINAL_START_RESULT_KIND: u8 = 4;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAXIMUM_FRAME_PAYLOAD_BYTES: usize = 40 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityNodeMessage {
    WorkerHandoffRequest(crate::WorkerHandoffRequest),
    WorkerHandoffResult(crate::WorkerHandoffResult),
    WorkerHandoffHello(crate::WorkerHandoffPeer),
    WorkerHandoffReady(crate::WorkerHandoffPeer),
    WorkerHandoffOwned(crate::WorkerHandoffPeer),
    TerminalGraphicsDecodeRequest(crate::TerminalGraphicsDecodeRequest),
    TerminalEditorCommandRequest(crate::TerminalEditorCommandRequest),
    TerminalEditorCommandResult(crate::TerminalEditorCommandResult),
    TerminalTitlesRequest(crate::TerminalTitlesRequest),
    TerminalTitlesResult(crate::TerminalTitlesResult),
    PluginJobRequest(crate::PluginJobRequest),
    PluginJobResult(crate::PluginJobResult),
    WorktreeMutationRequest(crate::WorktreeMutationRequest),
    WorktreeMutationResult(crate::WorktreeMutationResult),
    WorkerShutdownRequest(crate::WorkerShutdownRequest),
    WorkerShutdownResult(crate::WorkerShutdownResult),
    TerminalLifecycleRequest(crate::TerminalLifecycleRequest),
    TerminalLifecycleResult(crate::TerminalLifecycleResult),
    TerminalControlRequest(crate::TerminalControlRequest),
    TerminalControlResult(crate::TerminalControlResult),
    TerminalCwdValidationRequest(crate::TerminalCwdValidationRequest),
    TerminalCwdValidationResult(crate::TerminalCwdValidationResult),
    AuthorizedTerminalLaunch(AuthorizedTerminalLaunch),
    TerminalLaunchResult(TerminalLaunchResult),
    AuthorizedTerminalStart(AuthorizedTerminalStart),
    TerminalStartResult(TerminalStartResult),
    TerminalViewerRequest(crate::TerminalViewerRequest),
    TerminalViewerResult(crate::TerminalViewerResult),
    TerminalSendRequest(crate::TerminalSendRequest),
    TerminalSendResult(crate::TerminalSendResult),
    WorktreeJobRequest(crate::WorktreeJobRequest),
    WorktreeJobResult(crate::WorktreeJobResult),
    AgentDrainRequest(crate::AgentDrainRequest),
    AgentDrainReply(crate::AgentDrainReply),
}

pub fn encode_message(message: &CapabilityNodeMessage) -> Result<Vec<u8>, ProtocolError> {
    if let CapabilityNodeMessage::TerminalGraphicsDecodeRequest(request) = message {
        return graphics_decode::encode(request);
    }
    if matches!(
        message,
        CapabilityNodeMessage::PluginJobRequest(_) | CapabilityNodeMessage::PluginJobResult(_)
    ) {
        return plugin_job::encode(message);
    }
    if let CapabilityNodeMessage::TerminalViewerResult(crate::TerminalViewerResult {
        outcome: crate::TerminalViewerOutcome::Record(_),
        ..
    }) = message
    {
        return viewer::encode_record(message);
    }
    if let CapabilityNodeMessage::TerminalControlRequest(request) = message
        && let crate::TerminalControlAction::Paste { .. } = request.action
    {
        return control::encode_paste_record(request);
    }
    if let CapabilityNodeMessage::TerminalSendRequest(request) = message {
        return send::encode_request_record(request);
    }
    if let CapabilityNodeMessage::TerminalTitlesResult(value) = message {
        return terminal_titles::encode_result(value);
    }
    if let CapabilityNodeMessage::TerminalTitlesRequest(value) = message {
        value.validate()?;
    }
    let payload = encode_payload(message);
    if payload.len() > MAXIMUM_FRAME_PAYLOAD_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: payload.len(),
            maximum: MAXIMUM_FRAME_PAYLOAD_BYTES,
        });
    }
    let payload_length =
        u16::try_from(payload.len()).map_err(|_error| ProtocolError::PayloadTooLarge {
            actual: payload.len(),
            maximum: MAXIMUM_FRAME_PAYLOAD_BYTES,
        })?;
    let mut frame = Vec::with_capacity(FRAME_HEADER_BYTES + payload.len());
    frame.extend_from_slice(&FRAME_MAGIC);
    frame.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    frame.extend_from_slice(&payload_length.to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn decode_message(frame: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    if frame.starts_with(b"OBNG") {
        return graphics_decode::decode(frame);
    }
    if frame.starts_with(b"OBNT") {
        return terminal_titles::decode_result(frame);
    }
    if frame.starts_with(b"OBPJ") {
        return plugin_job::decode(frame);
    }
    if frame.starts_with(b"OBNR") {
        return viewer::decode_record(frame);
    }
    if frame.starts_with(b"OBNQ") {
        return control::decode_paste_record(frame);
    }
    if frame.starts_with(b"OBNS") {
        return send::decode_request_record(frame).map(CapabilityNodeMessage::TerminalSendRequest);
    }
    let header: &[u8; FRAME_HEADER_BYTES] = frame
        .get(..FRAME_HEADER_BYTES)
        .ok_or(ProtocolError::InvalidPayload)?
        .try_into()
        .map_err(|_error| ProtocolError::InvalidPayload)?;
    let payload_length = decode_header(header)?;
    let expected_frame_length = FRAME_HEADER_BYTES
        .checked_add(payload_length)
        .ok_or(ProtocolError::InvalidPayload)?;
    if frame.len() != expected_frame_length {
        return Err(ProtocolError::InvalidPayload);
    }
    decode_payload(&frame[FRAME_HEADER_BYTES..])
}

pub fn read_message(reader: &mut impl Read) -> Result<CapabilityNodeMessage, ReadMessageError> {
    let mut header = [0_u8; FRAME_HEADER_BYTES];
    reader.read_exact(&mut header)?;
    if header.starts_with(b"OBNG") {
        return graphics_decode::read(reader, header);
    }
    if header.starts_with(b"OBNT") {
        return terminal_titles::read_result(reader, header);
    }
    if header.starts_with(b"OBPJ") {
        return plugin_job::read(reader, header);
    }
    if header.starts_with(b"OBNR") {
        return viewer::read_record(reader, header);
    }
    if header.starts_with(b"OBNQ") {
        return control::read_paste_record(reader, header);
    }
    if header.starts_with(b"OBNS") {
        return send::read_request_record(reader, header)
            .map(CapabilityNodeMessage::TerminalSendRequest);
    }
    let payload_length = decode_header(&header)?;
    let mut payload = vec![0_u8; payload_length];
    reader.read_exact(&mut payload)?;
    decode_payload(&payload).map_err(Into::into)
}

pub fn write_message(
    writer: &mut impl Write,
    message: &CapabilityNodeMessage,
) -> Result<(), WriteMessageError> {
    let frame = encode_message(message)?;
    writer.write_all(&frame).map_err(Into::into)
}

fn encode_payload(message: &CapabilityNodeMessage) -> Vec<u8> {
    match message {
        CapabilityNodeMessage::WorkerHandoffRequest(value) => worker_handoff::request(*value),
        CapabilityNodeMessage::WorkerHandoffResult(value) => worker_handoff::result(*value),
        CapabilityNodeMessage::WorkerHandoffHello(value) => worker_handoff::peer(28, *value),
        CapabilityNodeMessage::WorkerHandoffReady(value) => worker_handoff::peer(29, *value),
        CapabilityNodeMessage::WorkerHandoffOwned(value) => worker_handoff::peer(30, *value),
        CapabilityNodeMessage::TerminalEditorCommandRequest(value) => {
            terminal_editor::encode_request(*value)
        }
        CapabilityNodeMessage::TerminalEditorCommandResult(value) => {
            terminal_editor::encode_result(value)
        }
        CapabilityNodeMessage::PluginJobRequest(_) | CapabilityNodeMessage::PluginJobResult(_) => {
            unreachable!("plugin jobs use OBPJ records")
        }
        CapabilityNodeMessage::WorktreeMutationRequest(value) => {
            worktree_mutation::encode_request(value)
        }
        CapabilityNodeMessage::WorktreeMutationResult(value) => {
            worktree_mutation::encode_result(value)
        }
        CapabilityNodeMessage::WorkerShutdownRequest(value) => shutdown::encode_request(*value),
        CapabilityNodeMessage::WorkerShutdownResult(value) => shutdown::encode_result(*value),
        CapabilityNodeMessage::TerminalLifecycleRequest(value) => lifecycle::encode_request(*value),
        CapabilityNodeMessage::TerminalLifecycleResult(value) => lifecycle::encode_result(*value),
        CapabilityNodeMessage::TerminalControlRequest(value) => control::encode_request(value),
        CapabilityNodeMessage::TerminalControlResult(value) => control::encode_result(*value),
        CapabilityNodeMessage::TerminalCwdValidationRequest(value) => {
            cwd_validation::encode_request(value)
        }
        CapabilityNodeMessage::TerminalCwdValidationResult(value) => {
            cwd_validation::encode_result(value)
        }
        CapabilityNodeMessage::AuthorizedTerminalLaunch(value) => {
            encode_authorized_terminal_launch(value)
        }
        CapabilityNodeMessage::TerminalLaunchResult(value) => encode_terminal_launch_result(*value),
        CapabilityNodeMessage::AuthorizedTerminalStart(value) => {
            encode_authorized_terminal_start(*value)
        }
        CapabilityNodeMessage::TerminalStartResult(value) => encode_terminal_start_result(*value),
        CapabilityNodeMessage::TerminalGraphicsDecodeRequest(value) => {
            graphics_decode::encode(value).expect("validated graphics request")
        }
        CapabilityNodeMessage::TerminalViewerRequest(value) => viewer::encode_request(*value),
        CapabilityNodeMessage::TerminalViewerResult(value) => viewer::encode_result(value),
        CapabilityNodeMessage::TerminalSendRequest(_) => {
            unreachable!("send requests ride the dedicated OBNS record frame")
        }
        CapabilityNodeMessage::TerminalSendResult(value) => {
            send::encode_result(value).expect("validated send result")
        }
        CapabilityNodeMessage::WorktreeJobRequest(value) => worktree_job::encode_request(value),
        CapabilityNodeMessage::WorktreeJobResult(value) => worktree_job::encode_result(value),
        CapabilityNodeMessage::TerminalTitlesRequest(value) => {
            terminal_titles::encode_request(value).expect("validated title request")
        }
        CapabilityNodeMessage::TerminalTitlesResult(_) => unreachable!("title results use OBNT"),
        CapabilityNodeMessage::AgentDrainRequest(value) => agent_drain::encode_request(value),
        CapabilityNodeMessage::AgentDrainReply(value) => {
            agent_drain::encode_reply(value).expect("validated agent drain reply")
        }
    }
}

fn decode_header(header: &[u8; FRAME_HEADER_BYTES]) -> Result<usize, ProtocolError> {
    if header.get(..FRAME_MAGIC.len()) != Some(FRAME_MAGIC.as_slice()) {
        return Err(ProtocolError::InvalidMagic);
    }
    let version = u16::from_be_bytes([header[4], header[5]]);
    if version != PROTOCOL_VERSION {
        return Err(ProtocolError::IncompatibleVersion { received: version });
    }
    let payload_length = usize::from(u16::from_be_bytes([header[6], header[7]]));
    if payload_length > MAXIMUM_FRAME_PAYLOAD_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: payload_length,
            maximum: MAXIMUM_FRAME_PAYLOAD_BYTES,
        });
    }
    Ok(payload_length)
}

fn decode_payload(payload: &[u8]) -> Result<CapabilityNodeMessage, ProtocolError> {
    match payload.first().copied() {
        Some(26) => {
            worker_handoff::read_request(payload).map(CapabilityNodeMessage::WorkerHandoffRequest)
        }
        Some(27) => {
            worker_handoff::read_result(payload).map(CapabilityNodeMessage::WorkerHandoffResult)
        }
        Some(28) => {
            worker_handoff::read_peer(28, payload).map(CapabilityNodeMessage::WorkerHandoffHello)
        }
        Some(29) => {
            worker_handoff::read_peer(29, payload).map(CapabilityNodeMessage::WorkerHandoffReady)
        }
        Some(30) => {
            worker_handoff::read_peer(30, payload).map(CapabilityNodeMessage::WorkerHandoffOwned)
        }
        Some(24) => terminal_editor::decode_request(payload)
            .map(CapabilityNodeMessage::TerminalEditorCommandRequest),
        Some(25) => terminal_editor::decode_result(payload)
            .map(CapabilityNodeMessage::TerminalEditorCommandResult),
        Some(23) => terminal_titles::decode_request(payload)
            .map(CapabilityNodeMessage::TerminalTitlesRequest),
        Some(21) => worktree_mutation::decode_request(payload)
            .map(CapabilityNodeMessage::WorktreeMutationRequest),
        Some(22) => worktree_mutation::decode_result(payload)
            .map(CapabilityNodeMessage::WorktreeMutationResult),
        Some(19) => {
            shutdown::decode_request(payload).map(CapabilityNodeMessage::WorkerShutdownRequest)
        }
        Some(20) => {
            shutdown::decode_result(payload).map(CapabilityNodeMessage::WorkerShutdownResult)
        }
        Some(9) => {
            lifecycle::decode_request(payload).map(CapabilityNodeMessage::TerminalLifecycleRequest)
        }
        Some(10) => {
            lifecycle::decode_result(payload).map(CapabilityNodeMessage::TerminalLifecycleResult)
        }
        Some(11) => cwd_validation::decode_request(payload)
            .map(CapabilityNodeMessage::TerminalCwdValidationRequest),
        Some(12) => cwd_validation::decode_result(payload)
            .map(CapabilityNodeMessage::TerminalCwdValidationResult),
        Some(7) => {
            control::decode_request(payload).map(CapabilityNodeMessage::TerminalControlRequest)
        }
        Some(8) => {
            control::decode_result(payload).map(CapabilityNodeMessage::TerminalControlResult)
        }
        Some(5) => {
            viewer::decode_request(payload).map(CapabilityNodeMessage::TerminalViewerRequest)
        }
        Some(6) => viewer::decode_result(payload).map(CapabilityNodeMessage::TerminalViewerResult),
        Some(14) => send::decode_result(payload).map(CapabilityNodeMessage::TerminalSendResult),
        Some(15) => {
            worktree_job::decode_request(payload).map(CapabilityNodeMessage::WorktreeJobRequest)
        }
        Some(16) => {
            worktree_job::decode_result(payload).map(CapabilityNodeMessage::WorktreeJobResult)
        }
        Some(17) => {
            agent_drain::decode_request(payload).map(CapabilityNodeMessage::AgentDrainRequest)
        }
        Some(18) => agent_drain::decode_reply(payload).map(CapabilityNodeMessage::AgentDrainReply),
        Some(AUTHORIZED_TERMINAL_LAUNCH_KIND) => decode_authorized_terminal_launch(payload)
            .map(CapabilityNodeMessage::AuthorizedTerminalLaunch),
        Some(TERMINAL_LAUNCH_RESULT_KIND) => {
            decode_terminal_launch_result(payload).map(CapabilityNodeMessage::TerminalLaunchResult)
        }
        Some(AUTHORIZED_TERMINAL_START_KIND) => decode_authorized_terminal_start(payload)
            .map(CapabilityNodeMessage::AuthorizedTerminalStart),
        Some(TERMINAL_START_RESULT_KIND) => {
            decode_terminal_start_result(payload).map(CapabilityNodeMessage::TerminalStartResult)
        }
        Some(_) | None => Err(ProtocolError::InvalidPayload),
    }
}

fn encode_authorized_terminal_launch(value: &AuthorizedTerminalLaunch) -> Vec<u8> {
    let mut payload = Vec::with_capacity(128);
    payload.push(AUTHORIZED_TERMINAL_LAUNCH_KIND);
    encode_authorization(&mut payload, value.authorization());
    encode_cwd(&mut payload, value.cwd());
    encode_launch_mode(&mut payload, value.mode());
    if let Some(source) = value.editor_source() {
        payload.push(4);
        encode_launch_selection(
            &mut payload,
            value
                .launch()
                .expect("editor source requires explicit launch"),
        );
        encode_launch_environment(&mut payload, value.environment());
        payload.extend(source.attachment_id.as_bytes());
        payload.extend(source.host_epoch.as_bytes());
        payload.extend(source.terminal_id.as_bytes());
        payload.extend(source.generation.get().to_be_bytes());
        payload.extend(source.revision.to_be_bytes());
    } else if let Some(plugin) = value.plugin_environment() {
        payload.push(3);
        encode_launch_selection(
            &mut payload,
            value
                .launch()
                .expect("plugin environment requires explicit launch"),
        );
        encode_launch_environment(&mut payload, value.environment());
        encode_environment_entries(&mut payload, plugin.entries());
    } else if let Some(launch) = value.launch() {
        payload.push(1);
        encode_launch_selection(&mut payload, launch);
        encode_launch_environment(&mut payload, value.environment());
    } else {
        payload.push(0);
        encode_launch_environment(&mut payload, value.environment());
    }
    payload
}

fn decode_authorized_terminal_launch(
    payload: &[u8],
) -> Result<AuthorizedTerminalLaunch, ProtocolError> {
    let mut reader = PayloadReader::after_kind(payload, AUTHORIZED_TERMINAL_LAUNCH_KIND)?;
    let authorization = decode_authorization(&mut reader)?;
    let cwd = decode_cwd(&mut reader)?;
    let mode = decode_launch_mode(&mut reader)?;
    let mut plugin_environment = None;
    let mut editor_source = None;
    let (launch, environment) = match reader.read_u8()? {
        4 => {
            let launch = decode_launch_selection(&mut reader)?;
            let environment = decode_launch_environment(&mut reader)?;
            editor_source = Some(crate::TerminalEditorSource {
                attachment_id: OperationId::from_bytes(reader.read_array()?)?,
                host_epoch: HostEpoch::from_bytes(reader.read_array()?)?,
                terminal_id: HostTerminalId::from_bytes(reader.read_array()?)?,
                generation: NonZeroU64::new(reader.read_u64()?)
                    .ok_or(ProtocolError::InvalidPayload)?,
                revision: reader.read_u64()?,
            });
            (Some(launch), environment)
        }
        3 => {
            let launch = decode_launch_selection(&mut reader)?;
            let environment = decode_launch_environment(&mut reader)?;
            plugin_environment = Some(crate::PluginTerminalEnvironment::new(
                decode_environment_entries(
                    &mut reader,
                    crate::PLUGIN_TERMINAL_ENVIRONMENT_NAMES.len(),
                    crate::MAXIMUM_PLUGIN_TERMINAL_ENVIRONMENT_BYTES,
                )?,
            )?);
            (Some(launch), environment)
        }
        // Tag 2 carried the retired default-shell override.
        1 => {
            let launch = decode_launch_selection(&mut reader)?;
            (Some(launch), decode_launch_environment(&mut reader)?)
        }
        0 => (None, decode_launch_environment(&mut reader)?),
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    AuthorizedTerminalLaunch::new(authorization, cwd, mode, launch)
        .with_environment(environment)
        .with_plugin_environment(plugin_environment)?
        .with_editor_source(editor_source)
}

fn encode_terminal_launch_result(value: TerminalLaunchResult) -> Vec<u8> {
    let mut payload = Vec::with_capacity(128);
    payload.push(TERMINAL_LAUNCH_RESULT_KIND);
    encode_authorization(&mut payload, value.authorization());
    match value.outcome() {
        TerminalLaunchOutcome::HostAccepted(accepted) => {
            payload.push(HOST_ACCEPTED_OUTCOME);
            encode_host_identity(&mut payload, accepted.identity());
            match accepted.state() {
                HostLaunchState::AwaitingStart => payload.push(AWAITING_START_STATE),
                HostLaunchState::Started(geometry) => {
                    payload.push(STARTED_STATE);
                    encode_geometry(&mut payload, geometry);
                }
            }
        }
        TerminalLaunchOutcome::Rejected(reason) => {
            payload.push(REJECTED_OUTCOME);
            encode_launch_rejection(&mut payload, reason);
        }
        TerminalLaunchOutcome::Failed(reason) => {
            payload.push(FAILED_OUTCOME);
            encode_launch_failure(&mut payload, reason);
        }
        TerminalLaunchOutcome::OutcomeUnknown { host_epoch } => {
            payload.push(OUTCOME_UNKNOWN);
            payload.extend_from_slice(&host_epoch.as_bytes());
        }
    }
    payload
}

fn decode_terminal_launch_result(payload: &[u8]) -> Result<TerminalLaunchResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(payload, TERMINAL_LAUNCH_RESULT_KIND)?;
    let authorization = decode_authorization(&mut reader)?;
    let outcome = match reader.read_u8()? {
        HOST_ACCEPTED_OUTCOME => {
            let identity = decode_host_identity(&mut reader)?;
            let state = match reader.read_u8()? {
                AWAITING_START_STATE => HostLaunchState::AwaitingStart,
                STARTED_STATE => HostLaunchState::Started(decode_geometry(&mut reader)?),
                _ => return Err(ProtocolError::InvalidPayload),
            };
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(identity, state))
        }
        REJECTED_OUTCOME => TerminalLaunchOutcome::Rejected(decode_launch_rejection(&mut reader)?),
        FAILED_OUTCOME => TerminalLaunchOutcome::Failed(decode_launch_failure(&mut reader)?),
        OUTCOME_UNKNOWN => TerminalLaunchOutcome::OutcomeUnknown {
            host_epoch: HostEpoch::from_bytes(reader.read_array()?)?,
        },
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    TerminalLaunchResult::new(authorization, outcome)
}

fn encode_authorized_terminal_start(value: AuthorizedTerminalStart) -> Vec<u8> {
    let mut payload = Vec::with_capacity(160);
    payload.push(AUTHORIZED_TERMINAL_START_KIND);
    encode_authorization(&mut payload, value.authorization());
    encode_host_identity(&mut payload, value.identity());
    encode_geometry(&mut payload, value.geometry());
    payload
}

fn decode_authorized_terminal_start(
    payload: &[u8],
) -> Result<AuthorizedTerminalStart, ProtocolError> {
    let mut reader = PayloadReader::after_kind(payload, AUTHORIZED_TERMINAL_START_KIND)?;
    let authorization = decode_authorization(&mut reader)?;
    let identity = decode_host_identity(&mut reader)?;
    let geometry = decode_geometry(&mut reader)?;
    reader.finish()?;
    Ok(AuthorizedTerminalStart::new(
        authorization,
        identity,
        geometry,
    ))
}

fn encode_terminal_start_result(value: TerminalStartResult) -> Vec<u8> {
    let mut payload = Vec::with_capacity(160);
    payload.push(TERMINAL_START_RESULT_KIND);
    encode_authorization(&mut payload, value.authorization());
    encode_host_identity(&mut payload, value.target());
    match value.outcome() {
        TerminalStartOutcome::HostAccepted(accepted) => {
            payload.push(HOST_ACCEPTED_OUTCOME);
            encode_host_identity(&mut payload, accepted.identity());
            encode_geometry(&mut payload, accepted.geometry());
            payload.push(match accepted.transition() {
                StartTransition::Started => 1,
                StartTransition::AlreadyStarted => 2,
            });
        }
        TerminalStartOutcome::Rejected(reason) => {
            payload.push(REJECTED_OUTCOME);
            encode_start_rejection(&mut payload, reason);
        }
        TerminalStartOutcome::Failed(reason) => {
            payload.push(FAILED_OUTCOME);
            encode_start_failure(&mut payload, reason);
        }
        TerminalStartOutcome::OutcomeUnknown => payload.push(OUTCOME_UNKNOWN),
    }
    payload
}

fn decode_terminal_start_result(payload: &[u8]) -> Result<TerminalStartResult, ProtocolError> {
    let mut reader = PayloadReader::after_kind(payload, TERMINAL_START_RESULT_KIND)?;
    let authorization = decode_authorization(&mut reader)?;
    let target = decode_host_identity(&mut reader)?;
    let outcome = match reader.read_u8()? {
        HOST_ACCEPTED_OUTCOME => {
            let identity = decode_host_identity(&mut reader)?;
            let geometry = decode_geometry(&mut reader)?;
            let transition = match reader.read_u8()? {
                1 => StartTransition::Started,
                2 => StartTransition::AlreadyStarted,
                _ => return Err(ProtocolError::InvalidPayload),
            };
            TerminalStartOutcome::HostAccepted(HostStartAccepted::new(
                identity, geometry, transition,
            ))
        }
        REJECTED_OUTCOME => TerminalStartOutcome::Rejected(decode_start_rejection(&mut reader)?),
        FAILED_OUTCOME => TerminalStartOutcome::Failed(decode_start_failure(&mut reader)?),
        OUTCOME_UNKNOWN => TerminalStartOutcome::OutcomeUnknown,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    reader.finish()?;
    TerminalStartResult::new(authorization, target, outcome)
}

fn encode_authorization(payload: &mut Vec<u8>, value: TerminalOperationAuthorization) {
    let lease = value.lease();
    payload.extend_from_slice(&value.terminal_id().as_bytes());
    payload.extend_from_slice(&value.operation_id().as_bytes());
    payload.extend_from_slice(&value.worker_id().as_bytes());
    payload.extend_from_slice(&value.worker_generation().get().to_be_bytes());
    payload.extend_from_slice(&value.fence().get().to_be_bytes());
    payload.extend_from_slice(&lease.id().as_bytes());
    payload.extend_from_slice(&lease.expires_at().get().to_be_bytes());
    payload.extend_from_slice(&value.capability_version().get().to_be_bytes());
}

fn decode_authorization(
    reader: &mut PayloadReader<'_>,
) -> Result<TerminalOperationAuthorization, ProtocolError> {
    let terminal_id = TerminalId::from_bytes(reader.read_array()?)?;
    let operation_id = OperationId::from_bytes(reader.read_array()?)?;
    let worker_id = WorkerId::from_bytes(reader.read_array()?)?;
    let worker_generation = WorkerGeneration::new(
        NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
    );
    let fence = NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?;
    let lease_id = LeaseId::from_bytes(reader.read_array()?)?;
    let expires_at = NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?;
    let capability_version =
        NonZeroU16::new(reader.read_u16()?).ok_or(ProtocolError::InvalidPayload)?;
    Ok(TerminalOperationAuthorization::new(
        terminal_id,
        operation_id,
        worker_id,
        worker_generation,
        TerminalFence::new(fence),
        TerminalOperationLease::new(lease_id, LeaseExpiresAtUnixMilliseconds::new(expires_at)),
        TerminalLaunchCapabilityVersion::new(capability_version),
    ))
}

fn encode_cwd(payload: &mut Vec<u8>, cwd: Option<&AbsoluteCwd>) {
    match cwd {
        None => payload.extend_from_slice(&0_u16.to_be_bytes()),
        Some(cwd) => {
            payload.extend_from_slice(&cwd.encoded_length().to_be_bytes());
            payload.extend_from_slice(cwd.as_str().as_bytes());
        }
    }
}

fn decode_cwd(reader: &mut PayloadReader<'_>) -> Result<Option<AbsoluteCwd>, ProtocolError> {
    let length = usize::from(reader.read_u16()?);
    if length == 0 {
        return Ok(None);
    }
    let bytes = reader.read_slice(length)?;
    let value = std::str::from_utf8(bytes).map_err(|_error| ProtocolError::InvalidCwd)?;
    AbsoluteCwd::new(value).map(Some)
}

fn encode_launch_mode(payload: &mut Vec<u8>, mode: TerminalLaunchMode) {
    match mode {
        TerminalLaunchMode::Immediate(geometry) => {
            payload.push(IMMEDIATE_LAUNCH_KIND);
            encode_geometry(payload, geometry);
        }
        TerminalLaunchMode::Deferred => payload.push(DEFERRED_LAUNCH_KIND),
    }
}

fn decode_launch_mode(reader: &mut PayloadReader<'_>) -> Result<TerminalLaunchMode, ProtocolError> {
    match reader.read_u8()? {
        IMMEDIATE_LAUNCH_KIND => decode_geometry(reader).map(TerminalLaunchMode::Immediate),
        DEFERRED_LAUNCH_KIND => Ok(TerminalLaunchMode::Deferred),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

fn encode_launch_selection(payload: &mut Vec<u8>, selection: &crate::LaunchSelection) {
    let executable = selection.executable().as_bytes();
    payload.extend_from_slice(
        &u16::try_from(executable.len())
            .expect("bounded launch selection")
            .to_be_bytes(),
    );
    payload.extend_from_slice(executable);
    payload.extend_from_slice(
        &u16::try_from(selection.arguments().len())
            .expect("bounded launch selection")
            .to_be_bytes(),
    );
    for argument in selection.arguments() {
        let bytes = argument.as_bytes();
        payload.extend_from_slice(
            &u16::try_from(bytes.len())
                .expect("bounded launch selection")
                .to_be_bytes(),
        );
        payload.extend_from_slice(bytes);
    }
}

fn decode_launch_selection(
    reader: &mut PayloadReader<'_>,
) -> Result<crate::LaunchSelection, ProtocolError> {
    let executable_length = usize::from(reader.read_u16()?);
    decode_launch_selection_body(reader, executable_length)
}
fn decode_launch_selection_body(
    reader: &mut PayloadReader<'_>,
    executable_length: usize,
) -> Result<crate::LaunchSelection, ProtocolError> {
    let executable = String::from_utf8(reader.read_slice(executable_length)?.to_vec())
        .map_err(|_error| ProtocolError::InvalidPayload)?;
    let argument_count = usize::from(reader.read_u16()?);
    let mut arguments = Vec::with_capacity(argument_count.min(1024));
    for _ in 0..argument_count {
        let length = usize::from(reader.read_u16()?);
        arguments.push(
            String::from_utf8(reader.read_slice(length)?.to_vec())
                .map_err(|_error| ProtocolError::InvalidPayload)?,
        );
    }
    crate::LaunchSelection::new(executable, arguments)
}

fn encode_launch_environment(payload: &mut Vec<u8>, environment: &crate::LaunchEnvironment) {
    encode_environment_entries(payload, environment.entries());
}
fn encode_environment_entries(payload: &mut Vec<u8>, entries: &[(String, String)]) {
    payload.extend_from_slice(
        &u16::try_from(entries.len())
            .expect("bounded environment")
            .to_be_bytes(),
    );
    for (name, value) in entries {
        for field in [name, value] {
            payload.extend_from_slice(
                &u16::try_from(field.len())
                    .expect("bounded environment")
                    .to_be_bytes(),
            );
            payload.extend_from_slice(field.as_bytes());
        }
    }
}
fn decode_launch_environment(
    reader: &mut PayloadReader<'_>,
) -> Result<crate::LaunchEnvironment, ProtocolError> {
    crate::LaunchEnvironment::new(decode_environment_entries(
        reader,
        crate::MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES,
        crate::MAXIMUM_LAUNCH_ENVIRONMENT_BYTES,
    )?)
}
fn decode_environment_entries(
    reader: &mut PayloadReader<'_>,
    maximum_entries: usize,
    maximum_bytes: usize,
) -> Result<Vec<(String, String)>, ProtocolError> {
    let count = usize::from(reader.read_u16()?);
    if count > maximum_entries {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut entries = Vec::with_capacity(count);
    let mut bytes = 0usize;
    for _ in 0..count {
        let mut pair = Vec::with_capacity(2);
        for _ in 0..2 {
            let length = usize::from(reader.read_u16()?);
            bytes = bytes
                .checked_add(length)
                .ok_or(ProtocolError::InvalidPayload)?;
            if bytes > maximum_bytes {
                return Err(ProtocolError::InvalidPayload);
            }
            pair.push(
                String::from_utf8(reader.read_slice(length)?.to_vec())
                    .map_err(|_| ProtocolError::InvalidPayload)?,
            );
        }
        let value = pair.pop().expect("two fields");
        let name = pair.pop().expect("two fields");
        entries.push((name, value));
    }
    Ok(entries)
}

fn encode_geometry(payload: &mut Vec<u8>, geometry: TerminalPixelGeometry) {
    payload.extend_from_slice(&geometry.columns().to_be_bytes());
    payload.extend_from_slice(&geometry.rows().to_be_bytes());
    payload.extend_from_slice(&geometry.cell_pixel_width().to_be_bytes());
    payload.extend_from_slice(&geometry.cell_pixel_height().to_be_bytes());
}

fn decode_geometry(reader: &mut PayloadReader<'_>) -> Result<TerminalPixelGeometry, ProtocolError> {
    TerminalPixelGeometry::new(
        reader.read_u16()?,
        reader.read_u16()?,
        reader.read_u16()?,
        reader.read_u16()?,
    )
}

fn encode_host_identity(payload: &mut Vec<u8>, identity: HostTerminalIdentity) {
    payload.extend_from_slice(&identity.creation_id().as_bytes());
    payload.extend_from_slice(&identity.host_epoch().as_bytes());
    payload.extend_from_slice(&identity.terminal_id().as_bytes());
}

fn decode_host_identity(
    reader: &mut PayloadReader<'_>,
) -> Result<HostTerminalIdentity, ProtocolError> {
    Ok(HostTerminalIdentity::new(
        HostCreationId::from_bytes(reader.read_array()?)?,
        HostEpoch::from_bytes(reader.read_array()?)?,
        HostTerminalId::from_bytes(reader.read_array()?)?,
    ))
}

fn encode_launch_rejection(payload: &mut Vec<u8>, reason: TerminalLaunchRejection) {
    match reason {
        TerminalLaunchRejection::LifecycleActive => payload.push(9),
        TerminalLaunchRejection::WrongWorker => payload.push(1),
        TerminalLaunchRejection::StaleWorkerGeneration => payload.push(2),
        TerminalLaunchRejection::LeaseExpired => payload.push(3),
        TerminalLaunchRejection::StaleFence => payload.push(4),
        TerminalLaunchRejection::ConflictingReplay => payload.push(5),
        TerminalLaunchRejection::UnsupportedCapability => payload.push(6),
        TerminalLaunchRejection::HostRejected { host_epoch } => {
            payload.push(7);
            payload.extend_from_slice(&host_epoch.as_bytes());
        }
        TerminalLaunchRejection::CapacityExceeded => payload.push(8),
    }
}

fn decode_launch_rejection(
    reader: &mut PayloadReader<'_>,
) -> Result<TerminalLaunchRejection, ProtocolError> {
    match reader.read_u8()? {
        1 => Ok(TerminalLaunchRejection::WrongWorker),
        2 => Ok(TerminalLaunchRejection::StaleWorkerGeneration),
        3 => Ok(TerminalLaunchRejection::LeaseExpired),
        4 => Ok(TerminalLaunchRejection::StaleFence),
        5 => Ok(TerminalLaunchRejection::ConflictingReplay),
        6 => Ok(TerminalLaunchRejection::UnsupportedCapability),
        7 => Ok(TerminalLaunchRejection::HostRejected {
            host_epoch: HostEpoch::from_bytes(reader.read_array()?)?,
        }),
        8 => Ok(TerminalLaunchRejection::CapacityExceeded),
        9 => Ok(TerminalLaunchRejection::LifecycleActive),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

fn encode_start_rejection(payload: &mut Vec<u8>, reason: TerminalStartRejection) {
    match reason {
        TerminalStartRejection::LifecycleActive => payload.push(10),
        TerminalStartRejection::WrongWorker => payload.push(1),
        TerminalStartRejection::StaleWorkerGeneration => payload.push(2),
        TerminalStartRejection::LeaseExpired => payload.push(3),
        TerminalStartRejection::StaleFence => payload.push(4),
        TerminalStartRejection::ConflictingReplay => payload.push(5),
        TerminalStartRejection::UnsupportedCapability => payload.push(6),
        TerminalStartRejection::HostRejected { host_epoch } => {
            payload.push(7);
            payload.extend_from_slice(&host_epoch.as_bytes());
        }
        TerminalStartRejection::HostIdentityChanged {
            observed_host_epoch,
        } => {
            payload.push(8);
            payload.extend_from_slice(&observed_host_epoch.as_bytes());
        }
        TerminalStartRejection::CapacityExceeded => payload.push(9),
    }
}

fn decode_start_rejection(
    reader: &mut PayloadReader<'_>,
) -> Result<TerminalStartRejection, ProtocolError> {
    match reader.read_u8()? {
        1 => Ok(TerminalStartRejection::WrongWorker),
        2 => Ok(TerminalStartRejection::StaleWorkerGeneration),
        3 => Ok(TerminalStartRejection::LeaseExpired),
        4 => Ok(TerminalStartRejection::StaleFence),
        5 => Ok(TerminalStartRejection::ConflictingReplay),
        6 => Ok(TerminalStartRejection::UnsupportedCapability),
        7 => Ok(TerminalStartRejection::HostRejected {
            host_epoch: HostEpoch::from_bytes(reader.read_array()?)?,
        }),
        8 => Ok(TerminalStartRejection::HostIdentityChanged {
            observed_host_epoch: HostEpoch::from_bytes(reader.read_array()?)?,
        }),
        9 => Ok(TerminalStartRejection::CapacityExceeded),
        10 => Ok(TerminalStartRejection::LifecycleActive),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

fn encode_launch_failure(payload: &mut Vec<u8>, reason: TerminalLaunchFailure) {
    match reason {
        TerminalLaunchFailure::HostUnavailableBeforeHandoff => payload.push(1),
        TerminalLaunchFailure::HostReportedFailure { host_epoch } => {
            payload.push(2);
            payload.extend_from_slice(&host_epoch.as_bytes());
        }
    }
}

fn decode_launch_failure(
    reader: &mut PayloadReader<'_>,
) -> Result<TerminalLaunchFailure, ProtocolError> {
    match reader.read_u8()? {
        1 => Ok(TerminalLaunchFailure::HostUnavailableBeforeHandoff),
        2 => Ok(TerminalLaunchFailure::HostReportedFailure {
            host_epoch: HostEpoch::from_bytes(reader.read_array()?)?,
        }),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

fn encode_start_failure(payload: &mut Vec<u8>, reason: TerminalStartFailure) {
    match reason {
        TerminalStartFailure::HostUnavailableBeforeHandoff => payload.push(1),
        TerminalStartFailure::HostReportedFailure { host_epoch } => {
            payload.push(2);
            payload.extend_from_slice(&host_epoch.as_bytes());
        }
    }
}

fn decode_start_failure(
    reader: &mut PayloadReader<'_>,
) -> Result<TerminalStartFailure, ProtocolError> {
    match reader.read_u8()? {
        1 => Ok(TerminalStartFailure::HostUnavailableBeforeHandoff),
        2 => Ok(TerminalStartFailure::HostReportedFailure {
            host_epoch: HostEpoch::from_bytes(reader.read_array()?)?,
        }),
        _ => Err(ProtocolError::InvalidPayload),
    }
}

struct PayloadReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> PayloadReader<'a> {
    /// A reader over a whole record body that carries no leading kind byte —
    /// the dedicated record frames (send request) dispatch on magic instead.
    fn whole(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn after_kind(bytes: &'a [u8], expected_kind: u8) -> Result<Self, ProtocolError> {
        if bytes.first().copied() != Some(expected_kind) {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self { bytes, offset: 1 })
    }

    fn read_array<const LENGTH: usize>(&mut self) -> Result<[u8; LENGTH], ProtocolError> {
        self.read_slice(LENGTH)?
            .try_into()
            .map_err(|_error| ProtocolError::InvalidPayload)
    }

    fn read_slice(&mut self, length: usize) -> Result<&'a [u8], ProtocolError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ProtocolError::InvalidPayload)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProtocolError::InvalidPayload)?;
        self.offset = end;
        Ok(value)
    }

    fn read_u8(&mut self) -> Result<u8, ProtocolError> {
        let [value] = self.read_array()?;
        Ok(value)
    }

    fn read_u16(&mut self) -> Result<u16, ProtocolError> {
        self.read_array().map(u16::from_be_bytes)
    }

    fn read_u32(&mut self) -> Result<u32, ProtocolError> {
        self.read_array().map(u32::from_be_bytes)
    }

    fn read_i32(&mut self) -> Result<i32, ProtocolError> {
        self.read_array().map(i32::from_be_bytes)
    }

    fn read_u64(&mut self) -> Result<u64, ProtocolError> {
        self.read_array().map(u64::from_be_bytes)
    }

    fn finish(self) -> Result<(), ProtocolError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ProtocolError::InvalidPayload)
        }
    }
}
