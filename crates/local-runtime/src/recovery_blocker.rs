//! The process that keeps cold recovery from starting a Session again: one an earlier start
//! registered whose exact identity, its process ID and creation time, is still alive; one running
//! that role's program that holds the registered ID and that this account may not open, so it
//! cannot be proven another process; or one a restart's successor entry names that cannot be
//! proven gone or ended. A refusal names it so the surface that reports the refusal can say which
//! process to end; naming it grants no termination authority, and nothing here ends a process.
use crate::{
    CohortArtifact,
    process_registration::{ProcessRole, RecoveryVerdict, SessionProcesses},
};
use std::{error::Error, fmt, io};

/// A process an earlier start registered that is still running. Cold recovery refuses while it
/// runs; once it ends, or the computer restarts, the next start recovers the Session by itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryBlocker {
    pub role: BlockerRole,
    /// Its operating-system process ID, never zero.
    pub process_id: u32,
}

/// Which of the Session's processes a [`RecoveryBlocker`] is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockerRole {
    Server,
    Worker,
    TerminalHost,
}

impl BlockerRole {
    /// The program this role runs, whose file name the operating system shows for the process.
    pub const fn program(self) -> CohortArtifact {
        match self {
            Self::Server => CohortArtifact::Server,
            Self::Worker => CohortArtifact::Worker,
            Self::TerminalHost => CohortArtifact::TerminalHost,
        }
    }

    pub(crate) const fn of(role: ProcessRole) -> Self {
        match role {
            ProcessRole::Server => Self::Server,
            ProcessRole::Worker => Self::Worker,
            // The Keeper runs the Terminal Host's program, and ending it ends what a Host would.
            ProcessRole::Host(_) | ProcessRole::Keeper(_) => Self::TerminalHost,
        }
    }
}

impl RecoveryBlocker {
    /// The blocker a refused cold recovery carries, if `error` names one.
    pub fn of(error: &io::Error) -> Option<Self> {
        error.get_ref()?.downcast_ref().copied()
    }

    /// The process `verdict` found alive, or could not tell apart from the registered one, on
    /// `processes`. An unknown observation is a process running that role's program that holds the
    /// registered ID and that this account may not open: ending it, or restarting the computer,
    /// clears the refusal whether or not it is the registered process.
    pub(crate) fn named(processes: &SessionProcesses, verdict: RecoveryVerdict) -> Option<Self> {
        match verdict {
            RecoveryVerdict::Live(role) | RecoveryVerdict::Uncertain(role) => processes
                .registration(role)
                .identity()
                .map(|identity| Self {
                    role: BlockerRole::of(role),
                    process_id: identity.pid(),
                }),
            RecoveryVerdict::Gone => None,
        }
    }

    /// The refusal cold recovery returns for a `verdict` other than gone on `processes`, as
    /// InvalidData, carrying the process [`Self::named`] finds.
    pub(crate) fn refusal(processes: &SessionProcesses, verdict: RecoveryVerdict) -> io::Error {
        Self::carried(Self::named(processes, verdict))
    }

    /// The refusal cold recovery returns when what a successor entry names could not be proven
    /// gone or ended with `error`, as InvalidData, carrying the process `error` names.
    #[cfg(any(windows, target_os = "macos"))]
    pub(crate) fn successor_refusal(error: &io::Error) -> io::Error {
        Self::carried(Self::of(error))
    }

    fn carried(blocker: Option<Self>) -> io::Error {
        match blocker {
            Some(blocker) => io::Error::new(io::ErrorKind::InvalidData, blocker),
            None => io::ErrorKind::InvalidData.into(),
        }
    }
}

impl fmt::Display for RecoveryBlocker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the {} process {} from an earlier start is still running",
            self.role.program().file_name(),
            self.process_id
        )
    }
}

impl Error for RecoveryBlocker {}
