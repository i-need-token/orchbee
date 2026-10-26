//! Literal Windows shell encoding is deterministic data translation on every host.
mod windows;
pub use windows::{windows_encoded_command, windows_raw_command_encoded};
