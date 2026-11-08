//! Windows facts for destructive admission: the compiled environment, console handles, the
//! Session name prompt, and the destructive request log record.
use super::{Console, Typed};
use crate::cli::console_interrupt::{ArmedSignals, Finish};
use serde_json::json;
use std::{
    io::{self, BufRead, Read, Write},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use winsafe::{self as w, co};

/// A typed Session name is at most 64 bytes; a longer line can only mismatch.
const LINE_LIMIT: u64 = 1024;
/// 100-nanosecond intervals from 1601-01-01 to the Unix epoch.
const UNIX_EPOCH_AS_FILETIME: u64 = 116_444_736_000_000_000;

pub(super) fn release() -> bool {
    orchbee_local_runtime::is_release()
}

/// A console handle answers `GetConsoleMode`; a pipe, file, null device or pseudo-terminal pipe
/// such as MSYS does not.
pub(super) fn console() -> Console {
    Console {
        stdin: winapi_util::console::mode(winapi_util::HandleRef::stdin()).is_ok(),
        stdout: winapi_util::console::mode(winapi_util::HandleRef::stdout()).is_ok(),
    }
}

pub(super) fn target_session_name() -> Option<String> {
    crate::local_activation::target_session_name().map(|name| name.as_str().to_owned())
}

/// Writes the prompt to standard error and reads one line from the console. The read runs on its
/// own thread so a console interrupt cancels the wait at once; any other failure reads as an
/// empty line, which never matches a Session name.
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
    let Ok(signals) = ArmedSignals::arm_interrupts() else {
        return Ok(String::new());
    };
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        let read = io::stdin().lock().take(LINE_LIMIT).read_line(&mut line);
        let _ = sender.send(read.map(|_| line).unwrap_or_default());
    });
    let mut line = String::new();
    while !signals.requested() {
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
        Ok(Finish::Cancelled { finalization }) => Err(finalization),
        Ok(Finish::Clear) => Ok(line),
        Err(_) => Ok(String::new()),
    }
}

/// Appends the invocation's record. A failed append never blocks or changes the command.
pub(super) fn record(command: &str, target: Option<&str>, console: Console, outcome: &str) {
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

/// The parent PID from a process snapshot and, when that process still exists and started before
/// this one, its image path. A reused PID keeps the number but reports no image.
fn parent_process() -> (Option<u32>, Option<String>) {
    let own = std::process::id();
    let Ok(mut snapshot) = w::HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None)
    else {
        return (None, None);
    };
    let Some(parent) = snapshot
        .iter_processes()
        .filter_map(Result::ok)
        .find(|entry| entry.th32ProcessID == own)
        .map(|entry| entry.th32ParentProcessID)
    else {
        return (None, None);
    };
    let image = (|| {
        let process =
            w::HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, parent).ok()?;
        let (started, ..) = process.GetProcessTimes().ok()?;
        let (own_started, ..) = w::HPROCESS::GetCurrentProcess().GetProcessTimes().ok()?;
        if u64::from(started) > u64::from(own_started) {
            return None;
        }
        process
            .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
            .ok()
    })();
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
    let intervals = milliseconds
        .checked_mul(10_000)?
        .checked_add(UNIX_EPOCH_AS_FILETIME)?;
    let time = w::FileTimeToSystemTime(&w::FILETIME::from(intervals)).ok()?;
    Some(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        time.wYear,
        time.wMonth,
        time.wDay,
        time.wHour,
        time.wMinute,
        time.wSecond,
        time.wMilliseconds
    ))
}
