//! Native stop exchange and strict activation records; completion policy stays in the workflow.
use super::ExchangeFailure;
use orchbee_local_client_protocol::{SessionStopRequest, SessionStopResult};
use orchbee_local_ipc::LocalIo;
use orchbee_local_runtime::{ActivationState, ModeCheckedLocalSessionRendezvous};
use std::{io, path::Path, time::Instant};

pub(super) fn exchange_before(
    endpoint: &Path,
    request: SessionStopRequest,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<SessionStopResult, ExchangeFailure> {
    orchbee_local_client_protocol::exchange_session_stop_before(endpoint, request, deadline, wake)
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
