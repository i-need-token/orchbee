//! Shared notification framing; host admission and physical output belong to the OS port.
use crate::{notifications::terminal, terminal_device::InheritedTerminal};
use orchbee_workbench_protocol::Notification;
use std::io;

#[cfg(windows)]
#[path = "notification/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "notification/unix.rs"]
mod platform;

pub(super) fn send(terminal: &InheritedTerminal, message: &Notification) -> io::Result<()> {
    let host = platform::host()?;
    let bytes = terminal::sequence(message, &host)?;
    platform::write(terminal, &bytes)
}
