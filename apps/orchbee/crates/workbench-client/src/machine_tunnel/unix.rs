//! Unix saved-machine tunnels have no process or endpoint implementation.
use super::StartedCarrier;
use crate::{
    host_capabilities::{Capability, Unsupported},
    machines::Profile,
};
use std::io;

pub(super) fn prepare(
    _profile: &Profile,
    _admitted: &crate::machine_ssh::Client,
) -> io::Result<StartedCarrier> {
    Err(Unsupported(Capability::SavedMachines).into())
}
