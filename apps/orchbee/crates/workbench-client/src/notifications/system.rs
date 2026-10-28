//! Native system notification capability; the Client owns fallback presentation.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::SystemNotifications;
#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
pub use unix::SystemNotifications;
