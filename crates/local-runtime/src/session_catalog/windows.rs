//! Cooperating native Windows activators and stopped-session deletion share one namespace owner.
use crate::{
    ActivationState, LockAttempt, ModeCheckedLocalSessionRendezvous as Rendezvous, SessionName,
    SessionScope,
    durable_sessions::{self as durable, CatalogRecord, Directory, Lifecycle, Record},
    private_filesystem as permissions,
    process_observation::ProcessSnapshot,
    process_registration::RecoveryVerdict,
    session_name,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

const MAXIMUM_SESSIONS: usize = 256;
const INTENT_SUFFIX: &str = ".deleting";
const RUNTIME_FILES: &[&str] = &[
    "session-name.json",
    "activation.started",
    "activation.lock",
    "session-lifetime.lock",
];
const CHECKPOINT_FILES: &[&str] = &[
    "checkpoint.json",
    "checkpoint.previous-1.json",
    "checkpoint.previous-2.json",
    "checkpoint.pending",
    "state.lock",
    crate::checkpoint_restore_notice::FILE,
    crate::checkpoint_restore_notice::PENDING,
];

pub struct SessionCatalog {
    base: PathBuf,
    local: PathBuf,
    durable: Directory,
    _lock: File,
    _directories: Vec<File>,
}

#[derive(Debug)]
pub struct NamedSessionEntry {
    pub name: SessionName,
    pub runtime: PathBuf,
    pub deleting: bool,
    /// The catalog identity is known, but its retained record cannot be admitted.
    pub unreadable: bool,
    /// A record stored outside the current location names this Session; activation and
    /// deletion refuse while it remains.
    pub conflicted: bool,
    retained: Option<ActivationState>,
    checkpoint: PathBuf,
    local: PathBuf,
}

use super::{
    NamedSessionDeleteFailure, NamedSessionDeleteOutcome, NamedSessionDeleteReason,
    UnrecognizedRecord, UnrecognizedRecordKind as RecordKind,
    UnrecognizedRecordReason as RecordReason,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    version: u8,
    name: String,
    stopped_request_id: Option<[u8; 16]>,
}

fn scope_matches(scope: &str) -> bool {
    scope.len() == 63
        && scope
            .strip_prefix(session_name::NAMED_PREFIX)
            .is_some_and(|hash| {
                hash.bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
}

fn intent_path(base: &Path, scope: &str) -> PathBuf {
    base.join(format!("{scope}{INTENT_SUFFIX}"))
}

fn read_intent(base: &Path, scope: &str) -> io::Result<Option<Intent>> {
    let file = match permissions::open_file(&intent_path(base, scope), false, false) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(1025).read_to_end(&mut bytes)?;
    let intent: Intent = serde_json::from_slice(&bytes).map_err(|_| io::ErrorKind::InvalidData)?;
    let name = SessionName::parse(&intent.name).map_err(|_| io::ErrorKind::InvalidData)?;
    if bytes.len() > 1024
        || intent.version != 1
        || name.as_str() == "default"
        || intent.stopped_request_id == Some([0; 16])
        || name.scope_for_user(&orchbee_local_ipc::current_user_sid()?) != scope
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(Some(intent))
}

pub(crate) fn require_not_deleting(base: &Path, scope: &str) -> io::Result<()> {
    if scope_matches(scope) && read_intent(base, scope)?.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "named session deletion is pending",
        ));
    }
    Ok(())
}

/// Identity metadata of a deletion intent, without admitting its version or location.
fn intent_name(base: &Path, scope: &str) -> io::Result<Option<SessionName>> {
    #[derive(Deserialize)]
    struct Identity {
        name: String,
    }
    let file = match permissions::open_file(&intent_path(base, scope), false, false) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(1025).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let identity: Identity =
        serde_json::from_slice(&bytes).map_err(|_| io::ErrorKind::InvalidData)?;
    SessionName::parse(&identity.name)
        .map(Some)
        .map_err(|_| io::ErrorKind::InvalidData.into())
}

/// Current admission first. Only a readable name that derives another location becomes an
/// obsolete claim; every other failure keeps its existing meaning.
fn intent_identity(base: &Path, scope: &str, sid: &str) -> io::Result<Option<(SessionName, bool)>> {
    match read_intent(base, scope) {
        Ok(Some(intent)) => Ok(Some((SessionName::parse(&intent.name)?, true))),
        Ok(None) => Ok(None),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            match intent_name(base, scope) {
                Ok(Some(name)) if name.scope_for_user(sid) != scope => Ok(Some((name, false))),
                _ => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

fn runtime_identity(runtime: &Path, location: &str, sid: &str) -> io::Result<(SessionName, bool)> {
    match session_name::read(runtime) {
        Ok(name) => Ok((name, true)),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            match session_name::claimed(runtime) {
                Ok(name) if name.scope_for_user(sid) != location => Ok((name, false)),
                _ => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

/// One retained namespace object, attributed only by the identity metadata it records itself.
struct Claim {
    kind: RecordKind,
    location: String,
    path: PathBuf,
    identity: Option<SessionName>,
}

impl Claim {
    /// A readable identity that derives `scope` while stored elsewhere still names that Session.
    fn names_elsewhere(&self, scope: &str, sid: &str) -> bool {
        self.location != scope
            && self
                .identity
                .as_ref()
                .is_some_and(|name| name.scope_for_user(sid) == scope)
    }
}

/// Inventory catalog records and named runtime objects without admitting any of them. An object
/// whose identity cannot be read keeps only its location; the bounds and deadline fail closed.
fn claims(base: &Path, durable: Option<&Directory>, deadline: Instant) -> io::Result<Vec<Claim>> {
    let mut claims = Vec::new();
    if let Some(durable) = durable {
        for (scope, record) in durable.list(deadline)? {
            let identity = match record {
                CatalogRecord::Readable(record) => SessionName::parse(&record.name).ok(),
                CatalogRecord::Unreadable(name) => Some(name),
                CatalogRecord::Unrecognized(name) => name,
            };
            claims.push(Claim {
                kind: RecordKind::CatalogRecord,
                path: durable.record_file(&scope)?,
                location: scope,
                identity,
            });
        }
    }
    let mut scopes = BTreeSet::new();
    for entry in fs::read_dir(base)? {
        check_deadline(deadline)?;
        let entry = entry?;
        let Some(leaf) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let (scope, kind) = match leaf.strip_suffix(INTENT_SUFFIX) {
            Some(scope) => (scope, RecordKind::DeletionIntent),
            None => (leaf.as_str(), RecordKind::RuntimeDirectory),
        };
        if !scope_matches(scope) {
            continue;
        }
        scopes.insert(scope.to_owned());
        if scopes.len() > MAXIMUM_SESSIONS {
            return Err(io::ErrorKind::FileTooLarge.into());
        }
        let identity = if kind == RecordKind::DeletionIntent {
            intent_name(base, scope).ok().flatten()
        } else {
            permissions::open_tree(&entry.path())
                .and_then(|_pins| session_name::claimed(&entry.path()))
                .ok()
        };
        claims.push(Claim {
            kind,
            location: scope.to_owned(),
            path: entry.path(),
            identity,
        });
    }
    Ok(claims)
}

#[derive(Default)]
struct Listed {
    deleting: bool,
    retained: Option<ActivationState>,
    unreadable: bool,
    conflicted: bool,
}

impl SessionCatalog {
    /// Only declared activation/deletion effects acquire this owner. Reads use `list` instead.
    pub fn try_lock(base: &Path, local: &Path) -> io::Result<Self> {
        let directories = permissions::open_tree(base)?;
        let base = base.canonicalize()?;
        let user =
            SessionName::parse("default")?.scope_for_user(&orchbee_local_ipc::current_user_sid()?);
        let path = base.join(format!("{user}.sessions.lock"));
        let lock = match permissions::open_file(&path, true, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                permissions::open_file(&path, false, true)?
            }
            Err(error) => return Err(error),
        };
        match lock.try_lock() {
            Ok(()) => {
                let durable = Directory::create(local, &base)?;
                Ok(Self {
                    durable,
                    local: local.canonicalize()?,
                    base,
                    _lock: lock,
                    _directories: directories,
                })
            }
            Err(TryLockError::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }

    pub fn supports_scope(scope: &str) -> io::Result<bool> {
        Ok(durable::supports_scope(
            scope,
            &orchbee_local_ipc::current_user_sid()?,
        ))
    }

    /// Validate recovery before creating a missing runtime. This owner already holds the namespace gate.
    pub fn prepare_activation(
        &self,
        scope: &str,
        explicit: Option<&SessionName>,
        deadline: Instant,
    ) -> io::Result<Option<SessionName>> {
        if !Self::supports_scope(scope)? {
            return Ok(None);
        }
        self.require_unclaimed(scope, deadline)?;
        let record = self.durable.read(scope)?;
        if record.as_ref().is_some_and(Record::is_deleting) {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let runtime = self.base.join(scope);
        let name = match explicit {
            Some(name) => name.clone(),
            None => match &record {
                Some(record) => SessionName::parse(&record.name)?,
                None if scope.starts_with(session_name::NAMED_PREFIX) => {
                    session_name::read(&runtime)?
                }
                None => SessionName::parse("default")?,
            },
        };
        session_name::validate_scope(&runtime, &name)?;
        match permissions::open_tree(&runtime) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                no_endpoints(&runtime)?;
                let checkpoint = checkpoint_path(&self.local, &runtime);
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
        Ok(Some(name))
    }

    fn require_unclaimed(&self, scope: &str, deadline: Instant) -> io::Result<()> {
        // A changed derivation must not turn this Session's retained records into apparent
        // absence. Only a record whose own identity derives this scope while stored elsewhere
        // refuses it; other Sessions' records and unattributable objects stay with their own
        // location's admission. Nothing is migrated or replaced.
        let sid = orchbee_local_ipc::current_user_sid()?;
        if claims(&self.base, Some(&self.durable), deadline)?
            .iter()
            .any(|claim| claim.names_elsewhere(scope, &sid))
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(())
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
        if runtime.parent() != Some(self.base.as_path()) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let scope = runtime
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(io::ErrorKind::InvalidInput)?;
        let _writer = self.durable.try_writer(scope)?;
        let Some(mut record) = self.durable.read(scope)? else {
            return Ok(false);
        };
        if matches!(record.lifecycle, Lifecycle::Stopped { .. }) {
            return Ok(false);
        }
        if !matches!(record.lifecycle, Lifecycle::Active | Lifecycle::Crashed) {
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
        let _storage = storage_guard(&checkpoint_path(&self.local, runtime))?;
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
        if runtime.parent() != Some(self.base.as_path()) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let scope = runtime
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(io::ErrorKind::InvalidInput)?;
        let _writer = self.durable.try_writer(scope)?;
        let previous = self.durable.read(scope)?;
        if previous.as_ref().is_some_and(|record| {
            !matches!(
                record.lifecycle,
                Lifecycle::Stopped { .. } | Lifecycle::Crashed
            ) || record.activation_id == Some(activation_id)
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
        let checkpoint = checkpoint_path(&self.local, runtime);
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
        let name = if scope.starts_with(session_name::NAMED_PREFIX) {
            session_name::read(runtime)?
        } else {
            SessionName::parse("default")?
        };
        let active = Record::registered(&self.base, &name, activation_id)?;
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
        let runtime = session.runtime_directory();
        if runtime.parent() != Some(self.base.as_path()) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let scope = runtime
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(io::ErrorKind::InvalidInput)?;
        crate::registered_child::spawn_registered(
            &self.durable,
            scope,
            crate::process_registration::ProcessRole::Server,
            executable,
            arguments,
            deadline,
        )
    }

    pub fn list(
        base: &Path,
        local: &Path,
        deadline: Instant,
    ) -> io::Result<Vec<NamedSessionEntry>> {
        let _directories = permissions::open_tree(base)?;
        let base = base.canonicalize()?;
        let sid = orchbee_local_ipc::current_user_sid()?;
        let mut names: BTreeMap<String, Listed> = BTreeMap::new();
        // A location that holds both a current identity and a record naming another Session
        // cannot be attributed to either one, so that inventory still fails explicitly.
        let mut current = BTreeSet::new();
        let mut obsolete = BTreeSet::new();
        if let Some(directory) = Directory::open(local, &base)? {
            for (scope, record) in directory.list(deadline)? {
                match record {
                    CatalogRecord::Readable(record) => {
                        let listed = names.entry(record.name.clone()).or_default();
                        listed.deleting = record.is_deleting();
                        listed.retained = record.activation();
                        current.insert(scope);
                    }
                    CatalogRecord::Unreadable(name) => {
                        names
                            .entry(name.as_str().to_owned())
                            .or_default()
                            .unreadable = true;
                        current.insert(scope);
                    }
                    // Another derivation's record still names its Session; nothing else is read.
                    CatalogRecord::Unrecognized(Some(name)) => {
                        names
                            .entry(name.as_str().to_owned())
                            .or_default()
                            .conflicted = true;
                        obsolete.insert(scope);
                    }
                    CatalogRecord::Unrecognized(None) => {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                }
            }
        }
        for entry in fs::read_dir(&base)? {
            check_deadline(deadline)?;
            let entry = entry?;
            let Some(leaf) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let (location, name, deleting, admitted) = if scope_matches(&leaf) {
                let _pins = permissions::open_tree(&entry.path())?;
                let (name, deleting, admitted) = match intent_identity(&base, &leaf, &sid)? {
                    Some((name, true)) => (name, true, true),
                    Some((name, false)) => {
                        // A current identity in this leaf contradicts an intent naming another
                        // Session, so the location is attributed to both and fails below.
                        if session_name::read(&entry.path()).is_ok() {
                            current.insert(leaf.clone());
                        }
                        (name, false, false)
                    }
                    None => {
                        let pending = names.iter().find(|(name, listed)| {
                            listed.deleting
                                && SessionName::parse(name)
                                    .is_ok_and(|name| name.scope_for_user(&sid) == leaf)
                        });
                        match pending {
                            Some((name, _)) => (SessionName::parse(name)?, true, true),
                            None => {
                                let (name, admitted) =
                                    runtime_identity(&entry.path(), &leaf, &sid)?;
                                (name, false, admitted)
                            }
                        }
                    }
                };
                (leaf.clone(), name, deleting, admitted)
            } else if let Some(scope) = leaf
                .strip_suffix(INTENT_SUFFIX)
                .filter(|scope| scope_matches(scope))
            {
                let (name, admitted) =
                    intent_identity(&base, scope, &sid)?.ok_or(io::ErrorKind::NotFound)?;
                (scope.to_owned(), name, admitted, admitted)
            } else {
                continue;
            };
            let listed = names.entry(name.as_str().to_owned()).or_default();
            listed.deleting |= deleting;
            listed.conflicted |= !admitted;
            if admitted {
                current.insert(location);
            } else {
                obsolete.insert(location);
            }
            if names.len() + usize::from(!names.contains_key("default")) > MAXIMUM_SESSIONS {
                return Err(io::ErrorKind::FileTooLarge.into());
            }
        }
        if !obsolete.is_disjoint(&current) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let default = names.remove("default").unwrap_or_default();
        if names.len() >= MAXIMUM_SESSIONS {
            return Err(io::ErrorKind::FileTooLarge.into());
        }
        let mut result = Vec::with_capacity(names.len() + 1);
        for (text, listed) in std::iter::once(("default".to_owned(), default)).chain(names) {
            let name = SessionName::parse(&text)?;
            let runtime = base.join(name.scope_for_user(&sid));
            result.push(NamedSessionEntry {
                local: local.to_owned(),
                checkpoint: checkpoint_path(local, &runtime),
                runtime,
                name,
                deleting: listed.deleting,
                retained: listed.retained,
                unreadable: listed.unreadable,
                conflicted: listed.conflicted,
            });
        }
        Ok(result)
    }

    /// Read-only diagnosis of the records that refuse `scope`: records naming its Session from
    /// another location, and records at its own location that its admission cannot accept.
    /// Paths keep the caller's spelling of `base` and `local` so people can act on them.
    pub fn unrecognized_records(
        base: &Path,
        local: &Path,
        scope: &str,
        deadline: Instant,
    ) -> io::Result<Vec<UnrecognizedRecord>> {
        let sid = orchbee_local_ipc::current_user_sid()?;
        if !durable::supports_scope(scope, &sid) {
            return Ok(Vec::new());
        }
        let _directories = permissions::open_tree(base)?;
        let directory = Directory::open(local, &base.canonicalize()?)?;
        let mut records: Vec<_> = claims(base, directory.as_ref(), deadline)?
            .into_iter()
            .filter(|claim| claim.names_elsewhere(scope, &sid))
            .map(|claim| UnrecognizedRecord {
                kind: claim.kind,
                reason: RecordReason::ObsoleteIdentity,
                path: claim.path,
            })
            .collect();
        let unreadable = |kind, path| UnrecognizedRecord {
            kind,
            reason: RecordReason::Unreadable,
            path,
        };
        if let Some(directory) = &directory
            && directory.read(scope).is_err()
        {
            records.push(unreadable(
                RecordKind::CatalogRecord,
                directory.record_file(scope)?,
            ));
        }
        if scope_matches(scope) {
            if read_intent(base, scope).is_err() {
                records.push(unreadable(
                    RecordKind::DeletionIntent,
                    intent_path(base, scope),
                ));
            }
            // A missing identity is registered by the next activation; an unsafe runtime is a
            // runtime-directory fact rather than a record.
            let runtime = base.join(scope);
            if let Ok(_pins) = permissions::open_tree(&runtime)
                && session_name::read(&runtime)
                    .is_err_and(|error| error.kind() != io::ErrorKind::NotFound)
            {
                records.push(unreadable(RecordKind::RuntimeDirectory, runtime));
            }
        }
        Ok(records)
    }

    pub fn delete(
        &self,
        name: &SessionName,
        local_app_data: &Path,
        deadline: Instant,
        mut interrupted: impl FnMut() -> io::Result<bool>,
    ) -> Result<NamedSessionDeleteOutcome, NamedSessionDeleteFailure> {
        use NamedSessionDeleteReason as Reason;
        let failure = |reason, pending| NamedSessionDeleteFailure { reason, pending };
        if name.as_str() == "default" {
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
            let sid = orchbee_local_ipc::current_user_sid()?;
            let scope = name.scope_for_user(&sid);
            let runtime = self.base.join(&scope);
            let previous = read_intent(&self.base, &scope)?;
            let (durable, unreadable) = match self.durable.read(&scope) {
                Ok(record) => (record, false),
                Err(error) if error.kind() == io::ErrorKind::InvalidData => (None, true),
                Err(error) => return Err(error),
            };
            pending = previous.is_some() || durable.as_ref().is_some_and(Record::is_deleting);
            // A record naming this Session from elsewhere is neither removable here nor absence.
            if !pending
                && claims(&self.base, Some(&self.durable), deadline)?
                    .iter()
                    .any(|claim| claim.names_elsewhere(&scope, &sid))
            {
                refused = Some(Reason::RecoveryRequired);
                return Err(io::ErrorKind::InvalidData.into());
            }
            check()?;
            let _app_data = permissions::open_tree(local_app_data)?;
            if local_app_data.canonicalize()? != self.local {
                return Err(io::ErrorKind::InvalidInput.into());
            }
            let mut session = match Rendezvous::for_named_deletion(
                &self.base,
                SessionScope::parse(&scope).map_err(io::Error::other)?,
                pending,
            ) {
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
            let mut stopped = previous
                .as_ref()
                .and_then(|intent| intent.stopped_request_id)
                .or_else(|| durable.as_ref().and_then(Record::stopped_id));
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
                    || previous
                        .as_ref()
                        .is_some_and(|intent| intent.stopped_request_id != record.stopped_id())
            }) {
                refused = Some(Reason::RecoveryRequired);
                return Err(io::ErrorKind::InvalidData.into());
            }
            for endpoint in Rendezvous::socket_budget_paths(&runtime)
                .into_iter()
                .chain([Rendezvous::observation_endpoint_path(&runtime)])
            {
                if orchbee_local_ipc::endpoint_exists(&endpoint)? {
                    refused = Some(Reason::InUse);
                    return Err(io::ErrorKind::WouldBlock.into());
                }
            }
            let state_path = local_app_data
                .join("Orchbee")
                .join("native-sessions")
                .join(Rendezvous::host_endpoint_id(&runtime));
            let state = OwnedDirectory::open_optional(&state_path, CHECKPOINT_FILES, false)?;
            let runtime_directory = OwnedDirectory::open_optional(&runtime, RUNTIME_FILES, true)?;
            if session.is_none() && runtime_directory.is_some() && !pending {
                return Err(io::ErrorKind::InvalidData.into());
            }
            if session.is_none()
                && state.is_none()
                && runtime_directory.is_none()
                && durable.is_none()
                && !unreadable
                && !pending
            {
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
            let state_lock = match &state {
                Some(state) => state.try_existing_lock("state.lock")?,
                None => None,
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
                        &self.base,
                        name,
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
            if previous.is_none() {
                let intent = Intent {
                    version: 1,
                    name: name.as_str().to_owned(),
                    stopped_request_id: stopped,
                };
                let bytes = serde_json::to_vec(&intent).map_err(io::Error::other)?;
                let staging = self.base.join(format!("{scope}.deleting.pending"));
                remove_private_file_if_present(&staging)?;
                let mut file = permissions::open_file(&staging, true, true)?;
                let written = file.write_all(&bytes).and_then(|()| file.sync_all());
                drop(file);
                if let Err(error) = written {
                    remove_private_file_if_present(&staging)?;
                    return Err(error);
                }
                let target = intent_path(&self.base, &scope);
                if let Err(error) = winsafe::MoveFileEx(
                    staging.to_str().ok_or(io::ErrorKind::InvalidInput)?,
                    Some(target.to_str().ok_or(io::ErrorKind::InvalidInput)?),
                    winsafe::co::MOVEFILE::REPLACE_EXISTING,
                ) {
                    remove_private_file_if_present(&staging)?;
                    return Err(io::Error::other(error));
                }
                pending = true;
            }
            drop(state_lock);
            if let Some(state) = state {
                state.remove(CHECKPOINT_FILES, false, &mut check)?;
            }
            drop(lifetime.take());
            drop(activation.take());
            drop(session.take());
            if let Some(runtime_directory) = runtime_directory {
                runtime_directory.remove(RUNTIME_FILES, true, &mut check)?;
            }
            check()?;
            remove_private_file_if_present(&intent_path(&self.base, &scope))?;
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
            let sid = orchbee_local_ipc::current_user_sid()?;
            let user = SessionName::parse("default")?.scope_for_user(&sid);
            let _namespace = existing_lock(&base.join(format!("{user}.sessions.lock")), true)?;
            let _activation = existing_lock(&self.runtime.join("activation.lock"), false)?;
            let _lifetime = existing_lock(&self.runtime.join("session-lifetime.lock"), false)?;
            let scope = self
                .runtime
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(io::ErrorKind::InvalidInput)?;
            let Some(directory) = Directory::open(&self.local, base)? else {
                return Ok(false);
            };
            let _writer = directory
                .existing_writer(scope)?
                .ok_or(io::ErrorKind::NotFound)?;
            let Some(record) = directory.read(scope)? else {
                return Ok(false);
            };
            if !matches!(record.lifecycle, Lifecycle::Active | Lifecycle::Crashed) {
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

fn checkpoint_path(local: &Path, runtime: &Path) -> PathBuf {
    local
        .join("Orchbee")
        .join("native-sessions")
        .join(Rendezvous::host_endpoint_id(runtime))
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
            return Ok(IdleStorage {
                exists: true,
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

fn remove_private_file_if_present(path: &Path) -> io::Result<()> {
    match permissions::open_file(path, false, false) {
        Ok(file) => {
            drop(file);
            fs::remove_file(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

struct OwnedDirectory {
    path: PathBuf,
    pins: Vec<File>,
}
impl OwnedDirectory {
    fn open_optional(path: &Path, files: &[&str], host: bool) -> io::Result<Option<Self>> {
        let pins = match permissions::open_tree(path) {
            Ok(pins) => pins,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let leaf = entry.file_name();
            let name = leaf.to_str().ok_or(io::ErrorKind::InvalidData)?;
            if files.contains(&name)
                || (files == CHECKPOINT_FILES
                    && crate::checkpoint_rotation::is_corrupt_original(name))
            {
                permissions::open_file(&entry.path(), false, false)?;
            } else if host && name == "host" {
                let _pins = permissions::open_tree(&entry.path())?;
                if fs::read_dir(entry.path())?.next().is_some() {
                    return Err(io::ErrorKind::InvalidData.into());
                }
            } else {
                return Err(io::ErrorKind::InvalidData.into());
            }
        }
        Ok(Some(Self {
            path: path.to_owned(),
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

    fn remove(
        mut self,
        files: &[&str],
        host: bool,
        check: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        let mut names: Vec<String> = files.iter().map(|name| (*name).to_owned()).collect();
        if files == CHECKPOINT_FILES {
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
        if host {
            let path = self.path.join("host");
            match permissions::open_tree(&path) {
                Ok(pins) => {
                    drop(pins);
                    fs::remove_dir(path)?;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        check()?;
        self.pins.pop();
        fs::remove_dir(&self.path)
    }
}
