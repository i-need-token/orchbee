//! One durable device/configuration preference authority for native and Desktop clients.
mod launch_admission;
mod terminal_defaults;
pub use terminal_defaults::UserTerminalLaunchDefaults;
mod scope;
pub use scope::UserPreferencesScope;
#[cfg(windows)]
mod native_import_windows;
#[cfg(windows)]
#[path = "user_preferences/storage_windows.rs"]
mod storage;
#[cfg(unix)]
#[path = "user_preferences/storage_unix.rs"]
mod storage;
#[cfg(test)]
mod tests;

use orchbee_workbench_protocol::{
    DesktopPreferencesPart, MAXIMUM_USER_PREFERENCES_BYTES, PreferenceKey, PreferencesFailure,
    UserPreferencesPatch, UserPreferencesRequest, UserPreferencesResponse, UserPreferencesSnapshot,
    UserPreferencesValues,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const RECORD_NAME: &str = "preferences.json";

#[derive(Debug, Eq, PartialEq)]
pub(super) enum CommitError {
    Refused(PreferencesFailure),
    #[cfg(unix)]
    OutcomeUnknown,
}
impl From<PreferencesFailure> for CommitError {
    fn from(reason: PreferencesFailure) -> Self {
        Self::Refused(reason)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Record {
    version: u8,
    values: UserPreferencesValues,
    #[serde(deserialize_with = "unique_claimed_keys")]
    claimed: BTreeSet<PreferenceKey>,
    desktop_imported: bool,
    desktop_main_imported: bool,
}

fn unique_claimed_keys<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeSet<PreferenceKey>, D::Error> {
    let keys = Vec::<PreferenceKey>::deserialize(deserializer)?;
    let unique: BTreeSet<_> = keys.iter().copied().collect();
    if keys.len() != unique.len() {
        return Err(serde::de::Error::custom(
            "Claimed preference keys must be unique",
        ));
    }
    Ok(unique)
}

/// The application freezes one scope at startup; requests never choose a file or another device.
pub struct UserPreferencesStore {
    scope: UserPreferencesScope,
}
impl UserPreferencesStore {
    /// Construction is read-only; even the record's parent is created only by an accepted write.
    pub const fn new(scope: UserPreferencesScope) -> Self {
        Self { scope }
    }

    /// Exact authority identity, also used by callers to explain a deliberately isolated scope.
    pub const fn scope(&self) -> &UserPreferencesScope {
        &self.scope
    }

    /// One bounded operation; callers own transport deadlines and live application acknowledgments.
    pub fn execute(&self, request: &UserPreferencesRequest) -> UserPreferencesResponse {
        match self.perform(request) {
            Ok((snapshot, changed)) => UserPreferencesResponse::Snapshot { snapshot, changed },
            Err(CommitError::Refused(reason)) => UserPreferencesResponse::Refused { reason },
            #[cfg(unix)]
            Err(CommitError::OutcomeUnknown) => UserPreferencesResponse::OutcomeUnknown {},
        }
    }

    /// Reads never create a record, lock, directory, or migration receipt.
    pub fn read(&self) -> Result<UserPreferencesSnapshot, PreferencesFailure> {
        let bytes = storage::read(&self.scope)?;
        let (record, source) = self.decode(bytes.as_deref())?;
        Ok(snapshot(&record, source))
    }

    fn decode(&self, bytes: Option<&[u8]>) -> Result<(Record, String), PreferencesFailure> {
        match bytes {
            Some(bytes) => {
                if bytes.len() > MAXIMUM_USER_PREFERENCES_BYTES {
                    return Err(PreferencesFailure::InvalidSource);
                }
                let record: Record =
                    serde_json::from_slice(bytes).map_err(|_| PreferencesFailure::InvalidSource)?;
                if record.version != 1 {
                    return Err(PreferencesFailure::InvalidSource);
                }
                record
                    .values
                    .validate()
                    .map_err(|_| PreferencesFailure::InvalidSource)?;
                launch_admission::validate(&record.values.launch_profiles)
                    .map_err(|_| PreferencesFailure::InvalidSource)?;
                Ok((record, source_token(Some(bytes))))
            }
            None => {
                let mut record = Record {
                    version: 1,
                    values: UserPreferencesValues::default(),
                    claimed: BTreeSet::new(),
                    desktop_imported: false,
                    desktop_main_imported: false,
                };
                let (imported, source) = self.native_import()?;
                record.values = imported
                    .apply(&record.values)
                    .map_err(|_| PreferencesFailure::InvalidSource)?;
                record.claimed.extend(imported.keys());
                launch_admission::validate(&record.values.launch_profiles)
                    .map_err(|_| PreferencesFailure::InvalidSource)?;
                Ok((record, source))
            }
        }
    }

    #[cfg(windows)]
    fn native_import(&self) -> Result<(UserPreferencesPatch, String), PreferencesFailure> {
        native_import_windows::read(&self.scope)
    }
    #[cfg(unix)]
    fn native_import(&self) -> Result<(UserPreferencesPatch, String), PreferencesFailure> {
        // The existing Unix client has no TOML preference source to migrate.
        Ok((UserPreferencesPatch::default(), "missing".into()))
    }

    fn perform(
        &self,
        request: &UserPreferencesRequest,
    ) -> Result<(UserPreferencesSnapshot, bool), CommitError> {
        if matches!(request, UserPreferencesRequest::Read {}) {
            return self
                .read()
                .map(|snapshot| (snapshot, false))
                .map_err(Into::into);
        }
        let changes = match request {
            UserPreferencesRequest::Update { changes, .. }
            | UserPreferencesRequest::ImportDesktop { changes, .. } => changes,
            UserPreferencesRequest::Read {} => unreachable!("read handled before write admission"),
        };
        changes.apply(&UserPreferencesValues::default())?;
        if matches!(request, UserPreferencesRequest::Update { .. }) && changes.is_empty() {
            return Err(PreferencesFailure::InvalidChanges.into());
        }
        let mut writer = storage::Writer::acquire(&self.scope)?;
        let before = writer.read()?;
        let (previous, previous_source) = self.decode(before.as_deref())?;
        let mut candidate = previous.clone();
        match request {
            UserPreferencesRequest::Update { source, changes } => {
                if *source != previous_source {
                    return Err(PreferencesFailure::SourceChanged.into());
                }
                candidate.values = changes.apply(&candidate.values)?;
                candidate.claimed.extend(changes.keys());
            }
            UserPreferencesRequest::ImportDesktop { part, changes } => {
                if changes.keys().any(|key| !part.admits(key)) {
                    return Err(PreferencesFailure::InvalidChanges.into());
                }
                let imported = match part {
                    DesktopPreferencesPart::Main => candidate.desktop_main_imported,
                    DesktopPreferencesPart::Renderer => candidate.desktop_imported,
                };
                if imported {
                    return Ok((snapshot(&previous, previous_source), false));
                }
                let changes = changes.excluding(&candidate.claimed);
                candidate.values = changes.apply(&candidate.values)?;
                candidate.claimed.extend(changes.keys());
                match part {
                    DesktopPreferencesPart::Main => candidate.desktop_main_imported = true,
                    DesktopPreferencesPart::Renderer => candidate.desktop_imported = true,
                }
            }
            UserPreferencesRequest::Read {} => unreachable!("read handled before write admission"),
        }
        launch_admission::validate(&candidate.values.launch_profiles)?;
        if previous == candidate {
            return Ok((snapshot(&previous, previous_source), false));
        }
        let after =
            serde_json::to_vec(&candidate).map_err(|_| PreferencesFailure::InvalidChanges)?;
        if after.len() > MAXIMUM_USER_PREFERENCES_BYTES {
            return Err(PreferencesFailure::InvalidChanges.into());
        }
        // Before the first materialization the legacy source also belongs to the observation.
        if before.is_none() && self.native_import()?.1 != previous_source {
            return Err(PreferencesFailure::SourceChanged.into());
        }
        writer.commit(before.as_deref(), &after)?;
        Ok((snapshot(&candidate, source_token(Some(&after))), true))
    }
}

fn source_token(bytes: Option<&[u8]>) -> String {
    bytes.map_or_else(
        || "missing".into(),
        |bytes| format!("{:x}", Sha256::digest(bytes)),
    )
}
fn snapshot(record: &Record, source: String) -> UserPreferencesSnapshot {
    UserPreferencesSnapshot {
        source,
        values: record.values.clone(),
        claimed: record.claimed.clone(),
        desktop_imported: record.desktop_imported,
        desktop_main_imported: record.desktop_main_imported,
    }
}
