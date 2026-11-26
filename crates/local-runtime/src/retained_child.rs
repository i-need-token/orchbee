//! Retained child observation delegates process creation and exit semantics to the host adapter.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub use unix::RetainedChild;
#[cfg(windows)]
pub use windows::{JobPlacement, RetainedChild};
