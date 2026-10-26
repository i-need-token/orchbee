//! Frozen console-host inputs and writes through the already admitted output handle.
use crate::{notifications::terminal::Host, terminal_device::InheritedTerminal};
use std::io::{self, Write};

pub(super) fn host() -> io::Result<Host> {
    Ok(Host {
        program: std::env::var("TERM_PROGRAM").ok(),
        term: std::env::var("TERM").ok(),
        kitty: std::env::var_os("KITTY_WINDOW_ID").is_some(),
        tmux: std::env::var_os("TMUX").is_some(),
    })
}

pub(super) fn write(terminal: &InheritedTerminal, bytes: &[u8]) -> io::Result<()> {
    let mut output = terminal.output()?;
    output.write_all(bytes)?;
    output.flush()
}
