//! Prepares and executes one authorized Host launch or start, preserving pre-handoff failure
//! versus uncertain delivery and recording only validated responses in the kernel. It does not
//! retry an unknown effect or own the Host process.

use orchbee_local_ipc::LocalStream as UnixStream;
use std::{fmt, time::Duration};

use orchbee_capability_node_protocol::HostEpoch as CapabilityHostEpoch;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, CreationId, Envelope, HostEpoch,
    LifecycleControl, MessageRole, PayloadEncoding, ProtocolError, TerminalCreationControl,
    decode_connection_control, decode_lifecycle_control, decode_terminal_creation_control,
    decode_terminal_output_staged, decode_transport_envelope, encode_lifecycle_control,
    encode_stage_terminal_output, encode_terminal_creation_control, encode_transport_envelope,
};

use crate::{
    host_connection::HostBrokerConnection,
    terminal_authorization::{
        AuthorizationKernelError, AuthorizedHostDispatch, AuthorizedHostOperationKernel,
        HostHandoffAdmission,
    },
    terminal_host_launch_adapter::{
        HostLaunchAdapterError, ObservedTerminalHostResponse, ResolvedTerminalLaunchFacts,
        TerminalHostResponse, map_launch_creation, map_launch_host_response, map_start_dispatch,
        map_start_host_response,
    },
};

use crate::diagnostic_events::{self, Anomaly};
use crate::host_connection::{HostExchange, PeerFailure, record_kind, response_envelope};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostOperationDisposition {
    ResultReady,
    HostClosed,
    ServerClosed,
}

#[derive(Debug)]
pub(crate) enum HostOperationError {
    Preparation(HostLaunchAdapterError),
    Protocol(ProtocolError),
    Kernel(AuthorizationKernelError),
    InvalidBudget,
}

impl fmt::Display for HostOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preparation(error) => error.fmt(formatter),
            Self::Protocol(error) => error.fmt(formatter),
            Self::Kernel(error) => error.fmt(formatter),
            Self::InvalidBudget => {
                formatter.write_str("Host operation requires a positive representable I/O budget")
            }
        }
    }
}

impl std::error::Error for HostOperationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Preparation(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::Kernel(error) => Some(error),
            Self::InvalidBudget => None,
        }
    }
}

impl From<HostLaunchAdapterError> for HostOperationError {
    fn from(error: HostLaunchAdapterError) -> Self {
        Self::Preparation(error)
    }
}

impl From<ProtocolError> for HostOperationError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

impl From<AuthorizationKernelError> for HostOperationError {
    fn from(error: AuthorizationKernelError) -> Self {
        Self::Kernel(error)
    }
}

#[derive(Clone, Copy)]
enum ExpectedResponse {
    Created(CreationId),
    Started,
}

/// Why earlier output was not staged before a creation.
enum StageFailure {
    /// The Host answered and refused it, or the stage could not be written; the creation proceeds
    /// without it.
    Refused,
    /// The Host's answer, of this kind, arrived whole but is neither a receipt nor a refusal. The
    /// stream is still in step, so the creation proceeds, with or without the seed.
    Unexpected(u16),
    /// The Host connection failed before the creation was written.
    Peer(PeerFailure),
}

pub(crate) struct PreparedHostOperation {
    dispatch: AuthorizedHostDispatch,
    correlation: CorrelationId,
    kind: u16,
    payload: Vec<u8>,
    expected: ExpectedResponse,
    /// Every launch follows its controlling Client's query palette once created.
    host_theme: Option<orchbee_terminal_protocol::TerminalTheme>,
    /// Earlier output the Host stages for this creation before it is written.
    output_seed: Option<Vec<u8>>,
}

impl PreparedHostOperation {
    pub(crate) fn launch(
        dispatch: &AuthorizedHostDispatch,
        facts: ResolvedTerminalLaunchFacts,
        correlation: CorrelationId,
        session_runtime: Option<&std::path::Path>,
    ) -> Result<Self, HostOperationError> {
        let host_theme = Some(facts.terminal_theme());
        let creation = map_launch_creation(dispatch, facts, session_runtime)?;
        let expected = ExpectedResponse::Created(match &creation {
            TerminalCreationControl::CreateTerminal(request) => request.creation_id,
            TerminalCreationControl::TerminalCreated(_) => {
                return Err(HostLaunchAdapterError::UnexpectedHostResponse.into());
            }
        });
        let control = encode_terminal_creation_control(&creation)?;
        Ok(Self {
            dispatch: dispatch.clone(),
            correlation,
            kind: control.kind,
            payload: control.payload,
            expected,
            host_theme,
            output_seed: None,
        })
    }

    /// Attaches the earlier output admission removed from the launch, so the authorization
    /// kernel never retains it. A start carries none.
    pub(crate) fn with_output_seed(mut self, seed: Option<Vec<u8>>) -> Self {
        if matches!(self.expected, ExpectedResponse::Created(_)) {
            self.output_seed = seed;
        }
        self
    }

    pub(crate) fn start(
        dispatch: &AuthorizedHostDispatch,
        correlation: CorrelationId,
    ) -> Result<Self, HostOperationError> {
        let control = encode_lifecycle_control(&LifecycleControl::StartTerminal(
            map_start_dispatch(dispatch)?,
        ))?;
        Ok(Self {
            dispatch: dispatch.clone(),
            correlation,
            kind: control.kind,
            payload: control.payload,
            expected: ExpectedResponse::Started,
            host_theme: None,
            output_seed: None,
        })
    }

    pub(crate) fn execute(
        self,
        kernel: &mut AuthorizedHostOperationKernel,
        host: &mut HostBrokerConnection,
        server: &UnixStream,
        now_unix_milliseconds: impl FnOnce() -> u64,
        budget: Duration,
    ) -> Result<HostOperationDisposition, HostOperationError> {
        let epoch = host.host_epoch();
        let record = encode_transport_envelope(&Envelope {
            correlation_id: Some(self.correlation),
            host_epoch: Some(epoch),
            kind: self.kind,
            optional_message: false,
            payload: &self.payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Request,
        })?;
        if budget.is_zero() {
            return Err(HostOperationError::InvalidBudget);
        }
        // Staging earlier output and creating the terminal are two exchanges, each with the whole
        // `budget`, and each starts only if that bound ends within the request's bound. A stage
        // with no room is skipped, since earlier output is optional; a creation with no room is
        // never sent, also after a successful stage, and the Launch fails before handoff, with no
        // effect, for the Client to ask again. The seed such a stage left is inert: the Host keeps
        // one staged seed, gives it only to a creation of the same identity on the same
        // connection, and discards it at the next creation on this connection or the next stage.
        if let (Some(seed), ExpectedResponse::Created(creation)) =
            (self.output_seed.as_deref(), self.expected)
            && let Ok(mut stage) = HostExchange::new(host, server, budget)
        {
            let staged = stage
                .prepare()
                .map_err(StageFailure::Peer)
                .and_then(|()| stage_output(&mut stage, epoch, creation, seed));
            match staged {
                Ok(()) => {}
                // Earlier output is optional: the terminal still starts, only without it.
                Err(StageFailure::Refused) => eprintln!(
                    "orchbee-worker diagnostic component=terminal-launch operation=stage-output stage=host cause=refused"
                ),
                Err(StageFailure::Unexpected(kind)) => {
                    diagnostic_events::host_unexpected(host.pacing.site(), kind, Anomaly::Answer);
                }
                Err(StageFailure::Peer(failure)) => {
                    stage.close(failure);
                    if failure == PeerFailure::Server {
                        return Ok(HostOperationDisposition::ServerClosed);
                    }
                    kernel.record_pre_handoff_failure(&self.dispatch, now_unix_milliseconds())?;
                    return Ok(HostOperationDisposition::HostClosed);
                }
            }
        }
        let Ok(mut exchange) = HostExchange::new(host, server, budget) else {
            kernel.record_pre_handoff_failure(&self.dispatch, now_unix_milliseconds())?;
            return Ok(HostOperationDisposition::ResultReady);
        };
        if let Err(failure) = exchange.prepare() {
            exchange.close(failure);
            if failure == PeerFailure::Server {
                return Ok(HostOperationDisposition::ServerClosed);
            }
            kernel.record_pre_handoff_failure(&self.dispatch, now_unix_milliseconds())?;
            return Ok(HostOperationDisposition::HostClosed);
        }
        let capability_epoch = CapabilityHostEpoch::from_bytes(*epoch.as_bytes())
            .map_err(AuthorizationKernelError::from)?;
        match kernel.begin_host_handoff(
            &self.dispatch,
            now_unix_milliseconds(),
            capability_epoch,
        )? {
            HostHandoffAdmission::ResultReady => return Ok(HostOperationDisposition::ResultReady),
            HostHandoffAdmission::HandedOff => {}
        }
        let failure = match exchange.round_trip(&record) {
            Ok(response) => match self.record_response(kernel, epoch, &response) {
                Ok(created) => {
                    if let (Some(terminal), Some(theme)) = (created, self.host_theme) {
                        host.host_colors
                            .borrow_mut()
                            .follow(*terminal.as_bytes(), theme);
                    }
                    return Ok(HostOperationDisposition::ResultReady);
                }
                // An answer that arrived whole but does not settle the operation leaves the stream
                // in step: only this operation's effect is unknown, and the connection serves on.
                // A kernel slot that no longer holds this dispatch fails here as well.
                Err(_invalid_response) => {
                    diagnostic_events::host_unexpected(
                        host.pacing.site(),
                        record_kind(&response),
                        Anomaly::Answer,
                    );
                    kernel.record_outcome_unknown(&self.dispatch)?;
                    return Ok(HostOperationDisposition::ResultReady);
                }
            },
            Err(failure) => failure,
        };
        // The first possible write follows the kernel handoff. Even a zero-byte
        // write failure from this point is conservatively unknown, never retried.
        exchange.close(failure);
        kernel.record_outcome_unknown(&self.dispatch)?;
        Ok(match failure {
            PeerFailure::Host | PeerFailure::Protocol => HostOperationDisposition::HostClosed,
            PeerFailure::Server => HostOperationDisposition::ServerClosed,
        })
    }

    fn record_response(
        &self,
        kernel: &mut AuthorizedHostOperationKernel,
        epoch: HostEpoch,
        record: &[u8],
    ) -> Result<Option<orchbee_terminal_protocol::TerminalId>, HostOperationError> {
        let envelope = decode_transport_envelope(record)?;
        if envelope.role != MessageRole::Response
            || envelope.correlation_id != Some(self.correlation)
            || envelope.host_epoch != Some(epoch)
            || envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.optional_message
            || envelope.payload_encoding != PayloadEncoding::ControlJson
        {
            return Err(HostLaunchAdapterError::UnexpectedHostResponse.into());
        }
        let mut created = None;
        let response = if let Ok(ConnectionControl::TypedFailure(failure)) =
            decode_connection_control(envelope.kind, envelope.role, envelope.payload)
        {
            TerminalHostResponse::TypedFailure(failure)
        } else {
            match self.expected {
                ExpectedResponse::Created(_) => {
                    let TerminalCreationControl::TerminalCreated(identity) =
                        decode_terminal_creation_control(
                            envelope.kind,
                            envelope.role,
                            envelope.payload,
                        )?
                    else {
                        return Err(HostLaunchAdapterError::UnexpectedHostResponse.into());
                    };
                    created = Some(identity.terminal_id);
                    TerminalHostResponse::TerminalCreated(identity)
                }
                ExpectedResponse::Started => {
                    let LifecycleControl::TerminalStartAccepted(accepted) =
                        decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload)?
                    else {
                        return Err(HostLaunchAdapterError::UnexpectedHostResponse.into());
                    };
                    TerminalHostResponse::TerminalStartAccepted(accepted)
                }
            }
        };
        let observed = ObservedTerminalHostResponse::new(epoch, response);
        match self.expected {
            ExpectedResponse::Created(creation) => kernel.record_launch_completion(
                &self.dispatch,
                map_launch_host_response(&self.dispatch, creation, observed)?,
            )?,
            ExpectedResponse::Started => kernel.record_start_completion(
                &self.dispatch,
                map_start_host_response(&self.dispatch, observed)?,
            )?,
        }
        Ok(created)
    }
}

/// Stages `seed` for `creation` in one exchange before the creation itself is written, so no
/// terminal effect can precede it.
fn stage_output(
    exchange: &mut HostExchange<'_>,
    epoch: HostEpoch,
    creation: CreationId,
    seed: &[u8],
) -> Result<(), StageFailure> {
    let payload =
        encode_stage_terminal_output(creation, seed).map_err(|_| StageFailure::Refused)?;
    let correlation = crate::worker_identity::fresh_identity()
        .ok()
        .and_then(|bytes| CorrelationId::from_bytes(bytes).ok())
        .ok_or(StageFailure::Refused)?;
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(correlation),
        host_epoch: Some(epoch),
        kind: STAGE_TERMINAL_OUTPUT_KIND,
        optional_message: false,
        payload: &payload,
        payload_encoding: PayloadEncoding::RawBytes,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Request,
    })
    .map_err(|_| StageFailure::Refused)?;
    let response = exchange
        .round_trip_bounded(&record, STAGE_RESPONSE_BYTES)
        .map_err(StageFailure::Peer)?;
    let envelope = response_envelope(&response, correlation, epoch)
        .map_err(|answer| StageFailure::Unexpected(answer.kind))?;
    if envelope.kind == TERMINAL_OUTPUT_STAGED_KIND
        && envelope.payload_encoding == PayloadEncoding::RawBytes
        && decode_terminal_output_staged(envelope.payload).is_ok_and(|staged| {
            staged.creation_id == creation && staged.bytes as usize == seed.len()
        })
    {
        return Ok(());
    }
    if matches!(
        decode_connection_control(envelope.kind, envelope.role, envelope.payload),
        Ok(ConnectionControl::TypedFailure(_))
    ) {
        return Err(StageFailure::Refused);
    }
    Err(StageFailure::Unexpected(envelope.kind))
}

const STAGE_TERMINAL_OUTPUT_KIND: u16 = 82;
const TERMINAL_OUTPUT_STAGED_KIND: u16 = 83;
/// A receipt or a typed refusal; both are far below this bound.
const STAGE_RESPONSE_BYTES: usize = 64 * 1024;

#[cfg(test)]
mod tests;
