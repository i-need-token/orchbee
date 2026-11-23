//! Exact registered-process observations. The Windows adapter observes each registered identity
//! through a read-only process handle, or the system's process list where it may not open one; the
//! macOS adapter observes it by process ID and start time; the Linux adapter has no registered
//! identities to observe.
#[cfg(target_os = "macos")]
mod macos;
#[cfg(all(unix, not(target_os = "macos")))]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "macos")]
pub use macos::running_program;
#[cfg(target_os = "macos")]
pub(crate) use macos::{
    ProcessSnapshot, current_identity, identity, observe, presence, role_absent,
};
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) use unix::ProcessSnapshot;
#[cfg(windows)]
pub(crate) use windows::{
    ProcessSnapshot, current_identity, identity, observe, presence, role_absent,
};
