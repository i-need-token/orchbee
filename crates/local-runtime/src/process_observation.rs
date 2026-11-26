//! Exact registered-process observations. The Windows adapter observes each registered identity
//! through a read-only process handle, or the system's process list where it may not open one; the
//! macOS and Linux adapter observes it by process ID and start time.
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub(crate) use unix::{
    ProcessSnapshot, current_identity, identity, observe, presence, role_absent,
};
#[cfg(unix)]
pub use unix::{process_ended, running_children, running_program};
#[cfg(windows)]
pub(crate) use windows::{
    ProcessSnapshot, current_identity, identity, observe, presence, role_absent,
};

/// What [`crate::RetainedSessionRecord::hosts`] observed of a generation's Terminal Host in one slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostPresence {
    /// No Host was started.
    NotStarted,
    /// The Host's creation is reserved but not identified, so it never ran.
    Spawning,
    /// The registered Host is gone.
    Gone,
    /// A Host runs, with its registered process ID and creation or start time.
    Running(Option<(u32, u64)>),
    /// The registered Host could not be observed.
    Unknown,
}
