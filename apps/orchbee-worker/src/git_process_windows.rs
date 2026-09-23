//! Retains one Windows Git command's Job through root and descendant settlement.
use std::{
    io,
    process::{Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};

use process_wrap::std::{ChildWrapper, CommandWrap, CreationFlags, JobObject};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

enum Settlement {
    Running,
    Failed,
    Complete(ExitStatus),
}

pub(crate) struct GitChild {
    child: Box<dyn ChildWrapper>,
    settlement: Settlement,
}

impl GitChild {
    /// Assignment happens while suspended, before any Git hook can spawn a child.
    pub(crate) fn spawn(command: Command) -> io::Result<Self> {
        let mut wrapped = CommandWrap::from(command);
        wrapped
            .wrap(CreationFlags(CREATE_NO_WINDOW))
            .wrap(JobObject);
        let child = wrapped.spawn()?;
        // Release duplicate parent pipe writers before the caller drains output.
        drop(wrapped);
        Ok(Self {
            child,
            settlement: Settlement::Running,
        })
    }

    pub(crate) fn id(&self) -> u32 {
        self.child.id()
    }

    /// Runs only on the retained Git owner thread. A root exit is not completion
    /// until the owned Job has settled, even when a descendant holds output open.
    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
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
    pub(crate) fn settle(&mut self) -> io::Result<ExitStatus> {
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

impl Drop for GitChild {
    fn drop(&mut self) {
        if !matches!(self.settlement, Settlement::Complete(_)) {
            // Best effort only: explicit settle owns every successful receipt.
            // Drop never hides a blocking wait or upgrades unknown settlement.
            let _ = self.child.start_kill();
        }
    }
}

fn unknown_settlement() -> io::Error {
    io::Error::other("Git Job settlement is unknown")
}

#[cfg(test)]
mod tests;
