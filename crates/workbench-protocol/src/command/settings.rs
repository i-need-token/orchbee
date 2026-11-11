//! Session-scoped reads and edits of `config.toml` on the Server's machine, keyed by setting path.
//!
//! The contract checks structure only: key grammar, value types and bounds. Which keys exist and
//! what each admits belongs to the settings registry in `orchbee-local-runtime`, so a new setting
//! needs no protocol change.
use crate::{
    ConfigurationDiagnostics, ContractError, ProtocolVersion, SessionId, UserLaunchProfiles,
    WorkbenchRequestId, WorkbenchRevision,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Largest encoded request or result; room for every platform's launch profiles.
pub const MAXIMUM_SETTINGS_BYTES: usize = 2 * 1024 * 1024;
pub const MAXIMUM_SETTINGS_REQUEST_BYTES: usize = MAXIMUM_SETTINGS_BYTES;
/// Most changes one edit carries.
pub const MAXIMUM_SETTINGS_CHANGES: usize = 64;
/// Longest key path, in bytes.
pub const MAXIMUM_SETTING_KEY_BYTES: usize = 128;
/// Longest string value, in bytes.
pub const MAXIMUM_SETTING_TEXT_BYTES: usize = 4096;
/// Most entries in a structured value such as `terminal.profiles.<platform>`.
pub const MAXIMUM_SETTING_ENTRIES: usize = 64;
/// Largest structured value, encoded as JSON.
pub const MAXIMUM_SETTING_ENTRIES_BYTES: usize = 64 * 1024;
/// Most keys a snapshot lists in `values` or `set`.
pub const MAXIMUM_SNAPSHOT_SETTING_KEYS: usize = 1024;
/// Longest `filePath`, in bytes.
pub const MAXIMUM_SETTINGS_FILE_PATH_BYTES: usize = 4096;
/// Longest reason or detail token, in bytes.
pub const MAXIMUM_SETTINGS_TOKEN_BYTES: usize = 64;

/// A dotted key path such as `appearance.mode`: lowercase ASCII letters, digits and underscores
/// in segments joined by dots.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SettingPath(String);
impl SettingPath {
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        if value.len() <= MAXIMUM_SETTING_KEY_BYTES && value.split('.').all(segment) {
            Ok(Self(value.into()))
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// The path's segments, in order.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('.')
    }
}
impl TryFrom<String> for SettingPath {
    type Error = ContractError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}
impl From<SettingPath> for String {
    fn from(value: SettingPath) -> Self {
        value.0
    }
}
impl std::fmt::Display for SettingPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn segment(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_SETTINGS_TOKEN_BYTES
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
        && value.as_bytes()[0].is_ascii_lowercase()
}

fn text(value: &str) -> bool {
    value.len() <= MAXIMUM_SETTING_TEXT_BYTES && !value.contains('\0')
}

/// One key's value as the file holds it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SettingScalar {
    Boolean(bool),
    Integer(i64),
    Text(String),
}
impl SettingScalar {
    fn valid(&self) -> bool {
        match self {
            Self::Boolean(_) | Self::Integer(_) => true,
            Self::Text(value) => text(value),
        }
    }
}

/// One member of a structured entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SettingEntryValue {
    Boolean(bool),
    Integer(i64),
    Text(String),
    List(Vec<String>),
}

/// One table of an array value, such as one launch profile, by member name.
pub type SettingEntry = BTreeMap<String, SettingEntryValue>;

/// A value an edit writes: a scalar, or for a structured key the array in the file's form.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SettingValue {
    Boolean(bool),
    Integer(i64),
    Text(String),
    Entries(Vec<SettingEntry>),
}
impl SettingValue {
    fn valid(&self) -> bool {
        match self {
            Self::Boolean(_) | Self::Integer(_) => true,
            Self::Text(value) => text(value),
            Self::Entries(entries) => {
                entries.len() <= MAXIMUM_SETTING_ENTRIES
                    && entries.iter().all(|entry| {
                        entry.iter().all(|(name, value)| {
                            segment(name)
                                && name.len() <= MAXIMUM_SETTING_KEY_BYTES
                                && match value {
                                    SettingEntryValue::Boolean(_)
                                    | SettingEntryValue::Integer(_) => true,
                                    SettingEntryValue::Text(value) => !value.contains('\0'),
                                    SettingEntryValue::List(values) => {
                                        values.iter().all(|value| !value.contains('\0'))
                                    }
                                }
                        })
                    })
                    && serde_json::to_vec(entries)
                        .is_ok_and(|bytes| bytes.len() <= MAXIMUM_SETTING_ENTRIES_BYTES)
            }
        }
    }
}

/// Set one key, or restore its default by deleting it from the file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ChangeDocument", into = "ChangeDocument")]
pub enum SettingChange {
    Set {
        key: SettingPath,
        value: SettingValue,
    },
    Restore {
        key: SettingPath,
    },
}
impl SettingChange {
    pub const fn key(&self) -> &SettingPath {
        match self {
            Self::Set { key, .. } | Self::Restore { key } => key,
        }
    }
}

/// The wire form of a change: `key` with exactly one of `value` and `restore: true`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChangeDocument {
    key: SettingPath,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    value: Option<SettingValue>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    restore: Option<bool>,
}
impl TryFrom<ChangeDocument> for SettingChange {
    type Error = ContractError;
    fn try_from(document: ChangeDocument) -> Result<Self, Self::Error> {
        match (document.value, document.restore) {
            (Some(value), None) => Ok(Self::Set {
                key: document.key,
                value,
            }),
            (None, Some(true)) => Ok(Self::Restore { key: document.key }),
            _ => Err(ContractError::InvalidDocument),
        }
    }
}
impl From<SettingChange> for ChangeDocument {
    fn from(change: SettingChange) -> Self {
        match change {
            SettingChange::Set { key, value } => Self {
                key,
                value: Some(value),
                restore: None,
            },
            SettingChange::Restore { key } => Self {
                key,
                value: None,
                restore: Some(true),
            },
        }
    }
}

/// An optional member that, when present, must hold a value rather than null.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SettingsOperation {
    /// The current snapshot; subscribable as state.
    Read {},
    /// Apply every change in one file replace, or none.
    Edit { changes: Vec<SettingChange> },
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
        let SettingsOperation::Edit { changes } = &self.operation else {
            return Ok(());
        };
        let mut keys = BTreeSet::new();
        let valid = (1..=MAXIMUM_SETTINGS_CHANGES).contains(&changes.len())
            && changes.iter().all(|change| {
                keys.insert(change.key())
                    && match change {
                        SettingChange::Set { value, .. } => value.valid(),
                        SettingChange::Restore { .. } => true,
                    }
            });
        if valid {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
}

/// The machine a snapshot describes; it selects the per-platform keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingsPlatform {
    Windows,
    Macos,
    Linux,
}
impl SettingsPlatform {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Macos => "macos",
            Self::Linux => "linux",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettingsSyntaxReason {
    InvalidToml,
    InvalidUtf8,
}

/// What reading the file found.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SettingsFile {
    Ok {},
    /// No file: every key holds its default.
    Missing {},
    /// The file stopped parsing at this one-based line and column; the values are the last valid
    /// ones.
    SyntaxError {
        reason: SettingsSyntaxReason,
        line: u32,
        column: u32,
    },
    /// The file could not be read; the values are the last valid ones.
    Unreadable {
        reason: String,
    },
}

/// The whole settings file of the machine that produced it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsSnapshot {
    /// The same counter as the configuration snapshot's revision; older snapshots are dropped.
    pub revision: WorkbenchRevision,
    /// Where `config.toml` lives on the producing machine, for display.
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub file_path: Option<String>,
    pub file: SettingsFile,
    pub platform: SettingsPlatform,
    /// Every scalar key: the file's spelling when the reader admits it, otherwise the default;
    /// null for a default that is unset.
    pub values: BTreeMap<SettingPath, Option<SettingScalar>>,
    /// Keys the file holds, sorted; a key here can be restored to its default.
    pub set: Vec<SettingPath>,
    /// The effective launch catalog for `platform`, built-in profile included.
    pub launch_profiles: UserLaunchProfiles,
    pub diagnostics: ConfigurationDiagnostics,
}
impl SettingsSnapshot {
    pub fn validate(&self) -> Result<(), ContractError> {
        let file = match &self.file {
            SettingsFile::Ok {} | SettingsFile::Missing {} => true,
            SettingsFile::SyntaxError { line, column, .. } => *line >= 1 && *column >= 1,
            SettingsFile::Unreadable { reason } => token(reason),
        };
        let valid =
            file && self.file_path.as_ref().is_none_or(|path| {
                !path.is_empty()
                    && path.len() <= MAXIMUM_SETTINGS_FILE_PATH_BYTES
                    && !path.chars().any(char::is_control)
            }) && self.values.len() <= MAXIMUM_SNAPSHOT_SETTING_KEYS
                && self
                    .values
                    .values()
                    .all(|value| value.as_ref().is_none_or(SettingScalar::valid))
                && self.set.len() <= MAXIMUM_SNAPSHOT_SETTING_KEYS
                && self.set.windows(2).all(|pair| pair[0] < pair[1])
                && self.launch_profiles.is_valid();
        if valid {
            self.diagnostics.validate()
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
}

/// Why a read or edit was refused. Edits map the editor's refusals one to one.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettingsRefusal {
    Denied,
    Unavailable,
    SessionMismatch,
    /// Another reload or edit held the file past the wait.
    Busy,
    /// `key` names no setting.
    UnknownKey,
    /// `key`'s value is not one it admits; `detail` names the rule.
    InvalidValue,
    /// The file does not parse at `line` and `column`; `detail` is `invalid-toml` or
    /// `invalid-utf8`. Nothing was written.
    SyntaxError,
    ReadOnly,
    FileInUse,
    /// Another writer held the settings lock.
    Locked,
    /// The file changed during the edit; nothing was written.
    Changed,
    /// The edited file would exceed the size limit.
    TooLarge,
    /// The file's location cannot be used; `detail` names why.
    UnusableFile,
    WriteFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "outcome",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SettingsOutcome {
    /// Answers `read`.
    Current { snapshot: Box<SettingsSnapshot> },
    /// Answers `edit`: a snapshot at or after the edit; `changed` is false when the file already
    /// held the values.
    Edited {
        snapshot: Box<SettingsSnapshot>,
        changed: bool,
    },
    /// Answers either; the members present depend on `reason`.
    Refused {
        reason: SettingsRefusal,
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        key: Option<SettingPath>,
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        detail: Option<String>,
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        line: Option<u32>,
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        column: Option<u32>,
    },
}
impl SettingsOutcome {
    /// A refusal that carries no member.
    pub const fn refused(reason: SettingsRefusal) -> Self {
        Self::Refused {
            reason,
            key: None,
            detail: None,
            line: None,
            column: None,
        }
    }
    fn valid(&self) -> bool {
        let Self::Refused {
            reason,
            key,
            detail,
            line,
            column,
        } = self
        else {
            return true;
        };
        let detail_valid = detail.as_deref().is_none_or(token);
        let position = line.is_some_and(|line| line >= 1) && column.is_some_and(|c| c >= 1);
        detail_valid
            && match reason {
                SettingsRefusal::UnknownKey => {
                    key.is_some() && detail.is_none() && line.is_none() && column.is_none()
                }
                SettingsRefusal::InvalidValue => {
                    key.is_some() && detail.is_some() && line.is_none() && column.is_none()
                }
                SettingsRefusal::SyntaxError => {
                    key.is_none()
                        && matches!(detail.as_deref(), Some("invalid-toml" | "invalid-utf8"))
                        && position
                }
                SettingsRefusal::UnusableFile => {
                    key.is_none() && detail.is_some() && line.is_none() && column.is_none()
                }
                _ => key.is_none() && detail.is_none() && line.is_none() && column.is_none(),
            }
    }
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
            && match (&request.operation, &self.outcome) {
                (_, SettingsOutcome::Refused { .. })
                | (SettingsOperation::Read {}, SettingsOutcome::Current { .. })
                | (SettingsOperation::Edit { .. }, SettingsOutcome::Edited { .. }) => true,
                (SettingsOperation::Read {}, SettingsOutcome::Edited { .. })
                | (SettingsOperation::Edit { .. }, SettingsOutcome::Current { .. }) => false,
            }
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            SettingsOutcome::Current { snapshot } | SettingsOutcome::Edited { snapshot, .. } => {
                snapshot.validate()
            }
            refused if refused.valid() => Ok(()),
            SettingsOutcome::Refused { .. } => Err(ContractError::InvalidDocument),
        }
    }
}
