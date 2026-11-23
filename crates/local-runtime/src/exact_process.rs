//! A process opened on a recorded exact identity, its process ID and creation time, or on macOS its
//! start time. A Session record names such a process outside its registrations only in the successor
//! entry of a Server restart, and whoever removes that entry ends what it names through this first.
//! Opening ends nothing.
use crate::{BlockerRole, RecoveryBlocker, Successor, process_registration::ProcessIdentity};
use std::{io, time::Instant};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "macos")]
pub use macos::ExactProcess;
#[cfg(windows)]
pub use windows::ExactProcess;

impl ExactProcess {
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

    /// Ends the Terminal Host `process` names by its ID and start time, and waits until it exited or
    /// `deadline` passes. An identity no live process has is gone, and nothing is ended for it.
    ///
    /// # Errors
    ///
    /// Returns an error when the process cannot be proven gone or ended, naming it.
    #[cfg(target_os = "macos")]
    pub fn end_host(process: (u32, u64), deadline: Instant) -> io::Result<()> {
        let identity = ProcessIdentity::new(process.0, process.1)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        match Self::open(identity, BlockerRole::TerminalHost)? {
            Some(host) => host.end_before(deadline),
            None => Ok(()),
        }
    }

    /// The recorded identity this process matched: its process ID and creation time.
    pub fn identity(&self) -> (u32, u64) {
        (self.identity.pid(), self.identity.creation_time())
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
