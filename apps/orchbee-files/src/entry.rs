//! The entry a Client sees for one item, as VS Code's disk provider reports it: what a link points
//! to rather than the link itself, with the link flagged; a link whose target cannot be read as
//! `unknown` with the link's own size and time; and `readonly` from the owner's write bit on macOS
//! and Linux and the read-only attribute on Windows. Junctions and every other name-surrogate
//! reparse point count as links, as the standard library's file type reports them.

use std::{
    ffi::OsStr,
    fs::{self, Metadata},
    io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use orchbee_files_protocol::{Entry, EntryKind, MAXIMUM_SAFE_INTEGER, SafeInteger, Signed, Text};

/// The entry of the item at `path`, whose final segment is `name`. It reads the filesystem, so it
/// runs where waiting is allowed.
///
/// # Errors
///
/// The operating system's failure to read the item itself, and [`io::ErrorKind::InvalidFilename`]
/// for a name longer than an entry carries.
pub(crate) fn read(path: &Path, name: &OsStr) -> io::Result<Entry> {
    let own = fs::symlink_metadata(path)?;
    let symlink = own.file_type().is_symlink();
    let target = if symlink {
        fs::metadata(path).ok()
    } else {
        None
    };
    let facts = target.as_ref().unwrap_or(&own);
    let kind = if symlink && target.is_none() {
        EntryKind::Unknown
    } else if facts.is_dir() {
        EntryKind::Directory
    } else if facts.is_file() {
        EntryKind::File
    } else {
        EntryKind::Unknown
    };
    let (name, unrepresentable) = match name.to_str() {
        Some(name) => (name.to_owned(), false),
        None => (name.to_string_lossy().into_owned(), true),
    };
    Ok(Entry {
        name: Text::new(name).map_err(|_| io::Error::from(io::ErrorKind::InvalidFilename))?,
        kind,
        symlink,
        size: SafeInteger::new(facts.len().min(MAXIMUM_SAFE_INTEGER)).unwrap_or_default(),
        mtime_ms: milliseconds(facts.modified()),
        readonly: readonly(facts),
        unrepresentable,
    })
}

/// Milliseconds since the Unix epoch, negative before it; 0 where the filesystem keeps no time.
fn milliseconds(time: io::Result<SystemTime>) -> Signed {
    let milliseconds = match time.map(|time| time.duration_since(UNIX_EPOCH)) {
        Ok(Ok(after)) => i64::try_from(after.as_millis()).unwrap_or(i64::MAX),
        Ok(Err(before)) => i64::try_from(before.duration().as_millis()).map_or(i64::MIN, |m| -m),
        Err(_) => 0,
    };
    Signed::new(milliseconds).unwrap_or_default()
}

#[cfg(unix)]
fn readonly(metadata: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    metadata.permissions().mode() & 0o200 == 0
}

#[cfg(windows)]
fn readonly(metadata: &Metadata) -> bool {
    metadata.permissions().readonly()
}
