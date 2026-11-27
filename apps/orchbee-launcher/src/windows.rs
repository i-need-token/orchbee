//! The launcher's effects: its own path and command line, the pointer read, each Client it starts
//! in a Job that ends the Client with the launcher while every process the Client starts stays
//! outside the Job, and the helper that records a failed Client.
use crate::relaunch::{self, Next, Outcome, Start, Verdict};
use crate::resolve::{self, Failure};
use process_wrap::std::{CommandWrap, JobObject, KillOnJobClose, SilentBreakaway};
use std::{
    fs::File,
    io::Read,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// The largest pointer the launcher reads, the same bound the store's own reader uses.
const POINTER_LIMIT: u64 = 16 * 1024;
/// Exit code `8`: the Client ran, but its exit status could not be read.
const UNKNOWN_OUTCOME: i32 = 8;
/// How long the launcher waits for the helper that records a failed Client, and how often it looks.
const RECORD_BOUND: Duration = Duration::from_secs(5);
const RECORD_POLL: Duration = Duration::from_millis(50);

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
        if relaunch::judge(outcome) == Verdict::Failed
            && let Ok(path) = &client
        {
            record_failure(&launcher, path);
        }
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
/// the Client receives the launcher's process id in both spellings of the protocol variable, so
/// a Client of any release reads it.
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
        let process = process.to_string();
        command
            .env(relaunch::PROTOCOL_VARIABLE.current, &process)
            .env(relaunch::PROTOCOL_VARIABLE.earlier, &process);
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

/// After the Client at `client` failed to start or to join, reads the pointer again and, when
/// [`orchbee_release_records::client_failure_helper`] names a helper, runs it, spelled as the
/// previous version reads it, outside the Client's Job with null standard streams and waits for it
/// at most five seconds. A helper still running then is left to finish; one that cannot start
/// records nothing. Either way the launcher goes on as it would without it.
fn record_failure(launcher: &Path, client: &Path) {
    let Some(root) = launcher.parent().and_then(Path::parent) else {
        return;
    };
    let pointer = read_pointer(&root.join("active.json"));
    let Some(helper) = orchbee_release_records::client_failure_helper(client, pointer.as_deref())
    else {
        return;
    };
    let Ok(mut child) = Command::new(&helper.program)
        .arg(orchbee_local_runtime::CLIENT_FAILURE_HELPER.for_release(&helper.previous))
        .arg(&helper.version)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    let give_up = Instant::now() + RECORD_BOUND;
    while matches!(child.try_wait(), Ok(None)) && Instant::now() < give_up {
        thread::sleep(RECORD_POLL);
    }
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
