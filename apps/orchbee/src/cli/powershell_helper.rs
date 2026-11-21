//! Starts the invocation-owned hidden system Windows PowerShell processes that make inherited
//! synchronous handles cancellable, and observes when each reports ready. A helper runs one fixed
//! script with profiles disabled; payload never becomes script or argument text.
use orchbee_local_ipc::ChildOutputReader;
use std::{
    io::{self, Read},
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};

/// How long a helper may take to report ready. Windows PowerShell can need seconds to start on a
/// loaded machine, so a helper's start never counts against a command's deadline; this allowance
/// still bounds a helper that never starts.
const START: Duration = Duration::from_secs(20);

/// The byte every helper script writes first to its standard error, once its standard streams are
/// open.
const READY: u8 = 2;

/// The system Windows PowerShell running `script`; the caller assigns its standard streams and
/// creation flags.
pub(in crate::cli) fn command(script: &str) -> io::Result<Command> {
    let executable = PathBuf::from(winsafe::GetSystemDirectory().map_err(io::Error::other)?)
        .join("WindowsPowerShell/v1.0/powershell.exe");
    let mut command = Command::new(executable);
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        script,
    ]);
    Ok(command)
}

/// Checks once whether the helper spawned at `spawned` has reported ready on its standard error
/// `status`. Still pending after the start allowance is `TimedOut`; an exit or any other byte
/// first is `BrokenPipe`.
pub(in crate::cli) fn poll_ready(
    status: &mut ChildOutputReader,
    spawned: Instant,
) -> io::Result<bool> {
    let mut byte = [0];
    match status.read(&mut byte) {
        Ok(1) if byte[0] == READY => Ok(true),
        Ok(_) => Err(io::ErrorKind::BrokenPipe.into()),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
            if spawned.elapsed() >= START {
                Err(io::ErrorKind::TimedOut.into())
            } else {
                Ok(false)
            }
        }
        Err(error) => Err(error),
    }
}

/// Waits until the helper spawned just now reports ready on `status`; `cancelled` interrupts the
/// wait with `Interrupted`.
pub(in crate::cli) fn await_ready(
    status: &mut ChildOutputReader,
    cancelled: &dyn Fn() -> bool,
) -> io::Result<()> {
    let spawned = Instant::now();
    while !poll_ready(status, spawned)? {
        if cancelled() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        thread::sleep(Duration::from_millis(2));
    }
    Ok(())
}
