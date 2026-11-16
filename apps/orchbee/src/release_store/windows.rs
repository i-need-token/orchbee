//! Windows stage allocation and filesystem mutations after private publisher admission.
use orchbee_local_runtime::private_filesystem as permissions;
use std::{
    fs::{self, File},
    io,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// What the Desktop installer writes into the store root before it runs the bundled `install`: the
/// window application, the folder the previous one waits in, and its uninstaller with that log.
const INSTALLER_ENTRIES: [&str; 4] = [
    "desktop",
    "desktop.previous",
    "unins000.exe",
    "unins000.dat",
];

/// Makes the store root private before the publisher lock admits it, pinning the folders above it
/// without judging them. A missing root below an existing parent is created with the protected
/// private access rule in place of the one it inherits. An existing root that admission refuses
/// only for its access list, as one Setup created below a `%LOCALAPPDATA%\Programs` that lets
/// another account in, is made private when the user, SYSTEM or Administrators owns it, it is no
/// link and it holds nothing but the installer's own entries; Windows carries the new rule down to
/// them, and the store never reads them anyway. Any other refusal stands. A root whose parent is
/// missing is left to the publisher lock, which creates it with its missing ancestors.
pub(super) fn prepare_root(root: &Path) -> io::Result<()> {
    match permissions::create_private_root(root, &INSTALLER_ENTRIES) {
        Ok(_pins) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
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
