//! Windows path observations and shell bootstrap text; no filesystem or process effect is performed.
mod windows;
pub use windows::{
    WINDOWS_AGENT_START_VARIABLE, windows_agent_start_command,
    windows_interactive_powershell_initialization, windows_powershell_application,
    windows_reported_directory,
};
