//! Retains one Unix command's process group through root and descendant settlement.
//!
//! The root leads a new process group, so every descendant that stays in the group is signalled
//! with it. The root is observed with `WNOWAIT` and reaped only after the group's last signal: its
//! unreaped entry keeps its process ID, and with it the group ID, from being given to another
//! process while the group is still being signalled.
use std::{
    io,
    os::unix::process::CommandExt,
    process::{Child, Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};

use rustix::{
    io::Errno,
    process::{
        Pid, Signal, WaitId, WaitIdOptions, WaitOptions, kill_process_group,
        test_kill_process_group, waitid, waitpid,
    },
};

use super::CommandSpawnFailure;

/// The cleanup budget the Windows Job owner also applies, counted from the first settle.
const SETTLE_BUDGET: Duration = Duration::from_secs(5);
/// Part of the budget a running root gets after `SIGTERM`, so Git can remove its lock files and a
/// partly created checkout, before the group is killed.
const TERM_GRACE: Duration = Duration::from_secs(2);
const POLL_SLICE: Duration = Duration::from_millis(10);

enum Settlement {
    Running,
    Failed,
    Complete(ExitStatus),
}

pub struct CommandChild {
    child: Child,
    group: Pid,
    reaped: bool,
    settlement: Settlement,
}

impl CommandChild {
    /// The group exists before the program runs, so no descendant can start outside it.
    pub fn spawn(mut command: Command) -> Result<Self, CommandSpawnFailure> {
        command.process_group(0);
        // The standard library reaps a child whose program failed to start before it returns
        // the error, so a failed spawn never leaves a process behind.
        let child = command.spawn().map_err(|error| CommandSpawnFailure {
            error,
            created: false,
        })?;
        // Release the parent's copies of the child's output writers before the caller drains.
        drop(command);
        Ok(Self {
            group: Pid::from_child(&child),
            child,
            reaped: false,
            settlement: Settlement::Running,
        })
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Runs only on the retained command owner thread. A root exit is not completion until the
    /// rest of its group has been killed and has left the group.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match self.settlement {
            Settlement::Complete(status) => return Ok(Some(status)),
            Settlement::Failed => return Err(unknown_settlement()),
            Settlement::Running => {}
        }
        if !self.root_exited()? {
            return Ok(None);
        }
        self.settle().map(Some)
    }

    /// Asks a running root to stop, kills the remaining group, reaps the root and waits until no
    /// member is left, all within one bounded cleanup budget.
    pub fn settle(&mut self) -> io::Result<ExitStatus> {
        match self.settlement {
            Settlement::Complete(status) => return Ok(status),
            Settlement::Failed => return Err(unknown_settlement()),
            Settlement::Running => {}
        }
        // A failed attempt must never become success on a later cached-root read.
        self.settlement = Settlement::Failed;
        let deadline = Instant::now() + SETTLE_BUDGET;
        if !self.root_exited()? {
            signal_group(self.group, Signal::TERM)?;
            let grace = Instant::now() + TERM_GRACE;
            while Instant::now() < grace && !self.root_exited()? {
                thread::sleep(POLL_SLICE);
            }
        }
        signal_group(self.group, Signal::KILL)?;
        let status = self.reap(deadline)?;
        // Once the root is reaped no further signal is sent: the group ID may be given away
        // after the last member leaves. Members killed above are reaped by the system.
        loop {
            match test_kill_process_group(self.group) {
                Err(Errno::SRCH) => break,
                Ok(()) | Err(Errno::PERM) => {}
                Err(error) => return Err(error.into()),
            }
            sleep_before(deadline)?;
        }
        self.settlement = Settlement::Complete(status);
        Ok(status)
    }

    fn root_exited(&self) -> io::Result<bool> {
        let options = WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT;
        match waitid(WaitId::Pid(self.group), options) {
            Ok(status) => Ok(status.is_some()),
            Err(Errno::INTR) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn reap(&mut self, deadline: Instant) -> io::Result<ExitStatus> {
        loop {
            if let Some(status) = self.child.try_wait()? {
                self.reaped = true;
                return Ok(status);
            }
            sleep_before(deadline)?;
        }
    }
}

impl Drop for CommandChild {
    fn drop(&mut self) {
        if matches!(self.settlement, Settlement::Complete(_)) || self.reaped {
            return;
        }
        // Best effort only: explicit settle owns every successful receipt, and Drop never hides
        // a blocking wait. The unreaped root still reserves the group ID for this signal.
        let _ = kill_process_group(self.group, Signal::KILL);
        // A detached waiter reaps the root, so no zombie outlives its owner.
        let group = self.group;
        let _ = thread::Builder::new()
            .name("orchbee-command-reaper".into())
            .spawn(move || waitpid(Some(group), WaitOptions::empty()));
    }
}

/// A group whose members have all exited has nothing left to signal. Linux accepts the signal for
/// the unreaped root; macOS refuses a group with no live member with `EPERM`, as the membership
/// test in `settle` already reads it. Settlement is decided by that test after the reap, which
/// waits within the budget for any member a signal could not reach.
fn signal_group(group: Pid, signal: Signal) -> io::Result<()> {
    match kill_process_group(group, signal) {
        Ok(()) | Err(Errno::SRCH | Errno::PERM) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn sleep_before(deadline: Instant) -> io::Result<()> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(io::ErrorKind::TimedOut.into());
    }
    thread::sleep(remaining.min(POLL_SLICE));
    Ok(())
}

fn unknown_settlement() -> io::Error {
    io::Error::other("Unix command process group settlement is unknown")
}

#[cfg(test)]
mod tests;
