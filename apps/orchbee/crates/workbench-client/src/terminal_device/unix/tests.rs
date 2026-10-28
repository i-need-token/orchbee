//! Inherited Unix terminal descriptors remain private and close on exec.

use std::os::fd::AsFd;

use nix::{
    fcntl::{FcntlArg, FdFlag, fcntl},
    pty::{Winsize, openpty},
};

use super::InheritedTerminal;

#[test]
fn admitted_and_duplicated_terminal_handles_do_not_survive_exec()
-> Result<(), Box<dyn std::error::Error>> {
    let pty = openpty(
        Some(&Winsize {
            ws_col: 80,
            ws_row: 24,
            ws_xpixel: 640,
            ws_ypixel: 384,
        }),
        None,
    )?;
    let terminal = InheritedTerminal::admit_from(pty.slave.as_fd(), pty.slave.as_fd())?;
    let input = terminal.duplicate_input()?;
    let output = terminal.duplicate_output()?;
    for fd in [
        terminal.input.as_fd(),
        terminal.output.as_fd(),
        input.as_fd(),
        output.as_fd(),
    ] {
        assert!(
            FdFlag::from_bits_retain(fcntl(fd, FcntlArg::F_GETFD)?).contains(FdFlag::FD_CLOEXEC)
        );
    }
    Ok(())
}
