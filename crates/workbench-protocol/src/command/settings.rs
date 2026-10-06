//! Explicit native settings edits; the Site owns paths, source comparison and publication.
use super::configuration::{
    ConfigurationOutcome, ConfigurationResult, ConfigurationSnapshot, ConfigurationThemeName,
};
use crate::{ContractError, ProtocolVersion, SiteId, WorkbenchRequestId, WorkbenchRevision};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES: usize = 4096;

/// Content digest or absence; no file paths or TOML cross this boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NativeSettingsSource(String);
impl NativeSettingsSource {
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        if value == "missing"
            || (value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')))
        {
            Ok(Self(value.into()))
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for NativeSettingsSource {
    type Error = ContractError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}
impl From<NativeSettingsSource> for String {
    fn from(value: NativeSettingsSource) -> Self {
        value.0
    }
}

/// Required nullable dirty leaves; omitted and all-null patches are refused.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSettingsChanges {
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub theme_name: Option<ConfigurationThemeName>,
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub background_reminders: Option<bool>,
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub sound_enabled: Option<bool>,
}
impl NativeSettingsChanges {
    pub const fn is_empty(self) -> bool {
        self.theme_name.is_none()
            && self.background_reminders.is_none()
            && self.sound_enabled.is_none()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum NativeSettingsOperation {
    Open {},
    Save {
        expected_revision: WorkbenchRevision,
        source: NativeSettingsSource,
        changes: NativeSettingsChanges,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSettingsRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub operation: NativeSettingsOperation,
}
impl NativeSettingsRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        if matches!(self.operation, NativeSettingsOperation::Save { changes, .. } if changes.is_empty())
        {
            Err(ContractError::InvalidDocument)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeSettingsFailure {
    Denied,
    Unavailable,
    SiteMismatch,
    Busy,
    SourceChanged,
    ConfigurationChanged,
    InvalidSource,
    InvalidChanges,
    WriteFailed,
    RevisionExhausted,
    ResponseTooLarge,
}

/// A committed disk save is never mislabeled as a pre-save refusal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum NativeSettingsOutcome {
    Opened {
        source: NativeSettingsSource,
        snapshot: ConfigurationSnapshot,
    },
    NoChange {
        snapshot: ConfigurationSnapshot,
    },
    SavedApplied {
        snapshot: ConfigurationSnapshot,
    },
    SavedNotApplied {
        snapshot: ConfigurationSnapshot,
    },
    Refused {
        reason: NativeSettingsFailure,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSettingsResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub outcome: NativeSettingsOutcome,
}
impl NativeSettingsResult {
    pub fn answers(&self, request: &NativeSettingsRequest) -> bool {
        self.request_id == request.request_id
            && self.site_id == request.site_id
            && match (&request.operation, &self.outcome) {
                (_, NativeSettingsOutcome::Refused { .. })
                | (NativeSettingsOperation::Open {}, NativeSettingsOutcome::Opened { .. }) => true,
                (
                    NativeSettingsOperation::Save {
                        expected_revision,
                        changes,
                        ..
                    },
                    NativeSettingsOutcome::SavedApplied { snapshot },
                ) => {
                    expected_revision.value().checked_add(1) == Some(snapshot.revision.value())
                        && changes
                            .theme_name
                            .is_none_or(|theme| theme == snapshot.theme_name)
                }
                (
                    NativeSettingsOperation::Save {
                        expected_revision,
                        changes,
                        ..
                    },
                    NativeSettingsOutcome::NoChange { snapshot },
                ) => {
                    *expected_revision == snapshot.revision
                        && changes
                            .theme_name
                            .is_none_or(|theme| theme == snapshot.theme_name)
                }
                (
                    NativeSettingsOperation::Save {
                        expected_revision, ..
                    },
                    NativeSettingsOutcome::SavedNotApplied { snapshot },
                ) => *expected_revision == snapshot.revision,
                _ => false,
            }
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            NativeSettingsOutcome::Opened { snapshot, .. }
            | NativeSettingsOutcome::NoChange { snapshot }
            | NativeSettingsOutcome::SavedApplied { snapshot }
            | NativeSettingsOutcome::SavedNotApplied { snapshot } => ConfigurationResult {
                protocol_version: self.protocol_version,
                request_id: self.request_id.clone(),
                site_id: self.site_id.clone(),
                outcome: ConfigurationOutcome::Current {
                    snapshot: snapshot.clone(),
                },
            }
            .validate(),
            NativeSettingsOutcome::Refused { .. } => Ok(()),
        }
    }
}
