//! Retains the exact Windows process handle for exit observation and explicit deadline-bounded
//! termination, including suspended startup support. Dropping the handle does not terminate
//! the Session, and descendant ownership stays with the caller.

use std::{
    ffi::OsStr, io, os::windows::process::ExitStatusExt, path::Path, process::ExitStatus,
    time::Instant,
};
use winsafe::{co, guard::CloseHandlePiGuard};

/// Windows reclaims exited processes without a Unix-style reaper. Dropping
/// this handle releases observation ownership and never terminates the Session.
pub struct RetainedChild(CloseHandlePiGuard);

/// Where a child asked to leave its creator's Job was created.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobPlacement {
    /// Outside the creator's Job, or the creator ran in none.
    Outside,
    /// Inside the creator's Job, which refused the departure with this error.
    Inside(io::ErrorKind),
}
impl RetainedChild {
    /// Terminates this exact process and waits for exit; descendants remain caller-owned.
    pub fn terminate_before(&mut self, deadline: Instant) -> io::Result<ExitStatus> {
        if let Some(status) = self.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        if let Err(error) = self.0.hProcess.TerminateProcess(1) {
            // The child can exit between the initial wait and termination.
            return self.try_wait()?.ok_or_else(|| system(error));
        }
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            self.0
                .hProcess
                .WaitForSingleObject(Some(remaining.as_millis().clamp(1, 100) as u32))
                .map_err(system)?;
        }
    }

    pub(crate) fn spawn(executable: &Path, arguments: &[&OsStr]) -> io::Result<Self> {
        Self::create(executable, arguments, co::CREATE::NoValue)
    }
    pub(crate) fn spawn_suspended(executable: &Path, arguments: &[&OsStr]) -> io::Result<Self> {
        Self::create(executable, arguments, co::CREATE::SUSPENDED)
    }
    /// [`Self::spawn_suspended`] outside the Job this process runs in, and outside each Job above
    /// it that also lets a process leave. A Job that keeps its processes refuses the request; the
    /// child is then created as [`Self::spawn_suspended`] creates it, inside that Job.
    pub(crate) fn spawn_suspended_outside_job(
        executable: &Path,
        arguments: &[&OsStr],
    ) -> io::Result<(Self, JobPlacement)> {
        match Self::create(
            executable,
            arguments,
            co::CREATE::SUSPENDED | co::CREATE::BREAKAWAY_FROM_JOB,
        ) {
            Ok(child) => Ok((child, JobPlacement::Outside)),
            Err(refused) => Self::spawn_suspended(executable, arguments)
                .map(|child| (child, JobPlacement::Inside(refused.kind()))),
        }
    }
    pub(crate) fn identity(&self) -> io::Result<crate::process_registration::ProcessIdentity> {
        crate::process_observation::identity(&self.0.hProcess)
    }
    pub(crate) fn resume(&mut self) -> io::Result<()> {
        match self.0.hThread.ResumeThread().map_err(system)? {
            1 => Ok(()),
            _ => Err(io::Error::other(
                "unexpected registered child suspend count",
            )),
        }
    }
    fn create(executable: &Path, arguments: &[&OsStr], flags: co::CREATE) -> io::Result<Self> {
        if !executable.is_absolute() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let program = text(executable.as_os_str())?;
        let mut command = quote(program);
        for argument in arguments {
            command.push(' ');
            command.push_str(&quote(text(argument)?));
        }
        if command.encode_utf16().count() >= 32_767 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        // A process keeps its working directory in use, so a child that outlives the activating
        // Client must not inherit a directory the person may later remove, such as a Worktree.
        let directory = winsafe::GetSystemDirectory().map_err(system)?;
        // bInheritHandles=false is essential: even when stdio is redirected to
        // NUL, a generic inheriting spawn can retain a caller's capture pipe.
        winsafe::CreateProcess(
            Some(program),
            Some(&command),
            None,
            None,
            false,
            co::CREATE::DETACHED_PROCESS | co::CREATE::NEW_PROCESS_GROUP | flags,
            &[],
            Some(&directory),
            &mut winsafe::STARTUPINFO::default(),
        )
        .map(Self)
        .map_err(system)
    }
    /// The held process's exact identity: its process ID and its creation time as a `FILETIME`,
    /// which a peer that checks processes by both, such as the Keeper, compares.
    ///
    /// # Errors
    ///
    /// Returns an error when the creation time cannot be read.
    pub fn exact_identity(&self) -> io::Result<(u32, u64)> {
        self.identity()
            .map(|identity| (identity.pid(), identity.creation_time()))
    }
    /// The operating-system process identifier of the held process.
    pub fn id(&self) -> Option<u32> {
        Some(self.0.dwProcessId)
    }
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match self
            .0
            .hProcess
            .WaitForSingleObject(Some(0))
            .map_err(system)?
        {
            co::WAIT::TIMEOUT => Ok(None),
            co::WAIT::OBJECT_0 => self
                .0
                .hProcess
                .GetExitCodeProcess()
                .map(|status| Some(ExitStatus::from_raw(status)))
                .map_err(system),
            _ => Err(io::Error::other("unexpected retained process wait result")),
        }
    }
}
/// A failed system call as the I/O error of its code, so callers and diagnostics read its kind and
/// operating-system error code instead of an uncoded wrapper.
fn system(error: co::ERROR) -> io::Error {
    io::Error::from_raw_os_error(error.raw() as i32)
}
fn text(value: &OsStr) -> io::Result<&str> {
    value
        .to_str()
        .filter(|text| !text.contains('\0'))
        .ok_or_else(|| io::ErrorKind::InvalidInput.into())
}
// Microsoft CRT argv quoting: every argument is quoted, backslashes double
// before a quote or the closing quote. There is no command shell evaluation.
fn quote(value: &str) -> String {
    let mut output = String::from("\"");
    let mut slashes = 0;
    for character in value.chars() {
        if character == '\\' {
            slashes += 1;
            continue;
        }
        output.extend(std::iter::repeat_n(
            '\\',
            if character == '"' {
                2 * slashes + 1
            } else {
                slashes
            },
        ));
        slashes = 0;
        output.push(character);
    }
    output.extend(std::iter::repeat_n('\\', 2 * slashes));
    output.push('"');
    output
}

#[cfg(test)]
mod tests;
