//! Native Windows refinement of the lock/marker state machine; pipes are not
//! filesystem socket leaves and are authenticated only at connection admission.
pub use crate::rendezvous_types::{
    EntryKind, EntryObservation, LockKind, RendezvousError, SiteLifetimeObservation, SiteScope,
};
use crate::windows_permissions as permissions;
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
    Acquired(ModeCheckedSiteLock),
    Contended,
}
#[derive(Debug)]
pub struct ModeCheckedSiteLock {
    _file: File,
}
#[derive(Debug)]
pub struct ModeCheckedLocalSiteRendezvous {
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

impl ModeCheckedLocalSiteRendezvous {
    pub fn prepare_mode_checked(base: &Path, scope: SiteScope) -> Result<Self, RendezvousError> {
        Self::open(base, scope, true)
    }
    pub fn validate_mode_checked(base: &Path, scope: SiteScope) -> Result<Self, RendezvousError> {
        Self::open(base, scope, false)
    }
    fn open(base: &Path, scope: SiteScope, create: bool) -> Result<Self, RendezvousError> {
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
        let [client, worker] = Self::socket_paths(&runtime);
        Ok(Self {
            runtime,
            client,
            worker,
            _directories: directories,
        })
    }
    pub fn from_runtime_directory(path: &Path) -> Result<Self, RendezvousError> {
        let parent = path.parent().ok_or(RendezvousError::InvalidSiteScope)?;
        let scope = path
            .file_name()
            .and_then(|part| part.to_str())
            .ok_or(RendezvousError::InvalidSiteScope)?;
        Self::validate_mode_checked(parent, SiteScope::parse(scope)?)
    }
    pub fn runtime_directory(&self) -> &Path {
        &self.runtime
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
    pub fn try_acquire_site_lifetime_lock(&self) -> Result<LockAttempt, RendezvousError> {
        self.try_acquire(LockKind::SiteLifetime)
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
            Ok(()) => Ok(LockAttempt::Acquired(ModeCheckedSiteLock { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(LockAttempt::Contended),
            Err(TryLockError::Error(error)) => Err(failure(&path, error)),
        }
    }
    pub fn activation_started(&self) -> io::Result<bool> {
        match permissions::open_file(&self.runtime.join("activation.started"), false, false) {
            Ok(file) if file.metadata()?.len() == 0 => Ok(true),
            Ok(_) => Err(permissions::denied()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }
    pub fn commit_activation(&self) -> io::Result<()> {
        permissions::open_file(&self.runtime.join("activation.started"), true, true).map(drop)
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
    pub fn observe_site_lifetime_lock(&self) -> io::Result<SiteLifetimeObservation> {
        let file = match permissions::open_file(
            &self.runtime.join(LockKind::SiteLifetime.name()),
            false,
            true,
        ) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(SiteLifetimeObservation::Absent);
            }
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                return Ok(SiteLifetimeObservation::Unsafe);
            }
            Err(error) => return Err(error),
        };
        match file.try_lock() {
            Ok(()) => Ok(SiteLifetimeObservation::Free),
            Err(TryLockError::WouldBlock) => Ok(SiteLifetimeObservation::Held),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }
    pub fn client_socket_exists(&self) -> io::Result<bool> {
        orchbee_local_ipc::endpoint_exists(&self.client)
    }
    pub fn worker_socket_exists(&self) -> io::Result<bool> {
        orchbee_local_ipc::endpoint_exists(&self.worker)
    }
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
