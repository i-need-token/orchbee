//! One durable device/configuration preference authority for native and Desktop clients.
mod launch_admission;
mod terminal_colors;
pub use terminal_colors::{UserColorThemes, UserTerminalColors};
mod terminal_defaults;
pub use terminal_defaults::UserTerminalLaunchIntent;
mod scope;
pub use scope::UserPreferencesScope;
mod storage;
#[cfg(test)]
mod tests;

use orchbee_workbench_protocol::{
    MAXIMUM_USER_PREFERENCES_BYTES, PreferenceKey, PreferencesFailure, UserPreferencesRequest,
    UserPreferencesResponse, UserPreferencesSnapshot, UserPreferencesValues,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const RECORD_NAME: &str = crate::StateRecord::Preferences.file_name();

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
    /// Fields a later release added, kept unread and written back unchanged; two installed
    /// versions read this record at once.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    extensions: serde_json::Map<String, serde_json::Value>,
    values: UserPreferencesValues,
    #[serde(deserialize_with = "unique_claimed_keys")]
    claimed: BTreeSet<PreferenceKey>,
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
    /// The values this process keeps while the record's version is one it cannot use: such a
    /// record is never rewritten, so the store works from defaults and keeps changes here only.
    memory: std::sync::Mutex<Option<Record>>,
}
impl UserPreferencesStore {
    /// Construction is read-only; even the record's parent is created only by an accepted write.
    pub const fn new(scope: UserPreferencesScope) -> Self {
        Self {
            scope,
            memory: std::sync::Mutex::new(None),
        }
    }

    /// Exact authority identity, also used by callers to explain a deliberately isolated scope.
    pub const fn scope(&self) -> &UserPreferencesScope {
        &self.scope
    }

    /// One bounded operation; callers own transport deadlines and live application acknowledgments.
    pub fn execute(&self, request: &UserPreferencesRequest) -> UserPreferencesResponse {
        match self.perform(request) {
            Ok((snapshot, changed)) => UserPreferencesResponse::Snapshot {
                snapshot: Box::new(snapshot),
                changed,
            },
            Err(CommitError::Refused(reason)) => UserPreferencesResponse::Refused { reason },
            #[cfg(unix)]
            Err(CommitError::OutcomeUnknown) => UserPreferencesResponse::OutcomeUnknown {},
        }
    }

    /// Reads never create a record, lock, or directory. A record of an earlier version is migrated
    /// in memory; the next accepted update persists the migration first.
    pub fn read(&self) -> Result<UserPreferencesSnapshot, PreferencesFailure> {
        let bytes = storage::read(&self.scope)?;
        let source = source_token(bytes.as_deref());
        let opened = crate::open_to_read(
            crate::VersionedRecord::Preferences,
            &self.record_path(),
            bytes,
            |bytes| strict(bytes).is_some(),
        );
        let (record, source) = self.decode(opened, source)?;
        Ok(snapshot(&record, source))
    }

    fn record_path(&self) -> std::path::PathBuf {
        self.scope.directory.join(RECORD_NAME)
    }

    /// The record an opening yields, with `source`, the token of the bytes as read: a migrated
    /// record keeps its original's token, since the migration changed no meaning a client saw.
    fn decode(
        &self,
        opened: crate::OpenedRecord,
        source: String,
    ) -> Result<(Record, String), PreferencesFailure> {
        match opened {
            crate::OpenedRecord::Readable(bytes) => {
                if bytes.len() > MAXIMUM_USER_PREFERENCES_BYTES {
                    return Err(PreferencesFailure::InvalidSource);
                }
                let record = strict(&bytes).ok_or(PreferencesFailure::InvalidSource)?;
                Ok((record, source))
            }
            crate::OpenedRecord::Unusable(_) => Ok(self.remembered()),
            crate::OpenedRecord::Absent => {
                let record = Record::empty();
                launch_admission::validate(&record.values.launch_profiles)
                    .map_err(|_| PreferencesFailure::InvalidSource)?;
                Ok((record, source))
            }
        }
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
            UserPreferencesRequest::Update { changes, .. } => changes,
            UserPreferencesRequest::Read {} => unreachable!("read handled before write admission"),
        };
        changes.apply(&UserPreferencesValues::default())?;
        if matches!(request, UserPreferencesRequest::Update { source, .. } if source == "unreadable")
            && !changes.is_reset_all()
        {
            return Err(PreferencesFailure::InvalidChanges.into());
        }
        if changes.is_empty() {
            return Err(PreferencesFailure::InvalidChanges.into());
        }
        let mut writer = storage::Writer::acquire(&self.scope)?;
        let before = writer.read()?;
        let source = source_token(before.as_deref());
        // An earlier record is backed up and persisted before anything else, so `stored`, what the
        // record holds when the update commits, is the migrated record from then on.
        let mut stored = before.clone();
        let opened = crate::open_to_write(
            &self.scope.root,
            crate::VersionedRecord::Preferences,
            &self.record_path(),
            before.clone(),
            |bytes| strict(bytes).is_some(),
            |migrated| {
                writer
                    .commit(before.as_deref(), migrated)
                    .map_err(|_| std::io::Error::from(std::io::ErrorKind::Other))?;
                stored = Some(migrated.to_vec());
                Ok(())
            },
        );
        let (previous, previous_source) = match self.decode(opened, source) {
            Ok(previous) => previous,
            Err(PreferencesFailure::InvalidSource)
                if matches!(request, UserPreferencesRequest::Update { source, .. }
                    if source == "unreadable")
                    && changes.is_reset_all() =>
            {
                (Record::empty(), "unreadable".into())
            }
            Err(reason) => return Err(reason.into()),
        };
        let in_memory = previous_source.starts_with(MEMORY_SOURCE);
        let mut candidate = previous.clone();
        match request {
            UserPreferencesRequest::Update { source, changes } => {
                if *source != previous_source {
                    return Err(PreferencesFailure::SourceChanged.into());
                }
                candidate.values = changes.apply(&candidate.values)?;
                candidate.claimed.extend(changes.keys());
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
        #[cfg(windows)]
        if in_memory
            && changes.is_reset_all()
            && let Some(original) = stored.as_deref()
        {
            // Resetting every setting is the one change that replaces a record this build cannot
            // use; the person asked for it, and the record is backed up first.
            crate::back_up_for_reset(
                &self.scope.root,
                "reset-settings",
                &self.record_path(),
                original,
            )
            .map_err(|_| PreferencesFailure::WriteFailed)?;
            writer.commit(stored.as_deref(), &after)?;
            if let Ok(mut memory) = self.memory.lock() {
                *memory = None;
            }
            return Ok((snapshot(&candidate, source_token(Some(&after))), true));
        }
        if in_memory {
            let source = memory_token(&after);
            if let Ok(mut memory) = self.memory.lock() {
                *memory = Some(candidate.clone());
            }
            return Ok((snapshot(&candidate, source), true));
        }
        writer.commit(stored.as_deref(), &after)?;
        Ok((snapshot(&candidate, source_token(Some(&after))), true))
    }
}

/// Source tokens of values kept in memory begin with this, so a write never reaches the record.
const MEMORY_SOURCE: &str = "memory-";

impl Record {
    fn empty() -> Self {
        Self {
            version: 1,
            extensions: serde_json::Map::new(),
            values: UserPreferencesValues::default(),
            claimed: BTreeSet::new(),
        }
    }
}

impl UserPreferencesStore {
    /// The values kept in memory, or the defaults before any change.
    fn remembered(&self) -> (Record, String) {
        let record = self
            .memory
            .lock()
            .ok()
            .and_then(|memory| memory.clone())
            .unwrap_or_else(Record::empty);
        let bytes = serde_json::to_vec(&record).unwrap_or_default();
        (record, memory_token(&bytes))
    }
}

fn memory_token(bytes: &[u8]) -> String {
    format!("{MEMORY_SOURCE}{:x}", Sha256::digest(bytes))
}

/// The current strict record, or `None` when `bytes` are not one.
fn strict(bytes: &[u8]) -> Option<Record> {
    let record: Record = serde_json::from_slice(bytes).ok()?;
    (record.version == 1
        && record.values.validate().is_ok()
        && launch_admission::validate(&record.values.launch_profiles).is_ok())
    .then_some(record)
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
    }
}
