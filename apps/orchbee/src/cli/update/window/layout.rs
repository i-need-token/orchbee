//! What every run that finds the window store does after its window step (AU § Installing a window
//! version, § Moving a flat layout, § Window cleanup), under the store root's `install.lock` and then
//! `desktop/install.lock`, whatever that step did.
//!
//! On Windows and Linux it places the window entry, the program the person starts, as a copy of
//! the active window version's window launcher: `desktop\<stem>-desktop.exe`, the path the Start
//! menu shortcut, the taskbar and the uninstall registration name, from
//! `resources\cohort\<stem>-desktop-launcher.exe`, and `desktop/<stem>-desktop` from
//! `resources/cohort/<stem>-desktop-launcher`. On Windows a running window launcher is renamed
//! aside as `desktop\.remove-flat-<process>-<number>.exe` so the new launcher takes its path at
//! once; a running flat Electron entry, which has `resources\app.asar` beside it, never moves, and
//! its placement waits until that window quits. Once the entry is placed, Windows removes what a
//! flat layout, which a window before the window self-update release kept directly in `desktop\`,
//! left there once no process runs from it, and a `desktop.previous` that an earlier Setup left;
//! Linux writes the person's menu entry and icons (see `menu_entry`). Both then remove window
//! versions as runtime versions go: neither active nor previous, and no process runs from them.
//!
//! On macOS the application bundle the person opens is the entry, which the window launcher backs
//! up and replaces with a copy of `pointer.active`, so cleanup keeps only `pointer.active` plus any
//! stored version a process still runs from. Each step is best effort: whatever runs, cannot be
//! told, or is refused stays for the next run, and a lock another update holds skips the whole
//! step.
//!
//! A Client older than the store's active window version runs only the version retention and
//! arranges nothing under `desktop`: a newer window version adds names there that an older Client
//! does not know, so it must neither move nor remove them as flat-layout leftovers.
use super::super::retention;
use crate::release_store::ReleaseStore;
use orchbee_release_records::Pointer;
use std::{fs, path::Path};

/// Places the entry, moves a flat layout on and removes unused window versions of the window store
/// below the store root `root`. Without a real `desktop` folder there is no window store, and one is
/// never created here; without a window pointer there is no active window version yet, so nothing is
/// placed or removed. A Client older than the active window version only removes unused versions
/// (see the module documentation). Changing this (the install and handover step) needs a manual
/// real-window self-test before merging; its steps are in `apps/desktop/README.md`, Window Update,
/// Manual self-test.
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
    let active = &pointer.active.product;
    #[cfg(windows)]
    if entry::may_arrange(active) {
        windows::arrange(&root_store, &store, active);
    }
    #[cfg(target_os = "linux")]
    if entry::may_arrange(active) {
        linux::arrange(root_store.root(), store.root(), active);
    }
    #[cfg(target_os = "macos")]
    let _ = (&root_store, active);
    #[cfg(target_os = "macos")]
    retention::remove_non_active_by(
        &store,
        &pointer,
        super::super::running_programs::no_process_started_from,
    );
    // Only Orchbee starts programs from a Linux window version, each by absolute path, so the
    // process scan alone tells whether one runs from it.
    #[cfg(target_os = "linux")]
    retention::remove_unused_with(
        &store,
        &pointer,
        true,
        super::super::running_programs::no_process_started_from,
    );
    #[cfg(windows)]
    retention::remove_unused(&store, &pointer);
}

#[cfg(not(target_os = "macos"))]
mod entry;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;
