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
pub(crate) use macos::{
    ProcessSnapshot, current_identity, identity, observe, presence, role_absent,
};
#[cfg(target_os = "macos")]
pub use macos::{process_ended, running_children, running_program};
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) use unix::ProcessSnapshot;
#[cfg(windows)]
pub(crate) use windows::{
    ProcessSnapshot, current_identity, identity, observe, presence, role_absent,
};

/// What [`crate::RetainedSessionRecord::hosts`] observed of a generation's Terminal Host in one slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostPresence {
    /// No Host was started (Windows only).
    NotStarted,
    /// The Host's creation is reserved but not identified, so it never ran (Windows and macOS only).
    Spawning,
    /// The registered Host is gone, or, on Linux, no process holds the Host lock.
    Gone,
    /// A Host runs: on Windows and macOS its registered process ID and creation or start time; Linux
    /// registers none.
    Running(Option<(u32, u64)>),
    /// The registered Host could not be observed.
    Unknown,
}
