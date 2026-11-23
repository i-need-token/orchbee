//! The Windows adapter: the process is held open with the rights to query and wait, so its ID names
//! no other process while the holder decides about it. Opening decides identity with those rights
//! alone, as [observation](crate::process_observation) does, so an ID that another account's or a
//! protected process now holds decides nothing about the recorded one; the right to end is asked for
//! only to end a process that matched, or by a holder that may have to end it later.
use super::blocked;
use crate::{
    BlockerRole, process_observation,
    process_registration::{ProcessIdentity, ProcessObservation},
};
use std::{io, time::Instant};
use winsafe::{HPROCESS, co, guard::CloseHandleGuard};

/// The live process a recorded identity names, held open so that its ID names no other process
/// while the holder decides about it.
pub struct ExactProcess {
    pub(super) identity: ProcessIdentity,
    pub(super) role: BlockerRole,
    /// The rights to query and wait.
    handle: CloseHandleGuard<HPROCESS>,
    /// The right to end it, held from [`Self::endable`] on.
    end: Option<CloseHandleGuard<HPROCESS>>,
}

impl ExactProcess {
    /// Opens the live process `identity` names, started from `role`'s program, or `None` when none
    /// does: the ID is free, its process ended, or the ID now names a process created at another
    /// time or, when this account may not open it, one running another program. A process running
    /// that program that this account may not open cannot be proven gone and fails, naming it.
    pub(crate) fn open(identity: ProcessIdentity, role: BlockerRole) -> io::Result<Option<Self>> {
        match process_observation::observe(identity, role.program()) {
            (ProcessObservation::Present(observed), Some(handle)) if observed == identity => {
                Ok(Some(Self {
                    identity,
                    role,
                    handle,
                    end: None,
                }))
            }
            (ProcessObservation::Unknown, _) => {
                Err(blocked(io::ErrorKind::PermissionDenied, role, identity))
            }
            _ => Ok(None),
        }
    }

    /// This process with the right to end it held from now on, for a holder that may have to end
    /// it later. A process this account may not end fails, naming it.
    pub fn endable(mut self) -> io::Result<Self> {
        let end = self
            .end_right()
            .map_err(|error| blocked(error.kind(), self.role, self.identity))?;
        self.end = Some(end);
        Ok(self)
    }

    /// The full path of the program the process runs.
    pub fn image_path(&self) -> io::Result<std::path::PathBuf> {
        self.handle
            .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
            .map(Into::into)
            .map_err(system)
    }

    /// Whether the process has exited.
    pub fn exited(&self) -> io::Result<bool> {
        match self.handle.WaitForSingleObject(Some(0)).map_err(system)? {
            co::WAIT::OBJECT_0 => Ok(true),
            co::WAIT::TIMEOUT => Ok(false),
            _ => Err(io::Error::other("unexpected process wait result")),
        }
    }

    /// Ends the process, suspended, hung or running, and waits until it has exited or `deadline`
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
        let asked;
        let end = match &self.end {
            Some(end) => end,
            None => match self.end_right() {
                Ok(end) => {
                    asked = end;
                    &asked
                }
                // The process can exit between the wait above and the request.
                Err(_) if self.exited()? => return Ok(()),
                Err(error) => return Err(error),
            },
        };
        if let Err(error) = end.TerminateProcess(1) {
            return if self.exited()? {
                Ok(())
            } else {
                Err(system(error))
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
            self.handle
                .WaitForSingleObject(Some(remaining.as_millis().clamp(1, 100) as u32))
                .map_err(system)?;
        }
    }

    /// The right to end this process. Its open handle keeps the ID from naming another process,
    /// and the identity is compared again anyway.
    fn end_right(&self) -> io::Result<CloseHandleGuard<HPROCESS>> {
        let end = HPROCESS::OpenProcess(
            co::PROCESS::QUERY_LIMITED_INFORMATION | co::PROCESS::TERMINATE,
            false,
            self.identity.pid(),
        )
        .map_err(system)?;
        // Not InvalidData, which a restart's reservation keeps for a record naming no running
        // generation.
        if process_observation::identity(&end)? != self.identity {
            return Err(io::Error::other("the process ID names another process"));
        }
        Ok(end)
    }
}

/// A failed system call as the I/O error of its code.
fn system(error: co::ERROR) -> io::Error {
    io::Error::from_raw_os_error(error.raw() as i32)
}
