//! Windows provider lookup uses a killable Job and a fixed system executable.
use super::{POLL, TEARDOWN, stopped};
use process_wrap::std::{ChildWrapper, CommandWrap, CreationFlags, JobObject, KillOnJobClose};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::AtomicBool,
    thread,
    time::Instant,
};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;
pub(super) fn directory() -> std::io::Result<PathBuf> {
    winsafe::GetSystemDirectory()
        .map(PathBuf::from)
        .map_err(std::io::Error::other)
}
/// The Job's kill-on-close policy is the final process-tree ownership guard.
/// Normal paths additionally request termination and confirm Job exit.
struct OwnedJob(Box<dyn ChildWrapper>);

impl Drop for OwnedJob {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}

pub(super) fn query(
    executable: &Path,
    system: &Path,
    path: Option<&OsStr>,
    name: &str,
    cancelled: &AtomicBool,
    run_deadline: Instant,
    slot_deadline: Instant,
) -> Result<bool, ()> {
    if stopped(cancelled, run_deadline) {
        return Err(());
    }
    let Some(root) = system.parent() else {
        return Err(());
    };
    let mut command = Command::new(executable);
    command
        .current_dir(system)
        .env_clear()
        .env("SystemRoot", root)
        .env("PATH", path.unwrap_or_else(|| OsStr::new("")))
        .env("PATHEXT", ".EXE;.COM;.CMD;.BAT;.PS1")
        // `$PATH:` excludes the helper's working directory. `/Q` makes the
        // output bound exactly zero; only the documented exit code is read.
        .arg("/Q")
        .arg(format!("$PATH:{name}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut wrapped = CommandWrap::from(command);
    wrapped
        .wrap(CreationFlags(CREATE_NO_WINDOW))
        .wrap(KillOnJobClose)
        .wrap(JobObject);
    if stopped(cancelled, run_deadline) {
        return Err(());
    }
    let mut child = match wrapped.spawn() {
        Ok(child) => OwnedJob(child),
        Err(_) => return Err(()),
    };
    drop(wrapped);

    loop {
        if stopped(cancelled, run_deadline) {
            break;
        }
        match child.0.try_wait() {
            Ok(Some(status)) => {
                return match status.code() {
                    Some(0) => Ok(true),
                    Some(1) => Ok(false),
                    _ => Err(()),
                };
            }
            Ok(None) => {
                thread::sleep(POLL.min(run_deadline.saturating_duration_since(Instant::now())))
            }
            Err(_) => break,
        }
    }

    let _ = child.0.start_kill();
    let teardown_deadline = slot_deadline.min(
        Instant::now()
            .checked_add(TEARDOWN)
            .unwrap_or(slot_deadline),
    );
    loop {
        match child.0.try_wait() {
            Ok(Some(_)) => return Err(()),
            Ok(None) if Instant::now() < teardown_deadline => {
                thread::sleep(POLL.min(teardown_deadline.saturating_duration_since(Instant::now())))
            }
            _ => return Err(()),
        }
    }
}
