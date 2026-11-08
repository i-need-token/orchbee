//! Native stop exchange and strict activation records; completion policy stays in the workflow.
use super::ExchangeFailure;
use orchbee_connection_handshake::{Offer, ProcessRole};
use orchbee_local_client_protocol::{
    ConnectClientError, ExitReply, SessionStopRequest, SessionStopResult,
};
use orchbee_local_ipc::LocalIo;
use orchbee_local_runtime::{ActivationState, ModeCheckedLocalSessionRendezvous};
use std::{io, path::Path, time::Instant};

pub(super) fn exchange_before(
    endpoint: &Path,
    offer: &Offer,
    request: SessionStopRequest,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<SessionStopResult, ExchangeFailure> {
    orchbee_local_client_protocol::exchange_session_stop_before(
        endpoint, offer, request, deadline, wake,
    )
    .map_err(|error| ExchangeFailure {
        failure: error.failure,
        submitted: error.submitted,
    })
}

/// The terminal count the Session's Server reports through one `query` control request, which
/// the handshake answers whatever the Server's build.
pub(super) fn server_terminals_before(
    endpoint: &Path,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<Option<u32>, ConnectClientError> {
    let result = orchbee_local_client_protocol::exchange_query_before(
        endpoint,
        orchbee_local_runtime::environment_name(),
        deadline,
        wake,
    )?;
    Ok(result
        .processes()
        .iter()
        .find(|process| process.role == ProcessRole::Server)
        .map(|process| process.terminals))
}

pub(super) fn exit_before(
    endpoint: &Path,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<ExitReply, ExchangeFailure> {
    orchbee_local_client_protocol::exchange_exit_before(
        endpoint,
        orchbee_local_runtime::environment_name(),
        deadline,
        wake,
    )
    .map_err(|error| ExchangeFailure {
        failure: error.failure,
        submitted: error.submitted,
    })
}

pub(super) fn activation_state(
    session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<ActivationState> {
    session.activation_state()
}
