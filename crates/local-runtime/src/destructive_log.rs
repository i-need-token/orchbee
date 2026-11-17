//! The destructive request log (ORC-382): one JSON line per `server stop`, `session stop` or
//! `session delete` invocation, appended to `logs/destructive-commands.jsonl` in the data root.
//! The CLI owns each record's fields and which platforms write them; this owner keeps the file
//! private and bounded. Writing is best effort: a caller reports nothing and changes nothing when
//! an append fails.
use crate::{HeldLock, private_filesystem as permissions};
use std::{
    fs::{File, TryLockError},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    time::Duration,
};

const CURRENT: &str = "destructive-commands.jsonl";
const PREVIOUS: &str = "destructive-commands.1.jsonl";
/// The size the current log never exceeds. An append that would pass it first moves the current
/// lines to `destructive-commands.1.jsonl`, replacing the generation kept there, and starts the log
/// empty.
pub const DESTRUCTIVE_LOG_ROTATION_BYTES: u64 = 1024 * 1024;
/// How long an append waits for a concurrent writer before it gives up.
const LOCK_WAIT: Duration = Duration::from_millis(500);

/// Appends `record` as one line to this environment's destructive request log, creating the
/// private `logs` directory and file on first use.
pub fn append_destructive_request(record: &serde_json::Value) -> io::Result<()> {
    let mut line = serde_json::to_vec(record)?;
    line.push(b'\n');
    let length = line.len() as u64;
    if length > DESTRUCTIVE_LOG_ROTATION_BYTES {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let directory = crate::DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?
        .logs();
    let _pins = crate::data_root::create_state_directory(&directory)?;
    let held = lock(open_or_create(&directory.join(CURRENT))?)?;
    let mut log: &File = &held;
    if log.metadata()?.len() + length > DESTRUCTIVE_LOG_ROTATION_BYTES {
        rotate(log, &directory.join(PREVIOUS))?;
    }
    log.seek(SeekFrom::End(0))?;
    log.write_all(&line)
}

fn open_or_create(path: &Path) -> io::Result<File> {
    match permissions::open_file(path, true, true) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            permissions::open_file(path, false, true)
        }
        result => result,
    }
}

/// Serializes concurrent invocations on the current log's own handle; the lock ends when the
/// returned owner drops.
fn lock(file: File) -> io::Result<HeldLock> {
    match HeldLock::try_exclusive_within(file, LOCK_WAIT, Duration::from_millis(10)) {
        Ok(held) => Ok(held),
        Err(TryLockError::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

/// Copies the current lines over the previous generation, then empties the current log in place,
/// so no rename has to wait for another reader's handle.
fn rotate(mut log: &File, previous: &Path) -> io::Result<()> {
    let mut kept = open_or_create(previous)?;
    kept.set_len(0)?;
    log.seek(SeekFrom::Start(0))?;
    io::copy(
        &mut Read::by_ref(&mut log).take(DESTRUCTIVE_LOG_ROTATION_BYTES),
        &mut kept,
    )?;
    log.set_len(0)
}
