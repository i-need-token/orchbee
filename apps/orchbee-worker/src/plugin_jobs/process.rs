use super::{Failure, PluginCommandIntent, PluginCommandLog, finish_failure, now_ms};
use crate::windows_command_child::WindowsCommandChild;
use orchbee_capability_node_protocol::MAXIMUM_PLUGIN_OUTPUT_BYTES;
use orchbee_local_ipc::ChildOutputReader;
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

pub(super) fn run(
    intent: PluginCommandIntent,
    log: PluginCommandLog,
    cancel: &AtomicBool,
) -> PluginCommandLog {
    if cancel.load(Ordering::Relaxed) {
        return finish_failure(log, Failure::Cancelled);
    }
    let mut command = Command::new(program(&intent.argv[0], Path::new(&intent.directory)));
    command
        .args(&intent.argv[1..])
        .current_dir(&intent.directory)
        .stdin(Stdio::null());
    // Context belongs to this invocation. Never leak a parent plugin's identifiers.
    for (name, _) in std::env::vars_os() {
        if name
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("ORCHBEE_PLUGIN_")
        {
            command.env_remove(name);
        }
    }
    for name in [
        "ORCHBEE_WORKSPACE_ID",
        "ORCHBEE_TAB_ID",
        "ORCHBEE_PANE_ID",
        "ORCHBEE_TERMINAL_ID",
        "ORCHBEE_WORKER_ID",
    ] {
        command.env_remove(name);
    }
    command.envs(intent.environment);
    let (out, stdout) = match ChildOutputReader::pipe() {
        Ok(v) => v,
        Err(_) => return finish_failure(log, Failure::Io),
    };
    let (err, stderr) = match ChildOutputReader::pipe() {
        Ok(v) => v,
        Err(_) => return finish_failure(log, Failure::Io),
    };
    command.stdout(stdout).stderr(stderr);
    let mut child = match WindowsCommandChild::spawn(command) {
        Ok(child) => child,
        Err(failure) => {
            return finish_failure(
                log,
                if failure.created {
                    Failure::PhysicalUnknown
                } else if failure.error.kind() == io::ErrorKind::NotFound {
                    Failure::SpawnUnavailable
                } else {
                    Failure::SpawnFailed
                },
            );
        }
    };
    let mut log = log;
    let mut out = Some(out);
    let mut err = Some(err);
    loop {
        let stop = if cancel.load(Ordering::Relaxed) {
            Some(Failure::Cancelled)
        } else if drain(&mut out, &mut log.stdout, &mut log.stdout_truncated, false)
            .and_then(|()| drain(&mut err, &mut log.stderr, &mut log.stderr_truncated, false))
            .is_err()
        {
            Some(Failure::Io)
        } else {
            None
        };
        if let Some(reason) = stop {
            let failure = if child.settle().is_err() {
                Failure::PhysicalUnknown
            } else {
                reason
            };
            let _drained = drain_tail(&mut out, &mut err, &mut log);
            return finish_failure(log, failure);
        }
        match child.try_wait() {
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Ok(Some(status)) => {
                if drain_tail(&mut out, &mut err, &mut log).is_err() {
                    return finish_failure(log, Failure::Io);
                }
                log.finished_ms = Some(now_ms().max(log.started_ms));
                log.exit_code = status.code();
                log.failure = if status.success() {
                    None
                } else {
                    Some(Failure::ExitFailure)
                };
                return log;
            }
            Err(_) => {
                let reason = if child.settle().is_err() {
                    Failure::PhysicalUnknown
                } else {
                    Failure::Io
                };
                return finish_failure(log, reason);
            }
        }
    }
}
fn drain_tail(
    out: &mut Option<ChildOutputReader>,
    err: &mut Option<ChildOutputReader>,
    log: &mut PluginCommandLog,
) -> io::Result<()> {
    // A settled Job has no writers. Bound even a faulty pipe implementation.
    for _ in 0..64 {
        drain(out, &mut log.stdout, &mut log.stdout_truncated, true)?;
        drain(err, &mut log.stderr, &mut log.stderr_truncated, true)?;
        if out.is_none() && err.is_none() {
            return Ok(());
        }
    }
    Err(io::ErrorKind::TimedOut.into())
}
fn drain(
    reader: &mut Option<ChildOutputReader>,
    kept: &mut Vec<u8>,
    truncated: &mut bool,
    settled: bool,
) -> io::Result<()> {
    let Some(pipe) = reader.as_mut() else {
        return Ok(());
    };
    let mut bytes = [0; 4096];
    for _ in 0..8 {
        match pipe.read(&mut bytes) {
            Ok(0) => {
                *reader = None;
                return Ok(());
            }
            Ok(count) => {
                let keep = count.min(MAXIMUM_PLUGIN_OUTPUT_BYTES.saturating_sub(kept.len()));
                kept.extend_from_slice(&bytes[..keep]);
                *truncated |= keep < count;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if settled {
                    *reader = None;
                }
                return Ok(());
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
fn program(value: &str, cwd: &Path) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        return path.into();
    }
    if value.contains(['/', '\\']) || cwd.join(path).is_file() {
        return cwd.join(path);
    }
    // Prefer executable resolution; a batch file is passed directly to Rust's
    // Windows Command adapter, which quotes its argv for cmd.exe safely.
    if let Some(paths) = std::env::var_os("PATH") {
        let extensions =
            std::env::var_os("PATHEXT").unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
        for dir in std::env::split_paths(&paths).filter(|p| p.is_absolute()) {
            if path.extension().is_some() {
                let candidate = dir.join(path);
                if candidate.is_file() {
                    return candidate;
                }
            } else {
                for ext in extensions.to_string_lossy().split(';').filter(|s| {
                    s.starts_with('.') && s[1..].bytes().all(|c| c.is_ascii_alphanumeric())
                }) {
                    let candidate = dir.join(format!("{value}{ext}"));
                    if candidate.is_file() {
                        return candidate;
                    }
                }
            }
        }
    }
    path.into()
}
