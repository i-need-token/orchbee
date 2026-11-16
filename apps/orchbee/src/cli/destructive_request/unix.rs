//! POSIX facts for destructive admission: the compiled environment, terminal streams, the Session
//! name prompt, and the destructive request log record. Only macOS applies them: a release macOS
//! build prompts in place of `--confirm`, and every macOS invocation is logged. Linux keeps
//! `--confirm` and writes no destructive request log until ORC-328.
use super::{Console, Typed};
use crate::cli::console_interrupt::{ArmedSignals, Finish};
use serde_json::json;
use std::{
    io::{self, BufRead, IsTerminal, Read, Write},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// A typed Session name is at most 64 bytes; a longer line can only mismatch.
const LINE_LIMIT: u64 = 1024;
/// Room for one program path in the parent's `ps` line.
const PARENT_LIMIT: usize = 4096;

pub(super) fn release() -> bool {
    cfg!(target_os = "macos") && orchbee_local_runtime::is_release()
}

/// A terminal answers `isatty`; a pipe, file or the null device does not.
pub(super) fn console() -> Console {
    Console {
        stdin: io::stdin().is_terminal(),
        stdout: io::stdout().is_terminal(),
    }
}

pub(super) fn target_session_name() -> Option<String> {
    crate::local_activation::target_session_name().map(|name| name.as_str().to_owned())
}

/// Writes the prompt to standard error and reads one line from the terminal. The read runs on its
/// own thread, started after the interrupt pair is captured so it inherits the blocked signals,
/// and an interrupt cancels the wait at once; any other failure reads as an empty line, which
/// never matches a Session name.
pub(super) fn prompt(text: &str) -> Typed {
    let mut stderr = io::stderr().lock();
    if stderr
        .write_all(text.as_bytes())
        .and_then(|()| stderr.flush())
        .is_err()
    {
        return Ok(String::new());
    }
    drop(stderr);
    let Ok(mut signals) = ArmedSignals::arm_interrupts_except_linux() else {
        return Ok(String::new());
    };
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        let read = io::stdin().lock().take(LINE_LIMIT).read_line(&mut line);
        let _ = sender.send(read.map(|_| line).unwrap_or_default());
    });
    let mut line = String::new();
    // A captured signal latches, so the settle below sees the interruption this loop saw.
    while matches!(signals.take(), Ok(None)) {
        match receiver.recv_timeout(Duration::from_millis(20)) {
            Ok(typed) => {
                line = typed;
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    match signals.finish() {
        Ok(Finish::Cancelled { finalization }) => Err(Box::new(finalization)),
        Ok(Finish::Clear) => Ok(line),
        Err(_) => Ok(String::new()),
    }
}

/// Appends the invocation's record on macOS. A failed append never blocks or changes the command.
pub(super) fn record(command: &str, target: Option<&str>, console: Console, outcome: &str) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let (parent, image) = parent_process();
    let document = json!({
        "version": 1,
        "time": utc_time(),
        "environment": orchbee_local_runtime::environment_stem(),
        "command": command,
        "session": target,
        "interactive": {"stdin": console.stdin, "stdout": console.stdout},
        "parent": {"pid": parent, "image": image},
        "outcome": outcome,
    });
    let _ = orchbee_local_runtime::append_destructive_request(&document);
}

/// The parent PID and its start path as `ps` reports it. An exited parent's children pass to
/// another process, so a parent PID that is the same after `ps` answered named one live process
/// throughout and its line is that process's; otherwise the image is not reported.
fn parent_process() -> (Option<u32>, Option<String>) {
    let parent = std::os::unix::process::parent_id();
    let mut command = std::process::Command::new("/bin/ps");
    command
        .args(["-o", "comm=", "-p", &parent.to_string()])
        .env_clear()
        .env("LC_ALL", "C");
    // A cancelled prompt leaves the interrupt pair blocked until the command settles, so `ps`
    // starts with an empty signal mask.
    let result = orchbee_local_runtime::run_unmasked_unix_command(
        command,
        orchbee_local_runtime::SpawnEnvironment::Cleared,
        PARENT_LIMIT,
        Some(Instant::now() + Duration::from_secs(2)),
        || Ok(false),
    );
    let image = (result.failure.is_none() && !result.stdout_truncated)
        .then(|| String::from_utf8_lossy(&result.stdout).trim().to_owned())
        .filter(|image| !image.is_empty() && std::os::unix::process::parent_id() == parent);
    (Some(parent), image)
}

/// The current UTC time as RFC 3339 with milliseconds, such as `2026-11-08T03:04:05.678Z`.
fn utc_time() -> Option<String> {
    let milliseconds = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis(),
    )
    .ok()?;
    Some(orchbee_local_runtime::utc_timestamp(milliseconds))
}
