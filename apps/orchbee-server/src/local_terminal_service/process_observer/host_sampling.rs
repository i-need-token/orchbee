//! Host process sampling exists only where the Terminal Host can bound a process sample: Windows
//! Hosts sample their Job. Other builds refuse to start the observer before any setting is read.
use std::io;

/// This build's Hosts sample terminal processes.
#[cfg(windows)]
pub(super) fn require() -> io::Result<()> {
    Ok(())
}

/// This build's Hosts report process sampling as unsupported.
#[cfg(not(windows))]
pub(super) fn require() -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}
