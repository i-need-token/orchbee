//! Saved-machine process probing is unsupported before any process is spawned.
use super::Output;
use std::{io, path::PathBuf, process::Command, time::Instant};
fn unsupported() -> io::Error {
    crate::host_capabilities::Unsupported(crate::host_capabilities::Capability::SavedMachines)
        .into()
}
pub fn executable() -> io::Result<PathBuf> {
    Err(unsupported())
}
pub fn execute(
    _command: Command,
    _deadline: Option<Instant>,
    _interrupted: impl FnMut() -> io::Result<bool>,
) -> io::Result<Output> {
    Err(unsupported())
}
