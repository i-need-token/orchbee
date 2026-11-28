//! One item at a time: `fs.create`, `fs.rename` and `fs.exists`, and the steps
//! [`transfer`](super::transfer) builds on. A new item never replaces one that appears at its path
//! meanwhile: it is created exclusively, and an item moves by renaming onto an empty item that
//! claimed its new name. Whether a name is taken is the filesystem's to say, by its own
//! comparison, so on Windows and macOS a name taken in another case is taken. A rename to the same
//! location in another case changes the item's spelling directly, as VS Code renames it.

use std::{
    ffi::OsStr,
    fs::{self, File, FileType},
    io,
    path::Path,
};

use orchbee_files_protocol::{CreateKind, Entry, ErrorCode, FileName, FilesError, Operation};

use super::names;
use crate::{blocking::Helpers, entry, error, scope::path::NativePath};

/// `fs.create` of `path`, with the folders above it.
///
/// # Errors
///
/// `exists` where something is at `path`, and the operating system's other failures.
pub(super) fn create(
    helpers: &Helpers,
    path: &NativePath,
    kind: CreateKind,
) -> Result<Entry, FilesError> {
    let path = path.clone();
    run(helpers, Operation::FsCreate, move || {
        if let Some(folder) = path.as_path().parent() {
            create_folders(folder)?;
        }
        match kind {
            CreateKind::File => File::create_new(path.as_path()).map(drop),
            CreateKind::Directory => fs::create_dir(path.as_path()),
        }?;
        entry::read(path.as_path(), OsStr::new(path.name()))
    })
}

/// `fs.rename` of `from` to `to`, with the folders above `to`.
///
/// # Errors
///
/// `invalid-request` for a path below `from`, `exists` where another item is at `to`, and the
/// operating system's other failures.
pub(super) fn rename(
    helpers: &Helpers,
    from: &NativePath,
    to: &NativePath,
) -> Result<Entry, FilesError> {
    let same = to.key() == from.key();
    if !same && to.is_within(from) {
        return Err(FilesError::new(ErrorCode::InvalidRequest));
    }
    let (from, to) = (from.clone(), to.clone());
    run(helpers, Operation::FsRename, move || {
        let kind = fs::symlink_metadata(from.as_path())?.file_type();
        if !same {
            if let Some(folder) = to.as_path().parent() {
                create_folders(folder)?;
            }
            relocate(from.as_path(), to.as_path(), &kind)?;
        } else if from.name() != to.name() {
            respell(&from, &to)?;
        }
        entry::read(to.as_path(), OsStr::new(to.name()))
    })
}

/// Renames `from` to `to`, the same location by this machine's comparison in another spelling.
/// A folder that lists `to`'s exact name besides `from`'s compares names case-sensitively, so
/// there they are two items; one that lists `to`'s name alone already spells the item so.
fn respell(from: &NativePath, to: &NativePath) -> io::Result<()> {
    let folder = to.as_path().parent().unwrap_or(to.as_path());
    let (mut spelled, mut listed) = (false, false);
    for item in fs::read_dir(folder)? {
        let name = item?.file_name();
        spelled |= name == OsStr::new(to.name());
        listed |= name == OsStr::new(from.name());
    }
    match (spelled, listed) {
        (false, _) => fs::rename(from.as_path(), to.as_path()),
        (true, false) => Ok(()),
        (true, true) => Err(io::ErrorKind::AlreadyExists.into()),
    }
}

/// `fs.exists`: which of `names` the folder at `directory` holds. A name this machine cannot hold
/// is never taken.
///
/// # Errors
///
/// `not-directory` when `directory` is not a folder, and the operating system's failure to read
/// it or a name in it.
pub(super) fn exists(
    helpers: &Helpers,
    directory: &NativePath,
    names: Vec<FileName>,
) -> Result<Vec<FileName>, FilesError> {
    let directory = directory.clone();
    run(helpers, Operation::FsExists, move || {
        if !fs::metadata(directory.as_path())?.is_dir() {
            return Err(io::ErrorKind::NotADirectory.into());
        }
        let mut existing = Vec::new();
        for name in names {
            let Ok(path) = names::child(&directory, name.as_str()) else {
                continue;
            };
            match fs::symlink_metadata(path.as_path()) {
                Ok(_) => existing.push(name),
                Err(failure) if failure.kind() == io::ErrorKind::NotFound => {}
                Err(failure) => return Err(failure),
            }
        }
        Ok(existing)
    })
}

/// Creates `folder` and the folders above it where they are missing.
///
/// # Errors
///
/// [`io::ErrorKind::NotADirectory`] where an item other than a folder is on the way, and the
/// operating system's other failures.
pub(super) fn create_folders(folder: &Path) -> io::Result<()> {
    fs::create_dir_all(folder).map_err(|failure| {
        // A folder that exists passes, so a taken name on the way is something else.
        if failure.kind() == io::ErrorKind::AlreadyExists {
            io::ErrorKind::NotADirectory.into()
        } else {
            failure
        }
    })
}

/// Moves the item at `from`, of type `kind`, to `to`, where nothing may be: an empty item of the
/// same kind claims `to`, and the rename replaces it, so an item that appears at `to` meanwhile is
/// never replaced. A failed rename gives the claim up.
///
/// # Errors
///
/// [`io::ErrorKind::AlreadyExists`] where something is at `to`,
/// [`io::ErrorKind::CrossesDevices`] where `to` lies on another volume, and the operating system's
/// other failures.
pub(super) fn relocate(from: &Path, to: &Path, kind: &FileType) -> io::Result<()> {
    let folder = folder(kind);
    if folder {
        fs::create_dir(to)?;
    } else {
        File::create_new(to)?;
    }
    let renamed = fs::rename(from, to);
    if renamed.is_err() {
        // The claim is given up only while it is still the empty item it was.
        let _ = if folder {
            fs::remove_dir(to)
        } else {
            match fs::symlink_metadata(to) {
                Ok(facts) if facts.is_file() && facts.len() == 0 => fs::remove_file(to),
                _ => Ok(()),
            }
        };
    }
    renamed
}

/// Removes the item at `path` permanently: a folder with everything in it, and a link itself,
/// never what it names.
pub(super) fn remove(path: &Path) -> io::Result<()> {
    if folder(&fs::symlink_metadata(path)?.file_type()) {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Whether an item of type `kind` is a folder to a rename or a removal: a directory, and on
/// Windows a link to one, such as a junction.
pub(super) fn folder(kind: &FileType) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTypeExt;
        kind.is_dir() || kind.is_symlink_dir()
    }
    #[cfg(not(windows))]
    {
        kind.is_dir()
    }
}

/// Runs one filesystem step whose time grows with the item, such as copying a file's content or
/// removing a folder, on the mutation's own thread: under a helper's deadline it would answer
/// `timeout` while it went on, and hold a helper that the process's other filesystem work needs.
/// The steps before it run on helpers and reach the same volumes, so a volume that does not answer
/// is a `timeout` before such a step starts.
pub(super) fn long<T>(work: impl FnOnce() -> io::Result<T>) -> Result<T, FilesError> {
    work().map_err(|failure| error::from_io(&failure))
}

/// Runs one filesystem step of `operation` on a helper.
pub(super) fn run<T: Send + 'static>(
    helpers: &Helpers,
    operation: Operation,
    work: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> Result<T, FilesError> {
    helpers
        .run(operation, work)?
        .map_err(|failure| error::from_io(&failure))
}

#[cfg(test)]
mod tests;
