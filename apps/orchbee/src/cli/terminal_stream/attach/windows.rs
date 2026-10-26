//! Direct attach borrows the admitted Windows console reader without replacing its state.
use crate::terminal_device::InheritedTerminal;
use std::{io, time::Duration};

pub(super) fn next_input(
    terminal: &InheritedTerminal,
    timeout: Duration,
) -> io::Result<Option<Vec<u8>>> {
    terminal.next_input(timeout)
}
