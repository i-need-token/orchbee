//! The Windows destructive request log (ORC-382): one JSON line per `server stop`, `session stop`
//! or `session delete` invocation, appended to `<local state root>\audit\destructive.jsonl`. The
//! CLI owns each record's fields; this owner keeps the file private and bounded. Writing is best
//! effort: a caller reports nothing and changes nothing when an append fails.
use crate::private_filesystem as permissions;
use std::{
    fs::{self, File, TryLockError},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    thread,
    time::{Duration, Instant},
};

const DIRECTORY: &str = "audit";
const CURRENT: &str = "destructive.jsonl";
const PREVIOUS: &str = "destructive.1.jsonl";
/// The size the current log never exceeds. An append that would pass it first moves the current
/// lines to `destructive.1.jsonl`, replacing the generation kept there, and starts the log empty.
pub const DESTRUCTIVE_LOG_ROTATION_BYTES: u64 = 1024 * 1024;
/// How long an append waits for a concurrent writer before it gives up.
const LOCK_WAIT: Duration = Duration::from_millis(500);

/// Appends `record` as one line to this environment's destructive request log, creating the
/// private `audit` directory and file on first use.
pub fn append_destructive_request(record: &serde_json::Value) -> io::Result<()> {
    let mut line = serde_json::to_vec(record)?;
    line.push(b'\n');
    let length = line.len() as u64;
    if length > DESTRUCTIVE_LOG_ROTATION_BYTES {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let local = crate::environment::local_state_root(|name| std::env::var_os(name))
        .ok_or(io::ErrorKind::NotFound)?;
    let mut pins = crate::environment::create_state_root(&local)?;
    let directory = local.join(DIRECTORY);
    pins.push(private_directory(&directory)?);
    let mut log = open_or_create(&directory.join(CURRENT))?;
    lock(&log)?;
    if log.metadata()?.len() + length > DESTRUCTIVE_LOG_ROTATION_BYTES {
        rotate(&mut log, &directory.join(PREVIOUS))?;
    }
    log.seek(SeekFrom::End(0))?;
    log.write_all(&line)
}

fn private_directory(path: &Path) -> io::Result<File> {
    let created = match fs::create_dir(path) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error),
    };
    let mut handle = permissions::open_directory(path, created)?;
    if created {
        permissions::protect_new(&mut handle)?;
    } else {
        permissions::require_private(&handle)?;
    }
    Ok(handle)
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
/// handle closes.
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

/// Copies the current lines over the previous generation, then empties the current log in place,
/// so no rename has to wait for another reader's handle.
fn rotate(log: &mut File, previous: &Path) -> io::Result<()> {
    let mut kept = open_or_create(previous)?;
    kept.set_len(0)?;
    log.seek(SeekFrom::Start(0))?;
    io::copy(
        &mut Read::by_ref(log).take(DESTRUCTIVE_LOG_ROTATION_BYTES),
        &mut kept,
    )?;
    log.set_len(0)
}
