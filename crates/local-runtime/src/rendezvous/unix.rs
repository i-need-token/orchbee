//! Admits descriptor-relative private Session runtime paths and the environment's socket folder,
//! and owns exact activation or lifetime locks, marker publication, and read-only endpoint
//! observations. Locks and markers live in the runtime directory; sockets live in
//! `/tmp/<stem>-<uid>` (`/private/tmp` on macOS), named `<id>-<role>.sock`. It never repairs
//! another owner's path or infers authenticated peer identity from a socket name.

#[cfg(not(target_os = "macos"))]
use crate::rendezvous_types::LINUX_SOCKET_PARENT as SOCKET_PARENT;
#[cfg(target_os = "macos")]
use crate::rendezvous_types::MACOS_SOCKET_PARENT as SOCKET_PARENT;
pub use crate::rendezvous_types::{
    EntryKind, EntryObservation, LockKind, RendezvousError, SessionLifetimeObservation,
    SessionScope,
};
use crate::rendezvous_types::{SocketRole, endpoint_id, socket_leaf, socket_root};
use crate::{AdmissionRefusal, RefusalReason};
use std::{
    ffi::OsStr,
    fs::{File, TryLockError},
    os::{fd::OwnedFd, unix::ffi::OsStrExt},
    path::{Path, PathBuf},
};

use nix::{
    errno::Errno,
    fcntl::{AtFlags, OFlag, open, openat},
    sys::stat::{FchmodatFlags, FileStat, Mode, SFlag, fchmod, fchmodat, fstat, fstatat, mkdirat},
    unistd::geteuid,
};

const ACTIVATION_LOCK_NAME: &str = "activation.lock";
const ACTIVATION_MARKER_NAME: &str = "activation.started";
const SESSION_LIFETIME_LOCK_NAME: &str = "session-lifetime.lock";
/// The Session scope name whose runtime directory holds the Host's lock.
pub const HOST_SCOPE_NAME: &str = "host";
const RUNTIME_DIRECTORY_MODE: u32 = 0o700;
const LOCK_FILE_MODE: u32 = 0o600;

const DIRECTORY_OPEN_FLAGS: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW);
/// Read-write, because Linux NFS refuses an exclusive lock on a read-only descriptor.
const LOCK_OPEN_FLAGS: OFlag = OFlag::O_RDWR
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_NONBLOCK);
const MARKER_OPEN_FLAGS: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_NONBLOCK);
/// The activation record is written in place, as on Windows, so its writers open it read-write.
const MARKER_WRITE_FLAGS: OFlag = OFlag::O_RDWR
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_NONBLOCK);

#[derive(Debug)]
pub enum LockAttempt {
    Acquired(ModeCheckedSessionLock),
    Contended,
}

#[derive(Debug)]
pub struct ModeCheckedSessionLock {
    _file: File,
    runtime: PathBuf,
    /// The runtime lock this is, or `None` for the shared hold on the socket folder.
    kind: Option<LockKind>,
}

#[derive(Debug)]
pub struct ModeCheckedLocalSessionRendezvous {
    paths: LocalSessionPaths,
    directory: OwnedFd,
    /// The admitted socket folder; a validation finds none when nothing has created it.
    sockets: Option<OwnedFd>,
    expected_uid: u32,
}

impl ModeCheckedLocalSessionRendezvous {
    pub fn prepare_mode_checked(
        runtime_base: &Path,
        scope: SessionScope,
    ) -> Result<Self, RendezvousError> {
        Self::open(
            runtime_base,
            scope,
            OpenPolicy::CreateIfMissing,
            effective_uid(),
        )
    }

    pub fn validate_mode_checked(
        runtime_base: &Path,
        scope: SessionScope,
    ) -> Result<Self, RendezvousError> {
        Self::open(
            runtime_base,
            scope,
            OpenPolicy::ExistingOnly,
            effective_uid(),
        )
    }

    pub fn runtime_directory(&self) -> &Path {
        &self.paths.runtime_directory
    }

    pub fn client_socket_path(&self) -> &Path {
        &self.paths.client_socket
    }

    pub fn worker_socket_path(&self) -> &Path {
        &self.paths.worker_socket
    }

    /// This environment's socket folder for the effective user, `/tmp/<stem>-<uid>` on Linux and
    /// `/private/tmp/<stem>-<uid>` on macOS. Pure path math; the Host binds its endpoint here.
    pub fn socket_root_path() -> PathBuf {
        PathBuf::from(socket_root(
            SOCKET_PARENT,
            crate::environment_stem(),
            effective_uid(),
        ))
    }

    /// The name every socket of this runtime directory starts with, which the Host also receives
    /// as its endpoint identity. Pure path math.
    pub fn host_endpoint_id(runtime_directory: &Path) -> String {
        runtime_endpoint_id(runtime_directory)
    }

    /// The pending client socket path a runtime directory derives — the socket the Server binds
    /// before atomically renaming it onto the client socket. Pure path math, like
    /// [`socket_paths`](Self::socket_paths).
    pub fn client_pending_socket_path(runtime_directory: &Path) -> PathBuf {
        socket_path(runtime_directory, SocketRole::Pending)
    }

    /// The Host runtime directory a Session runtime directory derives. It holds the Host's lock;
    /// the Host binds its endpoint in the socket folder.
    pub fn host_runtime_directory(runtime_directory: &Path) -> PathBuf {
        runtime_directory.join(HOST_SCOPE_NAME)
    }

    /// The Host endpoint path a Session runtime directory derives — the absolute
    /// address the Worker connects to. Pure path math.
    pub fn host_endpoint_path(runtime_directory: &Path) -> PathBuf {
        socket_path(runtime_directory, SocketRole::Host)
    }

    /// The managed Agent observation endpoint a Session runtime directory derives. Pure path
    /// math.
    pub fn observation_endpoint_path(runtime_directory: &Path) -> PathBuf {
        socket_path(runtime_directory, SocketRole::Agent)
    }

    /// The `[client, worker]` socket paths a runtime directory derives. Pure
    /// path math — it answers even when the runtime directory cannot be
    /// validated or is absent.
    pub fn socket_paths(runtime_directory: &Path) -> [PathBuf; 2] {
        [
            socket_path(runtime_directory, SocketRole::Client),
            socket_path(runtime_directory, SocketRole::Worker),
        ]
    }

    /// Every socket path a runtime directory derives, in stable
    /// `[client, worker, client-pending, host]` order — the two-address
    /// contract above plus the pending and Host helpers. Pure path math that
    /// answers even when the runtime directory cannot be validated or is
    /// absent, which is exactly when address-budget inspection still owes a
    /// verdict.
    pub fn socket_budget_paths(runtime_directory: &Path) -> [PathBuf; 4] {
        let [client, worker] = Self::socket_paths(runtime_directory);
        [
            client,
            worker,
            Self::client_pending_socket_path(runtime_directory),
            Self::host_endpoint_path(runtime_directory),
        ]
    }

    pub fn try_acquire_activation_lock(&self) -> Result<LockAttempt, RendezvousError> {
        self.try_acquire(LockKind::Activation)
    }

    pub fn try_acquire_session_lifetime_lock(&self) -> Result<LockAttempt, RendezvousError> {
        self.try_acquire(LockKind::SessionLifetime)
    }

    fn open(
        runtime_base: &Path,
        scope: SessionScope,
        policy: OpenPolicy,
        expected_uid: u32,
    ) -> Result<Self, RendezvousError> {
        let paths = LocalSessionPaths::new(runtime_base, &scope)?;
        let runtime_base_descriptor = open_runtime_base(runtime_base)?;

        let created_status = match policy {
            OpenPolicy::CreateIfMissing => match mkdirat(
                &runtime_base_descriptor,
                scope.as_str(),
                exact_mode(RUNTIME_DIRECTORY_MODE),
            ) {
                Ok(()) => Some(initialize_created_directory(
                    &runtime_base_descriptor,
                    OsStr::new(scope.as_str()),
                    &paths.runtime_directory,
                    expected_uid,
                )?),
                Err(Errno::EEXIST) => None,
                Err(error) => {
                    return Err(system_error(
                        "create runtime directory",
                        &paths.runtime_directory,
                        error,
                    ));
                }
            },
            OpenPolicy::ExistingOnly => None,
        };

        let directory = openat(
            &runtime_base_descriptor,
            scope.as_str(),
            DIRECTORY_OPEN_FLAGS,
            Mode::empty(),
        )
        .map_err(|error| {
            directory_open_error(
                &runtime_base_descriptor,
                OsStr::new(scope.as_str()),
                &paths.runtime_directory,
                DirectoryRole::Runtime,
                error,
            )
        })?;
        validate_directory(
            &directory,
            &paths.runtime_directory,
            expected_uid,
            created_status.as_ref(),
        )?;

        let sockets = match policy {
            OpenPolicy::CreateIfMissing => {
                Some(prepare_socket_root(&paths.socket_root, expected_uid)?)
            }
            OpenPolicy::ExistingOnly => open_socket_root(&paths.socket_root, expected_uid)?,
        };

        // A prior lifetime lock is incompatible evidence, never an unlocked current owner.
        match fstatat(
            &directory,
            "site-lifetime.lock",
            AtFlags::AT_SYMLINK_NOFOLLOW,
        ) {
            Ok(_) => {
                return Err(RendezvousError::System {
                    operation: "admit runtime identity",
                    path: paths.runtime_directory.join("site-lifetime.lock"),
                    source: std::io::ErrorKind::InvalidData.into(),
                });
            }
            Err(Errno::ENOENT) => {}
            Err(error) => {
                return Err(system_error(
                    "admit runtime identity",
                    &paths.runtime_directory,
                    error,
                ));
            }
        }

        Ok(Self {
            paths,
            directory,
            sockets,
            expected_uid,
        })
    }

    /// Holds a shared lock on the socket folder for as long as the returned lock lives, so a
    /// temporary-file cleaner that honours locks leaves this Session's sockets alone. Shared
    /// holders never contend; a cleaner holds its exclusive lock only while it scans.
    pub fn hold_socket_root(&self) -> std::io::Result<ModeCheckedSessionLock> {
        let sockets = self.sockets.as_ref().ok_or(std::io::ErrorKind::NotFound)?;
        // A descriptor of its own, so dropping the lock releases it.
        let file = File::from(openat(sockets, ".", DIRECTORY_OPEN_FLAGS, Mode::empty())?);
        file.lock_shared()?;
        Ok(ModeCheckedSessionLock {
            _file: file,
            runtime: self.paths.runtime_directory.clone(),
            kind: None,
        })
    }

    /// Validates an existing managed runtime without creating directories.
    pub fn from_runtime_directory(path: &Path) -> Result<Self, RendezvousError> {
        let base = path.parent().ok_or(RendezvousError::InvalidSessionScope)?;
        let scope = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or(RendezvousError::InvalidSessionScope)?;
        Self::validate_mode_checked(base, SessionScope::parse(scope)?)
    }

    /// The Session this runtime folder belongs to, spelled as the folder is.
    pub fn session_name(&self) -> std::io::Result<crate::SessionName> {
        self.paths
            .runtime_directory
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))
            .and_then(crate::SessionName::parse)
    }

    pub fn activation_started(&self) -> std::io::Result<bool> {
        self.activation_state()
            .map(|state| state == crate::ActivationState::Active)
    }

    /// Reads the bounded existing activation record without creating or repairing it. Invalid or
    /// partial records return InvalidData; absence remains a distinct state.
    pub fn activation_state(&self) -> std::io::Result<crate::ActivationState> {
        match self.open_marker(MARKER_OPEN_FLAGS) {
            Ok(mut file) => crate::activation_record::read(&mut file),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(crate::ActivationState::Absent)
            }
            Err(error) => Err(error),
        }
    }

    /// Retains a validated active record for the lifetime owner's later completion publication.
    /// Missing, unsafe or non-active records fail admission and are never replaced.
    pub fn retain_active_record(&self) -> std::io::Result<crate::RetainedActivationRecord> {
        crate::RetainedActivationRecord::new(self.open_marker(MARKER_WRITE_FLAGS)?)
    }

    /// Commits one attempt as an empty active record. An existing active or unreadable record is
    /// never overwritten or removed; a completed Stopped or Crashed receipt is cleared in place.
    pub fn commit_activation(&self) -> std::io::Result<()> {
        match self.create_marker() {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let mut file = self.open_marker(MARKER_WRITE_FLAGS)?;
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
    ) -> std::io::Result<()> {
        self.require_lock(activation, LockKind::Activation)?;
        self.require_lock(lifetime, LockKind::SessionLifetime)?;
        let mut file = match self.create_marker() {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                self.open_marker(MARKER_WRITE_FLAGS)?
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
    ) -> std::io::Result<()> {
        if lock.runtime != self.paths.runtime_directory || lock.kind != Some(kind) {
            Err(std::io::ErrorKind::PermissionDenied.into())
        } else {
            Ok(())
        }
    }

    /// Inspects the existing activation lock without creating or acquiring it.
    pub fn activation_lock_exists(&self) -> std::io::Result<bool> {
        match openat(
            &self.directory,
            ACTIVATION_LOCK_NAME,
            LOCK_OPEN_FLAGS,
            Mode::empty(),
        ) {
            Ok(descriptor) => {
                validate_lock_file(
                    &descriptor,
                    LockKind::Activation,
                    &self.paths.activation_lock,
                    self.expected_uid,
                    false,
                )
                .map_err(|error| {
                    std::io::Error::new(std::io::ErrorKind::PermissionDenied, error)
                })?;
                Ok(true)
            }
            Err(Errno::ENOENT) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    /// Creates the empty active record exclusively, as a private 0600 file.
    fn create_marker(&self) -> std::io::Result<File> {
        let fd = openat(
            &self.directory,
            ACTIVATION_MARKER_NAME,
            MARKER_WRITE_FLAGS | OFlag::O_CREAT | OFlag::O_EXCL,
            exact_mode(LOCK_FILE_MODE),
        )?;
        fchmod(&fd, exact_mode(LOCK_FILE_MODE))?;
        self.validate_marker(&fd)?;
        Ok(File::from(fd))
    }

    /// Opens the existing record and admits it before anything reads it.
    fn open_marker(&self, flags: OFlag) -> std::io::Result<File> {
        let fd = openat(
            &self.directory,
            ACTIVATION_MARKER_NAME,
            flags,
            Mode::empty(),
        )?;
        self.validate_marker(&fd)?;
        Ok(File::from(fd))
    }

    fn validate_marker(&self, fd: &OwnedFd) -> std::io::Result<()> {
        #[cfg(target_os = "macos")]
        crate::private_filesystem::macos::require_private(
            fd,
            &self.paths.runtime_directory.join(ACTIVATION_MARKER_NAME),
        )?;
        let status = fstat(fd)?;
        if SFlag::from_bits_truncate(status.st_mode) != SFlag::S_IFREG
            || status.st_uid != self.expected_uid
            || permission_bits(status.st_mode) != LOCK_FILE_MODE
            || status.st_nlink != 1
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "invalid local Session activation marker",
            ));
        }
        Ok(())
    }

    pub fn client_socket_exists(&self) -> std::io::Result<bool> {
        self.socket_exists(SocketRole::Client)
    }

    pub fn worker_socket_exists(&self) -> std::io::Result<bool> {
        self.socket_exists(SocketRole::Worker)
    }

    fn socket_exists(&self, role: SocketRole) -> std::io::Result<bool> {
        let Some(sockets) = &self.sockets else {
            return Ok(false);
        };
        match fstatat(
            sockets,
            socket_leaf(&self.paths.endpoint_id, role).as_str(),
            AtFlags::AT_SYMLINK_NOFOLLOW,
        ) {
            Ok(status)
                if SFlag::from_bits_truncate(status.st_mode) == SFlag::S_IFSOCK
                    && status.st_uid == self.expected_uid
                    && status.st_mode & 0o7777 == 0o600 =>
            {
                Ok(true)
            }
            Ok(_) => Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "invalid local Session socket",
            )),
            Err(Errno::ENOENT) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn try_acquire(&self, kind: LockKind) -> Result<LockAttempt, RendezvousError> {
        self.try_acquire_for_user(kind, self.expected_uid)
    }

    fn try_acquire_for_user(
        &self,
        kind: LockKind,
        expected_uid: u32,
    ) -> Result<LockAttempt, RendezvousError> {
        let lock_path = self.paths.lock_path(kind);
        let (descriptor, created) = match openat(
            &self.directory,
            kind.name(),
            LOCK_OPEN_FLAGS.union(OFlag::O_CREAT).union(OFlag::O_EXCL),
            exact_mode(LOCK_FILE_MODE),
        ) {
            Ok(descriptor) => (descriptor, true),
            Err(Errno::EEXIST) => (
                openat(&self.directory, kind.name(), LOCK_OPEN_FLAGS, Mode::empty())
                    .map_err(|error| lock_open_error(kind, &lock_path, error))?,
                false,
            ),
            Err(error) => return Err(lock_open_error(kind, &lock_path, error)),
        };
        validate_lock_file(&descriptor, kind, &lock_path, expected_uid, created)?;

        let file = File::from(descriptor);
        match file.try_lock() {
            Ok(()) => Ok(LockAttempt::Acquired(ModeCheckedSessionLock {
                _file: file,
                runtime: self.paths.runtime_directory.clone(),
                kind: Some(kind),
            })),
            Err(TryLockError::WouldBlock) => Ok(LockAttempt::Contended),
            Err(TryLockError::Error(source)) => Err(RendezvousError::System {
                operation: "acquire nonblocking lock",
                source: if locks_unsupported(&source) {
                    AdmissionRefusal::io(
                        RefusalReason::LocksUnsupported,
                        &self.paths.runtime_directory,
                    )
                } else {
                    source
                },
                path: lock_path,
            }),
        }
    }

    #[cfg(test)]
    fn prepare_mode_checked_for_user(
        runtime_base: &Path,
        scope: SessionScope,
        expected_uid: u32,
    ) -> Result<Self, RendezvousError> {
        Self::open(
            runtime_base,
            scope,
            OpenPolicy::CreateIfMissing,
            expected_uid,
        )
    }

    /// `server diagnose` observations: finite named facts through the same
    /// validated directory descriptor and owner policy the rendezvous
    /// enforces everywhere else. Nothing here creates, writes, connects,
    /// or follows an unadmitted object; every answer is a pathname or
    /// descriptor fact at its own instant, never a process claim.
    fn inspect(&self, name: &str) -> std::io::Result<EntryObservation> {
        self.inspect_at(&self.directory, name)
    }

    /// A socket of this runtime, observed in the socket folder; a missing folder holds none.
    fn inspect_socket(&self, role: SocketRole) -> std::io::Result<EntryObservation> {
        match &self.sockets {
            Some(sockets) => self.inspect_at(sockets, &socket_leaf(&self.paths.endpoint_id, role)),
            None => Ok(EntryObservation::Absent),
        }
    }

    fn inspect_at(&self, directory: &OwnedFd, name: &str) -> std::io::Result<EntryObservation> {
        match fstatat(directory, name, AtFlags::AT_SYMLINK_NOFOLLOW) {
            Ok(status) => Ok(EntryObservation::Present {
                kind: EntryKind::of(status.st_mode),
                owner_matches: status.st_uid == self.expected_uid,
                mode: permission_bits(status.st_mode),
                size: status.st_size.max(0) as u64,
                link_count: normalized_link_count(status.st_nlink),
            }),
            Err(Errno::ENOENT) => Ok(EntryObservation::Absent),
            Err(error) => Err(error.into()),
        }
    }

    pub fn observe_activation_marker(&self) -> std::io::Result<EntryObservation> {
        self.inspect(ACTIVATION_MARKER_NAME)
    }

    /// The activation lock is metadata only for diagnostics: presence,
    /// kind, owner, and mode — it is never acquired by an observation.
    pub fn observe_activation_lock(&self) -> std::io::Result<EntryObservation> {
        self.inspect(ACTIVATION_LOCK_NAME)
    }

    pub fn observe_client_socket(&self) -> std::io::Result<EntryObservation> {
        self.inspect_socket(SocketRole::Client)
    }

    pub fn observe_worker_socket(&self) -> std::io::Result<EntryObservation> {
        self.inspect_socket(SocketRole::Worker)
    }

    pub fn observe_client_pending_socket(&self) -> std::io::Result<EntryObservation> {
        self.inspect_socket(SocketRole::Pending)
    }

    pub fn observe_agent_observation_socket(&self) -> std::io::Result<EntryObservation> {
        self.inspect_socket(SocketRole::Agent)
    }

    pub fn observe_host_directory(&self) -> std::io::Result<EntryObservation> {
        self.inspect(HOST_SCOPE_NAME)
    }

    /// The Host endpoint, observed in the socket folder.
    pub fn observe_host_endpoint(&self) -> std::io::Result<EntryObservation> {
        self.inspect_socket(SocketRole::Host)
    }

    /// Observes the Session lifetime lock without creating it: a name that
    /// does not stat as a same-owner regular file is `Unsafe` without ever
    /// being opened; a validated lock is opened, probed with `try_lock`,
    /// and released on the same descriptor. A contended lock is `Held` —
    /// some owner held it at that instant, which is never proof that the
    /// owner is a Session or that any process is alive.
    pub fn observe_session_lifetime_lock(&self) -> std::io::Result<SessionLifetimeObservation> {
        match self.inspect(LockKind::SessionLifetime.name())? {
            EntryObservation::Absent => return Ok(SessionLifetimeObservation::Absent),
            EntryObservation::Present {
                kind: EntryKind::RegularFile,
                owner_matches: true,
                mode: LOCK_FILE_MODE,
                link_count: 1,
                ..
            } => {}
            EntryObservation::Present { .. } => return Ok(SessionLifetimeObservation::Unsafe),
        }
        let descriptor = match openat(
            &self.directory,
            LockKind::SessionLifetime.name(),
            LOCK_OPEN_FLAGS,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(Errno::ENOENT) => return Ok(SessionLifetimeObservation::Absent),
            Err(error) => return Err(error.into()),
        };
        if validate_lock_file(
            &descriptor,
            LockKind::SessionLifetime,
            &self.paths.session_lifetime_lock,
            self.expected_uid,
            false,
        )
        .is_err()
        {
            return Ok(SessionLifetimeObservation::Unsafe);
        }
        let file = File::from(descriptor);
        match file.try_lock() {
            Ok(()) => Ok(SessionLifetimeObservation::Free),
            Err(TryLockError::WouldBlock) => Ok(SessionLifetimeObservation::Held),
            Err(TryLockError::Error(source)) => Err(source),
        }
    }
}

#[derive(Debug)]
struct LocalSessionPaths {
    runtime_directory: PathBuf,
    activation_lock: PathBuf,
    session_lifetime_lock: PathBuf,
    socket_root: PathBuf,
    endpoint_id: String,
    client_socket: PathBuf,
    worker_socket: PathBuf,
}

impl LocalSessionPaths {
    fn new(runtime_base: &Path, scope: &SessionScope) -> Result<Self, RendezvousError> {
        validate_runtime_base_path(runtime_base)?;
        let runtime_directory = runtime_base.join(scope.as_str());
        let [client_socket, worker_socket] =
            ModeCheckedLocalSessionRendezvous::socket_paths(&runtime_directory);
        Ok(Self {
            activation_lock: runtime_directory.join(ACTIVATION_LOCK_NAME),
            session_lifetime_lock: runtime_directory.join(SESSION_LIFETIME_LOCK_NAME),
            socket_root: ModeCheckedLocalSessionRendezvous::socket_root_path(),
            endpoint_id: runtime_endpoint_id(&runtime_directory),
            client_socket,
            worker_socket,
            runtime_directory,
        })
    }

    fn lock_path(&self, kind: LockKind) -> PathBuf {
        match kind {
            LockKind::Activation => self.activation_lock.clone(),
            LockKind::SessionLifetime => self.session_lifetime_lock.clone(),
        }
    }
}

#[derive(Clone, Copy)]
enum OpenPolicy {
    CreateIfMissing,
    ExistingOnly,
}

#[derive(Clone, Copy)]
enum DirectoryRole {
    Base,
    Runtime,
}

fn effective_uid() -> u32 {
    geteuid().as_raw()
}

/// Whether any object occupies a socket path, observed without following it or connecting; a
/// missing socket folder holds none.
pub(crate) fn endpoint_exists(path: &Path) -> std::io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// The socket name prefix of a runtime directory, from its canonical base and its Session name.
fn runtime_endpoint_id(runtime_directory: &Path) -> String {
    let base = runtime_directory
        .parent()
        .map_or(&[][..], |base| base.as_os_str().as_bytes());
    let session = runtime_directory
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    endpoint_id(base, &session)
}

fn socket_path(runtime_directory: &Path, role: SocketRole) -> PathBuf {
    ModeCheckedLocalSessionRendezvous::socket_root_path()
        .join(socket_leaf(&runtime_endpoint_id(runtime_directory), role))
}

/// Opens the folder that holds every socket folder, which must belong to root and carry the
/// sticky bit, so no other user can replace this user's socket folder.
fn open_socket_parent() -> Result<OwnedFd, RendezvousError> {
    let parent = Path::new(SOCKET_PARENT);
    let descriptor = open(parent, DIRECTORY_OPEN_FLAGS, Mode::empty())
        .map_err(|error| system_error("open socket folder parent", parent, error))?;
    let status = descriptor_status(&descriptor, "inspect socket folder parent", parent)?;
    if status.st_uid != 0 || status.st_mode & nix::libc::S_ISVTX == 0 {
        return Err(unsafe_socket_folder(parent));
    }
    Ok(descriptor)
}

/// Creates this environment's socket folder as 0700 when it is missing and admits it.
fn prepare_socket_root(path: &Path, expected_uid: u32) -> Result<OwnedFd, RendezvousError> {
    let parent = open_socket_parent()?;
    let name = path.file_name().unwrap_or_default();
    let created = match mkdirat(&parent, name, exact_mode(RUNTIME_DIRECTORY_MODE)) {
        Ok(()) => Some(
            initialize_created_directory(&parent, name, path, expected_uid)
                .map_err(|error| socket_folder_error(path, error))?,
        ),
        Err(Errno::EEXIST) => None,
        Err(error) => return Err(system_error("create socket folder", path, error)),
    };
    admit_socket_root(&parent, path, expected_uid, created.as_ref())
}

/// Admits an existing socket folder; none exists before an activation creates it.
fn open_socket_root(path: &Path, expected_uid: u32) -> Result<Option<OwnedFd>, RendezvousError> {
    let parent = open_socket_parent()?;
    match fstatat(
        &parent,
        path.file_name().unwrap_or_default(),
        AtFlags::AT_SYMLINK_NOFOLLOW,
    ) {
        Err(Errno::ENOENT) => Ok(None),
        Err(error) => Err(system_error("inspect socket folder", path, error)),
        Ok(_) => admit_socket_root(&parent, path, expected_uid, None).map(Some),
    }
}

fn admit_socket_root(
    parent: &OwnedFd,
    path: &Path,
    expected_uid: u32,
    created_status: Option<&FileStat>,
) -> Result<OwnedFd, RendezvousError> {
    let descriptor = openat(
        parent,
        path.file_name().unwrap_or_default(),
        DIRECTORY_OPEN_FLAGS,
        Mode::empty(),
    )
    .map_err(|error| match error {
        Errno::ELOOP | Errno::ENOTDIR => unsafe_socket_folder(path),
        _ => system_error("open socket folder", path, error),
    })?;
    validate_directory(&descriptor, path, expected_uid, created_status)
        .map_err(|error| socket_folder_error(path, error))?;
    Ok(descriptor)
}

/// A socket folder that fails its owner, type, or mode rule is refused as unsafe; a refusal or an
/// I/O failure keeps its own meaning.
fn socket_folder_error(path: &Path, error: RendezvousError) -> RendezvousError {
    match error {
        RendezvousError::System { .. } => error,
        _ => unsafe_socket_folder(path),
    }
}

fn unsafe_socket_folder(path: &Path) -> RendezvousError {
    RendezvousError::System {
        operation: "admit socket folder",
        path: path.to_path_buf(),
        source: AdmissionRefusal::io(RefusalReason::UnsafeSocketFolder, path),
    }
}

fn validate_runtime_base_path(path: &Path) -> Result<(), RendezvousError> {
    let bytes = path.as_os_str().as_bytes();
    let Some(relative_bytes) = bytes.strip_prefix(b"/") else {
        return Err(RendezvousError::RuntimeBaseNotAbsolute {
            path: path.to_path_buf(),
        });
    };
    if relative_bytes.is_empty() {
        return Ok(());
    }
    if relative_bytes.split(|byte| *byte == b'/').any(|component| {
        component.is_empty() || component == b"." || component == b".." || component.contains(&0)
    }) {
        return Err(RendezvousError::RuntimeBaseNotNormalized {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

pub(crate) fn open_runtime_base(path: &Path) -> Result<OwnedFd, RendezvousError> {
    validate_runtime_base_path(path)?;
    let mut descriptor = open(Path::new("/"), DIRECTORY_OPEN_FLAGS, Mode::empty())
        .map_err(|error| system_error("open trusted filesystem root", Path::new("/"), error))?;
    let Some(relative_bytes) = path.as_os_str().as_bytes().strip_prefix(b"/") else {
        return Err(RendezvousError::RuntimeBaseNotAbsolute {
            path: path.to_path_buf(),
        });
    };
    if relative_bytes.is_empty() {
        #[cfg(target_os = "macos")]
        require_macos_permissions(&descriptor, path)?;
        return Ok(descriptor);
    }

    let mut opened_path = PathBuf::from("/");
    for component in relative_bytes.split(|byte| *byte == b'/') {
        let component = OsStr::from_bytes(component);
        opened_path.push(component);
        descriptor = openat(&descriptor, component, DIRECTORY_OPEN_FLAGS, Mode::empty()).map_err(
            |error| {
                directory_open_error(
                    &descriptor,
                    component,
                    &opened_path,
                    DirectoryRole::Base,
                    error,
                )
            },
        )?;
    }
    #[cfg(target_os = "macos")]
    require_macos_permissions(&descriptor, path)?;
    Ok(descriptor)
}

fn initialize_created_directory(
    parent: &OwnedFd,
    component: &OsStr,
    path: &Path,
    expected_uid: u32,
) -> Result<FileStat, RendezvousError> {
    let status = fstatat(parent, component, AtFlags::AT_SYMLINK_NOFOLLOW)
        .map_err(|error| system_error("inspect new runtime directory", path, error))?;
    require_type(&status, SFlag::S_IFDIR, path, None)?;
    require_owner(&status, path, expected_uid)?;
    fchmodat(
        parent,
        component,
        exact_mode(RUNTIME_DIRECTORY_MODE),
        FchmodatFlags::NoFollowSymlink,
    )
    .map_err(|error| system_error("initialize runtime directory mode", path, error))?;
    Ok(status)
}

fn validate_directory(
    descriptor: &OwnedFd,
    path: &Path,
    expected_uid: u32,
    created_status: Option<&FileStat>,
) -> Result<(), RendezvousError> {
    let status = descriptor_status(descriptor, "validate runtime directory", path)?;
    require_type(&status, SFlag::S_IFDIR, path, None)?;
    require_owner(&status, path, expected_uid)?;
    if let Some(created_status) = created_status {
        require_same_directory_identity(created_status, &status, path)?;
        require_kept_mode(status.st_mode, path, RUNTIME_DIRECTORY_MODE)?;
    }
    require_mode(status.st_mode, path, RUNTIME_DIRECTORY_MODE)?;
    #[cfg(target_os = "macos")]
    require_macos_permissions(descriptor, path)?;
    Ok(())
}

fn require_same_directory_identity(
    expected: &FileStat,
    actual: &FileStat,
    path: &Path,
) -> Result<(), RendezvousError> {
    if expected.st_dev == actual.st_dev && expected.st_ino == actual.st_ino {
        return Ok(());
    }
    Err(RendezvousError::RuntimeDirectoryIdentityChanged {
        path: path.to_path_buf(),
    })
}

fn validate_lock_file(
    descriptor: &OwnedFd,
    kind: LockKind,
    path: &Path,
    expected_uid: u32,
    created: bool,
) -> Result<(), RendezvousError> {
    let initial = descriptor_status(descriptor, "inspect lock file", path)?;
    require_type(&initial, SFlag::S_IFREG, path, Some(kind))?;
    require_owner(&initial, path, expected_uid)?;
    require_single_link(initial.st_nlink, kind, path)?;
    if created {
        fchmod(descriptor, exact_mode(LOCK_FILE_MODE))
            .map_err(|error| system_error("set lock file mode", path, error))?;
    }
    let status = descriptor_status(descriptor, "validate lock file", path)?;
    require_type(&status, SFlag::S_IFREG, path, Some(kind))?;
    require_owner(&status, path, expected_uid)?;
    require_single_link(status.st_nlink, kind, path)?;
    if created {
        require_kept_mode(status.st_mode, path, LOCK_FILE_MODE)?;
    }
    require_mode(status.st_mode, path, LOCK_FILE_MODE)?;
    #[cfg(target_os = "macos")]
    require_macos_permissions(descriptor, path)?;
    Ok(())
}

fn descriptor_status(
    descriptor: &OwnedFd,
    operation: &'static str,
    path: &Path,
) -> Result<FileStat, RendezvousError> {
    fstat(descriptor).map_err(|error| system_error(operation, path, error))
}

fn require_type(
    status: &FileStat,
    expected: SFlag,
    path: &Path,
    lock_kind: Option<LockKind>,
) -> Result<(), RendezvousError> {
    let actual = SFlag::from_bits_truncate(status.st_mode) & SFlag::S_IFMT;
    if actual == expected {
        return Ok(());
    }
    match lock_kind {
        Some(kind) => Err(RendezvousError::NotRegularLockFile {
            kind,
            path: path.to_path_buf(),
        }),
        None => Err(RendezvousError::NotDirectory {
            path: path.to_path_buf(),
        }),
    }
}

fn require_owner(status: &FileStat, path: &Path, expected_uid: u32) -> Result<(), RendezvousError> {
    if status.st_uid == expected_uid {
        return Ok(());
    }
    Err(RendezvousError::UnexpectedOwner {
        path: path.to_path_buf(),
        expected: expected_uid,
        actual: status.st_uid,
    })
}

// Native stat field widths differ across Unix platforms; normalize losslessly.
fn permission_bits(mode: impl Into<u32>) -> u32 {
    mode.into() & 0o7777
}

fn normalized_link_count(link_count: impl Into<u64>) -> u64 {
    link_count.into()
}

fn require_mode(mode: impl Into<u32>, path: &Path, expected: u32) -> Result<(), RendezvousError> {
    let actual = permission_bits(mode);
    if actual == expected {
        return Ok(());
    }
    Err(RendezvousError::UnexpectedMode {
        path: path.to_path_buf(),
        expected,
        actual,
    })
}

/// A mode Orchbee has just set that does not read back means the file system does not keep
/// permissions.
fn require_kept_mode(
    mode: impl Into<u32>,
    path: &Path,
    expected: u32,
) -> Result<(), RendezvousError> {
    if permission_bits(mode) == expected {
        return Ok(());
    }
    Err(RendezvousError::System {
        operation: "keep permissions",
        path: path.to_path_buf(),
        source: AdmissionRefusal::io(RefusalReason::PermissionsNotKept, path),
    })
}

fn require_single_link(
    link_count: impl Into<u64>,
    kind: LockKind,
    path: &Path,
) -> Result<(), RendezvousError> {
    let actual = normalized_link_count(link_count);
    if actual == 1 {
        return Ok(());
    }
    Err(RendezvousError::UnexpectedLockLinkCount {
        kind,
        path: path.to_path_buf(),
        actual,
    })
}

fn directory_open_error(
    parent: &OwnedFd,
    component: &OsStr,
    path: &Path,
    role: DirectoryRole,
    error: Errno,
) -> RendezvousError {
    match error {
        Errno::ELOOP => RendezvousError::DirectorySymlink {
            path: path.to_path_buf(),
        },
        Errno::ENOTDIR if entry_is_symlink(parent, component) => {
            RendezvousError::DirectorySymlink {
                path: path.to_path_buf(),
            }
        }
        Errno::ENOTDIR => RendezvousError::NotDirectory {
            path: path.to_path_buf(),
        },
        _ => system_error(
            match role {
                DirectoryRole::Base => "open runtime-base component",
                DirectoryRole::Runtime => "open runtime directory",
            },
            path,
            error,
        ),
    }
}

fn entry_is_symlink(parent: &OwnedFd, component: &OsStr) -> bool {
    matches!(
        fstatat(parent, component, AtFlags::AT_SYMLINK_NOFOLLOW),
        Ok(status)
            if SFlag::from_bits_truncate(status.st_mode) & SFlag::S_IFMT == SFlag::S_IFLNK
    )
}

fn lock_open_error(kind: LockKind, path: &Path, error: Errno) -> RendezvousError {
    match error {
        Errno::ELOOP => RendezvousError::LockFileSymlink {
            kind,
            path: path.to_path_buf(),
        },
        Errno::EISDIR | Errno::ENODEV | Errno::ENXIO => RendezvousError::NotRegularLockFile {
            kind,
            path: path.to_path_buf(),
        },
        _ => system_error("open lock file", path, error),
    }
}

/// A file system without working file locks answers a lock with one of these.
fn locks_unsupported(error: &std::io::Error) -> bool {
    error.raw_os_error().is_some_and(|code| {
        [Errno::ENOLCK, Errno::ENOTSUP, Errno::EOPNOTSUPP]
            .iter()
            .any(|errno| *errno as i32 == code)
    })
}

fn system_error(operation: &'static str, path: &Path, error: Errno) -> RendezvousError {
    RendezvousError::System {
        operation,
        path: path.to_path_buf(),
        source: std::io::Error::from(error),
    }
}

fn exact_mode(bits: u32) -> Mode {
    Mode::from_bits_truncate(bits as nix::libc::mode_t)
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(target_os = "macos")]
fn require_macos_permissions(descriptor: &OwnedFd, path: &Path) -> Result<(), RendezvousError> {
    crate::private_filesystem::macos::require_private(descriptor, path).map_err(|source| {
        RendezvousError::System {
            operation: "validate private macOS permissions",
            path: path.to_path_buf(),
            source,
        }
    })
}
