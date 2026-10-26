//! Windows private-tree removal retains the original ACL and reparse-point guards.
use super::{Path, PluginProvisionExecutor, PluginProvisionFailure, fs, io};
use crate::windows_permissions as permissions;
pub(super) fn remove_owned(
    path: &Path,
    depth: usize,
    budget: &mut usize,
    executor: &mut impl PluginProvisionExecutor,
) -> Result<(), PluginProvisionFailure> {
    executor
        .checkpoint()
        .map_err(PluginProvisionFailure::Command)?;
    if depth > 128 || *budget == 0 {
        return Err(PluginProvisionFailure::CleanupIncomplete);
    }
    *budget -= 1;
    // Ancestors are pinned without delete sharing. Reject every reparse point,
    // foreign ACL and hard-linked file instead of crossing a managed boundary.
    let pin = permissions::open_directory(path, false)?;
    permissions::require_private(&pin)?;
    for entry in fs::read_dir(path)? {
        executor
            .checkpoint()
            .map_err(PluginProvisionFailure::Command)?;
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            remove_owned(&entry.path(), depth + 1, budget, executor)?;
        } else {
            if *budget == 0 {
                return Err(PluginProvisionFailure::CleanupIncomplete);
            }
            *budget -= 1;
            let file = permissions::open_file(&entry.path(), false, false)?;
            if file.metadata()?.permissions().readonly() {
                let path = entry.path();
                let name = path
                    .to_str()
                    .ok_or(PluginProvisionFailure::CleanupIncomplete)?;
                let attributes = winsafe::GetFileAttributes(name).map_err(io::Error::other)?;
                winsafe::SetFileAttributes(
                    name,
                    attributes & !winsafe::co::FILE_ATTRIBUTE::READONLY,
                )
                .map_err(io::Error::other)?;
            }
            drop(file);
            fs::remove_file(entry.path())?;
        }
    }
    drop(pin);
    fs::remove_dir(path).map_err(PluginProvisionFailure::from)
}
