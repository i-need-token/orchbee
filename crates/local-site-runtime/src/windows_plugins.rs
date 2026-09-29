//! Shared Windows plugin catalog. Executing declared commands belongs to the runtime.
mod manifest;
mod store;
pub use manifest::{PluginActionDefinition, PluginManifest, PluginPlatform, plugin_identifier};
use serde::{Deserialize, Serialize};
pub use store::{
    PluginRegistry, PluginRegistryEntry, PluginRegistryMutation, PluginRegistryResult,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginRegistryError {
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
