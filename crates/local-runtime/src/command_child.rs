//! Retained command ownership reports process creation separately from descendant settlement.
//! Windows retains each command's Job; Unix retains each command's process group.
use std::io;
#[derive(Debug)]
pub struct CommandSpawnFailure {
    pub error: io::Error,
    pub created: bool,
}

mod output_drain;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub use unix::DetachedChild;
#[cfg(unix)]
pub use unix::{
    CommandChild, SpawnEnvironment, run_unix_command, run_unix_command_exchange,
    run_unmasked_unix_command, run_unmasked_unix_command_following,
};
#[cfg(windows)]
pub use windows::{
    CommandChild, resolve_windows_program, run_background_command_exchange,
    run_contained_command_exchange, run_windows_command, run_windows_command_exchange,
    windows_system_program,
};
