//! Reads the system time zone from `/etc/localtime`, the TZif file macOS and Linux link to the
//! configured zone, and keeps the parsed zone until that file changes, so a caller that converts
//! every log record reads the file only once per zone change.

use super::tzif::Zone;
use std::{
    fs,
    io::Read,
    os::unix::fs::MetadataExt,
    sync::{Mutex, PoisonError},
};

const PATH: &str = "/etc/localtime";
/// Far above any published zone file, which stays below a few KiB.
const LIMIT: usize = 64 * 1024;

/// What tells one zone file from another: changing the system zone relinks `/etc/localtime` to
/// another file, and an updated zone database replaces the file.
#[derive(Clone, Copy, Eq, PartialEq)]
struct Identity {
    device: u64,
    inode: u64,
    length: u64,
    modified: i64,
    modified_nanoseconds: i64,
}

impl Identity {
    fn of(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            modified: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
        }
    }
}

/// The last file read and its zone, `None` when that file was not a usable zone.
static ZONE: Mutex<Option<(Identity, Option<Zone>)>> = Mutex::new(None);

/// The system zone's offset east of UTC at `seconds`, or `None` when no usable zone is installed.
pub(super) fn offset_at(seconds: i64) -> Option<i32> {
    let identity = Identity::of(&fs::metadata(PATH).ok()?);
    // The cache holds no invariant a panicking holder could break, so a poisoned lock is reused.
    let mut held = ZONE.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((known, zone)) = held.as_ref()
        && *known == identity
    {
        return zone.as_ref()?.offset_at(seconds);
    }
    let (identity, zone) = read()?;
    let offset = zone.as_ref().and_then(|zone| zone.offset_at(seconds));
    *held = Some((identity, zone));
    offset
}

/// Reads the file once through one handle, so its identity and content agree.
fn read() -> Option<(Identity, Option<Zone>)> {
    let file = fs::File::open(PATH).ok()?;
    let identity = Identity::of(&file.metadata().ok()?);
    let mut bytes = Vec::new();
    file.take(LIMIT as u64 + 1).read_to_end(&mut bytes).ok()?;
    let zone = (bytes.len() <= LIMIT)
        .then(|| Zone::parse(&bytes))
        .flatten();
    Some((identity, zone))
}
