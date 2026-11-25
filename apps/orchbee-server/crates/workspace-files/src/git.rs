//! One bounded, isolated Git command: no inherited repository selection or configuration
//! injection, no prompt, no optional locks, a deadline and an output cap.
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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

/// Runs `git args…` in `directory`, feeding `input` on standard input, until `deadline`.
pub(crate) fn run(
    directory: &Path,
    args: &[&str],
    input: Option<Vec<u8>>,
    output_cap: usize,
    deadline: Instant,
) -> Run {
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
        .current_dir(directory)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: a console subsystem child of a windowless Server opens no console.
        command.creation_flags(0x0800_0000);
    }
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
