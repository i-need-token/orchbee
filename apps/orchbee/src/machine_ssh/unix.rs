//! Saved-machine process probing is unsupported before any process is spawned.
use super::Output;
use std::{io, path::PathBuf, process::Command, time::Instant};
fn unsupported() -> io::Error {
    crate::tui_capabilities::Unsupported(crate::tui_capabilities::Capability::SavedMachines).into()
}
pub(crate) fn executable() -> io::Result<PathBuf> {
    Err(unsupported())
}
pub(crate) fn execute(
    _command: Command,
    _deadline: Option<Instant>,
    _interrupted: impl FnMut() -> io::Result<bool>,
) -> io::Result<Output> {
    Err(unsupported())
}
