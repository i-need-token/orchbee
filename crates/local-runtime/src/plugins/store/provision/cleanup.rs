//! Cleanup only source folders recorded before creation, never arbitrary source operands.
use super::{
    Generation, Path, PluginProvisionExecutor, PluginProvisionFailure, Provision, storage,
};
use crate::data_root::PluginStorePaths;
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
        // Registered roots are canonical, so recorded folders compare in the canonical store.
        let base = PluginStorePaths::new(self.registry.root().canonicalize().map_err(storage)?);
        while let Some(key) = self.catalog.cleanup.first().cloned() {
            executor
                .checkpoint()
                .map_err(PluginProvisionFailure::Command)?;
            let generation =
                Generation::parse(&key).ok_or(PluginProvisionFailure::CleanupIncomplete)?;
            let path = generation.path(&base);
            // A committed catalog is the authority after a crash. Explicit local linking
            // into a generation also transfers that source out of automatic cleanup.
            let referenced = self
                .catalog
                .plugins
                .iter()
                .any(|entry| Path::new(&entry.plugin_root).starts_with(&path));
            if !referenced {
                match fs::symlink_metadata(&path) {
                    Ok(_) => {
                        let mut budget = 100_000;
                        remove_owned(&path, 0, &mut budget, executor)?;
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(_) => return Err(PluginProvisionFailure::CleanupIncomplete),
                }
                if let Generation::Source { folder, .. } = &generation {
                    // Emptied folders go too. Either may still hold another version or the
                    // plugin's own folders, or be recreated by a concurrent first use, so a
                    // refusal leaves it in place.
                    let _ = fs::remove_dir(base.sources(folder));
                    let _ = fs::remove_dir(base.plugin(folder));
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
