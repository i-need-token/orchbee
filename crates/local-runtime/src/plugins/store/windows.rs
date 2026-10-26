//! Windows registry admission and local-volume observation.
use super::Error;
use std::path::{Path, Prefix};
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
