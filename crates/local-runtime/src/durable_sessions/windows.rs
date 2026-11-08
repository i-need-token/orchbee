//! Private lifecycle records, one `session.json` in each Session's folder, outlive the runtime
//! folder. Effects derive their paths from admission.
use crate::{
    ActivationState, DataRoot, ModeCheckedLocalSessionRendezvous as Rendezvous, SessionName,
    private_filesystem as permissions,
    process_registration::{ProcessRole, SessionProcesses},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

const MAXIMUM_BYTES: usize = 8192;
const MAXIMUM_SESSIONS: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Lifecycle {
    Active,
    Crashed,
    Stopped {
        request_id: [u8; 16],
    },
    Deleting {
        #[serde(deserialize_with = "Option::deserialize")]
        stopped_request_id: Option<[u8; 16]>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    version: u8,
    pub(crate) name: String,
    runtime: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub(crate) activation_id: Option<[u8; 16]>,
    pub(crate) lifecycle: Lifecycle,
    pub(crate) processes: SessionProcesses,
}

impl Record {
    pub(crate) fn same_active_generation(&self, other: &Self) -> bool {
        self.lifecycle == Lifecycle::Active
            && other.lifecycle == Lifecycle::Active
            && self.name == other.name
            && self.runtime == other.runtime
            && self.version == other.version
            && self.activation_id == other.activation_id
    }
    /// A record for the Session `name` whose runtime folder is `runtime`, the admitted canonical
    /// folder named by the Session.
    pub(crate) fn new(
        runtime: &Path,
        name: &SessionName,
        activation_id: Option<[u8; 16]>,
        lifecycle: Lifecycle,
    ) -> io::Result<Self> {
        Ok(Self {
            version: 1,
            name: name.as_str().to_owned(),
            runtime: runtime
                .to_str()
                .ok_or(io::ErrorKind::InvalidInput)?
                .to_owned(),
            activation_id,
            lifecycle,
            processes: SessionProcesses::default(),
        })
    }
    pub(crate) fn registered(
        runtime: &Path,
        name: &SessionName,
        activation_id: [u8; 16],
    ) -> io::Result<Self> {
        Self::new(runtime, name, Some(activation_id), Lifecycle::Active)
    }
    /// The runtime folder this record's generation uses.
    pub(crate) fn runtime(&self) -> &Path {
        Path::new(&self.runtime)
    }
    /// Whether the generation uses `runtime`. Folder spellings are compared as Windows compares
    /// them, through the endpoint identity that names the Session's pipes.
    pub(crate) fn uses_runtime(&self, runtime: &Path) -> bool {
        Rendezvous::host_endpoint_id(self.runtime()) == Rendezvous::host_endpoint_id(runtime)
    }
    pub(crate) fn activation(&self) -> Option<ActivationState> {
        match self.lifecycle {
            Lifecycle::Active => Some(ActivationState::Active),
            Lifecycle::Crashed => self
                .activation_id
                .map(|activation_id| ActivationState::Crashed { activation_id }),
            Lifecycle::Stopped { request_id } => Some(ActivationState::Stopped { request_id }),
            Lifecycle::Deleting { .. } => None,
        }
    }
    pub(crate) fn is_deleting(&self) -> bool {
        matches!(self.lifecycle, Lifecycle::Deleting { .. })
    }
    pub(crate) fn stopped_id(&self) -> Option<[u8; 16]> {
        match self.lifecycle {
            Lifecycle::Stopped { request_id } => Some(request_id),
            Lifecycle::Deleting { stopped_request_id } => stopped_request_id,
            Lifecycle::Active | Lifecycle::Crashed => None,
        }
    }
}

/// One Session folder's catalog record as listed.
pub(crate) enum CatalogRecord {
    Readable(Record),
    /// The folder holds a record this build cannot admit for the Session the folder names.
    Unreadable,
    /// The folder holds no record: checkpoint data or a TUI position without its lifecycle.
    Missing,
}

/// The catalog record's name: `session.json`, beside `session.lock` and `session.pending`.
const RECORD_FILE: &str = "session.json";
const RECORD_LOCK: &str = "session.lock";
const RECORD_PENDING: &str = "session.pending";
/// The files the catalog record owns in a Session folder.
pub(crate) const RECORD_FILES: [&str; 3] = [RECORD_FILE, RECORD_LOCK, RECORD_PENDING];

/// Whether `name` is the record's writer lock or staging file, which alone hold no Session data:
/// an interrupted activation or deletion can leave them in an otherwise empty folder.
pub(crate) fn is_record_guard(name: &str) -> bool {
    name == RECORD_LOCK || name == RECORD_PENDING
}

/// The `sessions/` folder of one data root. A scope is the spelling of one Session's folder name;
/// [`Directory::spelling`] finds the folder whatever letter case the Session is asked for in.
pub(crate) struct Directory {
    path: PathBuf,
    _pins: Vec<File>,
}

impl Directory {
    /// The existing folder, or `None` when the data root or its `sessions/` folder is absent.
    /// Opening creates nothing.
    pub(crate) fn open(root: &DataRoot) -> io::Result<Option<Self>> {
        let Some(mut pins) = crate::data_root::open_state_root(root.path())? else {
            return Ok(None);
        };
        let path = root.sessions();
        match permissions::open_tree(&path) {
            Ok(tree) => pins.extend(tree),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
        Ok(Some(Self { path, _pins: pins }))
    }
    pub(crate) fn create(root: &DataRoot) -> io::Result<Self> {
        let path = root.sessions();
        let pins = crate::data_root::create_state_directory(&path)?;
        Ok(Self { path, _pins: pins })
    }
    /// One Session's folder.
    pub(crate) fn folder(&self, scope: &str) -> PathBuf {
        self.path.join(scope)
    }
    /// The spelling of the existing folder of the Session `name`, compared without letter case,
    /// or `None` when it has none. Two folders that differ only in letter case, which only a
    /// case-sensitive directory can hold, are one Session spelled twice and refuse it.
    pub(crate) fn spelling(&self, name: &SessionName) -> io::Result<Option<String>> {
        let mut found = None;
        for entry in fs::read_dir(&self.path)? {
            let leaf = entry?.file_name();
            let Some(leaf) = leaf.to_str() else {
                continue;
            };
            if leaf.eq_ignore_ascii_case(name.as_str()) {
                if found.is_some() {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                found = Some(leaf.to_owned());
            }
        }
        Ok(found)
    }
    /// A record belongs to the folder that names its Session, whatever the letter case, and its
    /// runtime folder takes the same name.
    fn validate(&self, scope: &str, record: &Record) -> io::Result<()> {
        let name = SessionName::parse(&record.name).map_err(|_| io::ErrorKind::InvalidData)?;
        let runtime = record.runtime();
        if record.version != 1
            || name.as_str() != record.name
            || !record.name.eq_ignore_ascii_case(scope)
            || !runtime.is_absolute()
            || !runtime
                .file_name()
                .and_then(|leaf| leaf.to_str())
                .is_some_and(|leaf| leaf.eq_ignore_ascii_case(scope))
            || record.activation_id == Some([0; 16])
            || (!record.is_deleting() && record.activation_id.is_none())
            || record.stopped_id() == Some([0; 16])
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(())
    }
    fn record_path(&self, scope: &str, leaf: &str) -> io::Result<PathBuf> {
        if !crate::session_name::valid(scope) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        Ok(self.folder(scope).join(leaf))
    }
    fn read_bytes(&self, scope: &str) -> io::Result<Option<Vec<u8>>> {
        let path = self.record_path(scope, RECORD_FILE)?;
        let file = match permissions::open_file(&path, false, false) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        file.take((MAXIMUM_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAXIMUM_BYTES {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(Some(bytes))
    }
    pub(crate) fn read(&self, scope: &str) -> io::Result<Option<Record>> {
        let Some(bytes) = self.read_bytes(scope)? else {
            return Ok(None);
        };
        let record: Record =
            serde_json::from_slice(&bytes).map_err(|_| io::ErrorKind::InvalidData)?;
        self.validate(scope, &record)?;
        Ok(Some(record))
    }
    /// Acquires the record's writer lock, creating the Session folder and the lock on first use.
    pub(crate) fn try_writer(&self, scope: &str) -> io::Result<File> {
        let path = self.record_path(scope, RECORD_LOCK)?;
        let _folder = crate::data_root::create_state_directory(&self.folder(scope))?;
        let file = match permissions::open_file(&path, true, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                permissions::open_file(&path, false, true)?
            }
            Err(error) => return Err(error),
        };
        match file.try_lock() {
            Ok(()) => Ok(file),
            Err(TryLockError::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }
    pub(crate) fn existing_writer(&self, scope: &str) -> io::Result<Option<File>> {
        let path = self.record_path(scope, RECORD_LOCK)?;
        let file = match permissions::open_file(&path, false, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        match file.try_lock() {
            Ok(()) => Ok(Some(file)),
            Err(TryLockError::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }
    /// Every Session folder with its record, read without admitting process facts. Each entry is a
    /// private folder named by a valid Session name; anything else in `sessions/` fails the
    /// inventory instead of being hidden. A folder holding at most the record's lock and staging
    /// file, which an interrupted activation or deletion leaves, is no Session.
    pub(crate) fn list(&self, deadline: Instant) -> io::Result<Vec<(String, CatalogRecord)>> {
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.path)? {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let entry = entry?;
            let leaf = entry.file_name();
            let scope = leaf
                .to_str()
                .filter(|leaf| crate::session_name::valid(leaf))
                .ok_or(io::ErrorKind::InvalidData)?;
            let _pins = permissions::open_tree(&entry.path())?;
            let mut holds_data = false;
            for held in fs::read_dir(entry.path())? {
                holds_data |= held?
                    .file_name()
                    .to_str()
                    .is_none_or(|name| !is_record_guard(name));
            }
            if !holds_data {
                continue;
            }
            let record = match self.read(scope) {
                Ok(Some(record)) => CatalogRecord::Readable(record),
                Ok(None) => CatalogRecord::Missing,
                Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                    CatalogRecord::Unreadable
                }
                Err(error) => return Err(error),
            };
            records.push((scope.to_owned(), record));
            if records.len() > MAXIMUM_SESSIONS {
                return Err(io::ErrorKind::FileTooLarge.into());
            }
        }
        Ok(records)
    }
    pub(crate) fn record_file(&self, scope: &str) -> io::Result<PathBuf> {
        self.record_path(scope, RECORD_FILE)
    }

    /// Explicit deletion may replace an unreadable record only with its durable deletion fence.
    pub(crate) fn publish_deleting(&self, scope: &str, record: &Record) -> io::Result<()> {
        if !record.is_deleting() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        self.publish_inner(scope, record, true)
    }

    pub(crate) fn publish(&self, scope: &str, record: &Record) -> io::Result<()> {
        self.publish_inner(scope, record, false)
    }

    fn publish_inner(&self, scope: &str, record: &Record, deleting: bool) -> io::Result<()> {
        self.validate(scope, record)?;
        // Ordinary publication never replaces unreadable user data. Only an explicit,
        // guarded deletion can install its recovery fence over such a record.
        match self.read(scope) {
            Ok(_) => {}
            Err(error) if deleting && error.kind() == io::ErrorKind::InvalidData => {}
            Err(error) => return Err(error),
        }
        let bytes = serde_json::to_vec(record).map_err(io::Error::other)?;
        if bytes.len() > MAXIMUM_BYTES {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let _folder = crate::data_root::create_state_directory(&self.folder(scope))?;
        let staging = self.record_path(scope, RECORD_PENDING)?;
        remove_private(&staging)?;
        let mut file = permissions::open_file(&staging, true, true)?;
        let written = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        let target = self.record_path(scope, RECORD_FILE)?;
        let published = written.and_then(|()| {
            winsafe::MoveFileEx(
                staging.to_str().ok_or(io::ErrorKind::InvalidInput)?,
                Some(target.to_str().ok_or(io::ErrorKind::InvalidInput)?),
                winsafe::co::MOVEFILE::REPLACE_EXISTING,
            )
            .map_err(io::Error::other)
        });
        if let Err(error) = published {
            remove_private(&staging)?;
            return Err(error);
        }
        Ok(())
    }
    /// Removes the record and its lock and staging file, last of everything a deletion removes,
    /// and then the Session folder, which holds nothing else by then.
    pub(crate) fn remove(&self, scope: &str) -> io::Result<()> {
        let _current = self.read(scope)?;
        remove_private(&self.record_path(scope, RECORD_PENDING)?)?;
        remove_private(&self.record_path(scope, RECORD_LOCK)?)?;
        remove_private(&self.record_path(scope, RECORD_FILE)?)?;
        remove_empty_folder(&self.folder(scope))
    }
}

/// Removes an empty Session folder; an absent one is already removed.
pub(crate) fn remove_empty_folder(path: &Path) -> io::Result<()> {
    match permissions::open_tree(path) {
        Ok(pins) => {
            drop(pins);
            fs::remove_dir(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn remove_private(path: &Path) -> io::Result<()> {
    match permissions::open_file(path, false, false) {
        Ok(file) => {
            drop(file);
            fs::remove_file(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// The Server retains one exact activation generation, never a pathname-only stop authority.
pub struct RetainedSessionRecord {
    directory: Directory,
    scope: String,
    active: Record,
}
impl RetainedSessionRecord {
    pub(crate) fn checkpoint_directory(&self) -> PathBuf {
        self.directory.folder(&self.scope)
    }
    pub(crate) fn current_activation_id(&self) -> io::Result<[u8; 16]> {
        let current = self
            .directory
            .read(&self.scope)?
            .ok_or(io::ErrorKind::NotFound)?;
        if !current.same_active_generation(&self.active) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        current
            .activation_id
            .ok_or_else(|| io::ErrorKind::InvalidData.into())
    }
    /// The Active generation of the Session whose runtime folder `session` admitted. A record
    /// whose generation uses another runtime folder is refused: that Session runs elsewhere.
    pub fn retain(root: &DataRoot, session: &Rendezvous) -> io::Result<Self> {
        let runtime = session.runtime_directory();
        let scope = runtime
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(io::ErrorKind::InvalidInput)?;
        let directory = Directory::open(root)?.ok_or(io::ErrorKind::NotFound)?;
        let active = directory.read(scope)?.ok_or(io::ErrorKind::NotFound)?;
        if active.lifecycle != Lifecycle::Active || !active.uses_runtime(runtime) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(Self {
            directory,
            scope: scope.to_owned(),
            active,
        })
    }
    /// Whether this generation's Terminal Host was never started or its exact process is gone.
    /// A reserved spawn or an unknown observation answers false. Observation grants no
    /// termination authority.
    pub fn host_absent(&self) -> io::Result<bool> {
        let current = self
            .directory
            .read(&self.scope)?
            .ok_or(io::ErrorKind::NotFound)?;
        if !current.same_active_generation(&self.active) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(crate::process_observation::role_absent(
            &current.processes,
            ProcessRole::Host,
        ))
    }
    /// Only after confirmed child exit and listener cleanup, before the runtime stop receipt.
    pub fn complete(&mut self, request_id: [u8; 16]) -> io::Result<()> {
        if request_id == [0; 16] {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let _writer = self.directory.try_writer(&self.scope)?;
        let mut current = self
            .directory
            .read(&self.scope)?
            .ok_or(io::ErrorKind::NotFound)?;
        if current.name != self.active.name
            || current.runtime != self.active.runtime
            || current.activation_id != self.active.activation_id
            || current.version != self.active.version
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        if current.lifecycle == (Lifecycle::Stopped { request_id }) {
            return Ok(());
        }
        if current.lifecycle != Lifecycle::Active {
            return Err(io::ErrorKind::InvalidData.into());
        }
        current.lifecycle = Lifecycle::Stopped { request_id };
        self.directory.publish(&self.scope, &current)
    }
}
