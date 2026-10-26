//! Bounded checkpoint payload and restore facts; the Server owns document meaning and the platform owns storage effects.
/// Visible startup recovery facts; filesystem admission errors never become an empty fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointRestoreNotice {
    Recovered,
    Empty,
}

pub struct CheckpointLoad<T> {
    pub checkpoint: Option<T>,
    pub notice: Option<CheckpointRestoreNotice>,
}

#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
pub use unsupported::SessionStore;
#[cfg(windows)]
pub use windows::SessionStore;
