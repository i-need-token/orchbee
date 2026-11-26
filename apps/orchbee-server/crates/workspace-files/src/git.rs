//! One bounded, isolated Git command: no inherited repository selection or configuration
//! injection, no prompt, no optional locks, a deadline and an output cap.
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// How long a Git command that changes the repository has, after its deadline, to stop on its own
/// and remove its lock files before it is killed.
pub(crate) const STOP_GRACE: Duration = Duration::from_secs(3);
/// How long the output of a finished command is still read, for a hook's process that outlives it
/// and holds the pipe open.
const DRAIN_GRACE: Duration = Duration::from_millis(250);

/// How one Git command ended.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Run {
    /// The command exited; `truncated` when its output passed the cap and the rest was dropped.
    Exited {
        status: Option<i32>,
        stdout: Vec<u8>,
        truncated: bool,
    },
    /// The deadline passed and the command was killed.
    TimedOut,
    /// Git could not be started.
    Unavailable,
}

/// How one Git command that changes the repository ended.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Change {
    /// The command exited; `output` is the start of what it and its hooks printed, in order, and
    /// `truncated` says more was printed.
    Exited {
        success: bool,
        output: Vec<u8>,
        truncated: bool,
    },
    /// The deadline passed and the command was stopped.
    TimedOut,
    /// Git could not be started.
    Unavailable,
}

/// `git` with the isolated environment, in `directory`, with `args`.
fn command(directory: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    for (name, _) in std::env::vars_os() {
        if name
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(name);
        }
    }
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.quotepath=false",
            "--no-pager",
        ])
        .args(args)
        .current_dir(directory);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: a console subsystem child of a windowless Server opens no console.
        command.creation_flags(0x0800_0000);
    }
    command
}

/// Runs `git args…` in `directory`, feeding `input` on standard input, until `deadline`.
pub(crate) fn run(
    directory: &Path,
    args: &[&str],
    input: Option<Vec<u8>>,
    output_cap: usize,
    deadline: Instant,
) -> Run {
    let mut command = command(directory, args);
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let Ok(mut child) = command.spawn() else {
        return Run::Unavailable;
    };
    let writer = input.and_then(|bytes| {
        let mut stdin = child.stdin.take()?;
        Some(std::thread::spawn(move || {
            // A write error means Git exited early; its status says why.
            let _written = stdin.write_all(&bytes);
        }))
    });
    let Some(mut stdout) = child.stdout.take() else {
        let _killed = child.kill();
        let _reaped = child.wait();
        return Run::Unavailable;
    };
    let reader = std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut buffer = [0_u8; 64 * 1024];
        let mut truncated = false;
        loop {
            match stdout.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                // Past the cap the rest is drained and dropped, so Git never blocks on a full pipe.
                Ok(_) if truncated => {}
                Ok(read) => {
                    let room = output_cap.saturating_sub(kept.len());
                    truncated = read > room;
                    kept.extend_from_slice(&buffer[..read.min(room)]);
                }
            }
        }
        (kept, truncated)
    });
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status.code()),
            Ok(None) if Instant::now() >= deadline => break None,
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
            Err(_) => break None,
        }
    };
    let Some(status) = status else {
        let _killed = child.kill();
        let _reaped = child.wait();
        let _joined = reader.join();
        if let Some(writer) = writer {
            let _joined = writer.join();
        }
        return Run::TimedOut;
    };
    if let Some(writer) = writer {
        let _joined = writer.join();
    }
    let (stdout, truncated) = match reader.join() {
        Ok(read) => read,
        Err(_) => return Run::Unavailable,
    };
    Run::Exited {
        status,
        stdout,
        truncated,
    }
}

/// Runs `git args…` in `directory` to change the repository, feeding `input` on standard input,
/// with the repository's hooks. What Git and its hooks print on either stream is kept in order up
/// to `output_cap`. At `deadline` the command is asked to stop, so Git removes its lock files, and
/// killed with its hooks [`STOP_GRACE`] later; on Windows only Git itself is killed.
pub(crate) fn change(
    directory: &Path,
    args: &[&str],
    input: Option<Vec<u8>>,
    output_cap: usize,
    deadline: Instant,
) -> Change {
    let Ok((mut reader, writer)) = std::io::pipe() else {
        return Change::Unavailable;
    };
    let Ok(error_writer) = writer.try_clone() else {
        return Change::Unavailable;
    };
    let mut command = command(directory, args);
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(writer)
        .stderr(error_writer);
    // Its own process group, so its hooks stop with it.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let spawned = command.spawn();
    // The command keeps the pipe's write ends; only the child may hold them, or no end is seen.
    drop(command);
    let Ok(mut child) = spawned else {
        return Change::Unavailable;
    };
    let writer = input.and_then(|bytes| {
        let mut stdin = child.stdin.take()?;
        Some(std::thread::spawn(move || {
            // A write error means Git exited early; its status says why.
            let _written = stdin.write_all(&bytes);
        }))
    });
    // The reader keeps what it reads where this thread can take it, even while a process that
    // outlived Git holds the pipe open and the read never ends.
    let kept = Arc::new(Mutex::new((Vec::new(), false)));
    let (ended, end) = mpsc::channel();
    let reading = Arc::clone(&kept);
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 16 * 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    let Ok(mut kept) = reading.lock() else {
                        break;
                    };
                    let (bytes, truncated) = &mut *kept;
                    let room = output_cap.saturating_sub(bytes.len());
                    *truncated |= read > room;
                    bytes.extend_from_slice(&buffer[..read.min(room)]);
                }
            }
        }
        let _sent = ended.send(());
    });
    let success = wait(&mut child, deadline).or_else(|| {
        stop(&mut child);
        None
    });
    if let Some(writer) = writer {
        let _joined = writer.join();
    }
    let Some(success) = success else {
        return Change::TimedOut;
    };
    let _drained = end.recv_timeout(DRAIN_GRACE);
    let Ok(kept) = kept.lock() else {
        return Change::Unavailable;
    };
    Change::Exited {
        success,
        output: kept.0.clone(),
        truncated: kept.1,
    }
}

/// Whether `child` succeeded, or none when `deadline` passed first or its state is unknown.
fn wait(child: &mut Child, deadline: Instant) -> Option<bool> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status.success()),
            Ok(None) if Instant::now() >= deadline => return None,
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(_) => return None,
        }
    }
}

/// Stops `child` and, on Unix, its process group: a termination signal it answers by removing
/// its lock files, then a kill once [`STOP_GRACE`] passes.
fn stop(child: &mut Child) {
    #[cfg(unix)]
    {
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;
        let group = i32::try_from(child.id()).ok().map(Pid::from_raw);
        if let Some(group) = group {
            let _asked = killpg(group, Signal::SIGTERM);
            let _stopped = wait(child, Instant::now() + STOP_GRACE);
            // Whatever of the group is left, Git or its hooks, ends now.
            let _killed = killpg(group, Signal::SIGKILL);
        }
    }
    let _killed = child.kill();
    let _reaped = child.wait();
}
