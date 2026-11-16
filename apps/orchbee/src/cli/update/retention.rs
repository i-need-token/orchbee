//! Bounded disk: which runtime versions a store keeps. After a completion moves the pointer, under
//! the same `install.lock`, every version that is neither active nor previous and from which no
//! process runs leaves the store, and so do the folders that earlier removals and stages left. A
//! version or leftover that cannot go, because something runs from it, nothing can be told, or the
//! system refuses, stays for the next completion; the store moves a version aside before deleting
//! it, so `versions\` only ever holds whole versions. A Session or Host refers to a version only
//! through the processes its registrations name, each started from that version's folder, and an
//! Agent observation Capsule is a copy in the data root that names no store version, so the process
//! check is the whole reference check. The window store's arrangement removes window versions by the
//! same rule, with the process check its platform gives a window version.
use super::running_programs::no_process_runs_from;
use crate::release_store::ReleaseStore;
use orchbee_release_records::Pointer;
use std::{io, path::Path};

/// Removes every version of `store` that `pointer`, the one the completion just wrote, names
/// neither active nor previous, and every leftover folder, when no process runs from it. Each
/// failure keeps what it concerns and goes on to the next.
pub(super) fn remove_unused(store: &ReleaseStore, pointer: &Pointer) {
    remove_unused_by(store, pointer, no_process_runs_from);
}

/// [`remove_unused`] with `unused` as the process check: true when no process runs from a folder;
/// false or an error keeps it.
pub(super) fn remove_unused_by(
    store: &ReleaseStore,
    pointer: &Pointer,
    unused: fn(&Path) -> io::Result<bool>,
) {
    let kept = |version: &str| {
        std::iter::once(&pointer.active)
            .chain(&pointer.previous)
            .any(|entry| entry.product.eq_ignore_ascii_case(version))
    };
    // A store that cannot be listed keeps everything; the next completion lists it again.
    for leftover in store.leftovers().unwrap_or_default() {
        if unused(&leftover).unwrap_or(false) {
            // What cannot be deleted now stays among the leftovers for the next completion.
            let _ = store.remove_leftover(&leftover);
        }
    }
    for version in store.versions().unwrap_or_default() {
        if kept(&version) {
            continue;
        }
        let Ok(directory) = store.version_path(&version) else {
            continue;
        };
        if unused(&directory).unwrap_or(false) {
            // A version that cannot be moved aside stays whole, and what cannot be deleted stays
            // among the leftovers; the next completion tries again.
            let _ = store.remove_version(&version);
        }
    }
}
