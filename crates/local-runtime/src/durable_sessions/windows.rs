//! Private lifecycle records outlive the runtime leaf. Effects derive their paths from admission.
use crate::{
    ActivationState, ModeCheckedLocalSessionRendezvous as Rendezvous, SessionName,
    private_filesystem as permissions,
    process_registration::{ProcessRole, SessionProcesses},
    session_name,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

const MAXIMUM_BYTES: usize = 8192;

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
    pub(crate) fn new(
        base: &Path,
        name: &SessionName,
        activation_id: Option<[u8; 16]>,
        lifecycle: Lifecycle,
    ) -> io::Result<Self> {
        let scope = name.scope_for_user(&orchbee_local_ipc::current_user_sid()?);
        let runtime = base.join(scope);
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
        base: &Path,
        name: &SessionName,
        activation_id: [u8; 16],
    ) -> io::Result<Self> {
        Self::new(base, name, Some(activation_id), Lifecycle::Active)
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

pub(crate) fn supports_scope(scope: &str, sid: &str) -> bool {
    scope
        == SessionName::parse("default")
            .expect("fixed name")
            .scope_for_user(sid)
        || (scope.len() == 63
            && scope
                .strip_prefix(session_name::NAMED_PREFIX)
                .is_some_and(|tail| {
                    tail.bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                }))
}

pub(crate) enum CatalogRecord {
    Readable(Record),
    /// Its identity names the Session derived for this scope, but the record is not admitted.
    Unreadable(SessionName),
    /// Its identity names a Session derived elsewhere, or cannot be read. Only the scope is known.
    Unrecognized(Option<SessionName>),
}

/// The checkpoint directory a runtime derives below the environment's local state root.
pub(crate) fn checkpoint_path(local: &Path, runtime: &Path) -> PathBuf {
    local
        .join(CHECKPOINT_DIRECTORY)
        .join(Rendezvous::host_endpoint_id(runtime))
}
pub(crate) const CHECKPOINT_DIRECTORY: &str = "native-sessions";
const CATALOG_DIRECTORY: &str = "session-catalog";

/// `local` is the environment's local state root throughout; the layout below it is fixed.
pub(crate) struct Directory {
    path: PathBuf,
    local: PathBuf,
    base: PathBuf,
    sid: String,
    _pins: Vec<File>,
}

impl Directory {
    fn path(local: &Path, base: &Path) -> PathBuf {
        local
            .join(CATALOG_DIRECTORY)
            .join(Rendezvous::host_endpoint_id(base))
    }
    pub(crate) fn open(local: &Path, base: &Path) -> io::Result<Option<Self>> {
        let Some(_local) = crate::environment::open_state_root(local)? else {
            return Ok(None);
        };
        let path = Self::path(local, base);
        let pins = match permissions::open_tree(&path) {
            Ok(pins) => pins,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        Ok(Some(Self {
            path,
            local: local.to_owned(),
            base: base.to_owned(),
            sid: orchbee_local_ipc::current_user_sid()?,
            _pins: pins,
        }))
    }
    pub(crate) fn create(local: &Path, base: &Path) -> io::Result<Self> {
        let mut pins = crate::environment::create_state_root(local)?;
        let mut path = local.to_owned();
        for component in [CATALOG_DIRECTORY, &Rendezvous::host_endpoint_id(base)] {
            path.push(component);
            let created = match fs::create_dir(&path) {
                Ok(()) => true,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
                Err(error) => return Err(error),
            };
            let mut pin = permissions::open_directory(&path, created)?;
            if created {
                permissions::protect_new(&mut pin)?;
            } else {
                permissions::require_private(&pin)?;
            }
            pins.push(pin);
        }
        Ok(Self {
            path,
            local: local.to_owned(),
            base: base.to_owned(),
            sid: orchbee_local_ipc::current_user_sid()?,
            _pins: pins,
        })
    }
    fn validate(&self, scope: &str, record: &Record) -> io::Result<()> {
        let name = SessionName::parse(&record.name).map_err(|_| io::ErrorKind::InvalidData)?;
        if record.version != 1
            || name.scope_for_user(&self.sid) != scope
            || Path::new(&record.runtime) != self.base.join(scope)
            || record.activation_id == Some([0; 16])
            || (!record.is_deleting() && record.activation_id.is_none())
            || record.stopped_id() == Some([0; 16])
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(())
    }
    fn record_path(&self, scope: &str, pending: bool) -> io::Result<PathBuf> {
        if !supports_scope(scope, &self.sid) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        Ok(self.path.join(format!(
            "{scope}.{}",
            if pending { "pending" } else { "json" }
        )))
    }
    fn read_bytes(&self, scope: &str) -> io::Result<Option<Vec<u8>>> {
        let file = match permissions::open_file(&self.record_path(scope, false)?, false, false) {
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
    pub(crate) fn try_writer(&self, scope: &str) -> io::Result<File> {
        if !supports_scope(scope, &self.sid) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let path = self.path.join(format!("{scope}.record.lock"));
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
        if !supports_scope(scope, &self.sid) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let file = match permissions::open_file(
            &self.path.join(format!("{scope}.record.lock")),
            false,
            true,
        ) {
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
    pub(crate) fn list(&self, deadline: Instant) -> io::Result<Vec<(String, CatalogRecord)>> {
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.path)? {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let entry = entry?;
            let leaf = entry.file_name();
            let leaf = leaf.to_str().ok_or(io::ErrorKind::InvalidData)?;
            if let Some(scope) = leaf.strip_suffix(".pending")
                && supports_scope(scope, &self.sid)
            {
                continue;
            }
            if let Some(scope) = leaf.strip_suffix(".record.lock")
                && supports_scope(scope, &self.sid)
            {
                permissions::open_file(&entry.path(), false, false)?;
                continue;
            }
            let scope = leaf
                .strip_suffix(".json")
                .ok_or(io::ErrorKind::InvalidData)?;
            let record = match self.read(scope) {
                Ok(Some(record)) => CatalogRecord::Readable(record),
                Ok(None) => return Err(io::ErrorKind::NotFound.into()),
                Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                    match self.identity(scope)? {
                        Some(name) if name.scope_for_user(&self.sid) == scope => {
                            CatalogRecord::Unreadable(name)
                        }
                        name => CatalogRecord::Unrecognized(name),
                    }
                }
                Err(error) => return Err(error),
            };
            records.push((scope.to_owned(), record));
            if records.len() > 256 {
                return Err(io::ErrorKind::FileTooLarge.into());
            }
        }
        Ok(records)
    }
    // Read only identity metadata for attribution and display; this never admits lifecycle or
    // process facts, and an older derivation's name is never translated into a current scope.
    fn identity(&self, scope: &str) -> io::Result<Option<SessionName>> {
        #[derive(Deserialize)]
        struct Identity {
            name: String,
        }
        let bytes = match self.read_bytes(scope) {
            Ok(bytes) => bytes.ok_or(io::ErrorKind::NotFound)?,
            Err(error) if error.kind() == io::ErrorKind::InvalidData => return Ok(None),
            Err(error) => return Err(error),
        };
        Ok(serde_json::from_slice::<Identity>(&bytes)
            .ok()
            .and_then(|identity| SessionName::parse(&identity.name).ok()))
    }
    pub(crate) fn record_file(&self, scope: &str) -> io::Result<PathBuf> {
        self.record_path(scope, false)
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
        let staging = self.record_path(scope, true)?;
        remove_private(&staging)?;
        let mut file = permissions::open_file(&staging, true, true)?;
        let written = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        let target = self.record_path(scope, false)?;
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
    pub(crate) fn remove(&self, scope: &str) -> io::Result<()> {
        let _current = self.read(scope)?;
        remove_private(&self.record_path(scope, true)?)?;
        remove_private(&self.path.join(format!("{scope}.record.lock")))?;
        remove_private(&self.record_path(scope, false)?)
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
        checkpoint_path(&self.directory.local, Path::new(&self.active.runtime))
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
    pub fn retain(local: &Path, session: &Rendezvous) -> io::Result<Option<Self>> {
        let runtime = session.runtime_directory();
        let scope = runtime
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(io::ErrorKind::InvalidInput)?;
        if !supports_scope(scope, &orchbee_local_ipc::current_user_sid()?) {
            return Ok(None);
        }
        let base = runtime.parent().ok_or(io::ErrorKind::InvalidInput)?;
        let directory = Directory::open(local, base)?.ok_or(io::ErrorKind::NotFound)?;
        let active = directory.read(scope)?.ok_or(io::ErrorKind::NotFound)?;
        if active.lifecycle != Lifecycle::Active {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(Some(Self {
            directory,
            scope: scope.to_owned(),
            active,
        }))
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
