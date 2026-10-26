//! Windows stage allocation and filesystem mutations after private publisher admission.
use orchbee_local_runtime::private_filesystem as permissions;
use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

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

pub(super) fn remove_stage(path: &Path) -> io::Result<()> {
    fs::remove_dir_all(path)
}

#[cfg(test)]
mod tests;
