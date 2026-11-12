//! Retained command ownership reports process creation separately from descendant settlement.
//! Windows retains each command's Job; Unix retains each command's process group.
use std::io;
#[derive(Debug)]
pub struct CommandSpawnFailure {
    pub error: io::Error,
    pub created: bool,
}

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub use unix::CommandChild;
#[cfg(windows)]
pub use windows::{
    CommandChild, resolve_windows_program, run_windows_command, run_windows_command_exchange,
    windows_system_program,
};
