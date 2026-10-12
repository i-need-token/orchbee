//! Site-scoped access to the same durable preference authority used by Desktop.
use crate::{
    ContractError, MAXIMUM_USER_PREFERENCES_BYTES, PreferencesFailure, ProtocolVersion, SiteId,
    UserPreferencesPatch, UserPreferencesRequest, UserPreferencesSnapshot, WorkbenchRequestId,
    parse_user_preferences_request,
};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_NATIVE_SETTINGS_REQUEST_BYTES: usize = MAXIMUM_USER_PREFERENCES_BYTES;

/// No paths or arbitrary configuration documents cross the settings boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NativeSettingsSource(String);
impl NativeSettingsSource {
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        if value == "missing"
            || value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
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
pub type NativeSettingsChanges = UserPreferencesPatch;

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
        source: NativeSettingsSource,
        changes: NativeSettingsChanges,
    },
}
impl NativeSettingsOperation {
    pub fn preferences_request(&self) -> UserPreferencesRequest {
        match self {
            Self::Open {} => UserPreferencesRequest::Read {},
            Self::Save { source, changes } => UserPreferencesRequest::Update {
                source: source.as_str().into(),
                changes: changes.clone(),
            },
        }
    }
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
        let bytes = serde_json::to_vec(&self.operation.preferences_request())
            .map_err(|_| ContractError::InvalidDocument)?;
        parse_user_preferences_request(&bytes)
            .map(|_| ())
            .map_err(|_| ContractError::InvalidDocument)
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
    InvalidSource,
    InvalidChanges,
    WriteFailed,
    ResponseTooLarge,
}
impl From<PreferencesFailure> for NativeSettingsFailure {
    fn from(reason: PreferencesFailure) -> Self {
        match reason {
            PreferencesFailure::InvalidRequest | PreferencesFailure::InvalidChanges => {
                Self::InvalidChanges
            }
            PreferencesFailure::InvalidSource => Self::InvalidSource,
            PreferencesFailure::Unavailable => Self::Unavailable,
            PreferencesFailure::Busy => Self::Busy,
            PreferencesFailure::SourceChanged => Self::SourceChanged,
            PreferencesFailure::WriteFailed => Self::WriteFailed,
        }
    }
}
/// Persistence is confirmed independently of each client's next live presentation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum NativeSettingsOutcome {
    Opened {
        snapshot: UserPreferencesSnapshot,
    },
    Saved {
        snapshot: UserPreferencesSnapshot,
        changed: bool,
    },
    OutcomeUnknown {},
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
                (_, NativeSettingsOutcome::Refused { .. }) => true,
                (NativeSettingsOperation::Open {}, NativeSettingsOutcome::Opened { .. }) => true,
                (
                    NativeSettingsOperation::Save { changes, .. },
                    NativeSettingsOutcome::Saved { snapshot, .. },
                ) => {
                    changes.keys().all(|key| snapshot.claimed.contains(&key))
                        && changes
                            .apply(&snapshot.values)
                            .is_ok_and(|values| values == snapshot.values)
                }
                (
                    NativeSettingsOperation::Save { .. },
                    NativeSettingsOutcome::OutcomeUnknown {},
                ) => true,
                _ => false,
            }
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            NativeSettingsOutcome::Opened { snapshot }
            | NativeSettingsOutcome::Saved { snapshot, .. } => {
                NativeSettingsSource::parse(&snapshot.source)?;
                snapshot
                    .values
                    .validate()
                    .map_err(|_| ContractError::InvalidDocument)
            }
            _ => Ok(()),
        }
    }
}
