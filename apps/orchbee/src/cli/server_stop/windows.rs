//! Strict activation-record observation; completion policy remains in the shared workflow.
use orchbee_local_runtime::{ActivationState, ModeCheckedLocalSessionRendezvous};
use std::io;

pub(super) fn activation_state(
    session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<ActivationState> {
    session.activation_state()
}
