//! Unix saved-machine tunnels have no process or endpoint implementation.
use crate::local_client::cancellation::Cancellation;
use crate::{
    local_client::LocalClient,
    machines::Profile,
    tui_capabilities::{Capability, Unsupported},
};
use std::{io, time::Instant};

pub(super) fn connect(
    _profile: &Profile,
    _admitted: crate::machine_ssh::Client,
    _deadline: Instant,
    _cancelled: Cancellation<'_>,
) -> io::Result<LocalClient> {
    Err(Unsupported(Capability::SavedMachines).into())
}
