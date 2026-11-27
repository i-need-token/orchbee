//! The Windows arrangement of the window store: the entry, flat-layout leftovers and a leftover
//! `desktop.previous` (see the parent module).
use super::super::super::running_programs::{
    no_process_runs_from, no_process_runs_from_except, same,
};
use super::entry::{COPY, entries, remove_stale_copies, same_bytes, write_copy};
use crate::release_store::{ReleaseStore, numbered_name, open_release_file, transient_name};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// The folder in the store root that Setup moved the previous flat window into before the
/// window self-update release.
const PREVIOUS_INSTALLATION: &str = "desktop.previous";
/// The prefix of a running flat-layout entry moved aside so the launcher can take its path.
const FLAT_ASIDE: &str = ".remove-flat-";

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
/// one now. A different entry is replaced by renaming a flushed `.new-*` copy over it. When
/// Windows refuses because a running window holds the entry, a launcher moves aside to
/// `.remove-flat-<process>-<number>.exe` first (and moves back if the second rename fails), so
/// the new launcher takes its path at once. A flat Electron entry never moves: the placement
/// waits until that window quits, and the flat files stay intact meanwhile. A version without a
/// launcher places nothing.
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
    if !flat_electron_entry(desktop)
        && fs::symlink_metadata(&entry).is_ok_and(|metadata| !metadata.is_dir())
        && let Some(aside) = move_entry_aside(desktop, &entry)
    {
        if fs::rename(&copy, &entry).is_ok() {
            let _ = fs::remove_file(&aside);
            return true;
        }
        let _ = fs::rename(&aside, &entry);
    }
    let _ = fs::remove_file(&copy);
    false
}

/// Whether the entry is a flat Electron window's executable: `resources\app.asar`, which only a
/// flat layout has, lies beside it. Anything at that name counts, so a doubtful entry never moves.
fn flat_electron_entry(desktop: &Path) -> bool {
    fs::symlink_metadata(desktop.join("resources").join("app.asar")).is_ok()
}

/// Renames a running `entry` to a fresh `desktop\.remove-flat-<process>-<number>.exe`, keeping
/// the `.exe` suffix so both the process scan and the write probe recognize it until it exits.
fn move_entry_aside(desktop: &Path, entry: &Path) -> Option<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    for _ in 0..32 {
        let aside = desktop.join(format!(
            "{FLAT_ASIDE}{}-{}.exe",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::symlink_metadata(&aside) {
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return None,
        }
        return fs::rename(entry, &aside).ok().map(|()| aside);
    }
    None
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
/// uses them, while `.remove-flat-*` stays outside so its flat files wait until that process
/// ends and then go with it.
fn layout_name(name: &str, entry: &str) -> bool {
    [
        "versions",
        "active.json",
        "active.pending",
        "install.lock",
        "setup-launch.json",
        entry,
    ]
    .iter()
    .any(|own| name.eq_ignore_ascii_case(own))
        || transient_name(name)
        || numbered_name(name, COPY)
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

/// Whether `path` is a real folder, no link or junction.
fn real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir() && !metadata.is_symlink())
}
