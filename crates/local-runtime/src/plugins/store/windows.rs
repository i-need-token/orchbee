//! Windows registry admission, local-volume observation, and pinned private folders.
use super::Error;
use std::{
    fs::File,
    io,
    path::{Path, Prefix},
};
pub(super) fn require() -> Result<(), Error> {
    Ok(())
}
pub(super) fn fixed_local_volume(path: &Path) -> bool {
    let Some(prefix) = path.components().next() else {
        return false;
    };
    let drive = match prefix {
        std::path::Component::Prefix(prefix) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter,
            _ => return false,
        },
        _ => return false,
    };
    let root = format!("{}:\\", char::from(drive));
    winsafe::GetDriveType(Some(&root)) == winsafe::co::DRIVE::FIXED
}
/// Creates `path` and its missing ancestors as private folders and holds each without delete
/// sharing until the handles drop, so none is replaced while a source moves or builds there.
pub(super) fn pin_private(path: &Path) -> io::Result<Vec<File>> {
    crate::data_root::create_state_directory(path)
}
