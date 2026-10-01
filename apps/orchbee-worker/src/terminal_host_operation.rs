use orchbee_local_ipc::LocalStream as UnixStream;
use std::{fmt, time::Duration};

use orchbee_capability_node_protocol::HostEpoch as CapabilityHostEpoch;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, CreationId, Envelope, HostEpoch,
    LifecycleControl, MessageRole, PayloadEncoding, ProtocolError, TerminalCreationControl,
    decode_connection_control, decode_lifecycle_control, decode_terminal_creation_control,
    decode_transport_envelope, encode_lifecycle_control, encode_terminal_creation_control,
    encode_transport_envelope,
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

use crate::host_connection::{HostExchange, PeerFailure};

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

enum ExpectedResponse {
    Created(CreationId),
    Started,
}

pub(crate) struct PreparedHostOperation {
    dispatch: AuthorizedHostDispatch,
    correlation: CorrelationId,
    kind: u16,
    payload: Vec<u8>,
    expected: ExpectedResponse,
    editor_source: Option<orchbee_capability_node_protocol::TerminalEditorSource>,
}

impl PreparedHostOperation {
    pub(crate) fn launch(
        dispatch: &AuthorizedHostDispatch,
        facts: ResolvedTerminalLaunchFacts,
        correlation: CorrelationId,
        site_runtime: Option<&std::path::Path>,
    ) -> Result<Self, HostOperationError> {
        let creation = map_launch_creation(dispatch, facts, site_runtime)?;
        let expected = ExpectedResponse::Created(match &creation {
            TerminalCreationControl::CreateTerminal(request) => request.creation_id,
            TerminalCreationControl::CreateScrollbackEditor(editor) => editor.creation.creation_id,
            TerminalCreationControl::TerminalCreated(_) => {
                return Err(HostLaunchAdapterError::UnexpectedHostResponse.into());
            }
        });
        let editor_source = match dispatch.operation() {
            crate::terminal_authorization::AuthorizedHostOperation::Launch(launch) => {
                launch.editor_source()
            }
            crate::terminal_authorization::AuthorizedHostOperation::Start(_) => None,
        };
        let control = encode_terminal_creation_control(&creation)?;
        Ok(Self {
            dispatch: dispatch.clone(),
            correlation,
            kind: control.kind,
            payload: control.payload,
            expected,
            editor_source,
        })
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
            editor_source: None,
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
        if let Some(source) = self.editor_source
            && (source.host_epoch.as_bytes() != *epoch.as_bytes()
                || host.viewers.borrow().target(source.attachment_id) != Some(source.terminal_id)
                || host.disposed_terminal(source.terminal_id))
        {
            kernel.record_pre_handoff_failure(&self.dispatch, now_unix_milliseconds())?;
            return Ok(HostOperationDisposition::ResultReady);
        }
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
        let mut exchange =
            HostExchange::new(host, server, budget).ok_or(HostOperationError::InvalidBudget)?;
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
        let response = exchange.round_trip(&record);
        let failure = match response {
            Ok(response) => match self.record_response(kernel, epoch, &response) {
                Ok(()) => return Ok(HostOperationDisposition::ResultReady),
                Err(_invalid_response) => PeerFailure::Host,
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
    ) -> Result<(), HostOperationError> {
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
        Ok(())
    }
}

#[cfg(test)]
mod tests;
