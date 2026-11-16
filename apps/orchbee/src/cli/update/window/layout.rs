//! What every run that finds the window store does after its window step (AU § Installing a window
//! version, § Moving a flat layout, § Window cleanup), under the store root's `install.lock` and then
//! `desktop/install.lock`, whatever that step did. On Windows it places the window entry
//! `desktop\<stem>-desktop.exe`, the path the Start menu shortcut, the taskbar and the uninstall
//! registration name, as a copy of the active window version's
//! `resources\cohort\<stem>-desktop-launcher.exe`; once the entry is placed it removes what a flat
//! layout, which a window before the window self-update release kept directly in `desktop\`, left
//! there, and it removes a `desktop.previous` that an earlier Setup left. Everywhere it then removes
//! window versions as runtime versions go: neither active nor previous, and no process runs from
//! them. Each step is best effort: whatever runs, cannot be told, or is refused stays for the next
//! run, and a lock another update holds skips the whole step. On macOS the application bundle is the
//! entry, which the window launcher swaps, so only the cleanup runs, and it removes nothing there
//! because no process check can be made.
use super::super::retention;
use crate::release_store::ReleaseStore;
use orchbee_release_records::Pointer;
use std::{fs, path::Path};

/// Places the entry, moves a flat layout on and removes unused window versions of the window store
/// below the store root `root`. Without a real `desktop` folder there is no window store, and one is
/// never created here; without a window pointer there is no active window version yet, so nothing is
/// placed or removed.
pub(in crate::cli::update) fn tend(root: &Path) {
    let desktop = root.join(super::FOLDER);
    if !fs::symlink_metadata(&desktop).is_ok_and(|metadata| metadata.is_dir()) {
        return;
    }
    // The store root's lock first, as every writer takes it; a held one is another update.
    let Ok(root_store) = ReleaseStore::acquire(root) else {
        return;
    };
    let Ok(store) = ReleaseStore::acquire(&desktop) else {
        return;
    };
    let Some(pointer) = store
        .pointer()
        .ok()
        .flatten()
        .and_then(|bytes| Pointer::parse(&bytes))
    else {
        return;
    };
    #[cfg(windows)]
    windows::arrange(&root_store, &store, &pointer.active.product);
    #[cfg(not(windows))]
    let _ = &root_store;
    retention::remove_unused(&store, &pointer);
}

#[cfg(windows)]
mod windows {
    use super::super::super::running_programs::{
        no_process_runs_from, no_process_runs_from_except, same,
    };
    use crate::release_store::{ReleaseStore, numbered_name, open_release_file};
    use std::{
        fs::{self, File, OpenOptions},
        io::{self, Read, Write},
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    /// The folder in the store root that Setup moved the previous flat window into before the
    /// window self-update release.
    const PREVIOUS_INSTALLATION: &str = "desktop.previous";
    /// The prefix of the entry copy a run writes before its rename.
    const COPY: &str = ".new-";

    /// Places the entry and, once it is placed, removes what a flat layout left; then removes the
    /// store root's leftover `desktop.previous` once nothing runs from it.
    pub(super) fn arrange(root_store: &ReleaseStore, store: &ReleaseStore, active: &str) {
        let (root, desktop) = (root_store.root(), store.root());
        let stem = orchbee_local_runtime::environment_stem();
        let entry = format!("{stem}-desktop.exe");
        remove_stale_copies(desktop);
        if place_entry(desktop, active, stem, &entry) {
            remove_flat_layout(store, &entry);
        }
        let previous = root.join(PREVIOUS_INSTALLATION);
        if real_directory(&previous) && no_process_runs_from(&previous).unwrap_or(false) {
            let _ = root_store.remove_aside(&previous);
        }
    }

    /// Makes the entry a byte copy of the active window version's launcher and says whether it is
    /// one now. A different entry is replaced by one rename of a flushed `.new-*` copy, which
    /// Windows refuses while the entry runs, as the flat window does until it quits; the copy then
    /// goes and the next run tries again. A version without a launcher places nothing.
    fn place_entry(desktop: &Path, active: &str, stem: &str, entry: &str) -> bool {
        let version = desktop.join("versions").join(active);
        let launcher = format!("resources/cohort/{stem}-desktop-launcher.exe");
        let Ok(mut source) = open_release_file(&version, &launcher) else {
            return false;
        };
        let entry = desktop.join(entry);
        if same_bytes(&entry, &mut source).unwrap_or(false) {
            return true;
        }
        let Ok(source) = open_release_file(&version, &launcher) else {
            return false;
        };
        let Some(copy) = write_copy(desktop, source) else {
            return false;
        };
        if fs::rename(&copy, &entry).is_ok() {
            return true;
        }
        let _ = fs::remove_file(&copy);
        false
    }

    /// Whether the regular file `path` holds exactly what `source` reads from its start.
    fn same_bytes(path: &Path, source: &mut File) -> io::Result<bool> {
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

    /// Writes and flushes `source` into a fresh `desktop\.new-<process>-<number>`.
    fn write_copy(desktop: &Path, mut source: File) -> Option<PathBuf> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let copy = desktop.join(format!(
            "{COPY}{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let written = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&copy)
            .and_then(|mut file| {
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
    fn remove_stale_copies(desktop: &Path) {
        for (path, name) in entries(desktop) {
            if numbered_name(&name, COPY)
                && fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file())
            {
                let _ = fs::remove_file(&path);
            }
        }
    }

    /// Step 4 of moving a flat layout, once the entry is placed: the flat bundled release directory
    /// `resources\cohort\`, which a Session an older window started may run from, goes whole once
    /// nothing runs from it, as a window version goes; everything else outside the layout's names,
    /// the flat window's own files, goes file by file once nothing runs from any of it.
    fn remove_flat_layout(store: &ReleaseStore, entry: &str) {
        let desktop = store.root();
        let (layout, flat): (Vec<_>, Vec<_>) = entries(desktop)
            .into_iter()
            .partition(|(_, name)| layout_name(name, entry));
        if flat.is_empty() {
            return;
        }
        let cohort = desktop.join("resources").join("cohort");
        if real_directory(&cohort) && no_process_runs_from(&cohort).unwrap_or(false) {
            let _ = store.remove_aside(&cohort);
        }
        let skip: Vec<PathBuf> = layout
            .into_iter()
            .map(|(path, _)| path)
            .chain([cohort.clone()])
            .collect();
        if no_process_runs_from_except(desktop, &skip).unwrap_or(false) {
            for (path, _) in flat {
                remove_tree_except(&path, &cohort);
            }
        }
    }

    /// The window store's own names, which no flat-layout step removes; no packaged window file
    /// uses them.
    fn layout_name(name: &str, entry: &str) -> bool {
        [
            "versions",
            "active.json",
            "active.pending",
            "install.lock",
            entry,
        ]
        .iter()
        .any(|own| name.eq_ignore_ascii_case(own))
            || [".stage-", COPY, ".remove-"]
                .iter()
                .any(|prefix| name.starts_with(prefix))
    }

    /// Deletes `path` file by file without following a link, keeping `keep` and the folders that
    /// hold it; whatever cannot be deleted stays.
    fn remove_tree_except(path: &Path, keep: &Path) {
        let Ok(metadata) = fs::symlink_metadata(path) else {
            return;
        };
        if same(path, keep) {
            return;
        }
        if metadata.is_dir() && !metadata.is_symlink() {
            for (child, _) in entries(path) {
                remove_tree_except(&child, keep);
            }
            let _ = fs::remove_dir(path);
        } else if metadata.is_dir() {
            let _ = fs::remove_dir(path);
        } else {
            let _ = fs::remove_file(path);
        }
    }

    /// The entries of `folder` with their names; none when it cannot be listed.
    fn entries(folder: &Path) -> Vec<(PathBuf, String)> {
        fs::read_dir(folder)
            .map(|listing| {
                listing
                    .filter_map(Result::ok)
                    .filter_map(|entry| Some((entry.path(), entry.file_name().into_string().ok()?)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Whether `path` is a real folder, no link or junction.
    fn real_directory(path: &Path) -> bool {
        fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir() && !metadata.is_symlink())
    }
}
