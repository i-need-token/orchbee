//! Cooperating native Windows activators and stopped-session deletion share one namespace owner,
//! `internal/sessions.lock`. Each Session keeps its records in `sessions/<session>/` and its
//! runtime in `<base>/<session>/`, both folders named by the Session.
use crate::{
    ActivationState, DataRoot, LockAttempt, ModeCheckedLocalSessionRendezvous as Rendezvous,
    SessionName,
    durable_sessions::{self as durable, CatalogRecord, Directory, Lifecycle, Record},
    private_filesystem as permissions,
    process_observation::ProcessSnapshot,
    process_registration::RecoveryVerdict,
};
use std::{
    collections::BTreeMap,
    fs::{self, File, TryLockError},
    io,
    path::{Path, PathBuf},
    time::Instant,
};

use super::{
    NamedSessionDeleteFailure, NamedSessionDeleteOutcome, NamedSessionDeleteReason,
    UnrecognizedRecord, UnrecognizedRecordKind as RecordKind,
    UnrecognizedRecordReason as RecordReason,
};

const MAXIMUM_SESSIONS: usize = 256;
const RUNTIME_FILES: &[&str] = &[
    "activation.started",
    "activation.lock",
    "session-lifetime.lock",
];
/// The files a Session folder holds beside its catalog record, its corrupt checkpoint originals
/// and its owned folders.
const SESSION_FILES: &[&str] = &[
    "checkpoint.json",
    "checkpoint.previous-1.json",
    "checkpoint.previous-2.json",
    "checkpoint.pending",
    "state.lock",
    crate::checkpoint_restore_notice::FILE,
    crate::checkpoint_restore_notice::PENDING,
    // The TUI position record, `data_root::TUI_POSITION_RECORD`, with its lock and staging file.
    "tui-position.json",
    TUI_POSITION_LOCK,
    "tui-position.pending",
];
const TUI_POSITION_LOCK: &str = "tui-position.lock";

pub struct SessionCatalog {
    base: PathBuf,
    durable: Directory,
    _lock: File,
    _directories: Vec<File>,
}

#[derive(Debug)]
pub struct NamedSessionEntry {
    pub name: SessionName,
    /// The runtime folder the Session's record names, or the folder this base gives it.
    pub runtime: PathBuf,
    pub deleting: bool,
    /// The Session folder holds a record that cannot be admitted.
    pub unreadable: bool,
    /// Two Session folders spell this name in different letter case; activation and deletion
    /// refuse while both remain.
    pub conflicted: bool,
    retained: Option<ActivationState>,
    checkpoint: PathBuf,
    root: DataRoot,
}

#[derive(Default)]
struct Listed {
    spelling: String,
    deleting: bool,
    retained: Option<ActivationState>,
    runtime: Option<PathBuf>,
    unreadable: bool,
    conflicted: bool,
}

impl SessionCatalog {
    /// Only declared activation/deletion effects acquire this owner. Reads use `list` instead.
    pub fn try_lock(base: &Path, root: &DataRoot) -> io::Result<Self> {
        let mut directories = permissions::open_tree(base)?;
        let base = base.canonicalize()?;
        directories.extend(crate::data_root::create_state_directory(&root.internal())?);
        let path = root.sessions_lock();
        let lock = match permissions::open_file(&path, true, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                permissions::open_file(&path, false, true)?
            }
            Err(error) => return Err(error),
        };
        match lock.try_lock() {
            Ok(()) => Ok(Self {
                durable: Directory::create(root)?,
                base,
                _lock: lock,
                _directories: directories,
            }),
            Err(TryLockError::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }

    /// The Session `name` spelled as its existing folder is, or as given when it has none. Two
    /// folders that spell it in different letter case refuse it. Reading creates nothing.
    pub fn spelling(root: &DataRoot, name: &SessionName) -> io::Result<SessionName> {
        let Some(directory) = Directory::open(root)? else {
            return Ok(name.clone());
        };
        match directory.spelling(name)? {
            Some(spelling) => SessionName::parse(&spelling),
            None => Ok(name.clone()),
        }
    }

    /// Validate recovery before creating a missing runtime. This owner already holds the namespace
    /// gate. A Session whose record names another runtime folder starts here only once stopped.
    pub fn prepare_activation(&self, name: &SessionName) -> io::Result<()> {
        let scope = name.as_str();
        // Two folders that spell this name in different letter case refuse it.
        self.durable.spelling(name)?;
        let record = self.durable.read(scope)?;
        if record.as_ref().is_some_and(Record::is_deleting) {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let runtime = self.base.join(scope);
        if record.as_ref().is_some_and(|record| {
            !record.uses_runtime(&runtime) && !matches!(record.lifecycle, Lifecycle::Stopped { .. })
        }) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        match permissions::open_tree(&runtime) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                no_endpoints(&runtime)?;
                let checkpoint = self.durable.folder(scope);
                let storage = storage_guard(&checkpoint)?;
                match record.as_ref().and_then(Record::activation) {
                    Some(ActivationState::Stopped { .. }) => {
                        stopped_checkpoint_evidence(&checkpoint)?;
                    }
                    Some(ActivationState::Active | ActivationState::Crashed { .. }) => {
                        let _writer = self.durable.try_writer(scope)?;
                        let record = self
                            .durable
                            .read(scope)?
                            .ok_or(io::ErrorKind::InvalidData)?;
                        let processes = &record.processes;
                        let snapshot = ProcessSnapshot::observe(processes);
                        if snapshot.verdict(processes) != RecoveryVerdict::Gone {
                            return Err(io::ErrorKind::InvalidData.into());
                        }
                    }
                    None if !storage.exists => {}
                    _ => return Err(io::ErrorKind::InvalidData.into()),
                }
            }
            Err(error) => return Err(error),
        }
        Ok(())
    }

    /// The admitted runtime's Session folder name. The runtime must sit in this catalog's base.
    fn scope_of<'a>(&self, runtime: &'a Path) -> io::Result<&'a str> {
        if runtime.parent() != Some(self.base.as_path()) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        runtime
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| io::ErrorKind::InvalidInput.into())
    }

    /// Proved-dead recovery uses the same namespace and exact runtime guards as new activation.
    pub fn recover_inactive(
        &self,
        session: &Rendezvous,
        activation: &crate::ModeCheckedSessionLock,
        lifetime: &crate::ModeCheckedSessionLock,
    ) -> io::Result<bool> {
        session.require_lock(activation, crate::LockKind::Activation)?;
        session.require_lock(lifetime, crate::LockKind::SessionLifetime)?;
        let runtime = session.runtime_directory();
        let scope = self.scope_of(runtime)?;
        let _writer = self.durable.try_writer(scope)?;
        let Some(mut record) = self.durable.read(scope)? else {
            return Ok(false);
        };
        if matches!(record.lifecycle, Lifecycle::Stopped { .. }) {
            return Ok(false);
        }
        if !matches!(record.lifecycle, Lifecycle::Active | Lifecycle::Crashed)
            || !record.uses_runtime(runtime)
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let processes = &record.processes;
        let snapshot = ProcessSnapshot::observe(processes);
        if snapshot.verdict(processes) != RecoveryVerdict::Gone {
            return Err(io::ErrorKind::InvalidData.into());
        }
        // begin_activation closes the prior generation before publishing the fresh
        // record. With every slot NotStarted, no CreateProcess can have happened,
        // so its complete predecessor receipt can survive interrupted marker commit.
        let allow_previous_receipt = processes.is_unstarted();
        no_endpoints(runtime)?;
        let _storage = storage_guard(&self.durable.folder(scope))?;
        let retry = record.lifecycle == Lifecycle::Crashed;
        if !retry {
            match session.activation_state()? {
                ActivationState::Absent | ActivationState::Active => {}
                ActivationState::Stopped { .. } | ActivationState::Crashed { .. }
                    if allow_previous_receipt => {}
                _ => return Err(io::ErrorKind::InvalidData.into()),
            }
            record.lifecycle = Lifecycle::Crashed;
            self.durable.publish(scope, &record)?;
        }
        let activation_id = record.activation_id.ok_or(io::ErrorKind::InvalidData)?;
        session.reconcile_crashed(
            activation,
            lifetime,
            activation_id,
            retry,
            allow_previous_receipt,
        )?;
        Ok(true)
    }

    /// One new generation is published before its runtime marker or Server can exist.
    pub fn begin_activation(
        &self,
        session: &Rendezvous,
        activation_id: [u8; 16],
    ) -> io::Result<()> {
        if activation_id == [0; 16] {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let runtime = session.runtime_directory();
        let scope = self.scope_of(runtime)?;
        let _writer = self.durable.try_writer(scope)?;
        let previous = self.durable.read(scope)?;
        if previous.as_ref().is_some_and(|record| {
            !matches!(
                record.lifecycle,
                Lifecycle::Stopped { .. } | Lifecycle::Crashed
            ) || record.activation_id == Some(activation_id)
                || (record.lifecycle == Lifecycle::Crashed && !record.uses_runtime(runtime))
        }) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let marker = session.activation_state()?;
        if marker == ActivationState::Active
            || (previous.is_none() && matches!(marker, ActivationState::Crashed { .. }))
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        no_endpoints(runtime)?;
        let checkpoint = self.durable.folder(scope);
        idle_storage(&checkpoint)?;
        if let Some(previous) = &previous {
            if let ActivationState::Stopped { request_id } = marker
                && previous.stopped_id() != Some(request_id)
            {
                return Err(io::ErrorKind::InvalidData.into());
            }
            if let ActivationState::Crashed { activation_id } = marker
                && (previous.lifecycle != Lifecycle::Crashed
                    || previous.activation_id != Some(activation_id))
            {
                return Err(io::ErrorKind::InvalidData.into());
            }
            if matches!(previous.lifecycle, Lifecycle::Stopped { .. }) {
                stopped_checkpoint_evidence(&checkpoint)?;
            }
        }
        // A Session keeps the spelling it was created with.
        let name = match &previous {
            Some(previous) => SessionName::parse(&previous.name)?,
            None => SessionName::parse(scope)?,
        };
        let active = Record::registered(runtime, &name, activation_id)?;
        self.durable.publish(scope, &active)
    }

    /// The namespace owner and matching activation guard are the Server creation capability.
    pub fn spawn_server(
        &self,
        session: &Rendezvous,
        activation: &crate::ModeCheckedSessionLock,
        executable: &Path,
        arguments: &[&std::ffi::OsStr],
        deadline: Instant,
    ) -> io::Result<crate::RetainedChild> {
        session.require_lock(activation, crate::LockKind::Activation)?;
        let scope = self.scope_of(session.runtime_directory())?;
        crate::registered_child::spawn_registered(
            &self.durable,
            scope,
            crate::process_registration::ProcessRole::Server,
            executable,
            arguments,
            deadline,
        )
    }

    /// Every Session of `root`, `default` first and the rest by name without letter case. A
    /// Session's runtime is the folder its record names; one without a record gets `base`'s.
    pub fn list(
        base: &Path,
        root: &DataRoot,
        deadline: Instant,
    ) -> io::Result<Vec<NamedSessionEntry>> {
        let _directories = permissions::open_tree(base)?;
        let base = base.canonicalize()?;
        let mut names: BTreeMap<String, Listed> = BTreeMap::new();
        if let Some(directory) = Directory::open(root)? {
            for (scope, record) in directory.list(deadline)? {
                let key = scope.to_ascii_lowercase();
                if let Some(listed) = names.get_mut(&key) {
                    listed.conflicted = true;
                    continue;
                }
                let mut listed = Listed {
                    spelling: scope,
                    ..Listed::default()
                };
                match record {
                    CatalogRecord::Readable(record) => {
                        listed.deleting = record.is_deleting();
                        listed.retained = record.activation();
                        listed.runtime = Some(record.runtime().to_owned());
                    }
                    CatalogRecord::Unreadable => listed.unreadable = true,
                    CatalogRecord::Missing => {}
                }
                names.insert(key, listed);
            }
        }
        let default = names.remove("default").unwrap_or_default();
        if names.len() >= MAXIMUM_SESSIONS {
            return Err(io::ErrorKind::FileTooLarge.into());
        }
        let mut result = Vec::with_capacity(names.len() + 1);
        for (_, listed) in std::iter::once((String::new(), default)).chain(names) {
            check_deadline(deadline)?;
            let name = if listed.spelling.is_empty() {
                SessionName::default_session()
            } else {
                SessionName::parse(&listed.spelling)?
            };
            result.push(NamedSessionEntry {
                checkpoint: root.sessions().join(name.as_str()),
                runtime: listed.runtime.unwrap_or_else(|| base.join(name.as_str())),
                root: root.clone(),
                name,
                deleting: listed.deleting,
                retained: listed.retained,
                unreadable: listed.unreadable,
                conflicted: listed.conflicted,
            });
        }
        Ok(result)
    }

    /// Read-only diagnosis of the records that refuse the Session `scope`: a record its admission
    /// rejects, and other folders that spell its name in different letter case.
    pub fn unrecognized_records(
        root: &DataRoot,
        scope: &str,
        deadline: Instant,
    ) -> io::Result<Vec<UnrecognizedRecord>> {
        let name = SessionName::parse(scope)?;
        let Some(directory) = Directory::open(root)? else {
            return Ok(Vec::new());
        };
        let mut records = Vec::new();
        let mut folders = Vec::new();
        for (folder, record) in directory.list(deadline)? {
            if !folder.eq_ignore_ascii_case(name.as_str()) {
                continue;
            }
            if matches!(record, CatalogRecord::Unreadable) {
                records.push(UnrecognizedRecord {
                    kind: RecordKind::CatalogRecord,
                    reason: RecordReason::Unreadable,
                    path: directory.record_file(&folder)?,
                });
            }
            folders.push(folder);
        }
        if folders.len() > 1 {
            for folder in folders {
                records.push(UnrecognizedRecord {
                    kind: RecordKind::CatalogRecord,
                    reason: RecordReason::LetterCase,
                    path: directory.folder(&folder),
                });
            }
        }
        Ok(records)
    }

    pub fn delete(
        &self,
        name: &SessionName,
        deadline: Instant,
        mut interrupted: impl FnMut() -> io::Result<bool>,
    ) -> Result<NamedSessionDeleteOutcome, NamedSessionDeleteFailure> {
        use NamedSessionDeleteReason as Reason;
        let failure = |reason, pending| NamedSessionDeleteFailure { reason, pending };
        if name.is_default() {
            return Err(failure(Reason::DefaultSession, false));
        }
        let mut check = || {
            if interrupted()? {
                Err(io::ErrorKind::Interrupted.into())
            } else {
                check_deadline(deadline)
            }
        };
        let mut pending = false;
        let mut refused = None;
        let result = (|| -> io::Result<NamedSessionDeleteOutcome> {
            let scope = match self.durable.spelling(name) {
                Ok(spelling) => spelling.unwrap_or_else(|| name.as_str().to_owned()),
                Err(error) => {
                    refused = Some(Reason::RecoveryRequired);
                    return Err(error);
                }
            };
            let (durable, unreadable) = match self.durable.read(&scope) {
                Ok(record) => (record, false),
                Err(error) if error.kind() == io::ErrorKind::InvalidData => (None, true),
                Err(error) => return Err(error),
            };
            pending = durable.as_ref().is_some_and(Record::is_deleting);
            check()?;
            let runtime = durable.as_ref().map_or_else(
                || self.base.join(&scope),
                |record| record.runtime().to_owned(),
            );
            let mut session = match Rendezvous::from_runtime_directory(&runtime) {
                Ok(session) => Some(session),
                Err(crate::RendezvousError::System { source, .. })
                    if source.kind() == io::ErrorKind::NotFound =>
                {
                    None
                }
                Err(error) => return Err(io::Error::other(error)),
            };
            let mut activation = None;
            let mut lifetime = None;
            let mut stopped = durable.as_ref().and_then(Record::stopped_id);
            if let Some(session) = &session {
                activation = match session
                    .try_acquire_activation_lock()
                    .map_err(io::Error::other)?
                {
                    LockAttempt::Acquired(lock) => Some(lock),
                    LockAttempt::Contended => {
                        refused = Some(Reason::InUse);
                        return Err(io::ErrorKind::WouldBlock.into());
                    }
                };
                lifetime = match session
                    .try_acquire_session_lifetime_lock()
                    .map_err(io::Error::other)?
                {
                    LockAttempt::Acquired(lock) => Some(lock),
                    LockAttempt::Contended => {
                        refused = Some(Reason::InUse);
                        return Err(io::ErrorKind::WouldBlock.into());
                    }
                };
                match session.activation_state()? {
                    ActivationState::Active | ActivationState::Crashed { .. } => {
                        refused = Some(Reason::RecoveryRequired);
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    ActivationState::Stopped { request_id } => {
                        if durable
                            .as_ref()
                            .and_then(Record::stopped_id)
                            .is_some_and(|retained| retained != request_id)
                        {
                            refused = Some(Reason::RecoveryRequired);
                            return Err(io::ErrorKind::InvalidData.into());
                        }
                        stopped = Some(request_id);
                    }
                    ActivationState::Absent => {}
                }
            }
            if durable.as_ref().is_some_and(|record| {
                matches!(record.lifecycle, Lifecycle::Active | Lifecycle::Crashed)
            }) {
                refused = Some(Reason::RecoveryRequired);
                return Err(io::ErrorKind::InvalidData.into());
            }
            match no_endpoints(&runtime) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    refused = Some(Reason::InUse);
                    return Err(error);
                }
                Err(error) => return Err(error),
            }
            let folder = self.durable.folder(&scope);
            let state = OwnedDirectory::open_optional(&folder, Kind::Session)?;
            let runtime_directory = OwnedDirectory::open_optional(&runtime, Kind::Runtime)?;
            if session.is_none() && runtime_directory.is_some() && !pending {
                return Err(io::ErrorKind::InvalidData.into());
            }
            let holds_data = state.as_ref().is_some_and(|state| state.holds_data);
            if session.is_none()
                && !holds_data
                && runtime_directory.is_none()
                && durable.is_none()
                && !unreadable
            {
                // An interrupted activation or deletion can leave the record's lock alone.
                if let Some(state) = state {
                    drop(state);
                    drop(self.durable.try_writer(&scope)?);
                    self.durable.remove(&scope)?;
                }
                return Ok(NamedSessionDeleteOutcome::Absent);
            }
            if session.is_none()
                && !pending
                && !unreadable
                && !durable
                    .as_ref()
                    .is_some_and(|record| matches!(record.lifecycle, Lifecycle::Stopped { .. }))
            {
                refused = Some(Reason::RecoveryRequired);
                return Err(io::ErrorKind::InvalidData.into());
            }
            let state_locks = match &state {
                Some(state) => (
                    state.try_existing_lock("state.lock")?,
                    state.try_existing_lock(TUI_POSITION_LOCK)?,
                ),
                None => (None, None),
            };
            check()?;
            let writer = self.durable.try_writer(&scope)?;
            if !durable.as_ref().is_some_and(Record::is_deleting) {
                let record = match durable {
                    Some(mut record) => {
                        record.lifecycle = Lifecycle::Deleting {
                            stopped_request_id: stopped,
                        };
                        record
                    }
                    None => Record::new(
                        &runtime,
                        &SessionName::parse(&scope)?,
                        None,
                        Lifecycle::Deleting {
                            stopped_request_id: stopped,
                        },
                    )?,
                };
                self.durable.publish_deleting(&scope, &record)?;
                pending = true;
            }
            check()?;
            drop(state_locks);
            if let Some(state) = state {
                state.remove(&mut check)?;
            }
            drop(lifetime.take());
            drop(activation.take());
            drop(session.take());
            if let Some(runtime_directory) = runtime_directory {
                runtime_directory.remove(&mut check)?;
            }
            check()?;
            drop(writer);
            self.durable.remove(&scope)?;
            Ok(NamedSessionDeleteOutcome::Deleted)
        })();
        result.map_err(|error| {
            failure(
                refused.unwrap_or(if error.kind() == io::ErrorKind::Interrupted {
                    Reason::Interrupted
                } else if error.kind() == io::ErrorKind::TimedOut {
                    Reason::TimedOut
                } else if error.kind() == io::ErrorKind::WouldBlock {
                    Reason::InUse
                } else {
                    Reason::Filesystem
                }),
                pending,
            )
        })
    }
}

impl NamedSessionEntry {
    /// A read-only, guarded observation of cold-recovery eligibility, never a future-start promise.
    pub fn cold_recoverable(&self) -> io::Result<bool> {
        if self.deleting || self.unreadable || self.conflicted {
            return Ok(false);
        }
        let observed = (|| -> io::Result<bool> {
            let base = self.runtime.parent().ok_or(io::ErrorKind::InvalidInput)?;
            let _base = permissions::open_tree(base)?;
            let _runtime = match permissions::open_tree(&self.runtime) {
                Ok(pins) => Some(pins),
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) => return Err(error),
            };
            let _namespace = existing_lock(&self.root.sessions_lock(), true)?;
            let _activation = existing_lock(&self.runtime.join("activation.lock"), false)?;
            let _lifetime = existing_lock(&self.runtime.join("session-lifetime.lock"), false)?;
            let scope = self.name.as_str();
            let Some(directory) = Directory::open(&self.root)? else {
                return Ok(false);
            };
            let _writer = directory
                .existing_writer(scope)?
                .ok_or(io::ErrorKind::NotFound)?;
            let Some(record) = directory.read(scope)? else {
                return Ok(false);
            };
            if !matches!(record.lifecycle, Lifecycle::Active | Lifecycle::Crashed)
                || !record.uses_runtime(&self.runtime)
            {
                return Ok(false);
            }
            let processes = &record.processes;
            let snapshot = ProcessSnapshot::observe(processes);
            if snapshot.verdict(processes) != RecoveryVerdict::Gone {
                return Ok(false);
            }
            no_endpoints(&self.runtime)?;
            let _storage = storage_guard(&self.checkpoint)?;
            // Only an entirely unstarted fresh generation may retain its predecessor receipt.
            if record.lifecycle == Lifecycle::Active {
                let session = match Rendezvous::from_runtime_directory(&self.runtime) {
                    Ok(session) => Some(session),
                    Err(crate::RendezvousError::System { source, .. })
                        if source.kind() == io::ErrorKind::NotFound =>
                    {
                        None
                    }
                    Err(error) => return Err(io::Error::other(error)),
                };
                if let Some(session) = session {
                    match session.activation_state() {
                        Ok(ActivationState::Absent | ActivationState::Active) => {}
                        Ok(ActivationState::Stopped { .. } | ActivationState::Crashed { .. })
                            if processes.is_unstarted() => {}
                        Ok(_) => return Ok(false),
                        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                            return Ok(false);
                        }
                        Err(error) => return Err(error),
                    }
                }
            } else {
                match permissions::open_file(&self.runtime.join("activation.started"), false, false)
                {
                    Ok(mut file) => {
                        if !crate::activation_record::crash_matches(
                            &mut file,
                            record.activation_id.ok_or(io::ErrorKind::InvalidData)?,
                            true,
                            processes.is_unstarted(),
                        )? {
                            return Ok(false);
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
            Ok(true)
        })();
        match observed {
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::NotFound
                ) =>
            {
                Ok(false)
            }
            other => other,
        }
    }
    /// A runtime-only receipt cannot override a conflicting retained activation generation.
    pub fn accepts_runtime_state(&self, marker: ActivationState) -> bool {
        self.retained.is_none_or(|retained| retained == marker)
    }

    /// Missing runtime alone is not stopped evidence. This read creates no lock or marker.
    pub fn absent_runtime_state(&self) -> io::Result<Option<ActivationState>> {
        let endpoints = no_endpoints(&self.runtime);
        if let Err(error) = endpoints {
            if error.kind() == io::ErrorKind::WouldBlock {
                return Ok(Some(ActivationState::Active));
            }
            return Err(error);
        }
        let storage = match idle_storage(&self.checkpoint) {
            Ok(storage) => storage,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Ok(Some(ActivationState::Active));
            }
            Err(error) => return Err(error),
        };
        match self.retained {
            Some(stopped @ ActivationState::Stopped { .. }) if storage => {
                match stopped_checkpoint_evidence(&self.checkpoint) {
                    Ok(_) => Ok(Some(stopped)),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        Ok(Some(ActivationState::Active))
                    }
                    Err(error) => Err(error),
                }
            }
            None if !storage => Ok(None),
            _ => Ok(Some(ActivationState::Active)),
        }
    }
}

fn existing_lock(path: &Path, required: bool) -> io::Result<Option<File>> {
    let file = match permissions::open_file(path, false, true) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound && !required => return Ok(None),
        Err(error) => return Err(error),
    };
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

fn stopped_checkpoint_evidence(path: &Path) -> io::Result<()> {
    let leaves = crate::checkpoint_rotation::SLOTS;
    for leaf in leaves {
        match permissions::open_file(&path.join(leaf), false, false) {
            Ok(file) => {
                drop(file);
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let leaf = entry.file_name();
        let name = leaf.to_str().ok_or(io::ErrorKind::InvalidData)?;
        if crate::checkpoint_rotation::is_corrupt_original(name) {
            let file = permissions::open_file(&entry.path(), false, false)?;
            drop(file);
            return Ok(());
        }
    }
    Err(io::ErrorKind::NotFound.into())
}

fn no_endpoints(runtime: &Path) -> io::Result<()> {
    for endpoint in Rendezvous::socket_budget_paths(runtime)
        .into_iter()
        .chain([Rendezvous::observation_endpoint_path(runtime)])
    {
        if orchbee_local_ipc::endpoint_exists(&endpoint)? {
            return Err(io::ErrorKind::WouldBlock.into());
        }
    }
    Ok(())
}
struct IdleStorage {
    /// The Session folder holds checkpoint storage: its lock, a slot or a corrupt original.
    exists: bool,
    _pins: Vec<File>,
    _lock: Option<File>,
}
fn storage_guard(path: &Path) -> io::Result<IdleStorage> {
    let pins = match permissions::open_tree(path) {
        Ok(pins) => pins,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(IdleStorage {
                exists: false,
                _pins: Vec::new(),
                _lock: None,
            });
        }
        Err(error) => return Err(error),
    };
    let file = match permissions::open_file(&path.join("state.lock"), false, true) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let exists = match stopped_checkpoint_evidence(path) {
                Ok(()) => true,
                Err(error) if error.kind() == io::ErrorKind::NotFound => false,
                Err(error) => return Err(error),
            };
            return Ok(IdleStorage {
                exists,
                _pins: pins,
                _lock: None,
            });
        }
        Err(error) => return Err(error),
    };
    match file.try_lock() {
        Ok(()) => Ok(IdleStorage {
            exists: true,
            _pins: pins,
            _lock: Some(file),
        }),
        Err(TryLockError::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
        Err(TryLockError::Error(error)) => Err(error),
    }
}
fn idle_storage(path: &Path) -> io::Result<bool> {
    Ok(storage_guard(path)?.exists)
}

fn check_deadline(deadline: Instant) -> io::Result<()> {
    if Instant::now() >= deadline {
        Err(io::ErrorKind::TimedOut.into())
    } else {
        Ok(())
    }
}

/// Whether a leaf name belongs in one owned folder.
type OwnsName = fn(&str) -> bool;
/// Folders a Session folder holds beside its files, each with the names it may hold.
const OWNED_SESSION_FOLDERS: [(&str, OwnsName); 2] = [
    (
        crate::layout_history::DIRECTORY,
        crate::layout_history::is_owned_name,
    ),
    (
        crate::terminal_output::DIRECTORY,
        crate::terminal_output::is_owned_name,
    ),
];

fn owned_folder(name: &str) -> Option<OwnsName> {
    OWNED_SESSION_FOLDERS
        .iter()
        .find(|(folder, _)| *folder == name)
        .map(|(_, owns)| *owns)
}

/// What a deletion removes: a Session folder's contents other than its catalog record, which the
/// catalog removes last, or a runtime folder with everything in it.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Kind {
    Session,
    Runtime,
}

struct OwnedDirectory {
    path: PathBuf,
    kind: Kind,
    /// The folder holds more than its catalog record's lock and staging file.
    holds_data: bool,
    pins: Vec<File>,
}
impl OwnedDirectory {
    /// Admits every entry before anything is removed; an entry the folder does not own refuses.
    fn open_optional(path: &Path, kind: Kind) -> io::Result<Option<Self>> {
        let pins = match permissions::open_tree(path) {
            Ok(pins) => pins,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut holds_data = false;
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let leaf = entry.file_name();
            let name = leaf.to_str().ok_or(io::ErrorKind::InvalidData)?;
            holds_data |= !durable::is_record_guard(name);
            match kind {
                Kind::Session
                    if SESSION_FILES.contains(&name)
                        || durable::RECORD_FILES.contains(&name)
                        || crate::checkpoint_rotation::is_corrupt_original(name) =>
                {
                    permissions::open_file(&entry.path(), false, false)?;
                }
                Kind::Session if owned_folder(name).is_some() => {
                    let owns = owned_folder(name).ok_or(io::ErrorKind::InvalidData)?;
                    let _pins = permissions::open_tree(&entry.path())?;
                    for owned in fs::read_dir(entry.path())? {
                        let owned = owned?;
                        let leaf = owned.file_name();
                        if !leaf.to_str().is_some_and(owns) {
                            return Err(io::ErrorKind::InvalidData.into());
                        }
                        permissions::open_file(&owned.path(), false, false)?;
                    }
                }
                Kind::Runtime if RUNTIME_FILES.contains(&name) => {
                    permissions::open_file(&entry.path(), false, false)?;
                }
                Kind::Runtime if name == "host" => {
                    let _pins = permissions::open_tree(&entry.path())?;
                    if fs::read_dir(entry.path())?.next().is_some() {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                }
                _ => return Err(io::ErrorKind::InvalidData.into()),
            }
        }
        Ok(Some(Self {
            path: path.to_owned(),
            kind,
            holds_data,
            pins,
        }))
    }

    fn try_existing_lock(&self, name: &str) -> io::Result<Option<File>> {
        let file = match permissions::open_file(&self.path.join(name), false, true) {
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

    fn remove(mut self, check: &mut dyn FnMut() -> io::Result<()>) -> io::Result<()> {
        let files = match self.kind {
            Kind::Session => SESSION_FILES,
            Kind::Runtime => RUNTIME_FILES,
        };
        let mut names: Vec<String> = files.iter().map(|name| (*name).to_owned()).collect();
        if self.kind == Kind::Session {
            for entry in fs::read_dir(&self.path)? {
                let entry = entry?;
                let leaf = entry.file_name();
                let name = leaf.to_str().ok_or(io::ErrorKind::InvalidData)?;
                if crate::checkpoint_rotation::is_corrupt_original(name) {
                    permissions::open_file(&entry.path(), false, false)?;
                    names.push(name.to_owned());
                }
            }
        }
        for name in &names {
            check()?;
            let path = self.path.join(name);
            match permissions::open_file(&path, false, false) {
                Ok(file) => {
                    drop(file);
                    fs::remove_file(path)?;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        if self.kind == Kind::Session {
            for (folder, owns) in OWNED_SESSION_FOLDERS {
                let path = self.path.join(folder);
                match permissions::open_tree(&path) {
                    Ok(pins) => {
                        for owned in fs::read_dir(&path)? {
                            let owned = owned?;
                            let leaf = owned.file_name();
                            let name = leaf.to_str().ok_or(io::ErrorKind::InvalidData)?;
                            if !owns(name) {
                                return Err(io::ErrorKind::InvalidData.into());
                            }
                            check()?;
                            permissions::open_file(&owned.path(), false, false)?;
                            fs::remove_file(owned.path())?;
                        }
                        drop(pins);
                        fs::remove_dir(path)?;
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
            // The catalog removes its record and then the folder.
            return Ok(());
        }
        let path = self.path.join("host");
        match permissions::open_tree(&path) {
            Ok(pins) => {
                drop(pins);
                fs::remove_dir(path)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        check()?;
        self.pins.pop();
        fs::remove_dir(&self.path)
    }
}
