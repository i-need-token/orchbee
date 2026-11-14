//! Persisted record versions and migration (ORC-397). Every record the data root layout versions
//! with a top-level integer `version` opens through here: a current record is parsed as usual, an
//! older one is migrated by pure N to N+1 steps whose result must pass the store's current strict
//! parser, and a newer one, or one no step can update, is never rewritten.
//!
//! A store receives a record's bytes only through [`open_to_read`], in a process that does not
//! write the record, which keeps a migration in memory, or through [`open_to_write`], under the
//! store's writer lock, which copies the original into an upgrade backup folder and persists the
//! migration through the store's own replacement before the store sees the bytes; [`VersionedFile`]
//! builds both on a private file. A failed step, backup, or replacement changes nothing. Since no
//! other path hands a store migrated bytes, a step added to [`VersionedRecord::steps`] reaches every
//! store, with its backup, without further wiring. Stores keep the record's meaning; this module
//! owns the steps, the backups and their retention, and the once-per-process report.
use crate::{DataRoot, LogValue, PrivateCache, SessionName};
use serde_json::Value;
use std::{
    io,
    path::{Path, PathBuf},
    sync::Mutex,
};

mod backup;
pub use backup::inspect_records;
pub(crate) use backup::persist_migration;
#[cfg(test)]
pub(crate) use backup::{KEPT_UPGRADE_FOLDERS, upgrade_folder};

/// A record whose format carries a top-level integer `version`. Checkpoint slots, the restore
/// notice and kept terminal output carry theirs in their frame header instead.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VersionedRecord {
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
    pub const ALL: [Self; 4] = [
        Self::Machines,
        Self::PluginRegistry,
        Self::SessionLifecycle,
        Self::TuiPosition,
    ];

    /// The record's stable machine name, as logs and machine-readable output spell it.
    pub const fn token(self) -> &'static str {
        match self {
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

    /// The record's name: its file is `<name>.json`, beside its writer lock `<name>.lock` and
    /// staging file `<name>.pending`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Machines => "machines",
            Self::PluginRegistry => "registry",
            Self::SessionLifecycle => "session",
            Self::TuiPosition => "tui-position",
        }
    }

    /// The record's file name.
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Machines => "machines.json",
            Self::PluginRegistry => "registry.json",
            Self::SessionLifecycle => "session.json",
            Self::TuiPosition => "tui-position.json",
        }
    }

    /// The steps that bring an earlier version to the current one. No format has changed since
    /// the first public release, so every record has none yet; the first change adds its step here
    /// and nowhere else.
    fn steps(self) -> &'static [MigrationStep] {
        #[cfg(test)]
        if let Some(steps) = crate::tests::record_migration::injected_steps(self) {
            return steps;
        }
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

/// How one record's bytes classify before [`open_to_read`] or [`open_to_write`] hands them to a
/// store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RecordOpening {
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
    pub(crate) const fn problem(&self) -> Option<(RecordProblem, u64)> {
        match self {
            Self::Newer { version } => Some((RecordProblem::Newer, *version)),
            Self::NotUpdated { version } => Some((RecordProblem::NotUpdated, *version)),
            Self::Current | Self::Migrated { .. } | Self::Unversioned => None,
        }
    }
}

/// Classifies `bytes` for `record`, migrating an earlier version in memory. `admits` is the
/// store's current strict parser; a migrated result it refuses counts as a failed step.
pub(crate) fn open_record(
    record: VersionedRecord,
    bytes: &[u8],
    admits: impl Fn(&[u8]) -> bool,
) -> RecordOpening {
    open_with(bytes, record.current_version(), record.steps(), admits)
}

/// A versioned record as its store opened it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenedRecord {
    /// No record exists.
    Absent,
    /// Bytes for the store's strict parser: the record as read, an earlier version converted to
    /// the current one, or a record without a version, which the parser refuses as invalid.
    Readable(Vec<u8>),
    /// A version this build cannot use: saved by a newer version, or an earlier one that no step
    /// updates or whose migration could not be saved. It is reported once per process and never
    /// rewritten.
    Unusable(RecordNotice),
}

/// Opens `bytes`, read from `path`, in a process that does not write the record. An earlier
/// version is migrated in memory only, so a Client never rewrites what an older running Server
/// owns.
pub fn open_to_read(
    record: VersionedRecord,
    path: &Path,
    bytes: Option<Vec<u8>>,
    admits: impl Fn(&[u8]) -> bool,
) -> OpenedRecord {
    let Some(bytes) = bytes else {
        return OpenedRecord::Absent;
    };
    match open_record(record, &bytes, admits) {
        RecordOpening::Migrated { from, .. } if not_updated_here(path) => {
            unusable(record, RecordProblem::NotUpdated, from, path)
        }
        RecordOpening::Migrated { bytes, .. } => OpenedRecord::Readable(bytes),
        RecordOpening::Current | RecordOpening::Unversioned => OpenedRecord::Readable(bytes),
        RecordOpening::Newer { version } => unusable(record, RecordProblem::Newer, version, path),
        RecordOpening::NotUpdated { version } => {
            unusable(record, RecordProblem::NotUpdated, version, path)
        }
    }
}

/// Opens `bytes` that a writer read from `path` under its own writer lock. An earlier version is
/// migrated, the original is copied into the upgrade backup folder of `root`, and `replace`, the
/// store's flushed atomic replacement, writes the migrated record, which the writer then uses as
/// current. A failed backup or replacement leaves the record byte-identical, and the writer treats
/// it as an earlier record no step could update.
pub fn open_to_write(
    root: &DataRoot,
    record: VersionedRecord,
    path: &Path,
    bytes: Option<Vec<u8>>,
    admits: impl Fn(&[u8]) -> bool,
    replace: impl FnOnce(&[u8]) -> io::Result<()>,
) -> OpenedRecord {
    open_writing(Some(root), record, path, bytes, admits, replace)
}

/// [`open_to_write`] for a writer that may lack a data root: without one there is no upgrade
/// backup folder, so an earlier record stays unchanged as one no step could update.
pub(crate) fn open_writing(
    root: Option<&DataRoot>,
    record: VersionedRecord,
    path: &Path,
    bytes: Option<Vec<u8>>,
    admits: impl Fn(&[u8]) -> bool,
    replace: impl FnOnce(&[u8]) -> io::Result<()>,
) -> OpenedRecord {
    let Some(original) = bytes else {
        return OpenedRecord::Absent;
    };
    match open_record(record, &original, admits) {
        RecordOpening::Migrated { from, .. } if not_updated_here(path) => {
            unusable(record, RecordProblem::NotUpdated, from, path)
        }
        RecordOpening::Migrated { from, bytes } => {
            let persisted = root
                .ok_or_else(|| io::Error::from(io::ErrorKind::Unsupported))
                .and_then(|root| persist_migration(root, path, &original, || replace(&bytes)));
            match persisted {
                Ok(_) => OpenedRecord::Readable(bytes),
                Err(_) => unusable(record, RecordProblem::NotUpdated, from, path),
            }
        }
        RecordOpening::Current | RecordOpening::Unversioned => OpenedRecord::Readable(original),
        RecordOpening::Newer { version } => unusable(record, RecordProblem::Newer, version, path),
        RecordOpening::NotUpdated { version } => {
            unusable(record, RecordProblem::NotUpdated, version, path)
        }
    }
}

/// Whether this process already reported the record at `path` as not updated. A migration this
/// process could not save stays unused until it ends, so its readers and writers agree on the
/// record, and it is not attempted again with every write.
fn not_updated_here(path: &Path) -> bool {
    REPORTED.lock().is_ok_and(|reported| {
        reported
            .iter()
            .any(|seen| seen.path == path && seen.problem == RecordProblem::NotUpdated)
    })
}

fn unusable(
    record: VersionedRecord,
    problem: RecordProblem,
    version: u64,
    path: &Path,
) -> OpenedRecord {
    let notice = RecordNotice {
        record,
        problem,
        version,
        path: path.to_owned(),
    };
    report_record_notice(&notice);
    OpenedRecord::Unusable(notice)
}

/// A versioned record kept as one private JSON file whose writer lock and staging file sit beside
/// it, named after [`VersionedRecord::name`]: the onboarding and machine records in
/// `internal/state/` and a Session's TUI position. A read takes no lock and keeps a migration in
/// memory; [`Self::acquire`] opens the record under the writer lock and persists one first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionedFile {
    root: DataRoot,
    record: VersionedRecord,
    folder: PathBuf,
}

impl VersionedFile {
    /// `record` in the device records folder, `internal/state/`.
    pub fn device(root: DataRoot, record: VersionedRecord) -> Self {
        let folder = root.state();
        Self {
            root,
            record,
            folder,
        }
    }

    /// `record` in the folder of `session`, `sessions/<session>/`.
    pub fn session(root: DataRoot, record: VersionedRecord, session: &SessionName) -> Self {
        let folder = root.sessions().join(session.as_str());
        Self {
            root,
            record,
            folder,
        }
    }

    /// The folder that holds the record, its writer lock, and its staging file.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The record's file.
    pub fn path(&self) -> PathBuf {
        self.folder.join(self.record.file_name())
    }

    /// Reads at most `maximum` bytes without a lock or creating anything, and opens them with
    /// [`open_to_read`].
    pub fn read(&self, maximum: usize, admits: impl Fn(&[u8]) -> bool) -> io::Result<OpenedRecord> {
        let bytes = PrivateCache::read_existing(&self.folder, self.record.file_name(), maximum)?;
        Ok(open_to_read(self.record, &self.path(), bytes, admits))
    }

    /// Acquires the record's writer lock, creating its folder and lock on first use, then reads at
    /// most `maximum` bytes and opens them with [`open_to_write`], whose replacement is the
    /// writer's own. The outer error is the lock's; the inner one is the read's, so a store can
    /// still replace a record too large to read. The writer holds the lock until dropped.
    pub fn acquire(
        &self,
        maximum: usize,
        admits: impl Fn(&[u8]) -> bool,
    ) -> io::Result<(VersionedWriter, io::Result<OpenedRecord>)> {
        let mut writer = VersionedWriter {
            cache: PrivateCache::acquire(&self.folder, self.record.name())?,
            file: self.record.file_name(),
        };
        let opened = writer.cache.read(writer.file, maximum).map(|bytes| {
            open_to_write(
                &self.root,
                self.record,
                &self.path(),
                bytes,
                admits,
                |migrated| writer.replace(migrated),
            )
        });
        Ok((writer, opened))
    }
}

/// The writer lock of one [`VersionedFile`], held until dropped.
pub struct VersionedWriter {
    cache: PrivateCache,
    file: &'static str,
}

impl VersionedWriter {
    /// Flushes `bytes` and atomically replaces the record with them.
    pub fn replace(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.cache.replace(self.file, bytes)
    }
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
