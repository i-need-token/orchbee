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
    /// A Session folder's `session.json`, or the folder itself.
    CatalogRecord,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnrecognizedRecordReason {
    /// It occupies the selected Session's folder but cannot be admitted for that Session.
    Unreadable,
    /// Its version is one this build cannot use: saved by a newer version of Orchbee, or an
    /// earlier one no step updates. It is never rewritten, so neither start nor delete proceeds.
    Version,
    /// Another folder spells the selected Session's name in different letter case, which only a
    /// case-sensitive directory can hold; both folders refuse the Session.
    LetterCase,
}

/// A read-only fact: the record is retained as found and is never migrated or repaired.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnrecognizedRecord {
    pub kind: UnrecognizedRecordKind,
    pub reason: UnrecognizedRecordReason,
    pub path: std::path::PathBuf,
}

mod catalog;
pub use catalog::{NamedSessionEntry, SessionCatalog};
