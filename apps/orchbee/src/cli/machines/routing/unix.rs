//! Saved-machine relays have no Unix process port; unsupported never denotes a submission.

use crate::tui_capabilities::{Capability, Unsupported};
use orchbee_local_runtime::CommandOutput;
use std::{io, path::PathBuf, process::Command, time::Instant};

pub(super) fn ssh_path() -> io::Result<PathBuf> {
    Err(Unsupported(Capability::SavedMachines).into())
}

pub(super) fn exchange(
    _command: Command,
    _input: Option<&[u8]>,
    _limit: usize,
    _deadline: Option<Instant>,
    _interrupted: impl FnMut() -> io::Result<bool>,
) -> io::Result<CommandOutput> {
    Err(Unsupported(Capability::SavedMachines).into())
}
