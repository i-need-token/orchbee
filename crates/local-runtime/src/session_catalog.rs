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

/// The retained object that holds a record the selected Session cannot admit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnrecognizedRecordKind {
    RuntimeDirectory,
    CatalogRecord,
    DeletionIntent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnrecognizedRecordReason {
    /// Its identity names the selected Session, but another derivation stored it elsewhere.
    ObsoleteIdentity,
    /// It occupies the selected Session's location but cannot be admitted for that Session.
    Unreadable,
}

/// A read-only fact: the record is retained as found and is never migrated or repaired.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnrecognizedRecord {
    pub kind: UnrecognizedRecordKind,
    pub reason: UnrecognizedRecordReason,
    pub path: std::path::PathBuf,
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
