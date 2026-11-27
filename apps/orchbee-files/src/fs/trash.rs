//! The trash of the machine this process runs on: the Recycle Bin through `IFileOperation`, the
//! macOS Trash through `NSFileManager trashItemAtURL`, which needs no Finder automation permission
//! and offers no Put Back, and the freedesktop Trash, home or per volume, also on headless Linux.
//! A path the trash does not take stays where it is, and the error says whether the machine has no
//! trash for it; the adapter never deletes permanently, which only the Client asks for after such
//! an error.

use crate::git::ports::{Trash, TrashError};
use std::path::Path;
use trash::TrashContext;
#[cfg(target_os = "macos")]
use trash::macos::{DeleteMethod, TrashContextExtMacos};

/// `COPYENGINE_E_RECYCLE_FORCE_NUKE` (`sherrors.h`): the volume's Recycle Bin is turned off or
/// absent, as on a network share, so the item could only be deleted permanently.
#[cfg(windows)]
const RECYCLE_FORCE_NUKE: u32 = 0x8027_0036;

/// `COPYENGINE_E_RECYCLE_BIN_NOT_FOUND` (`sherrors.h`): the Recycle Bin could not be found or is
/// unavailable.
#[cfg(windows)]
const RECYCLE_BIN_NOT_FOUND: u32 = 0x8027_003A;

/// The machine's trash. Each move stands alone, so several threads may move paths at once.
pub(crate) struct MachineTrash;

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

/// The crate's trash on this platform.
#[cfg(not(target_os = "macos"))]
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

/// Whether the shell did not recycle because the volume has no Recycle Bin. The crate passes
/// `FOF_WANTNUKEWARNING` under `FOF_NO_UI`, and the shell reports per item why it did not recycle.
#[cfg(windows)]
fn no_trash(error: &trash::Error) -> bool {
    matches!(
        error,
        trash::Error::Os { code, .. }
            if matches!(code.cast_unsigned(), RECYCLE_FORCE_NUKE | RECYCLE_BIN_NOT_FOUND)
    )
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

/// The Recycle Bin and the macOS Trash move a link itself.
#[cfg(any(windows, target_os = "macos"))]
const fn loops(_: &Path) -> bool {
    false
}
