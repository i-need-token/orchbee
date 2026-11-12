//! The process facts a `query-result` and a `drain-result` report. Both enums grow: an unknown
//! name decodes as `Unknown`, which encodes as `unknown`.

use crate::name::{growable_name, growable_serde, growable_value};

/// The role of a process a query reports.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProcessRole {
    /// The Session's Server.
    Server,
    /// The Server's Worker.
    Worker,
    /// A Terminal Host.
    TerminalHost,
    /// The remote-machine hub of one user and environment.
    MachineHub,
    /// The remote helper `orchbee --machine-link-v1` at the far end of a machine link.
    MachineLink,
    /// A role this build does not know.
    Unknown,
}

impl ProcessRole {
    const NAMES: [(Self, &'static str); 6] = [
        (Self::Server, "server"),
        (Self::Worker, "worker"),
        (Self::TerminalHost, "terminal-host"),
        (Self::MachineHub, "machine-hub"),
        (Self::MachineLink, "machine-link"),
        (Self::Unknown, "unknown"),
    ];

    /// The wire name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        growable_name(&Self::NAMES, self)
    }

    /// The role `name` denotes, `Unknown` for any other grammatical name.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        growable_value(&Self::NAMES, name, Self::Unknown)
    }
}

/// The lifecycle state of a process.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProcessState {
    /// Accepting new work.
    Ready,
    /// Finishing existing work and accepting none.
    Draining,
    /// Ending.
    Stopping,
    /// A state this build does not know.
    Unknown,
}

impl ProcessState {
    const NAMES: [(Self, &'static str); 4] = [
        (Self::Ready, "ready"),
        (Self::Draining, "draining"),
        (Self::Stopping, "stopping"),
        (Self::Unknown, "unknown"),
    ];

    /// The wire name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        growable_name(&Self::NAMES, self)
    }

    /// The state `name` denotes, `Unknown` for any other grammatical name.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        growable_value(&Self::NAMES, name, Self::Unknown)
    }
}

growable_serde!(ProcessRole);
growable_serde!(ProcessState);
