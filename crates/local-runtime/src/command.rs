//! Literal Windows and POSIX shell encoding is deterministic data translation on every host.
mod posix;
mod windows;
pub use posix::{posix_command_line, posix_shell_word};
pub use windows::{windows_encoded_command, windows_raw_command_encoded};
