//! Platform-selected local runtime admission; applications own activation and service policy.
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
pub use crate::rendezvous_types::{
    EntryKind, EntryObservation, LockKind, RendezvousError, SessionLifetimeObservation,
    SessionScope,
};
#[cfg(unix)]
pub(crate) use unix::open_runtime_base;
#[cfg(all(test, unix))]
pub(crate) use unix::tests;
#[cfg(unix)]
pub use unix::{
    HOST_SCOPE_NAME, LockAttempt, ModeCheckedLocalSessionRendezvous, ModeCheckedSessionLock,
};
#[cfg(windows)]
pub use windows::{
    CLIENT_PENDING_SOCKET_NAME, HOST_ENDPOINT_ID, HOST_ENDPOINT_NAME, HOST_SCOPE_NAME, LockAttempt,
    ModeCheckedLocalSessionRendezvous, ModeCheckedSessionLock,
};
