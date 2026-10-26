//! Terminal notification transport is unavailable before environment or output access.
use crate::{
    notifications::terminal::Host,
    terminal_device::InheritedTerminal,
    tui_capabilities::{Capability, Unsupported},
};
use std::io;

pub(super) fn host() -> io::Result<Host> {
    Err(Unsupported(Capability::SystemNotification).into())
}

pub(super) fn write(_terminal: &InheritedTerminal, _bytes: &[u8]) -> io::Result<()> {
    Err(Unsupported(Capability::SystemNotification).into())
}
