//! Interactive attach has no Unix input capability and does not read the inherited terminal.
use crate::terminal_device::InheritedTerminal;
use std::{io, time::Duration};

pub(super) fn next_input(
    _terminal: &InheritedTerminal,
    _timeout: Duration,
) -> io::Result<Option<Vec<u8>>> {
    Err(io::ErrorKind::Unsupported.into())
}
