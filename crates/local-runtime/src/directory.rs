//! Windows path observations and shell bootstrap text; no filesystem or process effect is performed.
mod windows;
pub(crate) use windows::WINDOWS_POWERSHELL_INITIALIZATION;
pub use windows::{windows_interactive_powershell_initialization, windows_reported_directory};
