//! Windows metadata observation delegates to the existing same-user exchange.
use orchbee_local_client_protocol::{ConnectClientError, ServerMetadata};
use orchbee_local_ipc::LocalIo;
use std::{path::Path, time::Instant};

pub(in crate::cli) fn read(
    endpoint: &Path,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<ServerMetadata, ConnectClientError> {
    orchbee_local_client_protocol::exchange_server_metadata_before(endpoint, deadline, wake)
}
