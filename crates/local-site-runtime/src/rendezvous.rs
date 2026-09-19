use std::{
    error::Error,
    ffi::OsStr,
    fmt,
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

const MAX_SITE_SCOPE_BYTES: usize = 63;
const ACTIVATION_LOCK_NAME: &str = "activation.lock";
const SITE_LIFETIME_LOCK_NAME: &str = "site-lifetime.lock";
const CLIENT_SOCKET_NAME: &str = "client.sock";
const WORKER_SOCKET_NAME: &str = "worker.sock";
/// The pending leaf the Server binds before atomically renaming it onto
/// `client.sock`; the name is shared so binders and reporters derive one fact.
pub const CLIENT_PENDING_SOCKET_NAME: &str = "client-pending.sock";
/// The Site scope name whose runtime directory the Host owns.
pub const HOST_SCOPE_NAME: &str = "host";
/// The fixed Host endpoint identity the lazy Worker passes to a spawned Host.
pub const HOST_ENDPOINT_ID: &str = "00000000000000000000000000000001";
/// The endpoint leaf the Host binds inside its own runtime directory — always
/// `{HOST_ENDPOINT_ID}.s`; the rendezvous tests pin that identity.
pub const HOST_ENDPOINT_NAME: &str = "00000000000000000000000000000001.s";
const RUNTIME_DIRECTORY_MODE: u32 = 0o700;
const LOCK_FILE_MODE: u32 = 0o600;

const DIRECTORY_OPEN_FLAGS: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW);
const LOCK_OPEN_FLAGS: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_NONBLOCK);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SiteScope(String);

impl SiteScope {
    pub fn parse(value: &str) -> Result<Self, RendezvousError> {
        if value.is_empty()
            || value.len() > MAX_SITE_SCOPE_BYTES
            || !value.is_ascii()
            || !valid_scope_segment(value)
        {
            return Err(RendezvousError::InvalidSiteScope);
        }
        Ok(Self(value.to_owned()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LockKind {
    Activation,
    SiteLifetime,
}

impl LockKind {
    fn name(self) -> &'static str {
        match self {
            Self::Activation => ACTIVATION_LOCK_NAME,
            Self::SiteLifetime => SITE_LIFETIME_LOCK_NAME,
        }
    }
}

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
    paths: LocalSitePaths,
    directory: OwnedFd,
    expected_uid: u32,
}

impl ModeCheckedLocalSiteRendezvous {
    pub fn prepare_mode_checked(
        runtime_base: &Path,
        scope: SiteScope,
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
        scope: SiteScope,
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

    /// The pending client socket path a runtime directory derives — the leaf
    /// the Server binds before atomically renaming it onto `client.sock`.
    /// Pure path math, like [`socket_paths`](Self::socket_paths).
    pub fn client_pending_socket_path(runtime_directory: &Path) -> PathBuf {
        runtime_directory.join(CLIENT_PENDING_SOCKET_NAME)
    }

    /// The Host runtime directory a Site runtime directory derives. The Host
    /// binds its endpoint relatively inside this directory while the Worker
    /// connects through this derived absolute path.
    pub fn host_runtime_directory(runtime_directory: &Path) -> PathBuf {
        runtime_directory.join(HOST_SCOPE_NAME)
    }

    /// The Host endpoint path a Site runtime directory derives — the absolute
    /// address the Worker connects to. Pure path math.
    pub fn host_endpoint_path(runtime_directory: &Path) -> PathBuf {
        Self::host_runtime_directory(runtime_directory).join(HOST_ENDPOINT_NAME)
    }

    /// The `[client, worker]` socket paths a runtime directory derives. Pure
    /// path math — it answers even when the runtime directory cannot be
    /// validated or is absent.
    pub fn socket_paths(runtime_directory: &Path) -> [PathBuf; 2] {
        [
            runtime_directory.join(CLIENT_SOCKET_NAME),
            runtime_directory.join(WORKER_SOCKET_NAME),
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

    pub fn try_acquire_site_lifetime_lock(&self) -> Result<LockAttempt, RendezvousError> {
        self.try_acquire(LockKind::SiteLifetime)
    }

    fn open(
        runtime_base: &Path,
        scope: SiteScope,
        policy: OpenPolicy,
        expected_uid: u32,
    ) -> Result<Self, RendezvousError> {
        let paths = LocalSitePaths::new(runtime_base, &scope)?;
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

        Ok(Self {
            paths,
            directory,
            expected_uid,
        })
    }

    /// Validates an existing managed runtime without creating directories.
    pub fn from_runtime_directory(path: &Path) -> Result<Self, RendezvousError> {
        let base = path.parent().ok_or(RendezvousError::InvalidSiteScope)?;
        let scope = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or(RendezvousError::InvalidSiteScope)?;
        Self::validate_mode_checked(base, SiteScope::parse(scope)?)
    }

    pub fn activation_started(&self) -> std::io::Result<bool> {
        match openat(
            &self.directory,
            "activation.started",
            LOCK_OPEN_FLAGS,
            Mode::empty(),
        ) {
            Ok(fd) => {
                self.validate_marker(&fd)?;
                Ok(true)
            }
            Err(Errno::ENOENT) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    /// Commits one attempt. An existing marker is never overwritten or removed.
    pub fn commit_activation(&self) -> std::io::Result<()> {
        let fd = openat(
            &self.directory,
            "activation.started",
            LOCK_OPEN_FLAGS | OFlag::O_CREAT | OFlag::O_EXCL,
            exact_mode(LOCK_FILE_MODE),
        )?;
        fchmod(&fd, exact_mode(LOCK_FILE_MODE))?;
        self.validate_marker(&fd)
    }

    fn validate_marker(&self, fd: &OwnedFd) -> std::io::Result<()> {
        #[cfg(target_os = "linux")]
        crate::linux_permissions::require_private(fd)?;
        let status = fstat(fd)?;
        if SFlag::from_bits_truncate(status.st_mode) != SFlag::S_IFREG
            || status.st_uid != self.expected_uid
            || status.st_mode & 0o7777 != LOCK_FILE_MODE
            || status.st_nlink != 1
            || status.st_size != 0
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "invalid local Site activation marker",
            ));
        }
        Ok(())
    }

    pub fn client_socket_exists(&self) -> std::io::Result<bool> {
        self.socket_exists(CLIENT_SOCKET_NAME)
    }

    pub fn worker_socket_exists(&self) -> std::io::Result<bool> {
        self.socket_exists(WORKER_SOCKET_NAME)
    }

    fn socket_exists(&self, name: &str) -> std::io::Result<bool> {
        match fstatat(&self.directory, name, AtFlags::AT_SYMLINK_NOFOLLOW) {
            Ok(status)
                if SFlag::from_bits_truncate(status.st_mode) == SFlag::S_IFSOCK
                    && status.st_uid == self.expected_uid
                    && status.st_mode & 0o7777 == 0o600 =>
            {
                Ok(true)
            }
            Ok(_) => Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "invalid local Site socket",
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
            Ok(()) => Ok(LockAttempt::Acquired(ModeCheckedSiteLock { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(LockAttempt::Contended),
            Err(TryLockError::Error(source)) => Err(RendezvousError::System {
                operation: "acquire nonblocking lock",
                path: lock_path,
                source,
            }),
        }
    }

    #[cfg(test)]
    fn prepare_mode_checked_for_user(
        runtime_base: &Path,
        scope: SiteScope,
        expected_uid: u32,
    ) -> Result<Self, RendezvousError> {
        Self::open(
            runtime_base,
            scope,
            OpenPolicy::CreateIfMissing,
            expected_uid,
        )
    }
}

#[derive(Debug)]
struct LocalSitePaths {
    runtime_directory: PathBuf,
    activation_lock: PathBuf,
    site_lifetime_lock: PathBuf,
    client_socket: PathBuf,
    worker_socket: PathBuf,
}

impl LocalSitePaths {
    fn new(runtime_base: &Path, scope: &SiteScope) -> Result<Self, RendezvousError> {
        validate_runtime_base_path(runtime_base)?;
        let runtime_directory = runtime_base.join(scope.as_str());
        Ok(Self {
            activation_lock: runtime_directory.join(ACTIVATION_LOCK_NAME),
            site_lifetime_lock: runtime_directory.join(SITE_LIFETIME_LOCK_NAME),
            client_socket: runtime_directory.join(CLIENT_SOCKET_NAME),
            worker_socket: runtime_directory.join(WORKER_SOCKET_NAME),
            runtime_directory,
        })
    }

    fn lock_path(&self, kind: LockKind) -> PathBuf {
        match kind {
            LockKind::Activation => self.activation_lock.clone(),
            LockKind::SiteLifetime => self.site_lifetime_lock.clone(),
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

#[derive(Debug)]
pub enum RendezvousError {
    InvalidSiteScope,
    RuntimeBaseNotAbsolute {
        path: PathBuf,
    },
    RuntimeBaseNotNormalized {
        path: PathBuf,
    },
    DirectorySymlink {
        path: PathBuf,
    },
    NotDirectory {
        path: PathBuf,
    },
    NotRegularLockFile {
        kind: LockKind,
        path: PathBuf,
    },
    UnexpectedOwner {
        path: PathBuf,
        expected: u32,
        actual: u32,
    },
    UnexpectedMode {
        path: PathBuf,
        expected: u32,
        actual: u32,
    },
    UnexpectedLockLinkCount {
        kind: LockKind,
        path: PathBuf,
        actual: u64,
    },
    RuntimeDirectoryIdentityChanged {
        path: PathBuf,
    },
    LockFileSymlink {
        kind: LockKind,
        path: PathBuf,
    },
    System {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for RendezvousError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSiteScope => formatter.write_str("invalid local Site scope"),
            Self::RuntimeBaseNotAbsolute { path } => {
                write!(
                    formatter,
                    "runtime base is not absolute: {}",
                    path.display()
                )
            }
            Self::RuntimeBaseNotNormalized { path } => write!(
                formatter,
                "runtime base is not an exact normalized path: {}",
                path.display()
            ),
            Self::DirectorySymlink { path } => {
                write!(formatter, "directory path is a symlink: {}", path.display())
            }
            Self::NotDirectory { path } => {
                write!(formatter, "path is not a directory: {}", path.display())
            }
            Self::NotRegularLockFile { kind, path } => write!(
                formatter,
                "{} lock path is not a regular file: {}",
                kind.name(),
                path.display()
            ),
            Self::UnexpectedOwner {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "path has owner {actual}, expected {expected}: {}",
                path.display()
            ),
            Self::UnexpectedMode {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "path has mode {actual:04o}, expected {expected:04o}: {}",
                path.display()
            ),
            Self::UnexpectedLockLinkCount { kind, path, actual } => write!(
                formatter,
                "{} lock path has {actual} hard links, expected 1: {}",
                kind.name(),
                path.display()
            ),
            Self::RuntimeDirectoryIdentityChanged { path } => write!(
                formatter,
                "runtime directory identity changed during initialization: {}",
                path.display()
            ),
            Self::LockFileSymlink { kind, path } => write!(
                formatter,
                "{} lock path is a symlink: {}",
                kind.name(),
                path.display()
            ),
            Self::System {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} at {}: {source}",
                path.display()
            ),
        }
    }
}

impl Error for RendezvousError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::System { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn valid_scope_segment(value: &str) -> bool {
    let mut segments = value.split('-');
    let Some(first) = segments.next() else {
        return false;
    };
    if first.is_empty()
        || !first.as_bytes()[0].is_ascii_lowercase()
        || !first
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return false;
    }
    segments.all(|segment| {
        !segment.is_empty()
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    })
}

fn effective_uid() -> u32 {
    geteuid().as_raw()
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

pub(super) fn open_runtime_base(path: &Path) -> Result<OwnedFd, RendezvousError> {
    validate_runtime_base_path(path)?;
    let mut descriptor = open(Path::new("/"), DIRECTORY_OPEN_FLAGS, Mode::empty())
        .map_err(|error| system_error("open trusted filesystem root", Path::new("/"), error))?;
    let Some(relative_bytes) = path.as_os_str().as_bytes().strip_prefix(b"/") else {
        return Err(RendezvousError::RuntimeBaseNotAbsolute {
            path: path.to_path_buf(),
        });
    };
    if relative_bytes.is_empty() {
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

pub(super) fn validate_directory(
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
    }
    require_mode(status.st_mode, path, RUNTIME_DIRECTORY_MODE)?;
    #[cfg(target_os = "linux")]
    require_linux_permissions(descriptor, path)?;
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
    require_mode(status.st_mode, path, LOCK_FILE_MODE)?;
    #[cfg(target_os = "linux")]
    require_linux_permissions(descriptor, path)?;
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
fn require_mode(mode: impl Into<u32>, path: &Path, expected: u32) -> Result<(), RendezvousError> {
    let actual = mode.into() & 0o7777;
    if actual == expected {
        return Ok(());
    }
    Err(RendezvousError::UnexpectedMode {
        path: path.to_path_buf(),
        expected,
        actual,
    })
}

fn require_single_link(
    link_count: impl Into<u64>,
    kind: LockKind,
    path: &Path,
) -> Result<(), RendezvousError> {
    let actual = link_count.into();
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
pub(super) mod tests;

#[cfg(target_os = "linux")]
fn require_linux_permissions(descriptor: &OwnedFd, path: &Path) -> Result<(), RendezvousError> {
    crate::linux_permissions::require_private(descriptor).map_err(|source| {
        RendezvousError::System {
            operation: "validate private Linux permissions",
            path: path.to_path_buf(),
            source,
        }
    })
}
