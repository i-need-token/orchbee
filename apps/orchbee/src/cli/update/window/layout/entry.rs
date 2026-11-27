//! The window entry both Windows and Linux place in `desktop/`: a byte copy of the active window
//! version's launcher, written as a `.new-*` copy and renamed over the entry, and whether this
//! Client may arrange the store at all.
use crate::release_store::numbered_name;
use orchbee_local_runtime::compare_product_versions;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// The prefix of the entry copy a run writes before its rename.
pub(super) const COPY: &str = ".new-";

/// Whether this Client may arrange the store: it is not older than the store's active window
/// version, by the store's product version precedence. An older Client only retains versions,
/// because the names a newer window version adds under `desktop` are unknown to it.
pub(super) fn may_arrange(active: &str) -> bool {
    compare_product_versions(env!("CARGO_PKG_VERSION"), active).is_ge()
}

/// Whether the regular file `path` holds exactly what `source` reads from its start.
pub(super) fn same_bytes(path: &Path, source: &mut File) -> io::Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() != source.metadata()?.len() {
        return Ok(false);
    }
    let mut file = File::open(path)?;
    let (mut left, mut right) = (vec![0_u8; 64 * 1024], vec![0_u8; 64 * 1024]);
    loop {
        let count = read_full(&mut file, &mut left)?;
        if count != read_full(source, &mut right)? || left[..count] != right[..count] {
            return Ok(false);
        }
        if count == 0 {
            return Ok(true);
        }
    }
}

/// Fills `buffer` from `file` as far as the file goes; the count is short only at its end.
fn read_full(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..])? {
            0 => break,
            count => filled += count,
        }
    }
    Ok(filled)
}

/// Writes and flushes `source` into a fresh `desktop/.new-<process>-<number>`, which on Linux its
/// owner alone may read, write and run.
pub(super) fn write_copy(desktop: &Path, mut source: File) -> Option<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let copy = desktop.join(format!(
        "{COPY}{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o700);
    let written = options.open(&copy).and_then(|mut file| {
        io::copy(&mut source, &mut file)?;
        file.flush()?;
        file.sync_all()
    });
    match written {
        Ok(()) => Some(copy),
        Err(_) => {
            let _ = fs::remove_file(&copy);
            None
        }
    }
}

/// Removes the `.new-*` copies a run that stopped between writing and renaming left; only a
/// holder of both locks writes one.
pub(super) fn remove_stale_copies(desktop: &Path) {
    for (path, name) in entries(desktop) {
        if numbered_name(&name, COPY)
            && fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file())
        {
            let _ = fs::remove_file(&path);
        }
    }
}

/// The entries of `folder` with their names; none when it cannot be listed.
pub(super) fn entries(folder: &Path) -> Vec<(PathBuf, String)> {
    fs::read_dir(folder)
        .map(|listing| {
            listing
                .filter_map(Result::ok)
                .filter_map(|entry| Some((entry.path(), entry.file_name().into_string().ok()?)))
                .collect()
        })
        .unwrap_or_default()
}
