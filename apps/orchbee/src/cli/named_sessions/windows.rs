//! Windows named-Session recovery observes the original private activation marker.
use orchbee_local_runtime::{ActivationState, ModeCheckedLocalSessionRendezvous};
use std::io;

pub(super) fn activation_state(
    session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<ActivationState> {
    session.activation_state()
}
