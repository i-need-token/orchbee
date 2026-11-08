//! Windows daily files: private, regular, single-link files in the data root's private `logs`
//! folder. An append holds the day's file with read sharing only, so concurrent writers, Desktop
//! Main's exclusive open included, admit one record at a time and wait up to half a second for each
//! other.
use super::{
    CAPPED_EVENT, DAILY_LOG_LIMIT_BYTES, FILE_SUFFIX, RETAINED_LOG_DAYS, Stamp, civil_from_days,
    days_from_civil, file_name,
};
use crate::private_filesystem as permissions;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

const SHARING_VIOLATION: i32 = 32;
const LOCK_VIOLATION: i32 = 33;
const REPARSE_POINT: u32 = 0x400;
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const SHARE_READ: u32 = 1;
const SHARE_READ_WRITE: u32 = 3;
/// How long an append or read waits for another writer before it gives up.
const SHARING_WAIT: Duration = Duration::from_millis(500);
/// Enough of a file's end to hold its last record.
const TAIL_BYTES: u64 = 1024;
/// The room kept below the limit for the `log-capped` record.
const CAPPED_RESERVE: u64 = 256;

/// The day whose earlier files this process last removed.
static PRUNED_DAY: Mutex<Option<i64>> = Mutex::new(None);

#[cfg(test)]
mod tests;

pub(super) fn append(record: &[u8], capped: &[u8], stamp: Stamp) -> io::Result<()> {
    let directory = crate::DataRoot::resolve(|name| std::env::var_os(name))
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?
        .logs();
    let _pins = crate::data_root::create_state_directory(&directory)?;
    append_in(&directory, record, capped, stamp)
}

/// Appends to the day's file in an existing private folder, then removes the expired daily files
/// once per day for this process.
pub(super) fn append_in(
    directory: &Path,
    record: &[u8],
    capped: &[u8],
    stamp: Stamp,
) -> io::Result<()> {
    let mut file = with_sharing_wait(|| open_daily(&directory.join(file_name(stamp.day))))?;
    let first_today = PRUNED_DAY
        .lock()
        .map(|mut pruned| pruned.replace(stamp.day) != Some(stamp.day))
        .unwrap_or(false);
    if first_today {
        prune(directory, stamp.day);
    }
    let size = file.metadata()?.len();
    let tail = if size == 0 {
        Vec::new()
    } else {
        let start = size.saturating_sub(TAIL_BYTES);
        file.seek(SeekFrom::Start(start))?;
        let mut tail = Vec::new();
        Read::by_ref(&mut file)
            .take(TAIL_BYTES)
            .read_to_end(&mut tail)?;
        tail
    };
    let line = match admission(size, ends_capped(&tail), record.len() as u64) {
        Admission::Append => record,
        Admission::Cap => capped,
        Admission::Closed => return Ok(()),
    };
    file.seek(SeekFrom::End(0))?;
    file.write_all(line)
}

/// Removes daily files dated more than six days before `today`. A file another writer holds open
/// stays until a later day's first append.
pub(super) fn prune(directory: &Path, today: i64) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let expired = entry
            .file_name()
            .to_str()
            .is_some_and(|name| expired(name, today));
        if expired && entry.file_type().is_ok_and(|kind| kind.is_file()) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// A daily file's bytes, or `None` when it is absent. Reading never creates anything.
pub(super) fn read_daily(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let file = with_sharing_wait(|| {
        OpenOptions::new()
            .read(true)
            .share_mode(SHARE_READ_WRITE)
            .custom_flags(OPEN_REPARSE_POINT)
            .open(path)
    });
    let file = match file {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    require_regular_private(&file)?;
    let mut bytes = Vec::new();
    file.take(DAILY_LOG_LIMIT_BYTES).read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

fn open_daily(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .share_mode(SHARE_READ)
        .custom_flags(OPEN_REPARSE_POINT)
        .open(path)?;
    require_regular_private(&file)?;
    Ok(file)
}

fn require_regular_private(file: &File) -> io::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.file_attributes() & REPARSE_POINT != 0
        || winapi_util::file::information(file)?.number_of_links() != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe daily log file",
        ));
    }
    permissions::require_private(file)
}

fn with_sharing_wait(mut open: impl FnMut() -> io::Result<File>) -> io::Result<File> {
    let deadline = Instant::now() + SHARING_WAIT;
    loop {
        match open() {
            Err(error)
                if matches!(
                    error.raw_os_error(),
                    Some(SHARING_VIOLATION | LOCK_VIOLATION)
                ) && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            result => return result,
        }
    }
}

/// What an append does with a day's file of `size` bytes whose last record is or is not
/// `log-capped`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Admission {
    Append,
    Cap,
    Closed,
}
fn admission(size: u64, ends_capped: bool, record: u64) -> Admission {
    if ends_capped {
        Admission::Closed
    } else if size.saturating_add(record).saturating_add(CAPPED_RESERVE) <= DAILY_LOG_LIMIT_BYTES {
        Admission::Append
    } else {
        Admission::Cap
    }
}

/// Whether the last complete line of a file's tail is the `log-capped` record.
fn ends_capped(tail: &[u8]) -> bool {
    let Some(body) = tail.strip_suffix(b"\n") else {
        return false;
    };
    let line = body
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(body, |index| &body[index + 1..]);
    serde_json::from_slice::<serde_json::Value>(line).is_ok_and(|record| {
        record.get("event").and_then(|event| event.as_str()) == Some(CAPPED_EVENT)
    })
}

/// Whether a daily file name is dated more than six days before `today`. The destructive request
/// log and any other name are never daily files.
fn expired(name: &str, today: i64) -> bool {
    parse_file_name(name).is_some_and(|day| day < today - (RETAINED_LOG_DAYS - 1))
}

fn parse_file_name(name: &str) -> Option<i64> {
    let date = name.strip_suffix(FILE_SUFFIX)?;
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return None;
    }
    let year = date[..4].parse().ok()?;
    let month = date[5..7].parse().ok()?;
    let day = date[8..].parse().ok()?;
    let days = days_from_civil(year, month, day);
    (civil_from_days(days) == (year, month, day)).then_some(days)
}
