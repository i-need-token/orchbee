//! The owner of an advisory lock on an open file, which unlocks it before the file closes. On macOS
//! and Linux the standard library's file lock is an `flock`, which belongs to the open file
//! description and lasts while any descriptor of it is open, and a child process that another
//! thread starts holds a copy of every descriptor until it replaces its image: closing alone could
//! leave the lock held for a moment after its owner let go, and the next taker, in this process or
//! another, would read `WouldBlock` with no other owner. A Windows handle is never inherited, and
//! the same unlock releases its lock at once. It opens no file and orders no locks; each lock
//! file's owner does both.
use std::{
    fs::{File, TryLockError},
    io,
    ops::Deref,
    thread,
    time::{Duration, Instant},
};

#[cfg(test)]
mod tests;

/// A file whose lock this owner holds until it drops; reads and writes go through the file.
#[derive(Debug)]
pub struct HeldLock(File);

impl HeldLock {
    /// Takes `file`'s exclusive lock without waiting.
    ///
    /// # Errors
    ///
    /// `TryLockError::WouldBlock` while another open file holds the lock, or the lock call's own
    /// failure; `file` closes either way.
    pub fn try_exclusive(file: File) -> Result<Self, TryLockError> {
        file.try_lock()?;
        Ok(Self(file))
    }

    /// [`Self::try_exclusive`], tried again every `pause` while another holder has the lock, until
    /// `wait` has passed since the first try; then `WouldBlock` stands.
    pub(crate) fn try_exclusive_within(
        file: File,
        wait: Duration,
        pause: Duration,
    ) -> Result<Self, TryLockError> {
        let deadline = Instant::now() + wait;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self(file)),
                Err(TryLockError::WouldBlock) if Instant::now() < deadline => thread::sleep(pause),
                Err(error) => return Err(error),
            }
        }
    }

    /// Takes `file`'s exclusive lock, waiting for every other holder.
    pub(crate) fn exclusive(file: File) -> io::Result<Self> {
        file.lock()?;
        Ok(Self(file))
    }

    /// Takes `file`'s shared lock, waiting while another file holds it exclusively. Only the POSIX
    /// socket folder is held shared.
    #[cfg(unix)]
    pub(crate) fn shared(file: File) -> io::Result<Self> {
        file.lock_shared()?;
        Ok(Self(file))
    }
}

impl Deref for HeldLock {
    type Target = File;

    fn deref(&self) -> &File {
        &self.0
    }
}

impl Drop for HeldLock {
    fn drop(&mut self) {
        // Best effort: when the unlock fails, the close that follows releases the lock as before.
        let _ = self.0.unlock();
    }
}
