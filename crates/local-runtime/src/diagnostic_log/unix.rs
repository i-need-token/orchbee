//! macOS and Linux daily files: private, regular, single-link files in the data root's private
//! `logs` folder, opened without following a symbolic link. An append holds the file's exclusive
//! `flock` lock, so writers admit one record at a time and wait up to half a second for each
//! other. The descriptor appends, so a writer that skips the lock still never overwrites a record.
use crate::private_filesystem as permissions;
use nix::fcntl::{FcntlArg, OFlag, fcntl};
use std::{
    fs::{File, TryLockError},
    io,
    path::Path,
    thread,
    time::{Duration, Instant},
};

/// How long an append waits for another writer before it gives up.
const LOCK_WAIT: Duration = Duration::from_millis(500);

/// Opens or creates the day's file and holds its exclusive lock until it closes.
pub(super) fn open_daily(path: &Path) -> io::Result<File> {
    let file = match permissions::open_file(path, true, true) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            permissions::open_file(path, false, true)
        }
        result => result,
    }?;
    // Replaces the open's non-blocking flag, which only guarded the open against a FIFO.
    fcntl(&file, FcntlArg::F_SETFL(OFlag::O_APPEND))?;
    lock(&file)?;
    Ok(file)
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

fn lock(file: &File) -> io::Result<()> {
    let deadline = Instant::now() + LOCK_WAIT;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(TryLockError::WouldBlock) => return Err(io::ErrorKind::WouldBlock.into()),
            Err(TryLockError::Error(error)) => return Err(error),
        }
    }
}
