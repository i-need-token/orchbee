//! `history.restore`'s write: an entry's content over its file, as VS Code copies the entry over
//! the file, but in place, as `fs.write` saves. An existing file opens for writing without
//! truncation, then is cut to nothing through that handle and rewritten from its start, so its
//! attributes, links, ownership and link target survive; its length is never set after the content,
//! which would fill with zeros a file another writer cut meanwhile. A missing file is created with
//! the folders missing above it, as VS Code's copy creates them. A read-only file is `read-only`, a
//! folder `is-directory`, and anything that is neither file nor folder is refused, since opening it
//! could block. Once the file is cut or created, a failure is `write-incomplete` with its cause. An
//! entry keeps at most 256 KiB, so the whole write is one helper call.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write as _},
    path::Path,
    time::SystemTime,
};

use orchbee_files_protocol::{ErrorCode, FilesError, Operation};

use crate::{blocking::Helpers, entry, error, scope::path::NativePath};

/// Writes `content` over the file at `path`, which scope admitted, on a helper, and returns the
/// file's modification time afterwards in milliseconds since the Unix epoch.
///
/// # Errors
///
/// As the module says, and the helper's own failures.
pub(super) fn write(
    helpers: &Helpers,
    path: &NativePath,
    content: Vec<u8>,
) -> Result<i64, FilesError> {
    let path = path.as_path().to_path_buf();
    helpers.run(Operation::HistoryRestore, move || rewrite(&path, &content))?
}

fn rewrite(path: &Path, content: &[u8]) -> Result<i64, FilesError> {
    let existing = match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => return Err(FilesError::new(ErrorCode::IsDirectory)),
        Ok(metadata) if !metadata.is_file() => return Err(FilesError::new(ErrorCode::Failed)),
        Ok(metadata) if entry::readonly(&metadata.permissions()) => {
            return Err(FilesError::new(ErrorCode::ReadOnly));
        }
        Ok(_) => true,
        Err(failure) if failure.kind() == io::ErrorKind::NotFound => false,
        Err(failure) => return Err(error::from_io(&failure)),
    };
    if !existing && let Some(folder) = path.parent() {
        create_folders(folder).map_err(|failure| error::from_io(&failure))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(!existing)
        .open(path)
        .map_err(|failure| error::from_io(&failure))?;
    if existing {
        file.set_len(0)
            .map_err(|failure| error::from_io(&failure))?;
    }
    file.write_all(content)
        .map_err(|failure| incomplete(&failure))?;
    drop(file);
    // The content is written; a file whose time cannot be read back now dates from now.
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or_else(|_| SystemTime::now());
    Ok(entry::milliseconds(Ok(modified)).get())
}

/// Creates `folder` and those missing above it; an item other than a folder on the way is
/// [`io::ErrorKind::NotADirectory`].
fn create_folders(folder: &Path) -> io::Result<()> {
    fs::create_dir_all(folder).map_err(|failure| {
        // A folder that exists passes, so a taken name on the way is something else.
        if failure.kind() == io::ErrorKind::AlreadyExists {
            io::ErrorKind::NotADirectory.into()
        } else {
            failure
        }
    })
}

/// A failure once the file opened for writing, which keeps its cause's operating system error.
fn incomplete(cause: &io::Error) -> FilesError {
    let mut failure = FilesError::new(ErrorCode::WriteIncomplete);
    failure.os_error = error::from_io(cause).os_error;
    failure
}
