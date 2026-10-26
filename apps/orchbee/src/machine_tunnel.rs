//! Saved-machine carrier capability; local Client transport is independent.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::{Tunnel, connect, serve};
#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
pub(crate) use unix::connect;
