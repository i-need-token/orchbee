//! Bounded checkpoint payload and restore facts; the Server owns document meaning and the platform owns storage effects.
/// Visible startup recovery facts; filesystem admission errors never become an empty fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointRestoreNotice {
    Recovered,
    Empty,
}

/// One kept earlier layout: its save time in Unix milliseconds and its payload, or `None` when the
/// file is unreadable or its frame does not match its name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutHistoryEntry {
    pub saved_at: u64,
    pub payload: Option<Vec<u8>>,
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
