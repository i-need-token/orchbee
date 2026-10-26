//! Single joined clipboard adapter; platform-specific ownership stays outside copy state.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(super) use windows::Writer;

#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
pub(super) use unix::Writer;
