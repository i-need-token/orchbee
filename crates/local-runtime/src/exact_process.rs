//! A process opened on a recorded exact identity, its process ID and creation time. A Session record
//! names such a process outside its registrations only in the successor entry of a Server restart,
//! and whoever removes that entry ends what it names through this handle first. Opening ends
//! nothing and decides identity with the rights to query and wait alone, as
//! [observation](crate::process_observation) does, so an ID that another account's or a protected
//! process now holds decides nothing about the recorded one; the right to end is asked for only to
//! end a process that matched, or by a holder that may have to end it later.
use crate::{
    BlockerRole, RecoveryBlocker, Successor, process_observation,
    process_registration::{ProcessIdentity, ProcessObservation},
};
use std::{io, time::Instant};
use winsafe::{HPROCESS, co, guard::CloseHandleGuard};

/// The live process a recorded identity names, held open so that its ID names no other process
/// while the holder decides about it.
pub struct ExactProcess {
    identity: ProcessIdentity,
    role: BlockerRole,
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

    /// The successor Server `entry` names, opened on its recorded identity, or `None` once it is
    /// gone.
    pub fn successor_server(entry: &Successor) -> io::Result<Option<Self>> {
        Self::open(entry.server(), BlockerRole::Server)
    }

    /// The successor's Worker `entry` names, opened on its recorded identity, or `None` when none
    /// is recorded or it is gone.
    pub fn successor_worker(entry: &Successor) -> io::Result<Option<Self>> {
        entry
            .worker()
            .map_or(Ok(None), |worker| Self::open(worker, BlockerRole::Worker))
    }

    /// Ends every live process `entry` names by its recorded identity, the Worker first, and waits
    /// for each until `deadline`. An identity no live process has is gone, and nothing is ended
    /// for it; a process that cannot be proven gone or ended fails, naming it.
    pub fn end_successor(entry: &Successor, deadline: Instant) -> io::Result<()> {
        if let Some(worker) = Self::successor_worker(entry)? {
            worker.end_before(deadline)?;
        }
        if let Some(server) = Self::successor_server(entry)? {
            server.end_before(deadline)?;
        }
        Ok(())
    }

    /// The recorded identity this process matched: its process ID and creation time.
    pub fn identity(&self) -> (u32, u64) {
        (self.identity.pid(), self.identity.creation_time())
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

/// A failure about the process `identity` names, which runs as `role`, carrying it so that a
/// refusal can name it.
fn blocked(kind: io::ErrorKind, role: BlockerRole, identity: ProcessIdentity) -> io::Error {
    io::Error::new(
        kind,
        RecoveryBlocker {
            role,
            process_id: identity.pid(),
        },
    )
}

/// A failed system call as the I/O error of its code.
fn system(error: co::ERROR) -> io::Error {
    io::Error::from_raw_os_error(error.raw() as i32)
}
