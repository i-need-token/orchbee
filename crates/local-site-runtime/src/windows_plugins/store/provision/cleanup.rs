//! Cleanup only generation paths recorded before creation, never arbitrary source operands.
use super::{
    Path, PluginProvisionExecutor, PluginProvisionFailure, Provision, WindowsPrivateCache, storage,
};
use crate::windows_permissions as permissions;
use std::{fs, io};

impl Provision<'_> {
    pub(super) fn cleanup(
        &mut self,
        executor: &mut impl PluginProvisionExecutor,
    ) -> Result<(), PluginProvisionFailure> {
        if !self.catalog.blocked_cleanup.is_empty() {
            return Err(PluginProvisionFailure::PhysicalUnknown);
        }
        if self.catalog.cleanup.is_empty() {
            return Ok(());
        }
        let managed = self.registry.root.join("managed");
        let _managed = WindowsPrivateCache::acquire(&managed).map_err(storage)?;
        let base = managed.canonicalize().map_err(storage)?;
        while let Some(id) = self.catalog.cleanup.first().cloned() {
            executor
                .checkpoint()
                .map_err(PluginProvisionFailure::Command)?;
            let path = base.join(format!("i-{id}"));
            // A committed catalog is the authority after a crash. Explicit local linking
            // into a generation also transfers that source out of automatic cleanup.
            let referenced = self.catalog.plugins.iter().any(|entry| {
                entry
                    .source
                    .as_ref()
                    .is_some_and(|s| s.installation_id == id)
                    || Path::new(&entry.plugin_root).starts_with(&path)
            });
            if !referenced {
                match fs::symlink_metadata(&path) {
                    Ok(_) => {
                        let mut budget = 100_000;
                        remove_owned(&path, 0, &mut budget, executor)?;
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(_) => return Err(PluginProvisionFailure::CleanupIncomplete),
                }
            }
            self.catalog.cleanup.remove(0);
            self.persist()?;
        }
        Ok(())
    }
}
impl From<io::Error> for PluginProvisionFailure {
    fn from(_: io::Error) -> Self {
        Self::CleanupIncomplete
    }
}
fn remove_owned(
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
