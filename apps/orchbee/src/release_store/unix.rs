//! macOS and Linux root preparation, stage allocation and filesystem mutations after private
//! publisher admission. A missing root is created here, with each missing folder above it, as a
//! private folder, so the publisher lock admits the root by the private rule alone: no folder above
//! it needs to be private, as a home folder others may read is not. An existing root is left to
//! that admission, which never repairs it. No exclusive directory rename is used: every rename here
//! first checks that its target is absent, which the writer lock keeps free of other Orchbee
//! writers.
use orchbee_local_runtime::private_filesystem as permissions;
use std::{
    fs::{self, DirBuilder, File},
    io,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// The mode of every folder created here, as the private rule asks.
const PRIVATE_MODE: u32 = 0o700;

pub(super) fn prepare_root(root: &Path) -> io::Result<()> {
    if let Some(parent) = root.parent() {
        DirBuilder::new()
            .recursive(true)
            .mode(PRIVATE_MODE)
            .create(parent)?;
    }
    match DirBuilder::new().mode(PRIVATE_MODE).create(root) {
        Ok(()) => permissions::protect_new(&mut permissions::open_directory(root, true)?),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

pub(super) fn create_stage(root: &Path) -> io::Result<(PathBuf, Vec<File>)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    for _ in 0..32 {
        let index = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = root.join(format!(".stage-{}-{index}", std::process::id()));
        match DirBuilder::new().mode(PRIVATE_MODE).create(&path) {
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
    DirBuilder::new().mode(PRIVATE_MODE).create(path)
}

/// Renames `stage` to the absent `target`; a rename would otherwise replace an empty folder there.
pub(super) fn publish(stage: &Path, target: &Path) -> io::Result<()> {
    rename_new(stage, target)
}

/// Renames `directory` to a fresh `.remove-<process>-<number>` in `root` and returns that path.
pub(super) fn move_aside(root: &Path, directory: &Path) -> io::Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    for _ in 0..32 {
        let index = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = root.join(format!(".remove-{}-{index}", std::process::id()));
        // A crashed run's leftover may hold this name; a rename never replaces it.
        match rename_new(directory, &path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::ErrorKind::AlreadyExists.into())
}

fn rename_new(source: &Path, target: &Path) -> io::Result<()> {
    match fs::symlink_metadata(target) {
        Ok(_) => Err(io::ErrorKind::AlreadyExists.into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::rename(source, target),
        Err(error) => Err(error),
    }
}

/// The names of the real directories in `folder`; links and files are left out.
pub(super) fn directories(folder: &Path) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        // The listing's own type, so a link is never followed to read it.
        if entry.file_type()?.is_dir()
            && let Ok(name) = entry.file_name().into_string()
        {
            names.push(name);
        }
    }
    Ok(names)
}

/// Creates the symbolic link `path` naming `target`, never replacing anything at `path`.
pub(super) fn create_link(target: &str, path: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, path)
}

/// Deletes a directory tree without following a link inside it.
pub(super) fn remove_directory(path: &Path) -> io::Result<()> {
    fs::remove_dir_all(path)
}

/// Makes a staged program file executable by its owner alone, as the cohort's admission asks of
/// its programs; the manifest records no file modes, and a stage creates every file private and
/// not executable.
pub(super) fn mark_executable(file: &File) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(PRIVATE_MODE))
}
