//! `fs.write`: the editor's text, encoded into the file in place. Nothing changes before the commit
//! point, where the file opens for writing, so a failure before it leaves the file as it was; from
//! the commit point on, a failure is `write-incomplete` with its cause.
//!
//! 1. The new content is measured first: its size, its SHA-256 digest and the characters its
//!    encoding replaces, encoded piece by piece and never held whole.
//! 2. The precondition is checked as VS Code's `validateWriteFile` checks it, except that the
//!    content's digest decides where VS Code compares times and sizes: a version precondition holds
//!    while the file still has its digest, or already holds the new content, and `content-newer`
//!    otherwise; a file the same size as neither is not read. `create` needs nothing at the path,
//!    and `none` checks nothing. A directory is `is-directory`, and anything that is neither file nor
//!    directory is refused, since opening it could block.
//! 3. A read-only file is `read-only`, unless `unlock` clears its read-only attribute or owner write
//!    bit first, which a failure before the commit point sets again. A save leaves it writable, as
//!    VS Code does.
//! 4. The commit point: an existing file opens for writing without truncation, is rewritten from
//!    its start and set to its new length, so its hidden attribute, alternate data streams,
//!    ownership, hard links and link target survive, as in VS Code. A missing file is created
//!    exclusively, and the folders missing above it first, as VS Code creates them. No fsync.
//!
//! Each filesystem call runs on a helper under its deadline, reading or writing at most one piece.
//! Writes to one path run one at a time in this process, so two Clients saving one file meet the
//! precondition in turn rather than interleave.

use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions, Permissions},
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use orchbee_files_protocol::{
    ErrorCode, FilesError, FsWriteResult, Operation, SafeInteger, Sha256Digest, Signed,
    WritePrecondition,
};
use sha2::{Digest as _, Sha256};

use super::entries;
use crate::{
    blocking::Helpers,
    entry, error,
    fs::{Encoder, Target},
};

/// The most bytes one helper call reads or writes.
const PIECE: usize = 1024 * 1024;

/// What one write puts where.
pub(super) struct Write {
    /// The file, as scope admitted it.
    pub(super) path: PathBuf,
    /// The text, as the Client sent it.
    pub(super) text: String,
    pub(super) target: Target,
    pub(super) precondition: WritePrecondition,
    pub(super) unlock: bool,
}

/// What is at a write's path, following links.
#[derive(Clone, Debug)]
pub(super) struct Found {
    kind: FoundKind,
    size: u64,
    permissions: Permissions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FoundKind {
    File,
    Directory,
    Other,
}

/// How the commit point opens the file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Open {
    /// The file exists and is rewritten in place.
    Existing,
    /// Nothing may be at the path.
    New,
    /// Whatever is at the path, or a new file.
    Any,
}

/// The filesystem calls a write makes, so tests can fail each one. Each runs on a helper.
pub(super) trait Disk: Clone + Send + Sync + 'static {
    type File: Send + 'static;

    /// What is at `path`, following links; `None` when nothing is.
    fn inspect(&self, path: &Path) -> io::Result<Option<Found>>;
    fn open_read(&self, path: &Path) -> io::Result<Self::File>;
    /// Fills `buffer` as far as the file goes; fewer bytes mean its end.
    fn read(&self, file: &mut Self::File, buffer: &mut [u8]) -> io::Result<usize>;
    fn set_permissions(&self, path: &Path, permissions: Permissions) -> io::Result<()>;
    fn create_folders(&self, path: &Path) -> io::Result<()>;
    fn open_write(&self, path: &Path, open: Open) -> io::Result<Self::File>;
    fn write(&self, file: &mut Self::File, bytes: &[u8]) -> io::Result<()>;
    /// Sets the file's length, then closes it.
    fn finish(&self, file: Self::File, length: u64) -> io::Result<()>;
    /// The file's size and modification time once it closed.
    fn written(&self, path: &Path) -> io::Result<(u64, Signed)>;
}

/// This machine's filesystem.
#[derive(Clone, Copy)]
pub(super) struct System;

impl Disk for System {
    type File = File;

    fn inspect(&self, path: &Path) -> io::Result<Option<Found>> {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let kind = if metadata.is_file() {
            FoundKind::File
        } else if metadata.is_dir() {
            FoundKind::Directory
        } else {
            FoundKind::Other
        };
        Ok(Some(Found {
            kind,
            size: metadata.len(),
            permissions: metadata.permissions(),
        }))
    }

    fn open_read(&self, path: &Path) -> io::Result<File> {
        File::open(path)
    }

    fn read(&self, file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
        let mut filled = 0;
        while filled < buffer.len() {
            match file.read(&mut buffer[filled..]) {
                Ok(0) => break,
                Ok(read) => filled += read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        Ok(filled)
    }

    fn set_permissions(&self, path: &Path, permissions: Permissions) -> io::Result<()> {
        set_permissions(path, permissions)
    }

    fn create_folders(&self, path: &Path) -> io::Result<()> {
        match path.parent() {
            Some(parent) => entries::create_folders(parent),
            None => Ok(()),
        }
    }

    fn open_write(&self, path: &Path, open: Open) -> io::Result<File> {
        let mut options = OpenOptions::new();
        options.write(true);
        match open {
            Open::Existing => {}
            Open::New => {
                options.create_new(true);
            }
            Open::Any => {
                options.create(true);
            }
        }
        options.open(path)
    }

    fn write(&self, file: &mut File, bytes: &[u8]) -> io::Result<()> {
        file.write_all(bytes)
    }

    fn finish(&self, file: File, length: u64) -> io::Result<()> {
        file.set_len(length)
    }

    fn written(&self, path: &Path) -> io::Result<(u64, Signed)> {
        let metadata = fs::metadata(path)?;
        Ok((metadata.len(), entry::milliseconds(metadata.modified())))
    }
}

/// Sets the permissions of what `path` names, through a link: on Windows through a handle opened
/// only to write attributes, which a read-only file admits.
#[cfg(windows)]
fn set_permissions(path: &Path, permissions: Permissions) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt as _;
    /// `FILE_WRITE_ATTRIBUTES` (`winnt.h`).
    const WRITE_ATTRIBUTES: u32 = 0x0100;
    OpenOptions::new()
        .access_mode(WRITE_ATTRIBUTES)
        .open(path)?
        .set_permissions(permissions)
}

#[cfg(not(windows))]
fn set_permissions(path: &Path, permissions: Permissions) -> io::Result<()> {
    fs::set_permissions(path, permissions)
}

/// The writes running in this process, by path, each held while its write runs.
#[derive(Default)]
pub(super) struct Writing {
    paths: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl Writing {
    /// Runs `work` once no other write of the path keyed `key` runs.
    pub(super) fn exclusive<T>(&self, key: String, work: impl FnOnce() -> T) -> T {
        let path = Arc::clone(lock(&self.paths).entry(key.clone()).or_default());
        let result = {
            let _held = lock(&path);
            work()
        };
        let mut paths = lock(&self.paths);
        // The map's own reference and this one: no other write waits for the path.
        if Arc::strong_count(&path) == 2 {
            paths.remove(&key);
        }
        result
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs `write` on `disk` through `helpers`, unless `cancelled` says its Client gave up before the
/// commit point.
pub(super) fn run<D: Disk>(
    helpers: &Helpers,
    disk: &D,
    write: &Write,
    cancelled: &dyn Fn() -> bool,
) -> Result<FsWriteResult, FilesError> {
    let measured = measure(write.target, &write.text);
    let path = &write.path;
    let found = call(helpers, disk, path, D::inspect)?;
    let open = admit(helpers, disk, write, found.as_ref(), &measured)?;
    let relock = match &found {
        Some(found) if readonly(&found.permissions) => {
            if !write.unlock {
                return Err(FilesError::new(ErrorCode::ReadOnly));
            }
            let mut writable = found.permissions.clone();
            make_writable(&mut writable);
            call(helpers, disk, path, move |disk, path| {
                disk.set_permissions(path, writable)
            })?;
            Some(found.permissions.clone())
        }
        _ => None,
    };
    let file = commit(helpers, disk, write, open, cancelled).inspect_err(|_| {
        if let Some(permissions) = relock {
            // The write failed already; a relock that fails leaves the file writable.
            let _ = call(helpers, disk, path, move |disk, path| {
                disk.set_permissions(path, permissions)
            });
        }
    })?;
    rewrite(helpers, disk, write, file, measured.size).map_err(incomplete)?;
    // The content is written; a file that cannot be read back now is reported as written.
    let (size, mtime_ms) =
        call(helpers, disk, path, D::written).unwrap_or((measured.size, Signed::default()));
    Ok(FsWriteResult {
        mtime_ms,
        size: SafeInteger::new(size).unwrap_or_default(),
        replaced: SafeInteger::new(measured.replaced).unwrap_or_default(),
        sha256: measured.digest,
    })
}

/// The new content's facts.
struct Measured {
    size: u64,
    digest: Sha256Digest,
    replaced: u64,
}

fn measure(target: Target, text: &str) -> Measured {
    let mut encoder = Encoder::new(target, text);
    let (mut piece, mut digest, mut size) = (Vec::new(), Sha256::new(), 0_u64);
    while encoder.next(&mut piece) {
        digest.update(&piece);
        size += piece.len() as u64;
    }
    Measured {
        size,
        digest: Sha256Digest::from_bytes(digest.finalize().into()),
        replaced: encoder.replaced(),
    }
}

/// Checks the write's precondition against `found` and decides how the commit point opens.
fn admit<D: Disk>(
    helpers: &Helpers,
    disk: &D,
    write: &Write,
    found: Option<&Found>,
    measured: &Measured,
) -> Result<Open, FilesError> {
    let Some(found) = found else {
        return Ok(match write.precondition {
            WritePrecondition::None => Open::Any,
            WritePrecondition::Create | WritePrecondition::Version { .. } => Open::New,
        });
    };
    match found.kind {
        FoundKind::File => {}
        FoundKind::Directory => return Err(FilesError::new(ErrorCode::IsDirectory)),
        FoundKind::Other => return Err(FilesError::new(ErrorCode::Failed)),
    }
    match write.precondition {
        WritePrecondition::Create => Err(FilesError::new(ErrorCode::Exists)),
        WritePrecondition::None => Ok(Open::Existing),
        WritePrecondition::Version { size, sha256, .. } => {
            let mut current = None;
            if found.size == size.get() {
                let digest = read_digest(helpers, disk, &write.path)?;
                if digest == sha256 {
                    return Ok(Open::Existing);
                }
                current = Some(digest);
            }
            if found.size == measured.size {
                let digest = match current {
                    Some(digest) => digest,
                    None => read_digest(helpers, disk, &write.path)?,
                };
                if digest == measured.digest {
                    return Ok(Open::Existing);
                }
            }
            Err(FilesError::new(ErrorCode::ContentNewer))
        }
    }
}

/// The digest of the file at `path`, read one piece per helper call.
fn read_digest<D: Disk>(
    helpers: &Helpers,
    disk: &D,
    path: &Path,
) -> Result<Sha256Digest, FilesError> {
    let mut file = call(helpers, disk, path, D::open_read)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0; PIECE];
    loop {
        let disk = disk.clone();
        let (returned, filled, read) = helpers.run(Operation::FsWrite, move || {
            let mut file = file;
            let mut buffer = buffer;
            let read = disk.read(&mut file, &mut buffer);
            (file, buffer, read)
        })?;
        let read = read.map_err(|failure| error::from_io(&failure))?;
        digest.update(&filled[..read]);
        if read < filled.len() {
            return Ok(Sha256Digest::from_bytes(digest.finalize().into()));
        }
        (file, buffer) = (returned, filled);
    }
}

/// Opens the file at the commit point, unless the Client gave up first.
fn commit<D: Disk>(
    helpers: &Helpers,
    disk: &D,
    write: &Write,
    open: Open,
    cancelled: &dyn Fn() -> bool,
) -> Result<D::File, FilesError> {
    if cancelled() {
        return Err(FilesError::new(ErrorCode::Cancelled));
    }
    let path = &write.path;
    if open != Open::Existing {
        call(helpers, disk, path, D::create_folders)?;
    }
    let opened = call(helpers, disk, path, move |disk, path| {
        disk.open_write(path, open)
    });
    match opened {
        Err(error) if open == Open::New && error.code == ErrorCode::Exists => {
            // Something appeared at the path since it was inspected.
            let code = match write.precondition {
                WritePrecondition::Create => ErrorCode::Exists,
                WritePrecondition::Version { .. } | WritePrecondition::None => {
                    ErrorCode::ContentNewer
                }
            };
            Err(FilesError::new(code))
        }
        opened => opened,
    }
}

/// Writes the new content over `file` from its start and sets its length.
fn rewrite<D: Disk>(
    helpers: &Helpers,
    disk: &D,
    write: &Write,
    mut file: D::File,
    size: u64,
) -> Result<(), FilesError> {
    let mut encoder = Encoder::new(write.target, &write.text);
    let mut piece = Vec::new();
    let mut pending = Vec::new();
    loop {
        let more = encoder.next(&mut piece);
        pending.extend_from_slice(&piece);
        if pending.len() >= PIECE || (!more && !pending.is_empty()) {
            let disk = disk.clone();
            let bytes = std::mem::take(&mut pending);
            let (returned, mut bytes, written) = helpers.run(Operation::FsWrite, move || {
                let mut file = file;
                let written = disk.write(&mut file, &bytes);
                (file, bytes, written)
            })?;
            written.map_err(|failure| error::from_io(&failure))?;
            bytes.clear();
            (file, pending) = (returned, bytes);
        }
        if !more {
            break;
        }
    }
    let disk = disk.clone();
    helpers
        .run(Operation::FsWrite, move || disk.finish(file, size))?
        .map_err(|failure| error::from_io(&failure))
}

/// Runs `work` with a clone of `disk` and `path` on a helper.
fn call<D: Disk, T: Send + 'static>(
    helpers: &Helpers,
    disk: &D,
    path: &Path,
    work: impl FnOnce(&D, &Path) -> io::Result<T> + Send + 'static,
) -> Result<T, FilesError> {
    let (disk, path) = (disk.clone(), path.to_path_buf());
    helpers
        .run(Operation::FsWrite, move || work(&disk, &path))?
        .map_err(|failure| error::from_io(&failure))
}

/// A failure after the commit point, which keeps its cause's operating system error.
fn incomplete(cause: FilesError) -> FilesError {
    let mut failure = FilesError::new(ErrorCode::WriteIncomplete);
    failure.os_error = cause.os_error;
    failure
}

fn readonly(permissions: &Permissions) -> bool {
    entry::readonly(permissions)
}

/// Clears the read-only attribute on Windows, or sets the owner write bit elsewhere.
#[cfg(windows)]
fn make_writable(permissions: &mut Permissions) {
    permissions.set_readonly(false);
}

#[cfg(unix)]
fn make_writable(permissions: &mut Permissions) {
    use std::os::unix::fs::PermissionsExt as _;
    permissions.set_mode(permissions.mode() | 0o200);
}

#[cfg(test)]
mod tests;
