//! One launch's cwd and environment overrides. These do not select a program,
//! mutate a catalog profile, or change Workspace directory membership.
use crate::{LaunchEnvironment, WorkbenchDirectoryPath};
use serde::{Deserialize, Serialize};

/// Catalog identity only. The Server, never the caller, resolves executable intent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    deny_unknown_fields,
    try_from = "PluginPaneSelectorWire"
)]
pub struct PluginPaneSelector {
    pub plugin_id: String,
    pub entrypoint_id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PluginPaneSelectorWire {
    plugin_id: String,
    entrypoint_id: String,
}
impl TryFrom<PluginPaneSelectorWire> for PluginPaneSelector {
    type Error = &'static str;
    fn try_from(value: PluginPaneSelectorWire) -> Result<Self, Self::Error> {
        Self::new(value.plugin_id, value.entrypoint_id)
    }
}
impl PluginPaneSelector {
    pub fn new(plugin_id: String, entrypoint_id: String) -> Result<Self, &'static str> {
        if !crate::plugin_identifier(&plugin_id, 120)
            || !crate::plugin_identifier(&entrypoint_id, 120)
            || entrypoint_id.contains('.')
        {
            return Err("invalid plugin Pane selector");
        }
        Ok(Self {
            plugin_id,
            entrypoint_id,
        })
    }
}

/// Validated per-launch overrides; empty options preserve the request's selected defaults.
#[derive(Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LaunchOptions {
    /// Absolute literal launch directory, checked for existence by the execution owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<WorkbenchDirectoryPath>,
    /// Bounded literal environment overlay, never emitted through debug formatting.
    #[serde(default, skip_serializing_if = "LaunchEnvironment::is_empty")]
    pub environment: LaunchEnvironment,
    /// Server-owned plugin Pane launch, exclusive with explicit launch/profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_pane: Option<PluginPaneSelector>,
    /// Observed source display data; never executable, directory or Worker authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_context: Option<Box<crate::PluginInvocationContext>>,
}
impl LaunchOptions {
    /// Whether no per-launch override is requested.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cwd.is_none()
            && self.environment.is_empty()
            && self.plugin_pane.is_none()
            && self.plugin_context.is_none()
    }
}

impl std::fmt::Debug for LaunchOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaunchOptions")
            .field("cwd", &self.cwd)
            .field("environment", &self.environment)
            .field("plugin_pane", &self.plugin_pane)
            .field("plugin_context_present", &self.plugin_context.is_some())
            .finish()
    }
}
