//! Cleanup only generation paths recorded before creation, never arbitrary source operands.
use super::{
    Path, PluginProvisionExecutor, PluginProvisionFailure, PrivateCache, Provision, storage,
};
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
        let _managed = PrivateCache::acquire(&managed).map_err(storage)?;
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
#[cfg(windows)]
#[path = "cleanup/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "cleanup/unix.rs"]
mod platform;
use platform::remove_owned;
