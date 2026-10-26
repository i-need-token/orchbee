//! Checkpoint restore feedback is bound to an admitted activation generation.
mod windows;
pub(crate) use windows::{FILE, PENDING, remove_pending};
