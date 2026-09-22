//! One launch's cwd and environment overrides. These do not select a program,
//! mutate a catalog profile, or change Workspace directory membership.
use crate::{LaunchEnvironment, WorkbenchDirectoryPath};
use serde::{Deserialize, Serialize};

/// Validated per-launch overrides; empty options preserve the legacy request.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LaunchOptions {
    /// Absolute literal launch directory, checked for existence by the execution owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<WorkbenchDirectoryPath>,
    /// Bounded literal environment overlay, never emitted through debug formatting.
    #[serde(default, skip_serializing_if = "LaunchEnvironment::is_empty")]
    pub environment: LaunchEnvironment,
}
impl LaunchOptions {
    /// Whether no per-launch override is requested.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cwd.is_none() && self.environment.is_empty()
    }
}
