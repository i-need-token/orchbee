//! Retained command ownership reports process creation separately from descendant settlement.
use std::io;
#[derive(Debug)]
pub struct CommandSpawnFailure {
    pub error: io::Error,
    pub created: bool,
}

#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
pub use unsupported::CommandChild;
#[cfg(windows)]
pub use windows::{
    CommandChild, resolve_windows_program, run_windows_command, run_windows_command_exchange,
};
