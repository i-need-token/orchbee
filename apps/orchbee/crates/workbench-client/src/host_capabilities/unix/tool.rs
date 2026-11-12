//! One bounded external program run for a host effect: an exact argument vector without a shell,
//! optional standard input, discarded output, and a deadline on the child.
use std::{
    ffi::OsStr,
    io::Write as _,
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

/// Interval at which the run observes an unfinished child again.
const OBSERVE_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Exit {
    /// The program exited successfully after reading all of its input.
    Succeeded,
    /// The program could not start, exited unsuccessfully, or did not read all of its input.
    Failed,
    /// The program was still running at the deadline.
    Overran,
}

/// What happens to a program still running at the deadline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Overrun {
    /// The run kills and reaps it.
    Kill,
    /// It keeps running, for example as the browser it opened; a detached named thread reaps it.
    /// Only a run without input releases its program.
    Release,
}

/// Runs `program` with exactly `arguments`, writing `input` to its standard input.
pub(crate) fn run(
    program: &Path,
    arguments: &[&OsStr],
    input: Option<&[u8]>,
    budget: Duration,
    overrun: Overrun,
) -> Exit {
    let deadline = Instant::now() + budget;
    let mut command = Command::new(program);
    command
        .args(arguments)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let Ok(mut child) = command.spawn() else {
        return Exit::Failed;
    };
    let pipe = child.stdin.take();
    thread::scope(|scope| {
        // A program that stops reading cannot hold the run past its deadline: killing it closes
        // the pipe and the blocked write returns.
        let writer = match input.zip(pipe) {
            Some((bytes, mut pipe)) => {
                let spawned = thread::Builder::new()
                    .name("orchbee-host-program-input".to_owned())
                    .spawn_scoped(scope, move || pipe.write_all(bytes).is_ok());
                let Ok(writer) = spawned else {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Exit::Failed;
                };
                Some(writer)
            }
            None => None,
        };
        let status = wait_until(&mut child, deadline);
        if status.is_none() {
            match overrun {
                Overrun::Kill => {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                Overrun::Release => release(child),
            }
        }
        let written = writer.is_none_or(|writer| writer.join().unwrap_or(false));
        match status {
            None => Exit::Overran,
            Some(status) if status.success() && written => Exit::Succeeded,
            Some(_) => Exit::Failed,
        }
    })
}

fn wait_until(child: &mut Child, deadline: Instant) -> Option<ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(_) => return None,
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        thread::sleep(remaining.min(OBSERVE_INTERVAL));
    }
}

/// Transfers a still-running child to a detached thread that only reaps it. If that thread cannot
/// start, the child is killed instead, so no child is left unreaped.
fn release(child: Child) {
    let slot = Arc::new(Mutex::new(Some(child)));
    let reaped = Arc::clone(&slot);
    let take = |slot: &Mutex<Option<Child>>| slot.lock().ok().and_then(|mut child| child.take());
    let spawned = thread::Builder::new()
        .name("orchbee-host-program-reaper".to_owned())
        .spawn(move || {
            if let Some(mut child) = take(&reaped) {
                let _ = child.wait();
            }
        });
    if spawned.is_err()
        && let Some(mut child) = take(&slot)
    {
        let _ = child.kill();
        let _ = child.wait();
    }
}
