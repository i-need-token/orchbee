//! Unix saved-machine tunnels have no process or endpoint implementation.
use crate::{
    local_client::LocalClient,
    machines::Profile,
    tui_capabilities::{Capability, Unsupported},
};
use std::{io, sync::atomic::AtomicU8, time::Instant};

pub(crate) fn connect(
    _profile: &Profile,
    _deadline: Instant,
    _cancelled: &AtomicU8,
) -> io::Result<LocalClient> {
    Err(Unsupported(Capability::SavedMachines).into())
}
