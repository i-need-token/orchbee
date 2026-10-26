//! Refuses plugin-tree cleanup on Unix without opening or deleting a path. It supplies the
//! storage adapter's typed unsupported outcome rather than implementing filesystem cleanup.

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
