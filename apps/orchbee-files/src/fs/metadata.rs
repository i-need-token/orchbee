//! Entries: what one filesystem object is, as the File Bar shows it. A link reports its own
//! `symlink` flag with its target's type, size, time and write protection; a link whose target
//! cannot be read is `unknown` with the link's own facts, as VS Code's `SymlinkSupport.stat`
//! reports a dangling link. Junctions and other name-surrogate reparse points count as links, as
//! the standard library classifies them. Write protection follows VS Code's
//! `DiskFileSystemProvider.stat`: the owner's write bit on macOS and Linux, the read-only
//! attribute on Windows.

use std::{
    ffi::OsString,
    fs, io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

/// The largest integer JavaScript represents exactly, which bounds sizes and times on the wire.
const MAXIMUM_SAFE_INTEGER: u64 = (1 << 53) - 1;

/// What an entry, or a link's target, is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntryKind {
    /// A file.
    File,
    /// A directory.
    Directory,
    /// Neither, or a link whose target cannot be read.
    Unknown,
}

/// One filesystem object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Entry {
    /// The final component: a lossy spelling when `unrepresentable`, empty for a filesystem root.
    pub(crate) name: String,
    /// What the object is; for a link, what its target is.
    pub(crate) kind: EntryKind,
    /// The object is a link.
    pub(crate) symlink: bool,
    /// The size in bytes, at most 2^53 − 1.
    pub(crate) size: u64,
    /// The modification time in milliseconds since the Unix epoch, rounded half up as Node.js
    /// reports it to VS Code, within ±(2^53 − 1).
    pub(crate) mtime_ms: i64,
    /// The object is write-protected.
    pub(crate) readonly: bool,
    /// The name is not valid Unicode, so the object cannot be operated on.
    pub(crate) unrepresentable: bool,
}

/// The entry at `path`, named by the path's final component.
///
/// # Errors
///
/// The error of reading the object's own metadata; a link's unreadable target is not an error.
pub(crate) fn stat(path: &Path) -> io::Result<Entry> {
    let own = fs::symlink_metadata(path)?;
    let (name, unrepresentable) = path
        .file_name()
        .map_or((String::new(), false), |name| spelling(name.to_os_string()));
    Ok(describe(name, unrepresentable, &own, || fs::metadata(path)))
}

/// The entry of a directory's child.
///
/// # Errors
///
/// The error of reading the child's own metadata, such as when it was removed after the
/// directory was read.
pub(super) fn child(entry: &fs::DirEntry) -> io::Result<Entry> {
    let own = entry.metadata()?;
    let (name, unrepresentable) = spelling(entry.file_name());
    Ok(describe(name, unrepresentable, &own, || {
        fs::metadata(entry.path())
    }))
}

/// A name as the wire spells it, and whether that spelling is lossy.
fn spelling(name: OsString) -> (String, bool) {
    match name.into_string() {
        Ok(name) => (name, false),
        Err(name) => (name.to_string_lossy().into_owned(), true),
    }
}

fn describe(
    name: String,
    unrepresentable: bool,
    own: &fs::Metadata,
    target: impl FnOnce() -> io::Result<fs::Metadata>,
) -> Entry {
    let symlink = own.file_type().is_symlink();
    let followed = if symlink { target().ok() } else { None };
    let (kind, facts) = match &followed {
        Some(target) => (kind(target), target),
        None if symlink => (EntryKind::Unknown, own),
        None => (kind(own), own),
    };
    Entry {
        name,
        kind,
        symlink,
        size: facts.len().min(MAXIMUM_SAFE_INTEGER),
        mtime_ms: milliseconds(facts.modified()),
        readonly: readonly(facts),
        unrepresentable,
    }
}

fn kind(metadata: &fs::Metadata) -> EntryKind {
    if metadata.is_dir() {
        EntryKind::Directory
    } else if metadata.is_file() {
        EntryKind::File
    } else {
        EntryKind::Unknown
    }
}

/// Node.js rounds `mtimeMs` half up to the millisecond of the `Date` VS Code reads. A time the
/// system cannot report reads as the epoch.
fn milliseconds(time: io::Result<SystemTime>) -> i64 {
    let Ok(time) = time else {
        return 0;
    };
    let nanoseconds = match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i128::try_from(after.as_nanos()).unwrap_or(i128::MAX),
        Err(before) => i128::try_from(before.duration().as_nanos()).map_or(i128::MIN, |ns| -ns),
    };
    let limit = i128::from(MAXIMUM_SAFE_INTEGER);
    let rounded = nanoseconds.saturating_add(500_000).div_euclid(1_000_000);
    // The clamp keeps the value within i64.
    i64::try_from(rounded.clamp(-limit, limit)).unwrap_or(0)
}

#[cfg(unix)]
fn readonly(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o200 == 0
}

#[cfg(not(unix))]
fn readonly(metadata: &fs::Metadata) -> bool {
    metadata.permissions().readonly()
}
