//! Session-owned runtime configuration publication; a revision is not a Client acknowledgement.
use crate::{ContractError, ProtocolVersion, SessionId, WorkbenchRequestId, WorkbenchRevision};
use serde::{Deserialize, Serialize};
pub const MAXIMUM_CONFIGURATION_BYTES: usize = 256 * 1024;
pub const MAXIMUM_CONFIGURATION_DIAGNOSTICS: usize = 128;
pub const MAXIMUM_CONFIGURATION_DIAGNOSTIC_FIELD_BYTES: usize = 256;
pub const MAXIMUM_CONFIGURATION_DIAGNOSTIC_REASON_BYTES: usize = 64;
pub const MAXIMUM_CONFIGURATION_DIAGNOSTICS_OMITTED: u32 = 65_536;
pub const MAXIMUM_CONFIGURATION_PATH_BYTES: usize = 4096;
pub const MAXIMUM_NOTIFICATION_PROFILE_BYTES: usize = 16 * 1024;

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
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub action: ConfigurationAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigurationDiagnosticDisposition {
    Defaulted,
    Disabled,
    Ignored,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationDiagnostic {
    pub field: String,
    pub reason: String,
    pub disposition: ConfigurationDiagnosticDisposition,
}
impl ConfigurationDiagnostic {
    fn valid(&self) -> bool {
        !self.field.is_empty()
            && self.field.len() <= MAXIMUM_CONFIGURATION_DIAGNOSTIC_FIELD_BYTES
            && !self.field.chars().any(char::is_control)
            && !self.reason.is_empty()
            && self.reason.len() <= MAXIMUM_CONFIGURATION_DIAGNOSTIC_REASON_BYTES
            && self.reason.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || (byte == b'-'
                        && index > 0
                        && index + 1 < self.reason.len()
                        && self.reason.as_bytes()[index - 1] != b'-')
            })
            && self.reason.as_bytes()[0].is_ascii_lowercase()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationDiagnostics {
    pub items: Vec<ConfigurationDiagnostic>,
    pub omitted_count: u32,
}
impl ConfigurationDiagnostics {
    pub fn validate(&self) -> Result<(), ContractError> {
        let valid = self.items.len() <= MAXIMUM_CONFIGURATION_DIAGNOSTICS
            && self.omitted_count <= MAXIMUM_CONFIGURATION_DIAGNOSTICS_OMITTED
            && self.items.iter().enumerate().all(|(index, item)| {
                item.valid()
                    && (index == 0
                        || (&self.items[index - 1].field, &self.items[index - 1].reason)
                            < (&item.field, &item.reason))
            });
        if valid {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationSnapshot {
    pub revision: WorkbenchRevision,
    pub diagnostics: ConfigurationDiagnostics,
    /// Canonical [keys] only, never arbitrary configuration-file contents.
    pub keys_profile: String,
    pub window_title: String,
    pub notification_profile: String,
    /// Canonical presentation-only JSON admitted by the Windows settings owner.
    pub agent_sidebar_profile: String,
    pub spaces_sidebar_profile: String,
    pub manifest_check: bool,
    /// `terminal.focus_new_panel`: whether a split moves focus to its new Panel.
    pub focus_new_panel: bool,
    /// `worktrees.directory` as an admitted absolute path; null selects the sibling checkout.
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub worktrees_directory: Option<String>,
    /// `terminal.new_cwd`: `follow`, `home`, `current`, `workspace`, or an admitted absolute path.
    pub new_cwd: String,
}

/// The `terminal.new_cwd` keywords; any other admitted value is an absolute path.
const NEW_CWD_POLICIES: [&str; 4] = ["follow", "home", "current", "workspace"];

fn configured_path(path: &str) -> bool {
    path.len() <= MAXIMUM_CONFIGURATION_PATH_BYTES
        && crate::directory::is_absolute_path(path.as_bytes())
        && !path.chars().any(char::is_control)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigurationFailure {
    Denied,
    Unavailable,
    #[serde(rename = "session-mismatch")]
    SessionMismatch,
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
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub outcome: ConfigurationOutcome,
}
impl ConfigurationResult {
    pub fn answers(&self, request: &ConfigurationRequest) -> bool {
        self.request_id == request.request_id
            && self.session_id == request.session_id
            && match request.action {
                ConfigurationAction::Current => matches!(
                    &self.outcome,
                    ConfigurationOutcome::Current { .. } | ConfigurationOutcome::Failed { .. }
                ),
                ConfigurationAction::Reload => matches!(
                    &self.outcome,
                    ConfigurationOutcome::Reloaded { .. } | ConfigurationOutcome::Failed { .. }
                ),
            }
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        let valid = match &self.outcome {
            ConfigurationOutcome::Current { snapshot }
            | ConfigurationOutcome::Reloaded { snapshot, .. } => {
                snapshot.diagnostics.validate().is_ok()
                    && !snapshot.keys_profile.is_empty()
                    && snapshot.keys_profile.len() <= 64 * 1024
                    && snapshot.window_title.len() <= 4096
                    && !snapshot.notification_profile.is_empty()
                    && snapshot.notification_profile.len() <= MAXIMUM_NOTIFICATION_PROFILE_BYTES
                    && !snapshot.spaces_sidebar_profile.is_empty()
                    && snapshot.spaces_sidebar_profile.len() <= 32 * 1024
                    && !snapshot.agent_sidebar_profile.is_empty()
                    && snapshot.agent_sidebar_profile.len() <= 32 * 1024
                    && snapshot
                        .worktrees_directory
                        .as_deref()
                        .is_none_or(configured_path)
                    && (NEW_CWD_POLICIES.contains(&snapshot.new_cwd.as_str())
                        || configured_path(&snapshot.new_cwd))
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
