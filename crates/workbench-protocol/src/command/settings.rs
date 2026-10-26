//! Session-scoped access to the same durable preference authority used by Desktop.
use crate::{
    ContractError, MAXIMUM_USER_PREFERENCES_BYTES, PreferencesFailure, ProtocolVersion, SessionId,
    UserPreferencesPatch, UserPreferencesRequest, UserPreferencesSnapshot, WorkbenchRequestId,
    parse_user_preferences_request,
};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_SETTINGS_REQUEST_BYTES: usize = MAXIMUM_USER_PREFERENCES_BYTES;

/// No paths or arbitrary configuration documents cross the settings boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SettingsSource(String);
impl SettingsSource {
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
impl TryFrom<String> for SettingsSource {
    type Error = ContractError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}
impl From<SettingsSource> for String {
    fn from(value: SettingsSource) -> Self {
        value.0
    }
}
pub type SettingsChanges = UserPreferencesPatch;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SettingsOperation {
    Open {},
    Save {
        source: SettingsSource,
        changes: SettingsChanges,
    },
}
impl SettingsOperation {
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
pub struct SettingsRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub operation: SettingsOperation,
}
impl SettingsRequest {
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
pub enum SettingsFailure {
    Denied,
    Unavailable,
    #[serde(rename = "session-mismatch")]
    SessionMismatch,
    Busy,
    SourceChanged,
    InvalidSource,
    InvalidChanges,
    WriteFailed,
    ResponseTooLarge,
}
impl From<PreferencesFailure> for SettingsFailure {
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
pub enum SettingsOutcome {
    Opened {
        snapshot: UserPreferencesSnapshot,
    },
    Saved {
        snapshot: UserPreferencesSnapshot,
        changed: bool,
    },
    OutcomeUnknown {},
    Refused {
        reason: SettingsFailure,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub outcome: SettingsOutcome,
}
impl SettingsResult {
    pub fn answers(&self, request: &SettingsRequest) -> bool {
        self.request_id == request.request_id
            && self.session_id == request.session_id
            && {
                let shared_failure = matches!(&self.outcome, SettingsOutcome::Refused { .. });
                match &request.operation {
                    SettingsOperation::Open {} => {
                        matches!(&self.outcome, SettingsOutcome::Opened { .. }) || shared_failure
                    }
                    SettingsOperation::Save { changes, .. } => {
                        matches!(&self.outcome, SettingsOutcome::Saved { snapshot, .. } if changes.keys().all(|key| snapshot.claimed.contains(&key))
                            && changes
                                .apply(&snapshot.values)
                                .is_ok_and(|values| values == snapshot.values))
                            || matches!(&self.outcome, SettingsOutcome::OutcomeUnknown {})
                            || shared_failure
                    }
                }
            }
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            SettingsOutcome::Opened { snapshot } | SettingsOutcome::Saved { snapshot, .. } => {
                SettingsSource::parse(&snapshot.source)?;
                snapshot
                    .values
                    .validate()
                    .map_err(|_| ContractError::InvalidDocument)
            }
            SettingsOutcome::OutcomeUnknown {} | SettingsOutcome::Refused { .. } => Ok(()),
        }
    }
}
