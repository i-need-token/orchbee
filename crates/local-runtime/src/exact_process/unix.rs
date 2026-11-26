//! The macOS and Linux adapter. Nothing holds a POSIX process ID, so the process's start time is
//! compared again right before each act on it; the window between that comparison and the signal is
//! the one every `kill` has. Opening decides identity from the ID and start time alone, as
//! [observation](crate::process_observation) does, so an ID that another account's process now
//! holds decides nothing about the recorded one.
use super::blocked;
#[cfg(target_os = "macos")]
use crate::process_identity_native as native;
#[cfg(target_os = "linux")]
use crate::process_identity_proc as native;
use crate::{
    BlockerRole, process_observation,
    process_registration::{ProcessIdentity, ProcessObservation},
};
use std::{
    io, thread,
    time::{Duration, Instant},
};

/// The live process a recorded identity names.
pub struct ExactProcess {
    pub(super) identity: ProcessIdentity,
    pub(super) role: BlockerRole,
}

impl ExactProcess {
    /// Opens the live process `identity` names, started from `role`'s program, or `None` when none
    /// does: the ID is free, its process ended, or the ID now names a process started at another
    /// time or, when it is another account's, one running another program. Another account's
    /// process running that program cannot be proven gone and fails, naming it.
    pub(crate) fn open(identity: ProcessIdentity, role: BlockerRole) -> io::Result<Option<Self>> {
        match process_observation::observe(identity, role.program()) {
            ProcessObservation::Present(observed) if observed == identity => {
                Ok(Some(Self { identity, role }))
            }
            ProcessObservation::Unknown => {
                Err(blocked(io::ErrorKind::PermissionDenied, role, identity))
            }
            _ => Ok(None),
        }
    }

    /// This process, for a holder that may have to end it later. Ending needs no right held in
    /// advance on macOS or Linux, so this only checks that it still runs as recorded.
    pub fn endable(self) -> io::Result<Self> {
        if self.exited()? {
            return Err(blocked(io::ErrorKind::NotFound, self.role, self.identity));
        }
        Ok(self)
    }

    /// The full path of the program the process runs.
    pub fn image_path(&self) -> io::Result<std::path::PathBuf> {
        native::program_path(self.identity.pid()).ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    /// Whether the process has exited: its ID is free, names a zombie, or names a later process.
    pub fn exited(&self) -> io::Result<bool> {
        match native::started(self.identity.pid())? {
            native::Started::At(start) => Ok(start != self.identity.creation_time()),
            native::Started::Gone => Ok(true),
            native::Started::Denied => Ok(true),
        }
    }

    /// Ends the process, stopped, hung or running, and waits until it has exited or `deadline`
    /// passes. A process that already exited counts as ended; one that cannot be ended, or does
    /// not exit in time, fails, naming it.
    pub fn end_before(&self, deadline: Instant) -> io::Result<()> {
        self.end_within(deadline)
            .map_err(|error| blocked(error.kind(), self.role, self.identity))
    }

    fn end_within(&self, deadline: Instant) -> io::Result<()> {
        if self.exited()? {
            return Ok(());
        }
        let pid = nix::unistd::Pid::from_raw(
            i32::try_from(self.identity.pid())
                .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?,
        );
        if let Err(error) = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL) {
            return if self.exited()? {
                Ok(())
            } else {
                Err(error.into())
            };
        }
        loop {
            if self.exited()? {
                return Ok(());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(remaining.min(Duration::from_millis(10)));
        }
    }
}
