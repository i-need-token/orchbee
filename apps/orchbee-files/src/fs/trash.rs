//! The trash of the machine this process runs on: the Recycle Bin through this process's own
//! `IFileOperation` adapter, the macOS Trash through `NSFileManager trashItemAtURL`, which needs no
//! Finder automation permission and offers no Put Back, and the freedesktop Trash, home or per
//! volume, also on headless Linux. A path the trash does not take stays where it is, and the error
//! says whether the machine has no trash for it; the adapter never deletes permanently, which only
//! the Client asks for after such an error.

use crate::git::ports::{Trash, TrashError};
#[cfg(windows)]
use crate::{
    recycle_bin_native::{self, NotRecycled},
    scope::path::{self, NativePath},
};
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;
#[cfg(unix)]
use trash::TrashContext;
#[cfg(target_os = "macos")]
use trash::macos::{DeleteMethod, TrashContextExtMacos};

/// The machine's trash. Each move stands alone, so several threads may move paths at once.
pub(crate) struct MachineTrash;

/// The Recycle Bin refuses whatever the shell would delete permanently instead of recycling,
/// such as an item on a volume without a Recycle Bin, and never shows a window.
#[cfg(windows)]
impl Trash for MachineTrash {
    fn trash(&self, path: &Path) -> Result<(), TrashError> {
        recycle_bin_native::recycle(&located(path)).map_err(|refused| match refused {
            NotRecycled::Unavailable => TrashError::Unavailable,
            NotRecycled::Failed => TrashError::Failed,
        })
    }
}

#[cfg(unix)]
impl Trash for MachineTrash {
    fn trash(&self, path: &Path) -> Result<(), TrashError> {
        if loops(path) {
            return Err(TrashError::Failed);
        }
        context().delete(path).map_err(|error| {
            if no_trash(&error) {
                TrashError::Unavailable
            } else {
                TrashError::Failed
            }
        })
    }
}

/// The entry at `path`, an absolute native path scope admitted, named below its folder's real
/// location, which scope checked: the entry itself stays unresolved, so a link or junction moves
/// without what it names. A folder whose real location has no drive path, such as one on a mapped
/// network drive, keeps `path` as it is.
#[cfg(windows)]
fn located(path: &Path) -> PathBuf {
    path.to_str()
        .and_then(|path| NativePath::parse(path).ok())
        .and_then(|entry| {
            let folder = path::real(&entry.parent()?).ok()?;
            Some(folder.as_path().join(entry.name()))
        })
        .unwrap_or_else(|| path.to_path_buf())
}

/// The crate's trash on this platform.
#[cfg(all(unix, not(target_os = "macos")))]
const fn context() -> TrashContext {
    TrashContext::new()
}

/// `NSFileManager trashItemAtURL` rather than the crate's default of asking Finder, which needs an
/// automation permission a process outside the user's GUI session cannot get.
#[cfg(target_os = "macos")]
fn context() -> TrashContext {
    let mut context = TrashContext::new();
    context.set_delete_method(DeleteMethod::NsFileManager);
    context
}

/// Whether the freedesktop trash could not establish where the trash lives: neither `XDG_DATA_HOME`
/// nor `HOME` is set, or the mount table cannot be read. Its delete path reports only those as
/// unknown errors.
#[cfg(all(unix, not(target_os = "macos")))]
fn no_trash(error: &trash::Error) -> bool {
    matches!(error, trash::Error::Unknown { .. })
}

/// The macOS trash reports a volume without a trash only in presentation text, so every refusal
/// there is a failure.
#[cfg(target_os = "macos")]
const fn no_trash(_: &trash::Error) -> bool {
    false
}

/// Whether the freedesktop trash would never finish moving `path`, a link to a folder. When the
/// volume's own trash cannot be made, the move goes to the home trash on another filesystem by
/// copying; the copy of the link's folder lands on the placeholder file made for the link, and the
/// trash takes that refusal for a name collision and tries the next name without end.
#[cfg(all(unix, not(target_os = "macos")))]
fn loops(path: &Path) -> bool {
    path.symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
        && path.is_dir()
}

/// The macOS Trash moves a link itself.
#[cfg(target_os = "macos")]
const fn loops(_: &Path) -> bool {
    false
}

#[cfg(all(test, windows))]
mod tests;
