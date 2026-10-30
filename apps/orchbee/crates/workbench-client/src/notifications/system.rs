//! Native system notification capability and host alert tone; the Client owns fallback
//! presentation.
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{SystemNotifications, sound};
#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
pub use unix::{SystemNotifications, sound};
