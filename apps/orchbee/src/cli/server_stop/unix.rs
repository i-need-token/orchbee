//! Unsupported stop exchange and activation observation never probe the Session runtime.
use super::ExchangeFailure;
use orchbee_connection_handshake::Offer;
use orchbee_local_client_protocol::{ConnectClientError, SessionStopRequest, SessionStopResult};
use orchbee_local_ipc::LocalIo;
use orchbee_local_runtime::{ActivationState, ModeCheckedLocalSessionRendezvous};
use std::{io, path::Path, time::Instant};

pub(super) fn exchange_before(
    _endpoint: &Path,
    _offer: &Offer,
    _request: SessionStopRequest,
    _deadline: Instant,
    _wake: Option<LocalIo<'_>>,
) -> Result<SessionStopResult, ExchangeFailure> {
    Err(ExchangeFailure {
        failure: ConnectClientError::Io(io::ErrorKind::Unsupported.into()),
        submitted: false,
    })
}

pub(super) fn server_terminals_before(
    _endpoint: &Path,
    _deadline: Instant,
    _wake: Option<LocalIo<'_>>,
) -> Result<Option<u32>, ConnectClientError> {
    Err(ConnectClientError::Io(io::ErrorKind::Unsupported.into()))
}

pub(super) fn exit_before(
    _endpoint: &Path,
    _deadline: Instant,
    _wake: Option<LocalIo<'_>>,
) -> Result<orchbee_local_client_protocol::ExitReply, ExchangeFailure> {
    Err(ExchangeFailure {
        failure: ConnectClientError::Io(io::ErrorKind::Unsupported.into()),
        submitted: false,
    })
}

pub(super) fn activation_state(
    _session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<ActivationState> {
    Err(io::ErrorKind::Unsupported.into())
}
