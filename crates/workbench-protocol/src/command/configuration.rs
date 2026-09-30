//! Site-owned runtime configuration publication; a revision is not a Client acknowledgement.
use crate::{ContractError, ProtocolVersion, SiteId, WorkbenchRequestId, WorkbenchRevision};
use serde::{Deserialize, Serialize};
pub const MAXIMUM_CONFIGURATION_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigurationAction {
    Current,
    Reload,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub action: ConfigurationAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ConfigurationRestart {
    #[serde(rename = "terminal.default_shell")]
    DefaultShell,
    #[serde(rename = "terminal.shell_mode")]
    ShellMode,
    #[serde(rename = "session.resume_agents_on_restore")]
    ResumeAgents,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationSnapshot {
    pub revision: WorkbenchRevision,
    /// Canonical [keys] only, never arbitrary configuration-file contents.
    pub keys_profile: String,
    pub window_title: String,
    pub notification_profile: String,
    /// Canonical presentation-only JSON admitted by the Windows settings owner.
    pub agent_sidebar_profile: String,
    pub spaces_sidebar_profile: String,
    pub restart_required: Vec<ConfigurationRestart>,
    pub manifest_check: bool,
    /// Admitted defaults for subsequent preparations, not resolved executable facts.
    pub default_shell: String,
    pub shell_mode: ConfigurationShellMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationShellMode {
    Auto,
    Login,
    NonLogin,
}
impl ConfigurationShellMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Login => "login",
            Self::NonLogin => "non_login",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigurationFailure {
    Denied,
    Unavailable,
    SiteMismatch,
    Busy,
    LoadFailed,
    RevisionExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ConfigurationOutcome {
    Current {
        snapshot: ConfigurationSnapshot,
    },
    Reloaded {
        snapshot: ConfigurationSnapshot,
        changed: bool,
    },
    Failed {
        reason: ConfigurationFailure,
        #[serde(deserialize_with = "super::agent_manifests::nullable")]
        field: Option<String>,
        #[serde(deserialize_with = "super::agent_manifests::nullable")]
        detail: Option<String>,
    },
}
impl ConfigurationOutcome {
    pub fn failed(reason: ConfigurationFailure) -> Self {
        Self::Failed {
            reason,
            field: None,
            detail: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub outcome: ConfigurationOutcome,
}
impl ConfigurationResult {
    pub fn answers(&self, request: &ConfigurationRequest) -> bool {
        self.request_id == request.request_id
            && self.site_id == request.site_id
            && matches!(
                (&request.action, &self.outcome),
                (_, ConfigurationOutcome::Failed { .. })
                    | (
                        ConfigurationAction::Current,
                        ConfigurationOutcome::Current { .. }
                    )
                    | (
                        ConfigurationAction::Reload,
                        ConfigurationOutcome::Reloaded { .. }
                    )
            )
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        let valid = match &self.outcome {
            ConfigurationOutcome::Current { snapshot }
            | ConfigurationOutcome::Reloaded { snapshot, .. } => {
                !snapshot.keys_profile.is_empty()
                    && snapshot.keys_profile.len() <= 64 * 1024
                    && snapshot.window_title.len() <= 4096
                    && !snapshot.notification_profile.is_empty()
                    && snapshot.notification_profile.len() <= 1024
                    && !snapshot.spaces_sidebar_profile.is_empty()
                    && snapshot.spaces_sidebar_profile.len() <= 32 * 1024
                    && !snapshot.agent_sidebar_profile.is_empty()
                    && snapshot.agent_sidebar_profile.len() <= 32 * 1024
                    && snapshot.default_shell.len() <= 4096
                    && !snapshot.default_shell.chars().any(char::is_control)
                    && snapshot.restart_required.len() <= 3
                    && snapshot
                        .restart_required
                        .iter()
                        .enumerate()
                        .all(|(index, field)| !snapshot.restart_required[..index].contains(field))
            }
            ConfigurationOutcome::Failed {
                reason,
                field,
                detail,
            } => {
                let bounded = |value: &Option<String>| {
                    value.as_ref().is_none_or(|value| {
                        !value.is_empty()
                            && value.len() <= 128
                            && !value.chars().any(char::is_control)
                    })
                };
                bounded(field)
                    && bounded(detail)
                    && field.is_some() == detail.is_some()
                    && (*reason == ConfigurationFailure::LoadFailed || field.is_none())
            }
        };
        if valid {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
}
