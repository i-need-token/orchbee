//! Optional activation-generation feedback, separate from a supported reader's I/O failure.
use orchbee_local_runtime::CheckpointRestoreNotice;
use std::io;

/// No restore-feedback reader exists; activation may continue without a notice.
pub(super) struct Unsupported;

type Read = Result<io::Result<Option<CheckpointRestoreNotice>>, Unsupported>;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(super) use windows::read;
#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
pub(super) use unix::read;
