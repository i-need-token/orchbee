//! Every store's migration wiring, driven through one scenario: a synthetic step from version 0
//! reaches the store through its ordinary opening, its writer backs the record up and persists the
//! migration, and its readers never write.
use super::{super::Fixture, inject};
use crate::{
    DataRoot, OpenedRecord, PRODUCT_VERSION, PluginRegistry, RecordProblem, SessionName,
    VersionedFile, VersionedRecord,
    durable_sessions::{Directory, Lifecycle, Record},
    record_migration::{MigrationStep, RecordOpening, open_record},
    reported_record_notices,
};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    fs,
    path::{Path, PathBuf},
};

/// Version 0 to 1 of any record: the earlier format carried a `legacy` marker.
fn drop_marker(mut value: Value) -> Option<Value> {
    let object = value.as_object_mut()?;
    object.remove("legacy")?;
    object.insert("version".into(), 1.into());
    Some(value)
}

fn refuse(_: Value) -> Option<Value> {
    None
}

const MIGRATE: &[MigrationStep] = &[MigrationStep {
    from: 0,
    apply: drop_marker,
}];
const FAIL: &[MigrationStep] = &[MigrationStep {
    from: 0,
    apply: refuse,
}];

/// One store, opened the way its owning process opens it.
trait Store {
    /// The record's file.
    fn path(&self) -> PathBuf;
    /// Whether the store's reader opened a record it can use.
    fn read(&self) -> bool;
    /// Whether the store's writer opened a record it can use and saved a change to it.
    fn write(&self) -> bool;
}

/// Rewrites the current record at `path` in the earlier format and answers its bytes.
fn make_earlier(path: &Path) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    value["version"] = json!(0);
    value["legacy"] = json!(true);
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(path, &bytes).unwrap();
    bytes
}

/// Every file under `path` with its contents, or nothing when `path` is absent.
fn tree(path: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap().path();
            if entry.is_dir() {
                files.extend(tree(&entry));
            } else {
                let bytes = fs::read(&entry).unwrap();
                files.push((entry, bytes));
            }
        }
    }
    files.sort();
    files
}

fn reported_not_updated(path: &Path) -> bool {
    reported_record_notices()
        .iter()
        .any(|notice| notice.path == path && notice.problem == RecordProblem::NotUpdated)
}

/// The proposal's rules for one record, each case in a fresh data root: `open` opens the store
/// and saves a current record with its own writer, before any step is injected.
fn scenario(record: VersionedRecord, open: impl Fn(&DataRoot) -> Box<dyn Store>) {
    // A writer backs the record up at its relative path, then replaces it with the migration; a
    // reader before it migrates in memory only.
    {
        let fixture = Fixture::new();
        let root = fixture.root();
        let store = open(&root);
        let path = store.path();
        let original = make_earlier(&path);
        let _steps = inject(record, MIGRATE);
        assert!(store.read(), "{record:?}: a reader uses the migration");
        assert_eq!(
            fs::read(&path).unwrap(),
            original,
            "{record:?}: a reader never writes"
        );
        assert!(
            !root.backups().exists(),
            "{record:?}: a reader backs nothing up"
        );

        assert!(store.write(), "{record:?}: the writer uses the migration");
        let folders: Vec<_> = fs::read_dir(root.backups())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(folders.len(), 1, "{record:?}");
        assert!(folders[0].ends_with(&format!("-upgrade-{PRODUCT_VERSION}")));
        let backup = root
            .backups()
            .join(&folders[0])
            .join(path.strip_prefix(root.path()).unwrap());
        assert_eq!(
            fs::read(&backup).unwrap(),
            original,
            "{record:?}: the backup"
        );
        let written: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(written["version"], json!(1), "{record:?}");
        assert!(
            written.get("legacy").is_none(),
            "{record:?}: the step applied"
        );

        // The record is current from then on: no later opening migrates or backs it up again.
        let backups = tree(&root.backups());
        assert_eq!(
            open_record(record, &fs::read(&path).unwrap(), |_| true),
            RecordOpening::Current
        );
        assert!(store.write() && store.read(), "{record:?}");
        assert_eq!(tree(&root.backups()), backups, "{record:?}");
    }
    // A failing step, or a backup that cannot be made, leaves the record byte-identical, and the
    // store treats it as not updated.
    for backup_fails in [false, true] {
        let fixture = Fixture::new();
        let root = fixture.root();
        let store = open(&root);
        let path = store.path();
        let original = make_earlier(&path);
        if backup_fails {
            fs::create_dir_all(root.internal()).unwrap();
            fs::write(root.backups(), b"not a folder").unwrap();
        }
        let _steps = inject(record, if backup_fails { MIGRATE } else { FAIL });
        assert!(!store.write(), "{record:?} backup_fails={backup_fails}");
        assert!(!store.read(), "{record:?} backup_fails={backup_fails}");
        assert_eq!(fs::read(&path).unwrap(), original, "{record:?}");
        assert!(reported_not_updated(&path), "{record:?}");
        if backup_fails {
            assert_eq!(fs::read(root.backups()).unwrap(), b"not a folder");
        } else {
            assert!(!root.backups().exists(), "{record:?}");
        }
    }
}

// Each writer persists a migration through its ordinary opening, and the scenario covers every
// versioned native record.
#[test]
fn every_store_persists_a_migration_only_in_its_writer() {
    let covered = [
        VersionedRecord::Onboarding,
        VersionedRecord::Machines,
        VersionedRecord::PluginRegistry,
        VersionedRecord::SessionLifecycle,
        VersionedRecord::TuiPosition,
    ];
    assert_eq!(covered, VersionedRecord::ALL);
    for record in [VersionedRecord::Onboarding, VersionedRecord::Machines] {
        scenario(record, |root| {
            File::open(VersionedFile::device(root.clone(), record))
        });
    }
    scenario(VersionedRecord::PluginRegistry, |root| {
        Box::new(Plugins::open(root))
    });
    scenario(VersionedRecord::SessionLifecycle, |root| {
        Box::new(Session::open(root))
    });
    scenario(VersionedRecord::TuiPosition, |root| {
        File::open(VersionedFile::session(
            root.clone(),
            VersionedRecord::TuiPosition,
            &SessionName::parse("work").unwrap(),
        ))
    });
}

/// A private file record: the onboarding and machine records and a Session's TUI position.
struct File {
    record: VersionedFile,
    saved: Cell<u64>,
}
impl File {
    fn open(record: VersionedFile) -> Box<dyn Store> {
        let file = Self {
            record,
            saved: Cell::new(0),
        };
        assert!(file.write());
        Box::new(file)
    }
}
fn admits_current(bytes: &[u8]) -> bool {
    serde_json::from_slice::<Value>(bytes)
        .is_ok_and(|value| value["version"] == json!(1) && value.get("legacy").is_none())
}
impl Store for File {
    fn path(&self) -> PathBuf {
        self.record.path()
    }
    fn read(&self) -> bool {
        matches!(
            self.record.read(1024, admits_current),
            Ok(OpenedRecord::Absent | OpenedRecord::Readable(_))
        )
    }
    fn write(&self) -> bool {
        let Ok((mut writer, Ok(OpenedRecord::Absent | OpenedRecord::Readable(_)))) =
            self.record.acquire(1024, admits_current)
        else {
            return false;
        };
        self.saved.set(self.saved.get() + 1);
        let bytes = serde_json::to_vec(&json!({"version": 1, "saved": self.saved.get()})).unwrap();
        writer.replace(&bytes).is_ok()
    }
}

/// The plugin registry: finding a recorded plugin's folders reads without the writer lock;
/// recording a new plugin's folders writes.
struct Plugins {
    registry: PluginRegistry,
    saved: Cell<u64>,
}
impl Plugins {
    fn open(root: &DataRoot) -> Self {
        let plugins = Self {
            registry: PluginRegistry::in_data_root(root).unwrap(),
            saved: Cell::new(0),
        };
        assert!(plugins.write());
        plugins
    }
}
impl Store for Plugins {
    fn path(&self) -> PathBuf {
        self.registry.root().join("registry.json")
    }
    fn read(&self) -> bool {
        self.registry.user_directories("plugin-1").is_ok()
    }
    fn write(&self) -> bool {
        self.saved.set(self.saved.get() + 1);
        self.registry
            .user_directories(&format!("plugin-{}", self.saved.get()))
            .is_ok()
    }
}

/// A Session's lifecycle record: a catalog read reads; a publication under the writer lock writes.
struct Session {
    directory: Directory,
    runtime: PathBuf,
    saved: Cell<u8>,
}
impl Session {
    const SCOPE: &str = "work";
    fn open(root: &DataRoot) -> Self {
        let session = Self {
            directory: Directory::create(root).unwrap(),
            runtime: root.path().join("runtime").join(Self::SCOPE),
            saved: Cell::new(0),
        };
        assert!(session.write());
        session
    }
}
impl Store for Session {
    fn path(&self) -> PathBuf {
        self.directory.record_file(Self::SCOPE).unwrap()
    }
    fn read(&self) -> bool {
        matches!(self.directory.read(Self::SCOPE), Ok(Some(_)))
    }
    fn write(&self) -> bool {
        let _writer = self.directory.try_writer(Self::SCOPE).unwrap();
        self.saved.set(self.saved.get() + 1);
        let record = Record::new(
            &self.runtime,
            &SessionName::parse(Self::SCOPE).unwrap(),
            Some([self.saved.get(); 16]),
            Lifecycle::Active,
        )
        .unwrap();
        self.directory.publish(Self::SCOPE, &record).is_ok()
    }
}
