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
            return Err("invalid plugin Panel selector");
        }
        Ok(Self {
            plugin_id,
            entrypoint_id,
        })
    }
}

/// A startable Agent's canonical provider id from the closed catalog. The Server resolves it to
/// the command the new shell runs first; the caller never supplies command text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AgentStartSelector(orchbee_agent_observation_protocol::StartableAgent);
impl AgentStartSelector {
    #[must_use]
    pub fn parse(provider_id: &str) -> Option<Self> {
        orchbee_agent_observation_protocol::StartableAgent::parse(provider_id).map(Self)
    }
    #[must_use]
    pub const fn agent(self) -> orchbee_agent_observation_protocol::StartableAgent {
        self.0
    }
}
impl TryFrom<String> for AgentStartSelector {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).ok_or("unknown startable Agent provider")
    }
}
impl From<AgentStartSelector> for String {
    fn from(value: AgentStartSelector) -> Self {
        value.0.id().to_owned()
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
    /// Server-owned plugin Panel launch, exclusive with explicit launch/profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_panel: Option<PluginPaneSelector>,
    /// Observed source display data; never executable, directory or Worker authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_context: Option<Box<crate::PluginInvocationContext>>,
    /// The Agent the new shell runs as its first command, once, leaving the shell behind when
    /// it exits. Exclusive with a plugin Panel; a shell that cannot take a first command starts
    /// without it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_start: Option<AgentStartSelector>,
}
impl LaunchOptions {
    /// Whether no per-launch override is requested.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cwd.is_none()
            && self.environment.is_empty()
            && self.plugin_panel.is_none()
            && self.plugin_context.is_none()
            && self.agent_start.is_none()
    }
}

impl std::fmt::Debug for LaunchOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaunchOptions")
            .field("cwd", &self.cwd)
            .field("environment", &self.environment)
            .field("plugin_panel", &self.plugin_panel)
            .field("plugin_context_present", &self.plugin_context.is_some())
            .field("agent_start", &self.agent_start)
            .finish()
    }
}
