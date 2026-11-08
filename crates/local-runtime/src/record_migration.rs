//! Persisted record versions and migration (ORC-397). Every record the data root layout versions
//! with a top-level integer `version` opens through here: a current record is parsed as usual, an
//! older one is migrated by pure N to N+1 steps whose result must pass the store's current strict
//! parser, and a newer one, or one no step can update, is never rewritten. Only a process that
//! writes a record persists its migration, after copying the original into an upgrade backup
//! folder; a failed step or backup changes nothing. Stores keep the record's meaning; this module
//! owns the steps, the backups and their retention, and the once-per-process report.
use crate::LogValue;
use serde_json::Value;
use std::{path::PathBuf, sync::Mutex};

#[cfg(windows)]
mod windows;
#[cfg(all(test, windows))]
pub(crate) use windows::{KEPT_UPGRADE_FOLDERS, upgrade_folder};
#[cfg(windows)]
pub use windows::{back_up_for_reset, inspect_records, persist_migration};

/// Migration backups are Windows effects until ORC-328, so POSIX refuses before any effect and the
/// record stays as it is.
#[cfg(not(windows))]
pub fn persist_migration(
    _root: &crate::DataRoot,
    _path: &std::path::Path,
    _original: &[u8],
    _replace: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<PathBuf> {
    Err(std::io::ErrorKind::Unsupported.into())
}

/// The versioned stores are Windows effects until ORC-328, so POSIX inspects none.
#[cfg(not(windows))]
pub fn inspect_records(_root: &crate::DataRoot, _session: Option<&str>) -> Vec<RecordNotice> {
    Vec::new()
}

/// A record whose format carries a top-level integer `version`. Checkpoint slots, the restore
/// notice, earlier layouts and kept terminal output carry theirs in their frame header instead.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VersionedRecord {
    /// `internal/state/preferences.json`.
    Preferences,
    /// `internal/state/onboarding.json`.
    Onboarding,
    /// `internal/state/machines.json`.
    Machines,
    /// `internal/plugins/registry.json`.
    PluginRegistry,
    /// `sessions/<session>/session.json`.
    SessionLifecycle,
    /// `sessions/<session>/tui-position.json`.
    TuiPosition,
}

impl VersionedRecord {
    /// Every native record, in the order `config check` lists them.
    pub const ALL: [Self; 6] = [
        Self::Preferences,
        Self::Onboarding,
        Self::Machines,
        Self::PluginRegistry,
        Self::SessionLifecycle,
        Self::TuiPosition,
    ];

    /// The record's stable machine name, as logs and machine-readable output spell it.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Preferences => "preferences",
            Self::Onboarding => "onboarding",
            Self::Machines => "machines",
            Self::PluginRegistry => "plugin-registry",
            Self::SessionLifecycle => "session",
            Self::TuiPosition => "tui-position",
        }
    }

    /// The record a machine name spells, as [`Self::token`] writes it.
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|record| record.token() == token)
    }

    /// The version this build writes.
    pub const fn current_version(self) -> u64 {
        1
    }

    /// The steps that bring an earlier version to the current one. No format has changed since
    /// the first public release, so every record has none yet; the first change adds its step here.
    const fn steps(self) -> &'static [MigrationStep] {
        &[]
    }
}

/// One step from version `from` to `from + 1`: a pure function of the parsed record that answers
/// `None` when it cannot convert it. The result must carry `version` `from + 1`.
#[derive(Clone, Copy)]
pub(crate) struct MigrationStep {
    pub(crate) from: u64,
    pub(crate) apply: fn(Value) -> Option<Value>,
}

/// How a store opens one record's bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordOpening {
    /// The record carries the current version; the store parses it as usual.
    Current,
    /// An earlier record converted to the current version; `bytes` passed the store's parser. A
    /// reader uses them in memory, and a writer persists them with [`persist_migration`].
    Migrated { from: u64, bytes: Vec<u8> },
    /// Saved by a newer version of Orchbee: never rewritten, so rollback stays possible.
    Newer { version: u64 },
    /// An earlier version that no step could update; left as it is until a later release migrates
    /// it or the person resets it.
    NotUpdated { version: u64 },
    /// No top-level integer `version`: the store refuses it as it refuses any invalid record.
    Unversioned,
}

impl RecordOpening {
    /// The problem a process reports for this opening, when the record cannot be used.
    pub const fn problem(&self) -> Option<(RecordProblem, u64)> {
        match self {
            Self::Newer { version } => Some((RecordProblem::Newer, *version)),
            Self::NotUpdated { version } => Some((RecordProblem::NotUpdated, *version)),
            Self::Current | Self::Migrated { .. } | Self::Unversioned => None,
        }
    }
}

/// Classifies `bytes` for `record`, migrating an earlier version in memory. `admits` is the
/// store's current strict parser; a migrated result it refuses counts as a failed step.
pub fn open_record(
    record: VersionedRecord,
    bytes: &[u8],
    admits: impl Fn(&[u8]) -> bool,
) -> RecordOpening {
    open_with(bytes, record.current_version(), record.steps(), admits)
}

pub(crate) fn open_with(
    bytes: &[u8],
    current: u64,
    steps: &[MigrationStep],
    admits: impl Fn(&[u8]) -> bool,
) -> RecordOpening {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return RecordOpening::Unversioned;
    };
    let Some(version) = value.get("version").and_then(Value::as_u64) else {
        return RecordOpening::Unversioned;
    };
    if version == current {
        return RecordOpening::Current;
    }
    if version > current {
        return RecordOpening::Newer { version };
    }
    let not_updated = RecordOpening::NotUpdated { version };
    let mut value = value;
    for from in version..current {
        let Some(step) = steps.iter().find(|step| step.from == from) else {
            return not_updated;
        };
        match (step.apply)(value) {
            Some(next) if next.get("version").and_then(Value::as_u64) == Some(from + 1) => {
                value = next;
            }
            _ => return not_updated,
        }
    }
    match serde_json::to_vec(&value) {
        Ok(bytes) if admits(&bytes) => RecordOpening::Migrated {
            from: version,
            bytes,
        },
        _ => not_updated,
    }
}

/// Why a process cannot use a record. Each selects one of the two localized messages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordProblem {
    /// "Orchbee couldn't update {item} from an earlier version and left it unchanged."
    NotUpdated,
    /// "{item} was saved by a newer version of Orchbee, so this version won't change it."
    Newer,
}

impl RecordProblem {
    /// The machine name logs and machine-readable output use.
    pub const fn token(self) -> &'static str {
        match self {
            Self::NotUpdated => "not-updated",
            Self::Newer => "newer-format",
        }
    }
}

/// One record a process cannot use, with the version it carries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordNotice {
    pub record: VersionedRecord,
    pub problem: RecordProblem,
    pub version: u64,
    pub path: PathBuf,
}

static REPORTED: Mutex<Vec<RecordNotice>> = Mutex::new(Vec::new());

/// Logs `notice` the first time this process meets that record in that state and answers whether
/// it was the first time, so the process shows its message once.
pub fn report_record_notice(notice: &RecordNotice) -> bool {
    let first = REPORTED.lock().is_ok_and(|mut reported| {
        let known = reported
            .iter()
            .any(|seen| seen.path == notice.path && seen.problem == notice.problem);
        if !known {
            reported.push(notice.clone());
        }
        !known
    });
    if first {
        crate::log_event(
            crate::LogLevel::Warning,
            "record-unusable",
            &[
                ("record", LogValue::Token(notice.record.token())),
                ("problem", LogValue::Token(notice.problem.token())),
                ("version", LogValue::Count(notice.version)),
            ],
        );
    }
    first
}

/// The notices this process has reported so far, oldest first, for a surface that shows them.
pub fn reported_record_notices() -> Vec<RecordNotice> {
    REPORTED
        .lock()
        .map(|reported| reported.clone())
        .unwrap_or_default()
}

/// How many reported notices a surface has already shown.
static SHOWN: Mutex<usize> = Mutex::new(0);

/// The reported notices no surface has shown yet, oldest first, now marked shown, so a process
/// that lists them itself keeps a later surface from repeating them.
pub fn take_unshown_record_notices() -> Vec<RecordNotice> {
    let reported = reported_record_notices();
    let Ok(mut shown) = SHOWN.lock() else {
        return Vec::new();
    };
    let unshown = reported.get(*shown..).unwrap_or_default().to_vec();
    *shown = reported.len();
    unshown
}

/// The folder-name spelling of a product version: ASCII letters, digits, `.`, `_`, and `-` stay
/// and every other character becomes `-`.
#[cfg(windows)]
pub(crate) fn version_folder_name(version: &str) -> String {
    version
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '-'
            }
        })
        .collect()
}
