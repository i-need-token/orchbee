use std::os::fd::AsFd;

use nix::{
    pty::{Winsize, openpty},
    unistd::dup,
};

use super::{InheritedTerminal, TerminalAdmissionError, require_same_terminal_identity};

#[test]
fn admits_only_two_terminal_streams_with_the_same_device_identity() {
    for (stdin_is_terminal, stdout_is_terminal, stdin_id, stdout_id, expected) in [
        (true, true, 7_u64, 7_u64, Ok(())),
        (
            true,
            true,
            7,
            8,
            Err(TerminalAdmissionError::DifferentTerminals),
        ),
        (
            false,
            true,
            7,
            7,
            Err(TerminalAdmissionError::NotInteractive),
        ),
        (
            true,
            false,
            7,
            7,
            Err(TerminalAdmissionError::NotInteractive),
        ),
    ] {
        let result = require_same_terminal_identity(
            stdin_is_terminal,
            stdout_is_terminal,
            stdin_id,
            stdout_id,
        );
        assert_eq!(
            result.map_err(|error| error.to_string()),
            expected.map_err(|error| error.to_string())
        );
    }
}

#[test]
fn admits_matching_real_pty_descriptors_and_rejects_different_ptys()
-> Result<(), Box<dyn std::error::Error>> {
    let window = Winsize {
        ws_col: 113,
        ws_row: 31,
        ws_xpixel: 904,
        ws_ypixel: 620,
    };
    let first = openpty(Some(&window), None)?;
    let first_output = dup(&first.slave)?;
    let second = openpty(Some(&window), None)?;

    let admitted = InheritedTerminal::admit_from(first.slave.as_fd(), first_output.as_fd())?;
    assert_eq!(
        admitted.window_dimensions()?,
        super::WindowDimensions {
            columns: 113,
            pixel_height: 620,
            pixel_width: 904,
            rows: 31,
        }
    );

    assert!(matches!(
        InheritedTerminal::admit_from(first.slave.as_fd(), second.slave.as_fd()),
        Err(TerminalAdmissionError::DifferentTerminals)
    ));
    Ok(())
}

#[test]
fn admission_accepts_initially_zero_cells_and_observes_real_recovery()
-> Result<(), Box<dyn std::error::Error>> {
    use rustix::termios::{Winsize as RustixWinsize, tcsetwinsize};

    for (columns, rows) in [(0_u16, 0_u16), (0, 24), (80, 0)] {
        let pty = openpty(
            Some(&Winsize {
                ws_col: columns,
                ws_row: rows,
                ws_xpixel: 0,
                ws_ypixel: 0,
            }),
            None,
        )?;
        let terminal = InheritedTerminal::admit_from(pty.slave.as_fd(), pty.slave.as_fd())?;

        assert_eq!(terminal.observe_window_dimensions()?, None);
        assert!(terminal.window_dimensions().is_err());

        let other = openpty(
            Some(&Winsize {
                ws_col: 80,
                ws_row: 24,
                ws_xpixel: 640,
                ws_ypixel: 384,
            }),
            None,
        )?;
        assert!(matches!(
            InheritedTerminal::admit_from(pty.slave.as_fd(), other.slave.as_fd()),
            Err(TerminalAdmissionError::DifferentTerminals)
        ));

        tcsetwinsize(
            pty.slave.as_fd(),
            RustixWinsize {
                ws_col: 110,
                ws_row: 30,
                ws_xpixel: 880,
                ws_ypixel: 600,
            },
        )?;
        assert_eq!(
            terminal.observe_window_dimensions()?,
            Some(super::WindowDimensions {
                columns: 110,
                pixel_height: 600,
                pixel_width: 880,
                rows: 30,
            })
        );
    }
    Ok(())
}

#[test]
fn runtime_observation_reports_zero_cells_as_absent_and_recovers()
-> Result<(), Box<dyn std::error::Error>> {
    use rustix::termios::{Winsize as RustixWinsize, tcsetwinsize};

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

    tcsetwinsize(
        pty.slave.as_fd(),
        RustixWinsize {
            ws_col: 0,
            ws_row: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
    )?;
    assert_eq!(terminal.observe_window_dimensions()?, None);
    assert!(terminal.window_dimensions().is_err());

    tcsetwinsize(
        pty.slave.as_fd(),
        RustixWinsize {
            ws_col: 110,
            ws_row: 30,
            ws_xpixel: 880,
            ws_ypixel: 600,
        },
    )?;
    assert_eq!(
        terminal.observe_window_dimensions()?,
        Some(super::WindowDimensions {
            columns: 110,
            pixel_height: 600,
            pixel_width: 880,
            rows: 30,
        })
    );
    Ok(())
}

#[test]
fn inherited_terminal_adapter_has_no_path_process_or_global_size_fallback() {
    let source = [
        include_str!("../terminal_device.rs"),
        include_str!("../terminal_lifecycle.rs"),
        include_str!("../tui_backend.rs"),
        include_str!("../tui_shell.rs"),
    ]
    .concat();

    for forbidden in [
        "/dev/tty",
        "Command::new",
        "File::open",
        "OpenOptions",
        "process::Command",
        "std::process",
        "crossterm::terminal::size",
        "crossterm::terminal::window_size",
    ] {
        assert!(
            !source.contains(forbidden),
            "inherited-terminal adapter contains forbidden fallback: {forbidden}"
        );
    }

    let manifest = include_str!("../../Cargo.toml");
    for forbidden_feature in ["features = [\"events\"", "features = [\"crossterm\""] {
        assert!(
            !manifest.contains(forbidden_feature),
            "orchbee enables a dependency path that can acquire a different terminal: {forbidden_feature}"
        );
    }
}

#[test]
fn admitted_and_duplicated_terminal_handles_do_not_survive_exec()
-> Result<(), Box<dyn std::error::Error>> {
    use nix::fcntl::{FcntlArg, FdFlag, fcntl};
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
