//! Routed commands have no Unix process port; unsupported never denotes a submission.

use crate::tui_capabilities::{Capability, Unsupported};
use orchbee_local_runtime::CommandOutput;
use std::{io, process::Command, time::Instant};

pub(super) fn exchange(
    _command: Command,
    _input: Option<&[u8]>,
    _limit: usize,
    _deadline: Option<Instant>,
    _interrupted: impl FnMut() -> io::Result<bool>,
) -> io::Result<CommandOutput> {
    Err(Unsupported(Capability::SavedMachines).into())
}
