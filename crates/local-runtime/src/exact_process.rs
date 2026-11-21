//! A process opened on a recorded exact identity, its process ID and creation time, with the right
//! to end it. A Session record names such a process outside its registrations only in the
//! successor entry of a Server restart, and whoever removes that entry ends what it names through
//! this handle first. Opening ends nothing, and an identity no live process has opens nothing.
use crate::{Successor, process_registration::ProcessIdentity};
use std::{io, time::Instant};
use winsafe::{HPROCESS, co, guard::CloseHandleGuard};

/// The live process a recorded identity names, held open so that its ID names no other process
/// while the holder decides about it.
pub struct ExactProcess {
    handle: CloseHandleGuard<HPROCESS>,
}

impl ExactProcess {
    /// Opens the live process `identity` names, or `None` when none does: the ID is free, its
    /// process ended, or the ID now names a process created at another time. A protected system ID,
    /// or a process that cannot be opened for another reason, is an error, since it cannot be
    /// proven gone.
    pub(crate) fn open(identity: ProcessIdentity) -> io::Result<Option<Self>> {
        if identity.pid() <= 4 {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let handle = match HPROCESS::OpenProcess(
            co::PROCESS::QUERY_LIMITED_INFORMATION
                | co::PROCESS::SYNCHRONIZE
                | co::PROCESS::TERMINATE,
            false,
            identity.pid(),
        ) {
            Ok(handle) => handle,
            Err(error) if error == co::ERROR::INVALID_PARAMETER => return Ok(None),
            Err(error) => return Err(system(error)),
        };
        let process = Self { handle };
        if process.exited()? || crate::process_observation::identity(&process.handle)? != identity {
            return Ok(None);
        }
        Ok(Some(process))
    }

    /// The successor Server `entry` names, opened on its recorded identity, or `None` once it is
    /// gone.
    pub fn successor_server(entry: &Successor) -> io::Result<Option<Self>> {
        Self::open(entry.server())
    }

    /// The successor's Worker `entry` names, opened on its recorded identity, or `None` when none
    /// is recorded or it is gone.
    pub fn successor_worker(entry: &Successor) -> io::Result<Option<Self>> {
        entry.worker().map_or(Ok(None), Self::open)
    }

    /// Ends every live process `entry` names by its recorded identity, the Worker first, and waits
    /// for each until `deadline`. An identity no live process has is gone, and nothing is ended
    /// for it; a process that cannot be opened or ended fails.
    pub fn end_successor(entry: &Successor, deadline: Instant) -> io::Result<()> {
        if let Some(worker) = Self::successor_worker(entry)? {
            worker.end_before(deadline)?;
        }
        if let Some(server) = Self::successor_server(entry)? {
            server.end_before(deadline)?;
        }
        Ok(())
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
    /// passes. A process that already exited counts as ended.
    pub fn end_before(&self, deadline: Instant) -> io::Result<()> {
        if self.exited()? {
            return Ok(());
        }
        if let Err(error) = self.handle.TerminateProcess(1) {
            // The process can exit between the wait above and the termination.
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
}

/// A failed system call as the I/O error of its code.
fn system(error: co::ERROR) -> io::Error {
    io::Error::from_raw_os_error(error.raw() as i32)
}
