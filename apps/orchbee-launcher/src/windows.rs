//! The launcher's effects: its own path and command line, the pointer read, and the Client it
//! starts in a Job that ends the Client with the launcher while every process the Client starts
//! stays outside the Job.
use crate::resolve::{self, Failure};
use process_wrap::std::{CommandWrap, JobObject, KillOnJobClose, SilentBreakaway};
use std::{
    fs::File,
    io::Read,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};

/// The largest pointer the launcher reads, the same bound the store's own reader uses.
const POINTER_LIMIT: u64 = 16 * 1024;
/// Exit code `8`: the Client ran, but its exit status could not be read.
const UNKNOWN_OUTCOME: i32 = 8;

/// Runs the active version's Client to its end and returns its exit code, or why none started.
pub(crate) fn launch() -> Result<i32, Failure> {
    let launcher = std::env::current_exe()
        .map_err(|_| Failure::NoActiveVersion(PathBuf::from("active.json")))?;
    let client = resolve::client(&launcher, read_pointer)?;
    // Console events reach the Client through the shared console. Consuming them keeps the launcher
    // running to return the Client's own status; unlike ignoring them, a handler is not inherited.
    let _ = ctrlc::set_handler(|| {});
    let mut command = Command::new(&client);
    let line = winsafe::GetCommandLine();
    if line.contains(char::REPLACEMENT_CHARACTER) {
        // A command line that is not valid UTF-16 cannot pass through as text; passed argument by
        // argument, the Client still reads the same arguments.
        command.args(std::env::args_os().skip(1));
    } else {
        let tail = resolve::command_tail(&line);
        if !tail.is_empty() {
            command.raw_arg(tail);
        }
    }
    let mut wrapped = CommandWrap::from(command);
    wrapped
        .wrap(JobObject)
        .wrap(KillOnJobClose)
        .wrap(SilentBreakaway);
    let mut child = wrapped
        .spawn()
        .map_err(|_| Failure::ClientUnavailable(client))?;
    Ok(child.wait().map_or(UNKNOWN_OUTCOME, |status| {
        status.code().unwrap_or(UNKNOWN_OUTCOME)
    }))
}

/// The pointer's bytes, or `None` when it is missing, unreadable, or larger than the bound.
fn read_pointer(path: &Path) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take(POINTER_LIMIT + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= POINTER_LIMIT).then_some(bytes)
}
