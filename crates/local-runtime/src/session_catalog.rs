//! One shared namespace and generation boundary for activation, recovery and explicit deletion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamedSessionDeleteOutcome {
    Absent,
    Deleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamedSessionDeleteReason {
    DefaultSession,
    InUse,
    RecoveryRequired,
    Filesystem,
    TimedOut,
    Interrupted,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamedSessionDeleteFailure {
    pub reason: NamedSessionDeleteReason,
    pub pending: bool,
}

#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
pub use unsupported::{NamedSessionEntry, SessionCatalog};
#[cfg(windows)]
pub(crate) use windows::require_not_deleting;
#[cfg(windows)]
pub use windows::{NamedSessionEntry, SessionCatalog};
