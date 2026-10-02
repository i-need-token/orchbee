//! Cooperating native Windows activators and stopped-session deletion share one namespace owner.
use crate::{
    ActivationState, LockAttempt, ModeCheckedLocalSiteRendezvous as Rendezvous, SiteScope,
    WindowsSiteName,
    durable_sessions_windows::{self as durable, Directory, Lifecycle, Record},
    process_observation_windows::ProcessSnapshot,
    process_registration::RecoveryVerdict,
    site_name_windows, windows_permissions as permissions,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
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
    "site-lifetime.lock",
];
const CHECKPOINT_FILES: &[&str] = &[
    "checkpoint.json",
    "checkpoint.previous-1.json",
    "checkpoint.previous-2.json",
    "checkpoint.pending",
    "state.lock",
    crate::checkpoint_restore_notice_windows::FILE,
    crate::checkpoint_restore_notice_windows::PENDING,
];

pub struct WindowsNamedSiteCatalog {
    base: PathBuf,
    local: PathBuf,
    durable: Directory,
    _lock: File,
    _directories: Vec<File>,
}

#[derive(Debug)]
pub struct WindowsNamedSiteEntry {
    pub name: WindowsSiteName,
    pub runtime: PathBuf,
    pub deleting: bool,
    retained: Option<ActivationState>,
    framed_checkpoint: bool,
    checkpoint: PathBuf,
    local: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsNamedSiteDeleteOutcome {
    Absent,
    Deleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsNamedSiteDeleteReason {
    DefaultSession,
    InUse,
    RecoveryRequired,
    Filesystem,
    TimedOut,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowsNamedSiteDeleteFailure {
    pub reason: WindowsNamedSiteDeleteReason,
    pub pending: bool,
}

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
            .strip_prefix(site_name_windows::NAMED_PREFIX)
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
    let name = WindowsSiteName::parse(&intent.name).map_err(|_| io::ErrorKind::InvalidData)?;
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

impl WindowsNamedSiteCatalog {
    /// Only declared activation/deletion effects acquire this owner. Reads use `list` instead.
    pub fn try_lock(base: &Path, local: &Path) -> io::Result<Self> {
        let directories = permissions::open_tree(base)?;
        let base = base.canonicalize()?;
        let user = WindowsSiteName::parse("default")?
            .scope_for_user(&orchbee_local_ipc::current_user_sid()?);
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
        explicit: Option<&WindowsSiteName>,
    ) -> io::Result<Option<WindowsSiteName>> {
        if !Self::supports_scope(scope)? {
            return Ok(None);
        }
        let record = self.durable.read(scope)?;
        if record.as_ref().is_some_and(Record::is_deleting) {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let runtime = self.base.join(scope);
        let name = match explicit {
            Some(name) => name.clone(),
            None => match &record {
                Some(record) => WindowsSiteName::parse(&record.name)?,
                None if scope.starts_with(site_name_windows::NAMED_PREFIX) => {
                    site_name_windows::read(&runtime)?
                }
                None => WindowsSiteName::parse("default")?,
            },
        };
        site_name_windows::validate_scope(&runtime, &name)?;
        match permissions::open_tree(&runtime) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                no_endpoints(&runtime)?;
                let checkpoint = checkpoint_path(&self.local, &runtime);
                let storage = storage_guard(&checkpoint)?;
                match record.as_ref().and_then(Record::activation) {
                    Some(ActivationState::Stopped { .. }) => {
                        stopped_checkpoint_evidence(
                            &checkpoint,
                            record.as_ref().is_some_and(Record::uses_framed_checkpoint),
                        )?;
                    }
                    Some(ActivationState::Active | ActivationState::Crashed { .. }) => {
                        let _writer = self.durable.try_writer(scope)?;
                        let record = self
                            .durable
                            .read(scope)?
                            .ok_or(io::ErrorKind::InvalidData)?;
                        let processes = record
                            .processes
                            .as_ref()
                            .ok_or(io::ErrorKind::InvalidData)?;
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

    /// Proved-dead recovery uses the same namespace and exact runtime guards as new activation.
    pub fn recover_inactive(
        &self,
        site: &Rendezvous,
        activation: &crate::ModeCheckedSiteLock,
        lifetime: &crate::ModeCheckedSiteLock,
    ) -> io::Result<bool> {
        site.require_lock(activation, crate::LockKind::Activation)?;
        site.require_lock(lifetime, crate::LockKind::SiteLifetime)?;
        let runtime = site.runtime_directory();
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
        let processes = record
            .processes
            .as_ref()
            .ok_or(io::ErrorKind::InvalidData)?;
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
            match site.activation_state()? {
                ActivationState::Absent | ActivationState::Active => {}
                ActivationState::Stopped { .. } | ActivationState::Crashed { .. }
                    if allow_previous_receipt => {}
                _ => return Err(io::ErrorKind::InvalidData.into()),
            }
            record.lifecycle = Lifecycle::Crashed;
            self.durable.publish(scope, &record)?;
        }
        let activation_id = record.activation_id.ok_or(io::ErrorKind::InvalidData)?;
        site.reconcile_crashed(
            activation,
            lifetime,
            activation_id,
            retry,
            allow_previous_receipt,
        )?;
        Ok(true)
    }

    /// One new generation is published before its runtime marker or Server can exist.
    pub fn begin_activation(&self, site: &Rendezvous, activation_id: [u8; 16]) -> io::Result<()> {
        if activation_id == [0; 16] {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let runtime = site.runtime_directory();
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
        let marker = site.activation_state()?;
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
                stopped_checkpoint_evidence(&checkpoint, previous.uses_framed_checkpoint())?;
            }
        }
        let name = if scope.starts_with(site_name_windows::NAMED_PREFIX) {
            site_name_windows::read(runtime)?
        } else {
            WindowsSiteName::parse("default")?
        };
        let active = Record::registered(&self.base, &name, activation_id)?;
        self.durable.publish(scope, &active)
    }

    /// The namespace owner and matching activation guard are the Server creation capability.
    pub fn spawn_server(
        &self,
        site: &Rendezvous,
        activation: &crate::ModeCheckedSiteLock,
        executable: &Path,
        arguments: &[&std::ffi::OsStr],
        deadline: Instant,
    ) -> io::Result<crate::RetainedChild> {
        site.require_lock(activation, crate::LockKind::Activation)?;
        let runtime = site.runtime_directory();
        if runtime.parent() != Some(self.base.as_path()) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let scope = runtime
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(io::ErrorKind::InvalidInput)?;
        crate::registered_child_windows::spawn_registered(
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
    ) -> io::Result<Vec<WindowsNamedSiteEntry>> {
        let _directories = permissions::open_tree(base)?;
        let base = base.canonicalize()?;
        let sid = orchbee_local_ipc::current_user_sid()?;
        let mut names: BTreeMap<String, (bool, Option<ActivationState>, bool)> = BTreeMap::new();
        if let Some(directory) = Directory::open(local, &base)? {
            for (_, record) in directory.list(deadline)? {
                let state = record.activation();
                names.insert(
                    record.name.clone(),
                    (record.is_deleting(), state, record.uses_framed_checkpoint()),
                );
            }
        }
        for entry in fs::read_dir(&base)? {
            check_deadline(deadline)?;
            let entry = entry?;
            let Some(leaf) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let (name, deleting) = if scope_matches(&leaf) {
                let _pins = permissions::open_tree(&entry.path())?;
                match read_intent(&base, &leaf)? {
                    Some(intent) => (WindowsSiteName::parse(&intent.name)?, true),
                    None => {
                        let pending = names.iter().find(|(name, (deleting, _, _))| {
                            *deleting
                                && WindowsSiteName::parse(name)
                                    .is_ok_and(|name| name.scope_for_user(&sid) == leaf)
                        });
                        match pending {
                            Some((name, _)) => (WindowsSiteName::parse(name)?, true),
                            None => (site_name_windows::read(&entry.path())?, false),
                        }
                    }
                }
            } else if let Some(scope) = leaf
                .strip_suffix(INTENT_SUFFIX)
                .filter(|scope| scope_matches(scope))
            {
                let intent = read_intent(&base, scope)?.ok_or(io::ErrorKind::NotFound)?;
                (WindowsSiteName::parse(&intent.name)?, true)
            } else {
                continue;
            };
            names
                .entry(name.as_str().to_owned())
                .and_modify(|entry| entry.0 |= deleting)
                .or_insert((deleting, None, false));
            if names.len() + usize::from(!names.contains_key("default")) > MAXIMUM_SESSIONS {
                return Err(io::ErrorKind::FileTooLarge.into());
            }
        }
        let (deleting, retained, framed_checkpoint) =
            names.remove("default").unwrap_or((false, None, false));
        if names.len() >= MAXIMUM_SESSIONS {
            return Err(io::ErrorKind::FileTooLarge.into());
        }
        let mut result = Vec::with_capacity(names.len() + 1);
        for (text, (deleting, retained, framed_checkpoint)) in std::iter::once((
            "default".to_owned(),
            (deleting, retained, framed_checkpoint),
        ))
        .chain(names)
        {
            let name = WindowsSiteName::parse(&text)?;
            let runtime = base.join(name.scope_for_user(&sid));
            result.push(WindowsNamedSiteEntry {
                local: local.to_owned(),
                checkpoint: checkpoint_path(local, &runtime),
                runtime,
                name,
                deleting,
                retained,
                framed_checkpoint,
            });
        }
        Ok(result)
    }

    pub fn delete(
        &self,
        name: &WindowsSiteName,
        local_app_data: &Path,
        deadline: Instant,
        mut interrupted: impl FnMut() -> io::Result<bool>,
    ) -> Result<WindowsNamedSiteDeleteOutcome, WindowsNamedSiteDeleteFailure> {
        use WindowsNamedSiteDeleteReason as Reason;
        let failure = |reason, pending| WindowsNamedSiteDeleteFailure { reason, pending };
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
        let result = (|| -> io::Result<WindowsNamedSiteDeleteOutcome> {
            let scope = name.scope_for_user(&orchbee_local_ipc::current_user_sid()?);
            let runtime = self.base.join(&scope);
            let previous = read_intent(&self.base, &scope)?;
            let durable = self.durable.read(&scope)?;
            pending = previous.is_some() || durable.as_ref().is_some_and(Record::is_deleting);
            check()?;
            let _app_data = permissions::open_tree(local_app_data)?;
            if local_app_data.canonicalize()? != self.local {
                return Err(io::ErrorKind::InvalidInput.into());
            }
            let mut site = match Rendezvous::for_named_deletion(
                &self.base,
                SiteScope::parse(&scope).map_err(io::Error::other)?,
                pending,
            ) {
                Ok(site) => Some(site),
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
            if let Some(site) = &site {
                activation = match site
                    .try_acquire_activation_lock()
                    .map_err(io::Error::other)?
                {
                    LockAttempt::Acquired(lock) => Some(lock),
                    LockAttempt::Contended => {
                        refused = Some(Reason::InUse);
                        return Err(io::ErrorKind::WouldBlock.into());
                    }
                };
                lifetime = match site
                    .try_acquire_site_lifetime_lock()
                    .map_err(io::Error::other)?
                {
                    LockAttempt::Acquired(lock) => Some(lock),
                    LockAttempt::Contended => {
                        refused = Some(Reason::InUse);
                        return Err(io::ErrorKind::WouldBlock.into());
                    }
                };
                match site.activation_state()? {
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
            if site.is_none() && runtime_directory.is_some() && !pending {
                return Err(io::ErrorKind::InvalidData.into());
            }
            if site.is_none()
                && state.is_none()
                && runtime_directory.is_none()
                && durable.is_none()
                && !pending
            {
                return Ok(WindowsNamedSiteDeleteOutcome::Absent);
            }
            if site.is_none()
                && !pending
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
                self.durable.publish(&scope, &record)?;
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
            drop(site.take());
            if let Some(runtime_directory) = runtime_directory {
                runtime_directory.remove(RUNTIME_FILES, true, &mut check)?;
            }
            check()?;
            remove_private_file_if_present(&intent_path(&self.base, &scope))?;
            check()?;
            drop(writer);
            self.durable.remove(&scope)?;
            Ok(WindowsNamedSiteDeleteOutcome::Deleted)
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

impl WindowsNamedSiteEntry {
    /// A read-only, guarded observation of cold-recovery eligibility, never a future-start promise.
    pub fn cold_recoverable(&self) -> io::Result<bool> {
        if self.deleting {
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
            let user = WindowsSiteName::parse("default")?.scope_for_user(&sid);
            let _namespace = existing_lock(&base.join(format!("{user}.sessions.lock")), true)?;
            let _activation = existing_lock(&self.runtime.join("activation.lock"), false)?;
            let _lifetime = existing_lock(&self.runtime.join("site-lifetime.lock"), false)?;
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
            let Some(processes) = &record.processes else {
                return Ok(false);
            };
            let snapshot = ProcessSnapshot::observe(processes);
            if snapshot.verdict(processes) != RecoveryVerdict::Gone {
                return Ok(false);
            }
            no_endpoints(&self.runtime)?;
            let _storage = storage_guard(&self.checkpoint)?;
            // Only an entirely unstarted fresh generation may retain its predecessor receipt.
            if record.lifecycle == Lifecycle::Active {
                let site = match Rendezvous::from_runtime_directory(&self.runtime) {
                    Ok(site) => Some(site),
                    Err(crate::RendezvousError::System { source, .. })
                        if source.kind() == io::ErrorKind::NotFound =>
                    {
                        None
                    }
                    Err(error) => return Err(io::Error::other(error)),
                };
                if let Some(site) = site {
                    match site.activation_state() {
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
                        if !crate::activation_record_windows::crash_matches(
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
                match stopped_checkpoint_evidence(&self.checkpoint, self.framed_checkpoint) {
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

fn stopped_checkpoint_evidence(path: &Path, framed: bool) -> io::Result<()> {
    let leaves: &[&str] = if framed {
        &[
            "checkpoint.json",
            "checkpoint.previous-1.json",
            "checkpoint.previous-2.json",
        ]
    } else {
        &["checkpoint.json"]
    };
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
    if framed {
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
