//! Native system notification capability; the Client owns fallback presentation.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::SystemNotifications;
#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
pub(crate) use unix::SystemNotifications;
