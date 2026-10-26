//! Unix has no pre-admission metadata exchange; this port makes no connection attempt.
use orchbee_local_client_protocol::{ConnectClientError, ServerMetadata};
use orchbee_local_ipc::LocalIo;
use std::{io, path::Path, time::Instant};

pub(in crate::cli) fn read(
    _endpoint: &Path,
    _deadline: Instant,
    _wake: Option<LocalIo<'_>>,
) -> Result<ServerMetadata, ConnectClientError> {
    Err(ConnectClientError::Io(io::ErrorKind::Unsupported.into()))
}
