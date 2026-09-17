//! Test-only child-process guard shared by the `cli_query` and
//! `cli_mutation` integration tests. This directory is not a test target —
//! Cargo only compiles top-level `tests/*.rs` files — it exists so both
//! binaries share one bounded-output implementation.

use std::{
    io::{ErrorKind, Read},
    os::unix::process::ExitStatusExt,
    path::Path,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Wall-clock backstop on every spawned CLI child. A healthy run retires on
/// the CLI's own 5s network budget; this bound only guards the suite against
/// a child that never exits at all, leaving ample slack for a loaded machine.
pub const RUN_WALL_CLOCK_BACKSTOP: Duration = Duration::from_secs(45);

/// The spawned child never exited inside `budget` and had to be killed. The
/// reaped status and whatever was drained stay attached so the timeout is a
/// diagnosable failure, never a silent pass.
#[derive(Debug)]
pub struct BoundedChildTimeout {
    pub budget: Duration,
    pub status: std::process::ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl std::fmt::Display for BoundedChildTimeout {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cli child exceeded the {:?} wall-clock backstop; killed and reaped \
             ({}) with {}B stdout / {}B stderr captured",
            self.budget,
            self.status,
            self.stdout.len(),
            self.stderr.len()
        )
    }
}

/// `Command::output` with a deadline. Owns the stdio configuration — stdin
/// closed, both pipes drained on their own threads — so a child writing more
/// than the pipe buffer cannot deadlock the wait. On expiry only the spawned
/// child is killed (`Child::kill` targets its exact pid) and then reaped via
/// `wait`. A child that exited on its own in the kill race still returns its
/// real `Output`.
pub fn bounded_output(
    command: &mut Command,
    budget: Duration,
) -> Result<Output, BoundedChildTimeout> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn cli child");
    let mut stdout_pipe = child.stdout.take().expect("stdout piped");
    let mut stderr_pipe = child.stderr.take().expect("stderr piped");
    let stdout_drain = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut bytes);
        bytes
    });
    let stderr_drain = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut bytes);
        bytes
    });
    let deadline = Instant::now() + budget;
    let observed = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Ok(None) => break None,
            Err(error) => panic!("cli child wait failed: {error}"),
        }
    };
    let status = match observed {
        Some(status) => status,
        None => {
            match child.kill() {
                Ok(()) => {}
                // Exited between the last poll and the kill: nothing to
                // signal, `wait` reaps the real exit status below.
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => panic!("cli child kill failed: {error}"),
            }
            child.wait().expect("reap cli child")
        }
    };
    let stdout = stdout_drain.join().expect("stdout drain joined");
    let stderr = stderr_drain.join().expect("stderr drain joined");
    if observed.is_none() && status.signal().is_some() {
        return Err(BoundedChildTimeout {
            budget,
            status,
            stdout,
            stderr,
        });
    }
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// `Child::wait_with_output` under the same deadline, for callers that own a
/// custom stdio sink (e.g. a `/dev/full` stdout) and therefore cannot route
/// through `bounded_output`. Same kill/reap/race semantics; the deadline is
/// enforced on the already-spawned child.
pub fn bounded_wait_with_output(
    mut child: Child,
    budget: Duration,
) -> Result<Output, BoundedChildTimeout> {
    let deadline = Instant::now() + budget;
    let observed = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Ok(None) => break None,
            Err(error) => panic!("cli child wait failed: {error}"),
        }
    };
    if observed.is_none() {
        match child.kill() {
            Ok(()) => {}
            // Exited between the last poll and the kill: `wait_with_output`
            // below reaps the real exit status.
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => panic!("cli child kill failed: {error}"),
        }
    }
    let output = child.wait_with_output().expect("reap cli child");
    if observed.is_none() && output.status.signal().is_some() {
        return Err(BoundedChildTimeout {
            budget,
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
        });
    }
    Ok(output)
}

/// Spawn the staged CLI binary under the wall-clock backstop with the shared
/// deterministic-locale environment both suites use.
pub fn run(binary: &Path, base: &Path, arguments: &[&str]) -> Output {
    let mut command = Command::new(binary);
    command
        .args(arguments)
        .env("ORCHBEE_RUNTIME_BASE", base)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "en_US.UTF-8");
    bounded_output(&mut command, RUN_WALL_CLOCK_BACKSTOP)
        .unwrap_or_else(|timeout| panic!("{timeout}"))
}
