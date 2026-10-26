//! Unsupported activation observation never probes the Session runtime.
use orchbee_local_runtime::{ActivationState, ModeCheckedLocalSessionRendezvous};
use std::io;

pub(super) fn activation_state(
    _session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<ActivationState> {
    Err(io::ErrorKind::Unsupported.into())
}
