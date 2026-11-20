//! Retains one Unix command's process group through root and descendant settlement.
//!
//! The root leads a new process group, so every descendant that stays in the group is signalled
//! with it. The root is observed with `WNOWAIT` and reaped only after the group's last signal: its
//! unreaped entry keeps its process ID, and with it the group ID, from being given to another
//! process while the group is still being signalled.
//!
//! A root started by the standard library is reaped through its `Child`; one started by the
//! `posix_spawn` launch in `spawn.rs`, which clears the signal mask the standard library passes on,
//! is reaped by its process ID.
//!
//! `DetachedChild`, for a child that must outlive its owner, is the one owner that signals and
//! reaps nothing on drop.
use std::{
    io,
    os::{
        fd::OwnedFd,
        unix::process::{CommandExt, ExitStatusExt},
    },
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
#[cfg(target_os = "macos")]
use crate::retained_spawn_native;

mod capture;
#[cfg(target_os = "macos")]
mod native;
mod spawn;
pub use capture::{run_unix_command, run_unix_command_exchange, run_unmasked_unix_command};
pub use spawn::SpawnEnvironment;

/// The cleanup budget the Windows Job owner also applies, counted from the first settle.
const SETTLE_BUDGET: Duration = Duration::from_secs(5);
/// Part of the budget a running root gets after `SIGTERM`, so Git can remove its lock files and a
/// partly created checkout, before the group is killed.
const TERM_GRACE: Duration = Duration::from_secs(2);
const POLL_SLICE: Duration = Duration::from_millis(10);
/// How often a root wait looks again; a long-lived root, such as a machine carrier, costs little.
const ROOT_POLL: Duration = Duration::from_millis(50);

enum Settlement {
    Running,
    Failed,
    Complete(ExitStatus),
}

pub struct CommandChild {
    /// The standard library's handle of the root; none for a root the `posix_spawn` launch started.
    child: Option<Child>,
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
            child: Some(child),
            reaped: false,
            settlement: Settlement::Running,
        })
    }

    /// Starts `command` with `streams` as its standard input, output and error, as the leader of a
    /// new process group, in the environment `environment` names. On macOS the child starts through
    /// the native adapter, so it inherits no descriptor but those three, however many threads of this
    /// process create descriptors meanwhile. Elsewhere the standard library's spawn starts the same
    /// child, since Linux creates its close-on-exec descriptors atomically.
    pub fn spawn_with_streams(
        command: Command,
        environment: SpawnEnvironment,
        streams: [OwnedFd; 3],
    ) -> Result<Self, CommandSpawnFailure> {
        #[cfg(target_os = "macos")]
        {
            native::spawn(&command, environment, streams)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let mut command = command;
            if environment == SpawnEnvironment::Cleared {
                // The child starts from exactly the variables the command sets, as the adapter
                // starts it, so the two platforms agree when the caller clears its environment.
                let set: Vec<_> = command
                    .get_envs()
                    .filter_map(|(name, value)| Some((name.to_owned(), value?.to_owned())))
                    .collect();
                command.env_clear().envs(set);
            }
            let [input, output, errors] = streams;
            command.stdin(input).stdout(output).stderr(errors);
            Self::spawn(command)
        }
    }

    /// The owner of a root the `posix_spawn` launch started as the leader of its own group.
    fn spawned(root: Pid) -> Self {
        Self {
            child: None,
            group: root,
            reaped: false,
            settlement: Settlement::Running,
        }
    }

    pub fn id(&self) -> u32 {
        self.group.as_raw_nonzero().get().cast_unsigned()
    }

    /// Waits up to `timeout` for the root to exit, as the Windows owner's root wait does, without
    /// reaping it or signalling its group, so the group ID stays pinned for another thread that
    /// signals the group. True once the root has exited. Runs only on the retained command owner
    /// thread, before [`Self::settle`].
    pub fn wait_root_exit(&mut self, timeout: Duration) -> io::Result<bool> {
        let deadline = Instant::now() + timeout;
        loop {
            if self.root_exited()? {
                return Ok(true);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(false);
            }
            thread::sleep(remaining.min(ROOT_POLL));
        }
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
            let status = match self.child.as_mut() {
                Some(child) => child.try_wait()?,
                None => waitpid(Some(self.group), WaitOptions::NOHANG)?
                    .map(|(_, status)| ExitStatus::from_raw(status.as_raw())),
            };
            if let Some(status) = status {
                self.reaped = true;
                return Ok(status);
            }
            sleep_before(deadline)?;
        }
    }
}

/// A command started on macOS through the native adapter, for a child its owner may leave running:
/// it leads a new process group and inherits no descriptor but its three standard streams, as
/// [`CommandChild::spawn_with_streams`] starts it. Dropping it signals nothing and reaps nothing, so
/// the child keeps running; a child not yet waited for stays a zombie until this process exits, as
/// with the standard library's `Child`.
#[cfg(target_os = "macos")]
pub struct DetachedChild {
    process: u32,
    status: Option<ExitStatus>,
}

#[cfg(target_os = "macos")]
impl DetachedChild {
    /// Starts `command` with `streams` as its standard input, output and error, in the environment
    /// `environment` names and in the working directory it sets, or this process's when it sets
    /// none. A relative program is refused.
    pub fn spawn_with_streams(
        command: Command,
        environment: SpawnEnvironment,
        streams: [OwnedFd; 3],
    ) -> io::Result<Self> {
        let process = native::spawn_detached(&command, environment, streams)?;
        Ok(Self {
            process,
            status: None,
        })
    }

    /// The child's process identifier, which is also the identifier of its group.
    pub fn id(&self) -> u32 {
        self.process
    }

    /// The child's status once it has ended, or `None` while it runs. Keeps the first status read.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if let Some(status) = self.status {
            return Ok(Some(status));
        }
        let Some(raw) = retained_spawn_native::wait(self.process, false)? else {
            return Ok(None);
        };
        let status = ExitStatus::from_raw(raw);
        self.status = Some(status);
        Ok(Some(status))
    }

    /// Waits until the child ends and answers its status. Keeps the first status read.
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        if let Some(status) = self.status {
            return Ok(status);
        }
        let raw = retained_spawn_native::wait(self.process, true)?
            .ok_or_else(|| io::Error::other("a blocking wait reported no status"))?;
        let status = ExitStatus::from_raw(raw);
        self.status = Some(status);
        Ok(status)
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
