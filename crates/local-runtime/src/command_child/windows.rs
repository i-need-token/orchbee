//! Retains one Windows command's Job through root and descendant settlement.
use std::{
    io,
    process::{Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};

use ::windows::Win32::System::Threading::CREATE_NO_WINDOW;
use process_wrap::std::{ChildWrapper, CommandWrap, CreationFlags, JobObject, KillOnJobClose};
use winsafe::{HPROCESS, co};

mod capture;
mod program;
pub use capture::{
    run_contained_command_exchange, run_windows_command, run_windows_command_exchange,
};
pub use program::{resolve_windows_program, windows_system_program};

enum Settlement {
    Running,
    Failed,
    Complete(ExitStatus),
}

pub struct CommandChild {
    child: Box<dyn ChildWrapper>,
    settlement: Settlement,
}

use super::CommandSpawnFailure;

impl CommandChild {
    /// Assignment happens while suspended, before any command can spawn a child.
    pub fn spawn(command: Command) -> Result<Self, CommandSpawnFailure> {
        let mut wrapped = CommandWrap::from(command);
        wrapped
            .wrap(CreationFlags(CREATE_NO_WINDOW))
            .wrap(JobObject);
        Self::from_wrapped(wrapped)
    }

    /// As [`Self::spawn`], and the Job also ends every member when its last handle closes. Only
    /// this owner holds that handle, so the owning process's end, an exit, a kill or an abort that
    /// runs no destructor included, ends the command and everything it started.
    pub fn spawn_contained(command: Command) -> Result<Self, CommandSpawnFailure> {
        let mut wrapped = CommandWrap::from(command);
        wrapped
            .wrap(CreationFlags(CREATE_NO_WINDOW))
            .wrap(JobObject)
            .wrap(KillOnJobClose);
        Self::from_wrapped(wrapped)
    }

    fn from_wrapped(mut wrapped: CommandWrap) -> Result<Self, CommandSpawnFailure> {
        let mut created = false;
        let child = wrapped.spawn_with(|command| {
            let child = command.spawn()?;
            created = true;
            Ok(child)
        });
        // Release duplicate parent pipe writers before the caller drains output.
        drop(wrapped);
        let child = child.map_err(|error| CommandSpawnFailure { error, created })?;
        Ok(Self {
            child,
            settlement: Settlement::Running,
        })
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Runs only on the retained command owner thread. A root exit is not completion
    /// until the owned Job has settled, even when a descendant holds output open.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match self.settlement {
            Settlement::Complete(status) => return Ok(Some(status)),
            Settlement::Failed => return Err(unknown_settlement()),
            Settlement::Running => {}
        }
        // Root polling never consumes the private completion port. We terminate
        // remaining descendants before asking the corrected wrapper for all exit.
        if self.child.inner_mut().try_wait()?.is_none() {
            return Ok(None);
        }
        self.settle().map(Some)
    }

    /// Waits at most `timeout` for the root process to exit and wakes at that exit. It reports
    /// root exit only: `try_wait` still owns the Job settlement and the exit status.
    pub fn wait_root_exit(&mut self, timeout: Duration) -> io::Result<bool> {
        if !matches!(self.settlement, Settlement::Running) {
            return Ok(true);
        }
        self.wait_root(timeout)
    }

    fn wait_root(&mut self, timeout: Duration) -> io::Result<bool> {
        if self.child.inner_mut().try_wait()?.is_some() {
            return Ok(true);
        }
        // The retained root handle keeps this process ID from being reused while we wait.
        let root = HPROCESS::OpenProcess(co::PROCESS::SYNCHRONIZE, false, self.child.id())
            .map_err(io::Error::other)?;
        let milliseconds = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX - 1);
        Ok(root
            .WaitForSingleObject(Some(milliseconds))
            .map_err(io::Error::other)?
            == co::WAIT::OBJECT_0)
    }

    /// Terminates remaining members, then observes the corrected wrapper's
    /// root-and-Job completion with a bounded cleanup budget.
    pub fn settle(&mut self) -> io::Result<ExitStatus> {
        match self.settlement {
            Settlement::Complete(status) => return Ok(status),
            Settlement::Failed => return Err(unknown_settlement()),
            Settlement::Running => {}
        }
        // A failed attempt must never become success on a later cached-root read.
        self.settlement = Settlement::Failed;
        self.child.start_kill()?;
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut root_exited = false;
        loop {
            if let Some(status) = self.child.try_wait()? {
                self.settlement = Settlement::Complete(status);
                return Ok(status);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            // Sleep only while the Job's final notification trails the root's exit; each
            // read drains every notification already queued.
            if root_exited {
                thread::sleep(remaining.min(Duration::from_millis(1)));
            } else {
                root_exited = self.wait_root(remaining)?;
            }
        }
    }
}

impl Drop for CommandChild {
    fn drop(&mut self) {
        if !matches!(self.settlement, Settlement::Complete(_)) {
            // Best effort only: explicit settle owns every successful receipt.
            // Drop never hides a blocking wait or upgrades unknown settlement.
            let _ = self.child.start_kill();
        }
    }
}

fn unknown_settlement() -> io::Error {
    io::Error::other("Windows command Job settlement is unknown")
}

#[cfg(test)]
mod tests;
