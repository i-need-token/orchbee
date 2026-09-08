#![cfg(unix)]

use std::{
    error::Error,
    fs::{self, File},
    io::{self, Read},
    os::fd::OwnedFd,
    process::{Child, Command, ExitStatus, Stdio},
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
use std::io::Write;

#[cfg(target_os = "linux")]
use nix::errno::Errno;
use nix::{
    pty::{OpenptyResult, Winsize, openpty},
    sys::termios::{LocalFlags, Termios, tcgetattr},
    unistd::dup,
};

const PROCESS_TIMEOUT: Duration = Duration::from_secs(3);
const SCREEN_COLUMNS: u16 = 120;
const SCREEN_ROWS: u16 = 32;

struct RunningTui {
    child: Child,
    initial_termios: Termios,
    master: Option<OwnedFd>,
    screen_reader: Option<JoinHandle<io::Result<Vec<u8>>>>,
    slave: Option<OwnedFd>,
}

struct CompletedTui {
    restored_termios: Termios,
    screen: Vec<u8>,
    status: ExitStatus,
    stderr: Vec<u8>,
}

impl RunningTui {
    fn spawn(arguments: &[&str]) -> Result<Self, Box<dyn Error>> {
        let window = Winsize {
            ws_col: SCREEN_COLUMNS,
            ws_row: SCREEN_ROWS,
            ws_xpixel: 960,
            ws_ypixel: 640,
        };
        let OpenptyResult { master, slave } = openpty(Some(&window), None)?;
        let initial_termios = tcgetattr(&slave)?;
        let child = Command::new(env!("CARGO_BIN_EXE_orchbee"))
            .args(arguments)
            .env_remove("LC_ALL")
            .env_remove("LC_MESSAGES")
            .env_remove("LANG")
            .env("LANG", "en_US.UTF-8")
            .stdin(Stdio::from(File::from(dup(&slave)?)))
            .stdout(Stdio::from(File::from(dup(&slave)?)))
            .stderr(Stdio::piped())
            .spawn()?;
        let mut running = Self {
            child,
            initial_termios,
            master: Some(master),
            screen_reader: None,
            slave: Some(slave),
        };
        let master = match running.master.as_ref() {
            Some(master) => master,
            None => return Err(io::Error::other("PTY master was already closed").into()),
        };
        let screen_stream = File::from(dup(master)?);
        let screen_reader = std::thread::spawn(move || read_pty_screen(screen_stream));
        running.screen_reader = Some(screen_reader);
        Ok(running)
    }

    fn complete(mut self) -> Result<CompletedTui, Box<dyn Error>> {
        let status = wait_for_exit(&mut self.child)?;
        let restored_termios = match self.slave.as_ref() {
            Some(slave) => tcgetattr(slave)?,
            None => return Err(io::Error::other("PTY slave was already closed").into()),
        };
        let mut stderr = Vec::new();
        if let Some(mut stream) = self.child.stderr.take() {
            stream.read_to_end(&mut stderr)?;
        }
        let screen = self.close_pty_and_join_reader()?;
        Ok(CompletedTui {
            restored_termios,
            screen,
            status,
            stderr,
        })
    }

    fn close_pty_and_join_reader(&mut self) -> io::Result<Vec<u8>> {
        self.slave.take();
        self.master.take();
        let Some(screen_reader) = self.screen_reader.take() else {
            return Err(io::Error::other("PTY screen reader was already consumed"));
        };
        match screen_reader.join() {
            Ok(result) => result,
            Err(_) => Err(io::Error::other("PTY screen reader panicked")),
        }
    }
}

fn read_pty_screen(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let mut screen = Vec::new();
    match reader.read_to_end(&mut screen) {
        Ok(_) => Ok(screen),
        Err(error) if is_linux_pty_master_hangup(&error) => Ok(screen),
        Err(error) => Err(error),
    }
}

fn is_linux_pty_master_hangup(error: &io::Error) -> bool {
    #[cfg(target_os = "linux")]
    {
        error.raw_os_error() == Some(Errno::EIO as i32)
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = error;
        false
    }
}

impl Drop for RunningTui {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.slave.take();
        self.master.take();
        if let Some(reader) = self.screen_reader.take() {
            let _ = reader.join();
        }
    }
}

fn wait_for_exit(child: &mut Child) -> Result<ExitStatus, Box<dyn Error>> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            child.kill()?;
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "TUI did not exit before the process-test deadline",
            )
            .into());
        }
        std::thread::yield_now();
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|candidate| candidate == needle)
}

fn contains_ascii_token(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .enumerate()
        .any(|(offset, candidate)| {
            candidate == needle
                && offset
                    .checked_sub(1)
                    .and_then(|index| haystack.get(index))
                    .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
                && haystack
                    .get(offset + needle.len())
                    .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
        })
}

fn assert_no_terminal_mode_changes(completed: &CompletedTui, initial_termios: &Termios) {
    assert_restored_termios(&completed.restored_termios, initial_termios);
    for sequence in [
        &b"\x1b[?1049h"[..],
        &b"\x1b[?1049l"[..],
        &b"\x1b[?25l"[..],
        &b"\x1b[?25h"[..],
    ] {
        assert!(!contains_bytes(&completed.screen, sequence));
    }
}

fn assert_restored_termios(restored: &Termios, initial: &Termios) {
    assert_eq!(restored.input_flags, initial.input_flags);
    assert_eq!(restored.output_flags, initial.output_flags);
    assert_eq!(restored.control_flags, initial.control_flags);
    assert_eq!(
        restored.local_flags & !LocalFlags::PENDIN,
        initial.local_flags & !LocalFlags::PENDIN
    );
    assert_eq!(restored.control_chars, initial.control_chars);
}

#[test]
fn bare_tty_without_a_staged_cohort_keeps_terminal_modes_unchanged() -> Result<(), Box<dyn Error>> {
    let tui = RunningTui::spawn(&[])?;
    let initial_termios = tui.initial_termios.clone();
    let completed = tui.complete()?;

    assert_eq!(completed.status.code(), Some(1));
    assert_eq!(
        std::str::from_utf8(&completed.stderr)?,
        if cfg!(target_os = "linux") {
            "The native development cohort is missing, changed, or invalid. Stage all four binaries together before running orchbee.\n"
        } else {
            "The Orchbee TUI is unavailable in this development slice.\n"
        }
    );
    assert!(completed.screen.is_empty());
    assert_no_terminal_mode_changes(&completed, &initial_termios);
    Ok(())
}

#[test]
fn help_and_version_emit_no_terminal_control_sequences_on_a_tty() -> Result<(), Box<dyn Error>> {
    for arguments in [["--help"], ["--version"]] {
        let tui = RunningTui::spawn(&arguments)?;
        let initial_termios = tui.initial_termios.clone();
        let completed = tui.complete()?;

        assert_eq!(completed.status.code(), Some(0));
        assert!(completed.stderr.is_empty());
        assert_no_terminal_mode_changes(&completed, &initial_termios);
    }
    Ok(())
}

#[test]
fn linked_public_binary_contains_no_dev_tty_or_tput_fallback() -> Result<(), Box<dyn Error>> {
    let binary = fs::read(env!("CARGO_BIN_EXE_orchbee"))?;

    assert!(!contains_bytes(&binary, b"/dev/tty"));
    assert!(!contains_ascii_token(&binary, b"tput"));
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn linux_pty_master_eio_after_the_last_slave_closes_is_a_clean_hangup() -> Result<(), Box<dyn Error>>
{
    let OpenptyResult { master, slave } = openpty(None, None)?;
    let mut slave_stream = File::from(slave);
    slave_stream.write_all(b"screen bytes before hangup")?;
    drop(slave_stream);

    assert_eq!(
        read_pty_screen(File::from(master))?,
        b"screen bytes before hangup"
    );
    Ok(())
}
