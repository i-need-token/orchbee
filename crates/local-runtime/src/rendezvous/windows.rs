//! Native Windows refinement of the lock/marker state machine; pipes are not
//! filesystem socket leaves and are authenticated only at connection admission.
use crate::private_filesystem as permissions;
use crate::rendezvous_types::{
    LockKind, RendezvousError, SessionHolderObservation, SessionLifetimeObservation, SessionScope,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, TryLockError},
    io,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};

pub const CLIENT_PENDING_SOCKET_NAME: &str = "client-pending.sock";
pub const HOST_SCOPE_NAME: &str = "host";
// Unix-only compatibility names; Windows Host addressing uses host_endpoint_id.
pub const HOST_ENDPOINT_ID: &str = "00000000000000000000000000000001";
pub const HOST_ENDPOINT_NAME: &str = "00000000000000000000000000000001.s";

#[derive(Debug)]
pub enum LockAttempt {
    Acquired(ModeCheckedSessionLock),
    Contended,
}
#[derive(Debug)]
pub struct ModeCheckedSessionLock {
    _file: File,
    runtime: PathBuf,
    kind: LockKind,
}
#[derive(Debug)]
pub struct ModeCheckedLocalSessionRendezvous {
    runtime: PathBuf,
    client: PathBuf,
    worker: PathBuf,
    _directories: Vec<File>,
}

fn failure(path: &Path, source: io::Error) -> RendezvousError {
    RendezvousError::System {
        operation: "admit Windows runtime",
        path: path.to_owned(),
        source,
    }
}

impl ModeCheckedLocalSessionRendezvous {
    pub fn prepare_mode_checked(base: &Path, scope: SessionScope) -> Result<Self, RendezvousError> {
        Self::open(base, scope, true)
    }
    pub fn validate_mode_checked(
        base: &Path,
        scope: SessionScope,
    ) -> Result<Self, RendezvousError> {
        Self::open(base, scope, false)
    }
    /// The runtime folder of the Session `scope` names. A pending deletion is the Session record's
    /// fact, which activation and the Server check; the runtime folder records no identity.
    fn open(base: &Path, scope: SessionScope, create: bool) -> Result<Self, RendezvousError> {
        let mut directories = permissions::open_tree(base).map_err(|error| failure(base, error))?;
        let runtime = base.join(scope.as_str());
        let created = if create {
            match std::fs::create_dir(&runtime) {
                Ok(()) => true,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
                Err(error) => return Err(failure(&runtime, error)),
            }
        } else {
            false
        };
        let mut leaf = permissions::open_directory(&runtime, created)
            .map_err(|error| failure(&runtime, error))?;
        if created {
            permissions::protect_new(&mut leaf)
        } else {
            permissions::require_private(&leaf)
        }
        .map_err(|error| failure(&runtime, error))?;
        directories.push(leaf);
        let runtime = runtime
            .canonicalize()
            .map_err(|error| failure(&runtime, error))?;
        let obsolete_lock = runtime.join("site-lifetime.lock");
        match std::fs::symlink_metadata(&obsolete_lock) {
            Ok(_) => return Err(failure(&obsolete_lock, io::ErrorKind::InvalidData.into())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(failure(&obsolete_lock, error)),
        }
        let [client, worker] = Self::socket_paths(&runtime);
        Ok(Self {
            runtime,
            client,
            worker,
            _directories: directories,
        })
    }
    pub fn from_runtime_directory(path: &Path) -> Result<Self, RendezvousError> {
        let parent = path.parent().ok_or(RendezvousError::InvalidSessionScope)?;
        let scope = path
            .file_name()
            .and_then(|part| part.to_str())
            .ok_or(RendezvousError::InvalidSessionScope)?;
        Self::validate_mode_checked(parent, SessionScope::parse(scope)?)
    }
    pub fn runtime_directory(&self) -> &Path {
        &self.runtime
    }
    /// The Session this runtime folder belongs to, spelled as the folder is.
    pub fn session_name(&self) -> io::Result<crate::SessionName> {
        self.runtime
            .file_name()
            .and_then(|leaf| leaf.to_str())
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))
            .and_then(crate::SessionName::parse)
    }
    pub fn client_socket_path(&self) -> &Path {
        &self.client
    }
    pub fn worker_socket_path(&self) -> &Path {
        &self.worker
    }
    pub fn host_runtime_directory(runtime: &Path) -> PathBuf {
        runtime.join(HOST_SCOPE_NAME)
    }
    pub fn host_endpoint_id(runtime: &Path) -> String {
        endpoint_id(runtime)
    }
    pub fn host_endpoint_path(runtime: &Path) -> PathBuf {
        format!(r"\\.\pipe\orchbee-terminal-host-{}", endpoint_id(runtime)).into()
    }
    pub fn observation_endpoint_path(runtime: &Path) -> PathBuf {
        endpoint(runtime, "observation")
    }
    /// An explicit handoff ticket selects a separate private same-user pipe,
    /// scoped by the already admitted runtime. This never names a filesystem leaf.
    pub fn handoff_endpoint_path(runtime: &Path, ticket: [u8; 16]) -> PathBuf {
        endpoint(
            runtime,
            &format!("handoff-{:032x}", u128::from_be_bytes(ticket)),
        )
    }
    pub fn client_pending_socket_path(runtime: &Path) -> PathBuf {
        endpoint(runtime, "client-pending")
    }
    pub fn socket_paths(runtime: &Path) -> [PathBuf; 2] {
        [endpoint(runtime, "client"), endpoint(runtime, "worker")]
    }
    pub fn socket_budget_paths(runtime: &Path) -> [PathBuf; 4] {
        let [client, worker] = Self::socket_paths(runtime);
        [
            client,
            worker,
            Self::client_pending_socket_path(runtime),
            Self::host_endpoint_path(runtime),
        ]
    }
    pub fn try_acquire_activation_lock(&self) -> Result<LockAttempt, RendezvousError> {
        self.try_acquire(LockKind::Activation)
    }
    pub fn try_acquire_session_lifetime_lock(&self) -> Result<LockAttempt, RendezvousError> {
        self.try_acquire(LockKind::SessionLifetime)
    }
    fn try_acquire(&self, kind: LockKind) -> Result<LockAttempt, RendezvousError> {
        let path = self.runtime.join(kind.name());
        let file = match permissions::open_file(&path, true, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                permissions::open_file(&path, false, true).map_err(|error| failure(&path, error))?
            }
            Err(error) => return Err(failure(&path, error)),
        };
        match file.try_lock() {
            Ok(()) => Ok(LockAttempt::Acquired(ModeCheckedSessionLock {
                _file: file,
                runtime: self.runtime.clone(),
                kind,
            })),
            Err(TryLockError::WouldBlock) => Ok(LockAttempt::Contended),
            Err(TryLockError::Error(error)) => Err(failure(&path, error)),
        }
    }
    pub fn activation_started(&self) -> io::Result<bool> {
        self.activation_state()
            .map(|state| state == crate::ActivationState::Active)
    }
    /// Reads the bounded existing activation record without creating or repairing it.
    /// Invalid or partial records return InvalidData; absence remains a distinct state.
    pub fn activation_state(&self) -> io::Result<crate::ActivationState> {
        match permissions::open_file(&self.runtime.join("activation.started"), false, false) {
            Ok(mut file) => crate::activation_record::read(&mut file),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(crate::ActivationState::Absent)
            }
            Err(error) => Err(error),
        }
    }
    /// Retains a validated active file for the lifetime owner's later completion publication.
    /// Missing, unsafe or non-active files fail admission and are never replaced.
    pub fn retain_active_record(&self) -> io::Result<crate::RetainedActivationRecord> {
        crate::RetainedActivationRecord::new(permissions::open_file(
            &self.runtime.join("activation.started"),
            false,
            true,
        )?)
    }
    pub fn commit_activation(&self) -> io::Result<()> {
        let path = self.runtime.join("activation.started");
        match permissions::open_file(&path, true, true) {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let mut file = permissions::open_file(&path, false, true)?;
                if !matches!(
                    crate::activation_record::read(&mut file)?,
                    crate::ActivationState::Stopped { .. } | crate::ActivationState::Crashed { .. }
                ) {
                    return Err(error);
                }
                file.set_len(0)?;
                file.sync_data()
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn reconcile_crashed(
        &self,
        activation: &ModeCheckedSessionLock,
        lifetime: &ModeCheckedSessionLock,
        activation_id: [u8; 16],
        retry: bool,
        allow_previous_receipt: bool,
    ) -> io::Result<()> {
        if activation.runtime != self.runtime
            || activation.kind != LockKind::Activation
            || lifetime.runtime != self.runtime
            || lifetime.kind != LockKind::SessionLifetime
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let path = self.runtime.join("activation.started");
        let mut file = match permissions::open_file(&path, true, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                permissions::open_file(&path, false, true)?
            }
            Err(error) => return Err(error),
        };
        crate::activation_record::publish_crashed(
            &mut file,
            activation_id,
            retry,
            allow_previous_receipt,
        )
    }
    pub(crate) fn require_lock(
        &self,
        lock: &ModeCheckedSessionLock,
        kind: LockKind,
    ) -> io::Result<()> {
        if lock.runtime != self.runtime || lock.kind != kind {
            Err(io::ErrorKind::PermissionDenied.into())
        } else {
            Ok(())
        }
    }

    /// Inspect the existing activation lock without creating or acquiring it.
    pub fn activation_lock_exists(&self) -> io::Result<bool> {
        match permissions::open_file(
            &self.runtime.join(LockKind::Activation.name()),
            false,
            false,
        ) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// A momentary lock fact, not a process-health claim. No create or repair.
    pub fn observe_session_lifetime_lock(&self) -> io::Result<SessionLifetimeObservation> {
        let file = match permissions::open_file(
            &self.runtime.join(LockKind::SessionLifetime.name()),
            false,
            true,
        ) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(SessionLifetimeObservation::Absent);
            }
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                return Ok(SessionLifetimeObservation::Unsafe);
            }
            Err(error) => return Err(error),
        };
        match file.try_lock() {
            Ok(()) => Ok(SessionLifetimeObservation::Free),
            Err(TryLockError::WouldBlock) => Ok(SessionLifetimeObservation::Held),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }
    /// Whether a Server holds this Session's lifetime, read so that it never meets a Server taking
    /// or releasing it: the existing activation lock, which a Server's activation and its stop
    /// hold around that acquire and release, is held for the instant of the lifetime read. Neither
    /// lock file is created, and a held activation lock is reported, never waited for.
    pub fn observe_session_holder(&self) -> io::Result<SessionHolderObservation> {
        let activation = match permissions::open_file(
            &self.runtime.join(LockKind::Activation.name()),
            false,
            true,
        ) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(SessionHolderObservation::NoActivation);
            }
            Err(error) => return Err(error),
        };
        let observation = match activation.try_lock() {
            Ok(()) => self
                .observe_session_lifetime_lock()
                .map(SessionHolderObservation::Lifetime),
            Err(TryLockError::WouldBlock) => Ok(SessionHolderObservation::ActivationHeld),
            Err(TryLockError::Error(error)) => Err(error),
        };
        drop(activation);
        observation
    }
    pub fn client_socket_exists(&self) -> io::Result<bool> {
        orchbee_local_ipc::endpoint_exists(&self.client)
    }
    pub fn worker_socket_exists(&self) -> io::Result<bool> {
        orchbee_local_ipc::endpoint_exists(&self.worker)
    }
}

/// Whether a pipe of this name exists, observed without connecting.
pub(crate) fn endpoint_exists(path: &Path) -> io::Result<bool> {
    orchbee_local_ipc::endpoint_exists(path)
}

fn endpoint_id(runtime: &Path) -> String {
    let mut hash = Sha256::new();
    hash.update(b"orchbee-windows-runtime-v1\0");
    // All admitted callers use the canonical path. Preserve every UTF-16 code
    // unit and normalize ASCII case (including the drive letter).
    for unit in runtime.as_os_str().encode_wide() {
        hash.update(
            if (65..=90).contains(&unit) {
                unit + 32
            } else {
                unit
            }
            .to_le_bytes(),
        );
    }
    format!("{:x}", hash.finalize())[..32].to_owned()
}
fn endpoint(runtime: &Path, role: &str) -> PathBuf {
    format!(r"\\.\pipe\orchbee-{}-{role}", endpoint_id(runtime)).into()
}
