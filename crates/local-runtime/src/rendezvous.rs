//! Platform-selected local runtime admission; applications own activation and service policy.
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use crate::rendezvous_types::SessionHolderObservation;
pub use crate::rendezvous_types::{
    EntryKind, EntryObservation, LockKind, RendezvousError, SessionLifetimeObservation,
    SessionScope,
};
#[cfg(all(test, unix))]
pub(crate) use unix::tests;
#[cfg(unix)]
pub use unix::{
    HOST_SCOPE_NAME, LockAttempt, ModeCheckedLocalSessionRendezvous, ModeCheckedSessionLock,
};
#[cfg(unix)]
pub(crate) use unix::{endpoint_exists, open_runtime_base};
#[cfg(windows)]
pub(crate) use windows::endpoint_exists;
#[cfg(windows)]
pub use windows::{
    CLIENT_PENDING_SOCKET_NAME, HOST_ENDPOINT_ID, HOST_ENDPOINT_NAME, HOST_SCOPE_NAME, LockAttempt,
    ModeCheckedLocalSessionRendezvous, ModeCheckedSessionLock,
};

/// Tries `acquire` until it takes the lock or `MOMENTARY_HOLD` passes. An observer (`session list`,
/// `server diagnose`, `server stop` waiting for a release, the machine hub) takes a Session lock
/// only for the instant of one probe, so a lock still contended after that has an owner. Lock order
/// and who may take each lock are the caller's, as for one attempt.
pub fn acquire_past_probes(
    acquire: impl FnMut() -> Result<LockAttempt, RendezvousError>,
) -> Result<LockAttempt, RendezvousError> {
    crate::private_filesystem::outlast_momentary_hold(
        |outcome| matches!(outcome, Ok(LockAttempt::Contended)),
        acquire,
    )
}
