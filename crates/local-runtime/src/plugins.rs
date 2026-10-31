//! Shared Windows plugin catalog. Executing declared commands belongs to the runtime.
mod manifest;
mod root_spelling;
mod source;
mod store;
pub use manifest::{
    PluginActionDefinition, PluginManifest, PluginPaneDefinition, PluginPanePlacement,
    PluginPlatform, PluginPopupSize, plugin_identifier,
};
use serde::{Deserialize, Serialize};
pub use source::{PluginGithubSource, PluginManagedSource, plugin_git_reference};
pub use store::{
    PluginProvisionCommand, PluginProvisionExecutor, PluginProvisionFailure, PluginProvisionOutput,
    PluginProvisionReport, PluginProvisionStage, PluginRegisteredCatalog, PluginRegistry,
    PluginRegistryEntry, PluginRegistryMutation, PluginRegistryResult,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginRegistryError {
    Unsupported,
    InvalidManifest,
    InvalidIdentifier,
    InvalidPath,
    InvalidRegistry,
    ManifestUnavailable,
    PluginNotFound,
    LimitExceeded,
    Busy,
    StorageUnavailable,
}
impl std::fmt::Display for PluginRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for PluginRegistryError {}

#[cfg(test)]
mod tests;
