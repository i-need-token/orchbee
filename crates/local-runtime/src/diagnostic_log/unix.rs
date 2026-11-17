//! macOS and Linux daily files: private, regular, single-link files in the data root's private
//! `logs` folder, opened without following a symbolic link. An append holds the file's exclusive
//! `flock` lock, so writers admit one record at a time and wait up to half a second for each
//! other. The descriptor appends, so a writer that skips the lock still never overwrites a record.
use crate::{HeldLock, private_filesystem as permissions};
use nix::fcntl::{FcntlArg, OFlag, fcntl};
use std::{
    fs::{File, TryLockError},
    io,
    path::Path,
    time::Duration,
};

/// How long an append waits for another writer before it gives up.
const LOCK_WAIT: Duration = Duration::from_millis(500);

/// Opens or creates the day's file and holds its exclusive lock until the returned owner drops.
pub(super) fn open_daily(path: &Path) -> io::Result<HeldLock> {
    let file = match permissions::open_file(path, true, true) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            permissions::open_file(path, false, true)
        }
        result => result,
    }?;
    // Replaces the open's non-blocking flag, which only guarded the open against a FIFO.
    fcntl(&file, FcntlArg::F_SETFL(OFlag::O_APPEND))?;
    match HeldLock::try_exclusive_within(file, LOCK_WAIT, Duration::from_millis(10)) {
        Ok(held) => Ok(held),
        Err(TryLockError::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

/// Opens an existing daily file to read without taking the lock, or `None` when it is absent; a
/// record an append is still writing reads as an incomplete line.
pub(super) fn open_existing(path: &Path) -> io::Result<Option<File>> {
    match permissions::open_file(path, false, false) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
