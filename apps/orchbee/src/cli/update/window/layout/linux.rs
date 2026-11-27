//! The Linux arrangement of the window store: the entry and the person's menu entry (see the parent
//! module).
use super::entry::{remove_stale_copies, same_bytes, write_copy};
use crate::{cli::update::menu_entry::MenuEntry, release_store::open_release_file};
use std::{fs, path::Path};

/// Places the entry, then the menu entry and icons of the store root `root`.
pub(super) fn arrange(root: &Path, desktop: &Path, active: &str) {
    let stem = orchbee_local_runtime::environment_stem();
    let version = desktop.join("versions").join(active);
    remove_stale_copies(desktop);
    if place_entry(desktop, &version, stem)
        && let Some(menu) = MenuEntry::of(root, desktop.join(format!("{stem}-desktop")))
    {
        menu.place(&version);
    }
}

/// Makes the entry `<stem>-desktop` a byte copy of the active window version's launcher and says
/// whether it is one now. A different entry is replaced by renaming a flushed `.new-*` copy over
/// it, which a running launcher never prevents here. A version without a launcher places nothing.
fn place_entry(desktop: &Path, version: &Path, stem: &str) -> bool {
    let launcher = format!("resources/cohort/{stem}-desktop-launcher");
    let Ok(mut source) = open_release_file(version, &launcher) else {
        return false;
    };
    let entry = desktop.join(format!("{stem}-desktop"));
    if same_bytes(&entry, &mut source).unwrap_or(false) {
        return true;
    }
    let Ok(source) = open_release_file(version, &launcher) else {
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
