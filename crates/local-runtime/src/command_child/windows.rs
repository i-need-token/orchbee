//! Retains one Windows command's Job through root and descendant settlement.
use std::{
    io,
    process::{Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};

use ::windows::Win32::System::Threading::CREATE_NO_WINDOW;
use process_wrap::std::{ChildWrapper, CommandWrap, CreationFlags, JobObject};

mod capture;
mod program;
pub use capture::{run_windows_command, run_windows_command_exchange};
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
        loop {
            if let Some(status) = self.child.try_wait()? {
                self.settlement = Settlement::Complete(status);
                return Ok(status);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(remaining.min(Duration::from_millis(10)));
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
