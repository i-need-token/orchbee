//! macOS and Linux have no hub yet, so no carrier endpoint exists and none can be asked for.
use super::{Gate, StartedCarrier};
use crate::machines::ProfileId;
use std::io;

/// No hub owner exists on this platform.
#[derive(Clone)]
pub(crate) enum Hub {}

pub(super) fn start(hub: &Hub, _machine: &ProfileId, _online: Gate) -> io::Result<StartedCarrier> {
    match *hub {}
}
