//! Exact registered-process observations. The Windows adapter observes each registered identity
//! through a read-only process handle; the POSIX adapter has no registered identities to observe.
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub(crate) use unix::{ProcessSnapshot, role_absent};
#[cfg(windows)]
pub(crate) use windows::{ProcessSnapshot, current_identity, identity, role_absent};
