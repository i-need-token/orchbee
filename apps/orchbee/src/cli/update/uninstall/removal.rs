//! Removal that never follows a link: a link or junction is removed as a link, a file is removed,
//! and a folder is emptied and then removed. What could not be removed is reported, never retried
//! here, so a second run finds exactly what remains.
use super::plan::Presence;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// What one path holds, read without following a link. Any reparse point counts as a link on
/// Windows, so a junction and a cloud placeholder are never entered.
pub(super) fn presence(path: &Path) -> Presence {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link(&metadata) => Presence::Link,
        Ok(metadata) if metadata.is_dir() => Presence::Directory,
        Ok(_) => Presence::File,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Presence::Missing,
        Err(_) => Presence::Unreadable,
    }
}

#[cfg(windows)]
fn is_link(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    /// `FILE_ATTRIBUTE_REPARSE_POINT`.
    const REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink() || metadata.file_attributes() & REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

/// What a removal left: files and folders that still exist, deepest folders first, and the first
/// error met.
#[derive(Debug, Default)]
pub(super) struct Leftovers {
    pub(super) files: Vec<PathBuf>,
    pub(super) folders: Vec<PathBuf>,
    pub(super) error: Option<io::ErrorKind>,
}

impl Leftovers {
    pub(super) fn is_empty(&self) -> bool {
        self.files.is_empty() && self.folders.is_empty()
    }

    fn keep_file(&mut self, path: &Path, error: &io::Error) {
        self.files.push(path.to_owned());
        self.error.get_or_insert(error.kind());
    }

    fn keep_folder(&mut self, path: &Path, error: &io::Error) {
        self.folders.push(path.to_owned());
        self.error.get_or_insert(error.kind());
    }
}

/// Removes `path` and everything below it without following any link, continuing past entries
/// that cannot be removed so that the leftovers name everything that remains.
pub(super) fn remove_tree(path: &Path) -> Leftovers {
    let mut leftovers = Leftovers::default();
    remove_into(path, &mut leftovers);
    leftovers
}

fn remove_into(path: &Path, leftovers: &mut Leftovers) {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return,
        Err(error) => return leftovers.keep_file(path, &error),
    };
    if is_link(&metadata) {
        // A folder link or junction is removed as a folder entry, never entered.
        let removed = if link_is_folder(&metadata) {
            fs::remove_dir(path)
        } else {
            fs::remove_file(path)
        };
        if let Err(error) = removed {
            leftovers.keep_file(path, &error);
        }
        return;
    }
    if !metadata.is_dir() {
        if let Err(error) = remove_file(path, &metadata) {
            leftovers.keep_file(path, &error);
        }
        return;
    }
    match fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => remove_into(&entry.path(), leftovers),
                    Err(error) => {
                        leftovers.error.get_or_insert(error.kind());
                    }
                }
            }
        }
        Err(error) => {
            leftovers.error.get_or_insert(error.kind());
        }
    }
    match fs::remove_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => leftovers.keep_folder(path, &error),
    }
}

/// Removes a file, clearing a read-only attribute first when that alone refuses the removal, as
/// Git leaves its object files.
fn remove_file(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error)
            if error.kind() == io::ErrorKind::PermissionDenied
                && metadata.permissions().readonly() =>
        {
            clear_read_only(path)?;
            fs::remove_file(path)
        }
        other => other,
    }
}

#[cfg(windows)]
fn clear_read_only(path: &Path) -> io::Result<()> {
    let name = path.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    let attributes = winsafe::GetFileAttributes(name).map_err(io::Error::other)?;
    winsafe::SetFileAttributes(name, attributes & !winsafe::co::FILE_ATTRIBUTE::READONLY)
        .map_err(io::Error::other)
}

#[cfg(not(windows))]
fn clear_read_only(_path: &Path) -> io::Result<()> {
    // A POSIX file's own mode never refuses its removal; the folder's does.
    Err(io::ErrorKind::PermissionDenied.into())
}

#[cfg(windows)]
fn link_is_folder(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    /// `FILE_ATTRIBUTE_DIRECTORY`.
    const DIRECTORY: u32 = 0x10;
    metadata.file_attributes() & DIRECTORY != 0
}

#[cfg(not(windows))]
fn link_is_folder(_metadata: &fs::Metadata) -> bool {
    false
}
