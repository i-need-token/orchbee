//! No Unix plugin storage effects are admitted yet.
use super::{Path, PluginProvisionExecutor, PluginProvisionFailure};
pub(super) fn remove_owned(
    _path: &Path,
    _depth: usize,
    _budget: &mut usize,
    _executor: &mut impl PluginProvisionExecutor,
) -> Result<(), PluginProvisionFailure> {
    Err(PluginProvisionFailure::Registry(
        crate::PluginRegistryError::Unsupported,
    ))
}
