//! The launcher's effects: its own path and command line, the pointer read, and each Client it
//! starts in a Job that ends the Client with the launcher while every process the Client starts
//! stays outside the Job.
use crate::relaunch::{self, Next, Outcome, Start};
use crate::resolve::{self, Failure};
use process_wrap::std::{CommandWrap, JobObject, KillOnJobClose, SilentBreakaway};
use std::{
    fs::File,
    io::Read,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

/// The largest pointer the launcher reads, the same bound the store's own reader uses.
const POINTER_LIMIT: u64 = 16 * 1024;
/// Exit code `8`: the Client ran, but its exit status could not be read.
const UNKNOWN_OUTCOME: i32 = 8;

/// Runs the active version's Client to its end and returns its exit code, or why none started.
/// Where automatic updates run, it announces the relaunch protocol and follows its decisions.
pub(crate) fn launch() -> Result<i32, Failure> {
    let launcher = std::env::current_exe()
        .map_err(|_| Failure::NoActiveVersion(PathBuf::from("active.json")))?;
    let first = resolve::client(&launcher, read_pointer)?;
    // Console events reach the Client through the shared console. Consuming them keeps the launcher
    // running to return the Client's own status; unlike ignoring them, a handler is not inherited.
    let _ = ctrlc::set_handler(|| {});
    if !orchbee_local_runtime::automatic_updates_live() {
        return match run(&first, None) {
            Outcome::Exited(code) => Ok(code),
            Outcome::NotStarted => Err(Failure::ClientUnavailable(first)),
        };
    }
    let announced = std::process::id();
    let mut start = Start::First;
    let mut client = Ok(first);
    let mut replaced = None;
    let mut relaunches: Vec<Instant> = Vec::with_capacity(relaunch::RELAUNCH_LIMIT);
    loop {
        let outcome = match &client {
            Ok(path) => run(path, Some(announced)),
            Err(_) => Outcome::NotStarted,
        };
        let now = Instant::now();
        match relaunch::next(start, outcome, &relaunches, now) {
            Next::End(code) => return Ok(code),
            Next::Unavailable => {
                return Err(match client {
                    Ok(path) => Failure::ClientUnavailable(path),
                    Err(failure) => failure,
                });
            }
            Next::Relaunch => {
                if relaunches.len() == relaunch::RELAUNCH_LIMIT {
                    relaunches.remove(0);
                }
                relaunches.push(now);
                replaced = client.ok();
                start = Start::Relaunch;
                client = resolve::client(&launcher, read_pointer);
            }
            Next::Return => {
                // Only a relaunch returns, and it follows a Client that started.
                let Some(path) = replaced.take() else {
                    return Err(client.err().unwrap_or(Failure::NoActiveVersion(launcher)));
                };
                start = Start::Return;
                client = Ok(path);
            }
        }
    }
}

/// Starts `client` with the launcher's own command line in a Job and waits for it. With `announced`,
/// the Client receives the launcher's process id in the protocol variable.
fn run(client: &Path, announced: Option<u32>) -> Outcome {
    let mut command = Command::new(client);
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
    if let Some(process) = announced {
        command.env(relaunch::PROTOCOL_VARIABLE, process.to_string());
    }
    let mut wrapped = CommandWrap::from(command);
    wrapped
        .wrap(JobObject)
        .wrap(KillOnJobClose)
        .wrap(SilentBreakaway);
    let Ok(mut child) = wrapped.spawn() else {
        return Outcome::NotStarted;
    };
    Outcome::Exited(child.wait().map_or(UNKNOWN_OUTCOME, |status| {
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
