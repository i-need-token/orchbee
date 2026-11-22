//! Exact registered-process observations. The Windows adapter observes each registered identity
//! through a read-only process handle, or the system's process list where it may not open one; the
//! POSIX adapter has no registered identities to observe.
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub(crate) use unix::ProcessSnapshot;
#[cfg(windows)]
pub(crate) use windows::{
    ProcessSnapshot, current_identity, host_presence, identity, observe, role_absent,
};
