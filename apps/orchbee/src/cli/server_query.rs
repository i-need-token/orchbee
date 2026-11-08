//! The Session's `query` control request for CLI observations; it never enters the Client phase.

use orchbee_connection_handshake::QueryResult;
use orchbee_local_client_protocol::ConnectClientError;
use orchbee_local_ipc::LocalIo;
use std::{path::Path, time::Instant};

/// Asks the Server at `endpoint` for its processes, as this build's environment.
pub(super) fn read(
    endpoint: &Path,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<QueryResult, ConnectClientError> {
    orchbee_local_client_protocol::exchange_query_before(
        endpoint,
        orchbee_local_runtime::environment_name(),
        deadline,
        wake,
    )
}
