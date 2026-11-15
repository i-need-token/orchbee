//! Windows stage allocation and filesystem mutations after private publisher admission.
use orchbee_local_runtime::private_filesystem as permissions;
use std::{
    fs::{self, File},
    io,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Nothing to prepare: the publisher lock creates a missing root with a protected private access
/// rule, which every folder Windows creates below it inherits.
pub(super) fn prepare_root(_root: &Path) -> io::Result<()> {
    Ok(())
}

pub(super) fn create_stage(root: &Path) -> io::Result<(PathBuf, Vec<File>)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    for _ in 0..32 {
        let index = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = root.join(format!(".stage-{}-{index}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => {
                let pins = match permissions::open_tree(&path) {
                    Ok(pins) => pins,
                    Err(error) => {
                        // Empty directory created by this call; no child can own it.
                        fs::remove_dir(&path)?;
                        return Err(error);
                    }
                };
                return Ok((path, pins));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::ErrorKind::AlreadyExists.into())
}

pub(super) fn create_directory(path: &Path) -> io::Result<()> {
    fs::create_dir(path)
}

pub(super) fn publish(stage: &Path, target: &Path) -> io::Result<()> {
    fs::rename(stage, target)
}

/// Renames `directory` to a fresh `.remove-<process>-<number>` in `root` and returns that path.
pub(super) fn move_aside(root: &Path, directory: &Path) -> io::Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    for _ in 0..32 {
        let index = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = root.join(format!(".remove-{}-{index}", std::process::id()));
        // A crashed run's leftover may hold this name; a rename never replaces it.
        match fs::symlink_metadata(&path) {
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        fs::rename(directory, &path)?;
        return Ok(path);
    }
    Err(io::ErrorKind::AlreadyExists.into())
}

/// The names of the real directories in `folder`; links, junctions and files are left out.
pub(super) fn directories(folder: &Path) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        // The listing's own attributes, so a link is never followed to read them.
        let metadata = entry.metadata()?;
        if metadata.is_dir()
            && metadata.file_attributes() & 0x400 == 0
            && let Ok(name) = entry.file_name().into_string()
        {
            names.push(name);
        }
    }
    Ok(names)
}

/// Deletes a directory tree without following a link inside it.
pub(super) fn remove_directory(path: &Path) -> io::Result<()> {
    fs::remove_dir_all(path)
}

/// Nothing to mark: Windows runs any program file a person may read.
pub(super) fn mark_executable(_file: &File) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests;
