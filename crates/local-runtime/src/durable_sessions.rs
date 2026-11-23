//! Shared activation generations bind catalog admission, direct-child registration and clean-stop
//! publication. Private lifecycle records, one `session.json` in each Session's folder, outlive the
//! runtime folder. Effects derive their paths from admission.
#[cfg(windows)]
use crate::process_registration::ProcessIdentity;
use crate::{
    ActivationState, DataRoot, HeldLock, ModeCheckedLocalSessionRendezvous as Rendezvous,
    SessionName, private_filesystem as permissions, process_registration::SessionProcesses,
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

mod successor;
pub use successor::Successor;
#[cfg(windows)]
pub(crate) use successor::{
    commit as commit_successor, record_worker as record_successor_worker,
    remove as remove_successor, reserve as reserve_successor,
};

const MAXIMUM_BYTES: usize = 8192;
const MAXIMUM_SESSIONS: usize = 256;
/// The `extensions` member an `exit` writes: the activation identifier of the generation it ended.
const EXITED: &str = "exited";
/// The `extensions` member a stop the idle move began writes with `Stopped`: the store's active
/// version the Session moved to.
const MOVED_TO: &str = "moved_to";

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
    /// Fields a later release added, kept unread; two installed versions read this record at once.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    extensions: serde_json::Map<String, serde_json::Value>,
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
    /// Whether an `exit` ended this generation: `extensions.exited` names its activation.
    pub(crate) fn exited(&self) -> bool {
        let recorded = self
            .extensions
            .get(EXITED)
            .and_then(|value| serde_json::from_value::<[u8; 16]>(value.clone()).ok());
        recorded.is_some() && recorded == self.activation_id
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
            extensions: serde_json::Map::new(),
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
    /// The record of a new generation, which adopts `hosts`, the Hosts its predecessor left
    /// running.
    pub(crate) fn registered(
        runtime: &Path,
        name: &SessionName,
        activation_id: [u8; 16],
        hosts: &[crate::process_registration::KeptHost],
    ) -> io::Result<Self> {
        let mut record = Self::new(runtime, name, Some(activation_id), Lifecycle::Active)?;
        record.processes = SessionProcesses::adopting(hosts);
        Ok(record)
    }
    /// The runtime folder this record's generation uses.
    pub(crate) fn runtime(&self) -> &Path {
        Path::new(&self.runtime)
    }
    /// Whether the generation uses `runtime`. Folder spellings are compared through the endpoint
    /// identity that names the Session's pipes or sockets, as the host compares them.
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
    Readable(Box<Record>),
    /// The folder holds a record this build cannot admit for the Session the folder names.
    Unreadable,
    /// The folder holds a record whose version this build cannot use: saved by a newer version,
    /// or an earlier one no step updates. It is never rewritten.
    Unusable,
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

/// One Session record's held writer lock, `session.lock`. A registered child's creator holds it from
/// before its Spawning write until its identity write and resume, or its failure cleanup, so a
/// Spawning slot read under it names an attempt that ended without an identity.
pub(crate) struct RecordWriter {
    _lock: HeldLock,
}

/// The `sessions/` folder of one data root. A scope is the spelling of one Session's folder name;
/// [`Directory::spelling`] finds the folder whatever letter case the Session is asked for in.
pub(crate) struct Directory {
    root: DataRoot,
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
        Ok(Some(Self {
            root: root.clone(),
            path,
            _pins: pins,
        }))
    }
    pub(crate) fn create(root: &DataRoot) -> io::Result<Self> {
        let path = root.sessions();
        let pins = crate::data_root::create_state_directory(&path)?;
        Ok(Self {
            root: root.clone(),
            path,
            _pins: pins,
        })
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
        let file = match permissions::open_published(&path) {
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
    /// The record, refused as `InvalidData` when this build cannot admit it. A record whose
    /// version this build cannot use is refused the same way, carrying [`UnusableRecord`], and is
    /// reported once per process. An earlier version is migrated in memory only: every write
    /// persists it first, under the writer lock, through [`Self::read_to_replace`].
    pub(crate) fn read(&self, scope: &str) -> io::Result<Option<Record>> {
        let opened = crate::open_to_read(
            crate::VersionedRecord::SessionLifecycle,
            &self.record_path(scope, RECORD_FILE)?,
            self.read_bytes(scope)?,
            |bytes| self.admits(scope, bytes),
        );
        self.admit(scope, opened)
    }
    /// [`Self::read`] for a writer holding the record's writer lock, about to replace the record:
    /// an earlier version is first backed up and persisted, so no write replaces it unbacked.
    /// Session records therefore migrate only in a writer, which outside the Server writes only
    /// after it has proved that no Server of the Session runs.
    fn read_to_replace(&self, scope: &str) -> io::Result<Option<Record>> {
        let opened = crate::open_to_write(
            &self.root,
            crate::VersionedRecord::SessionLifecycle,
            &self.record_path(scope, RECORD_FILE)?,
            self.read_bytes(scope)?,
            |bytes| self.admits(scope, bytes),
            |migrated| self.replace(scope, migrated),
        );
        self.admit(scope, opened)
    }
    fn admits(&self, scope: &str, bytes: &[u8]) -> bool {
        serde_json::from_slice::<Record>(bytes)
            .is_ok_and(|record| self.validate(scope, &record).is_ok())
    }
    fn admit(&self, scope: &str, opened: crate::OpenedRecord) -> io::Result<Option<Record>> {
        let bytes = match opened {
            crate::OpenedRecord::Absent => return Ok(None),
            crate::OpenedRecord::Unusable(_) => {
                return Err(io::Error::new(io::ErrorKind::InvalidData, UnusableRecord));
            }
            crate::OpenedRecord::Readable(bytes) => bytes,
        };
        let record: Record =
            serde_json::from_slice(&bytes).map_err(|_| io::ErrorKind::InvalidData)?;
        self.validate(scope, &record)?;
        Ok(Some(record))
    }
    /// Acquires the record's writer lock, creating the Session folder and the lock on first use.
    pub(crate) fn try_writer(&self, scope: &str) -> io::Result<RecordWriter> {
        let path = self.record_path(scope, RECORD_LOCK)?;
        let _folder = crate::data_root::create_state_directory(&self.folder(scope))?;
        let file = match permissions::open_file(&path, true, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                permissions::open_file(&path, false, true)?
            }
            Err(error) => return Err(error),
        };
        match HeldLock::try_exclusive(file) {
            Ok(lock) => Ok(RecordWriter { _lock: lock }),
            Err(TryLockError::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }
    pub(crate) fn existing_writer(&self, scope: &str) -> io::Result<Option<RecordWriter>> {
        let path = self.record_path(scope, RECORD_LOCK)?;
        let file = match permissions::open_file(&path, false, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        match HeldLock::try_exclusive(file) {
            Ok(lock) => Ok(Some(RecordWriter { _lock: lock })),
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
        for leaf in self.listing()? {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            if let Some(listed) = self.listed(&leaf)? {
                records.push(listed);
                if records.len() > MAXIMUM_SESSIONS {
                    return Err(io::ErrorKind::FileTooLarge.into());
                }
            }
        }
        Ok(records)
    }
    /// The names `sessions/` lists, in the order the listing gives them.
    pub(crate) fn listing(&self) -> io::Result<Vec<OsString>> {
        fs::read_dir(&self.path)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect()
    }
    /// The record of one listed name, or `None` when its folder holds no Session. A name that is
    /// not a valid Session name fails the inventory as `InvalidData`. A folder a deletion removed
    /// after the listing named it is gone, so it holds no Session and is skipped, not failed.
    pub(crate) fn listed(&self, leaf: &OsStr) -> io::Result<Option<(String, CatalogRecord)>> {
        let scope = leaf
            .to_str()
            .filter(|leaf| crate::session_name::valid(leaf))
            .ok_or(io::ErrorKind::InvalidData)?;
        let pins = permissions::outlast_open(|| permissions::open_tree(&self.folder(scope)));
        let _pins = match pins {
            Ok(pins) => pins,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut holds_data = false;
        for held in fs::read_dir(self.folder(scope))? {
            holds_data |= held?
                .file_name()
                .to_str()
                .is_none_or(|name| !is_record_guard(name));
        }
        if !holds_data {
            return Ok(None);
        }
        let record = match self.read(scope) {
            Ok(Some(record)) => CatalogRecord::Readable(Box::new(record)),
            Ok(None) => CatalogRecord::Missing,
            Err(error) if unusable(&error) => CatalogRecord::Unusable,
            Err(error) if error.kind() == io::ErrorKind::InvalidData => CatalogRecord::Unreadable,
            Err(error) => return Err(error),
        };
        Ok(Some((scope.to_owned(), record)))
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
        match self.read_to_replace(scope) {
            Ok(_) => {}
            Err(error)
                if deleting && error.kind() == io::ErrorKind::InvalidData && !unusable(&error) => {}
            Err(error) => return Err(error),
        }
        let bytes = serde_json::to_vec(record).map_err(io::Error::other)?;
        self.replace(scope, &bytes)
    }
    /// Flushes `bytes` into the staging file and atomically moves it over the record.
    fn replace(&self, scope: &str, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() > MAXIMUM_BYTES {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let _folder = crate::data_root::create_state_directory(&self.folder(scope))?;
        let staging = self.record_path(scope, RECORD_PENDING)?;
        remove_private(&staging)?;
        let mut file = permissions::open_file(&staging, true, true)?;
        let written = file.write_all(bytes).and_then(|()| file.sync_all());
        drop(file);
        let target = self.record_path(scope, RECORD_FILE)?;
        let published = written.and_then(|()| permissions::replace(&staging, &target));
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

/// Marks a record whose version this build cannot use: it reads as invalid everywhere, but no
/// deletion fence replaces it, since such a record is never rewritten.
#[derive(Debug)]
pub(crate) struct UnusableRecord;

impl std::fmt::Display for UnusableRecord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the record's version is one this build cannot use")
    }
}

impl std::error::Error for UnusableRecord {}

/// Whether `error` refused a record whose version this build cannot use.
pub(crate) fn unusable(error: &io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(|inner| inner.is::<UnusableRecord>())
}

/// Removes an empty Session folder; an absent one is already removed.
pub(crate) fn remove_empty_folder(path: &Path) -> io::Result<()> {
    match permissions::open_tree(path) {
        Ok(pins) => {
            drop(pins);
            permissions::remove_dir(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn remove_private(path: &Path) -> io::Result<()> {
    match permissions::open_file(path, false, false) {
        Ok(file) => {
            drop(file);
            permissions::remove_file(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// The Keeper the Active generation of the Session at `runtime` records, for the Worker that adopts
/// it; `None` when the record names no identified Keeper.
#[cfg(windows)]
pub(crate) fn recorded_keeper(
    root: &DataRoot,
    runtime: &Path,
) -> io::Result<Option<ProcessIdentity>> {
    let scope = runtime
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let directory = Directory::open(root)?.ok_or(io::ErrorKind::NotFound)?;
    let active = directory.read(scope)?.ok_or(io::ErrorKind::NotFound)?;
    if active.lifecycle != Lifecycle::Active || !active.uses_runtime(runtime) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(active
        .processes
        .registration(crate::process_registration::ProcessRole::Keeper)
        .identity())
}

/// The Server retains one exact activation generation, never a pathname-only stop authority.
pub struct RetainedSessionRecord {
    directory: Directory,
    scope: String,
    active: Record,
}
/// The store's active version the Session whose runtime folder is `runtime` moved to, while its
/// record says the idle move stopped it; a later activation's record names none. Reading takes no
/// lock: a Client that lost its Server asks this while it waits to come back.
pub fn session_moved_to(root: &DataRoot, runtime: &Path) -> io::Result<Option<String>> {
    let scope = runtime
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let Some(directory) = Directory::open(root)? else {
        return Ok(None);
    };
    let Some(record) = directory.read(scope)? else {
        return Ok(None);
    };
    if !matches!(record.lifecycle, Lifecycle::Stopped { .. }) || !record.uses_runtime(runtime) {
        return Ok(None);
    }
    Ok(record
        .extensions
        .get(MOVED_TO)
        .and_then(|value| value.as_str())
        .map(str::to_owned))
}

/// What [`RetainedSessionRecord::hosts`] observed of a generation's Terminal Host in one slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostPresence {
    /// No Host was started (Windows only).
    NotStarted,
    /// The Host's creation is reserved but not identified, so it never ran (Windows only).
    Spawning,
    /// The registered Host is gone, or, on POSIX, no process holds the Host lock.
    Gone,
    /// A Host runs: on Windows its registered process ID and creation time; POSIX registers none.
    Running(Option<(u32, u64)>),
    /// The registered Host could not be observed.
    Unknown,
}

/// What [`RetainedSessionRecord::hosts`] observed of one Host slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostSlotPresence {
    pub slot: crate::HostSlot,
    pub presence: HostPresence,
    /// The epoch the slot's Host recorded once its Worker connected.
    pub epoch: Option<[u8; 16]>,
}

/// What `record` registers for `role`, the Host of a slot or the Keeper, observed now.
fn role_presence(
    record: &Record,
    role: crate::process_registration::ProcessRole,
) -> io::Result<HostPresence> {
    #[cfg(windows)]
    return Ok(crate::process_observation::presence(
        &record.processes,
        role,
    ));
    // POSIX runs its one Host in the first slot and starts no Keeper.
    #[cfg(unix)]
    return Ok(match role {
        crate::process_registration::ProcessRole::Host(crate::HostSlot::FIRST)
            if crate::rendezvous::host_lock_held(record.runtime())? =>
        {
            HostPresence::Running(None)
        }
        crate::process_registration::ProcessRole::Host(crate::HostSlot::FIRST) => {
            HostPresence::Gone
        }
        _ => HostPresence::NotStarted,
    });
}

impl RetainedSessionRecord {
    pub(crate) fn checkpoint_directory(&self) -> PathBuf {
        self.directory.folder(&self.scope)
    }
    /// The activation identifier of the retained generation, read again from the record: an error
    /// when the record no longer names that generation.
    pub fn current_activation_id(&self) -> io::Result<[u8; 16]> {
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
    /// A reserved spawn or an unknown observation answers false. POSIX registers no Host, so there
    /// it is gone once no process holds the Host lock; the caller asks only after its Worker
    /// exited, when no Host can start. Observation grants no termination authority.
    pub fn host_absent(&self) -> io::Result<bool> {
        let current = self
            .directory
            .read(&self.scope)?
            .ok_or(io::ErrorKind::NotFound)?;
        if !current.same_active_generation(&self.active) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        #[cfg(windows)]
        return Ok(crate::process_registration::HOST_ROLES
            .into_iter()
            .all(|role| crate::process_observation::role_absent(&current.processes, role)));
        #[cfg(unix)]
        return Ok(!crate::rendezvous::host_lock_held(current.runtime())?);
    }
    /// What each of this generation's Terminal Host slots holds now. A Spawning Host never ran,
    /// since a child runs only once identified. On Windows a running Host carries its registered
    /// process; a process with the registered ID but another creation time means the registered
    /// Host is gone. POSIX registers no Host and runs one, in the first slot: a Host holding the
    /// Host lock runs, and a free lock reads as gone, which also covers a Host that never started.
    /// Observation grants no termination authority.
    ///
    /// Each slot comes back in order, with the epoch its Host recorded once its Worker connected.
    pub fn hosts(&self) -> io::Result<Vec<HostSlotPresence>> {
        let current = self.current()?;
        crate::HostSlot::ALL
            .into_iter()
            .map(|slot| {
                let role = crate::process_registration::ProcessRole::Host(slot);
                Ok(HostSlotPresence {
                    slot,
                    presence: role_presence(&current, role)?,
                    epoch: current.processes.registration(role).epoch(),
                })
            })
            .collect()
    }
    /// The Keeper this generation records now, by its process ID and creation time; `None` when
    /// it records none.
    ///
    /// # Errors
    ///
    /// Returns an error when the record cannot be read or no longer names this generation.
    #[cfg(windows)]
    pub(crate) fn keeper_identity(&self) -> io::Result<Option<ProcessIdentity>> {
        let current = self
            .directory
            .read(&self.scope)?
            .ok_or(io::ErrorKind::NotFound)?;
        if !current.same_active_generation(&self.active) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(current
            .processes
            .registration(crate::process_registration::ProcessRole::Keeper)
            .identity())
    }
    /// What this generation's Keeper is now, in the same terms as [`Self::hosts`]. POSIX
    /// starts no Keeper, so there it reads as never started.
    pub fn keeper_presence(&self) -> io::Result<HostPresence> {
        role_presence(
            &self.current()?,
            crate::process_registration::ProcessRole::Keeper,
        )
    }
    /// The record as it stands now, which must still name this generation.
    fn current(&self) -> io::Result<Record> {
        let current = self
            .directory
            .read(&self.scope)?
            .ok_or(io::ErrorKind::NotFound)?;
        if !current.same_active_generation(&self.active) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(current)
    }
    /// Only after confirmed child exit and listener cleanup, before the runtime stop receipt.
    pub fn complete(&mut self, request_id: [u8; 16]) -> io::Result<()> {
        self.complete_moving(request_id, None)
    }
    /// [`Self::complete`] for a stop the idle move began, which names `moved_to`, the store's
    /// active version the Session moves to.
    pub fn complete_moving(
        &mut self,
        request_id: [u8; 16],
        moved_to: Option<&str>,
    ) -> io::Result<()> {
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
        match moved_to {
            Some(version) => {
                current
                    .extensions
                    .insert(MOVED_TO.to_owned(), version.to_owned().into());
            }
            None => {
                current.extensions.remove(MOVED_TO);
            }
        }
        self.directory.publish(&self.scope, &current)
    }
    /// Records that an `exit` ended the retained generation, after its Worker is gone, so the next
    /// activation recovers it without counting a crash. Waits for the record's writer until
    /// `deadline`; a record that no longer names the generation refuses.
    pub fn record_exit(&self, deadline: Instant) -> io::Result<()> {
        let _writer = self.writer_before(deadline)?;
        let mut current = self
            .directory
            .read(&self.scope)?
            .ok_or(io::ErrorKind::NotFound)?;
        if !current.same_active_generation(&self.active) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let activation_id = current.activation_id.ok_or(io::ErrorKind::InvalidData)?;
        let value = serde_json::to_value(activation_id).map_err(io::Error::other)?;
        current.extensions.insert(EXITED.to_owned(), value);
        self.directory.publish(&self.scope, &current)
    }

    /// The successor entry of a restart the record carries while it still names the retained
    /// generation, if this build can read it.
    pub fn successor(&self) -> io::Result<Option<Successor>> {
        let current = self
            .directory
            .read(&self.scope)?
            .ok_or(io::ErrorKind::NotFound)?;
        if !current.same_active_generation(&self.active) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(Successor::of(&current))
    }

    /// Whether the record names `successor`'s generation committed: its activation and Server.
    /// Read under the record's writer lock, waited for until `deadline`.
    pub fn names_committed(&self, successor: &Successor, deadline: Instant) -> io::Result<bool> {
        let _writer = self.writer_before(deadline)?;
        let current = self
            .directory
            .read(&self.scope)?
            .ok_or(io::ErrorKind::NotFound)?;
        Ok(successor.committed_in(&current))
    }

    /// Decides a restart under the record's writer lock, waited for until `deadline`. Once the
    /// record names `successor`'s generation committed, the successor holds the Session. While it
    /// still names the retained generation, `end` ends what `successor` names and the entry for its
    /// ticket is removed, so the retained generation keeps the Session. Any other record, or an
    /// `end` that fails, leaves the record unchanged and fails.
    pub fn fence(
        &self,
        successor: &Successor,
        deadline: Instant,
        end: impl FnOnce() -> io::Result<()>,
    ) -> io::Result<Fenced> {
        let _writer = self.writer_before(deadline)?;
        let mut current = self
            .directory
            .read(&self.scope)?
            .ok_or(io::ErrorKind::NotFound)?;
        if successor.committed_in(&current) {
            return Ok(Fenced::Committed);
        }
        if !current.same_active_generation(&self.active) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        end()?;
        if Successor::of(&current).is_some_and(|entry| entry.ticket() == successor.ticket()) {
            current.extensions.remove(successor::SUCCESSOR);
            self.directory.publish(&self.scope, &current)?;
        }
        Ok(Fenced::Kept)
    }

    /// Clears an abandoned successor entry under the record's writer lock, waited for until
    /// `deadline`: while the record still names the retained generation and carries an entry that
    /// `abandoned` judges abandoned, `end` ends every live process it names by its recorded
    /// identity, the Worker first ([`crate::ExactProcess::end_successor`]), and only then is the
    /// entry removed. Answers the entry it cleared; an `end` that fails leaves the record unchanged.
    #[cfg(windows)]
    pub fn clear_successor(
        &self,
        deadline: Instant,
        abandoned: impl FnOnce(&Successor) -> bool,
        end: impl FnOnce(&Successor) -> io::Result<()>,
    ) -> io::Result<Option<Successor>> {
        let _writer = self.writer_before(deadline)?;
        let mut current = self
            .directory
            .read(&self.scope)?
            .ok_or(io::ErrorKind::NotFound)?;
        if !current.same_active_generation(&self.active) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let Some(entry) = Successor::of(&current).filter(|entry| abandoned(entry)) else {
            return Ok(None);
        };
        end(&entry)?;
        current.extensions.remove(successor::SUCCESSOR);
        self.directory.publish(&self.scope, &current)?;
        Ok(Some(entry))
    }

    /// The record's writer lock, waited for until `deadline`.
    fn writer_before(&self, deadline: Instant) -> io::Result<RecordWriter> {
        loop {
            match self.directory.try_writer(&self.scope) {
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(10));
                }
                writer => return writer,
            }
        }
    }
}

/// Who holds the Session after [`RetainedSessionRecord::fence`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fenced {
    /// The retained generation: the entry is gone and what it named was ended.
    Kept,
    /// The successor: the record names its generation.
    Committed,
}
