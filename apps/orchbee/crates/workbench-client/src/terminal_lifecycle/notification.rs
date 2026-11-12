//! Outer-terminal notification framing from the terminal capability variables, written through
//! the already admitted output on every platform.
use crate::{
    notifications::terminal::{self, Host},
    terminal_device::InheritedTerminal,
};
use orchbee_workbench_protocol::Notification;
use std::io::{self, Write as _};

pub(super) fn send(terminal: &InheritedTerminal, message: &Notification) -> io::Result<()> {
    let bytes = terminal::sequence(message, &Host::detect(|name| std::env::var_os(name)))?;
    let mut output = terminal.output()?;
    output.write_all(&bytes)?;
    output.flush()
}
