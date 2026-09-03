use std::{
    error::Error,
    fmt,
    fs::{File, TryLockError},
    os::fd::OwnedFd,
    path::{Path, PathBuf},
};

use nix::{
    errno::Errno,
    fcntl::{OFlag, open, openat},
    sys::stat::{FileStat, Mode, SFlag, fchmod, fstat, mkdirat},
    unistd::geteuid,
};

const MAX_SITE_SCOPE_BYTES: usize = 63;
const ACTIVATION_LOCK_NAME: &str = "activation.lock";
const SITE_LIFETIME_LOCK_NAME: &str = "site-lifetime.lock";
const CLIENT_SOCKET_NAME: &str = "client.sock";
const WORKER_SOCKET_NAME: &str = "worker.sock";
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
pub(crate) struct SiteScope(String);

impl SiteScope {
    pub(crate) fn parse(value: &str) -> Result<Self, RendezvousError> {
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
pub(crate) enum LockKind {
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
pub(crate) enum LockAttempt {
    Acquired(ModeCheckedSiteLock),
    Contended,
}

#[derive(Debug)]
pub(crate) struct ModeCheckedSiteLock {
    _file: File,
}

#[derive(Debug)]
pub(crate) struct ModeCheckedLocalSiteRendezvous {
    paths: LocalSitePaths,
    directory: OwnedFd,
    expected_uid: u32,
}

impl ModeCheckedLocalSiteRendezvous {
    pub(crate) fn prepare_mode_checked(
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

    pub(crate) fn validate_mode_checked(
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

    pub(crate) fn runtime_directory(&self) -> &Path {
        &self.paths.runtime_directory
    }

    pub(crate) fn client_socket_path(&self) -> &Path {
        &self.paths.client_socket
    }

    pub(crate) fn worker_socket_path(&self) -> &Path {
        &self.paths.worker_socket
    }

    pub(crate) fn try_acquire_activation_lock(&self) -> Result<LockAttempt, RendezvousError> {
        self.try_acquire(LockKind::Activation)
    }

    pub(crate) fn try_acquire_site_lifetime_lock(&self) -> Result<LockAttempt, RendezvousError> {
        self.try_acquire(LockKind::SiteLifetime)
    }

    fn open(
        runtime_base: &Path,
        scope: SiteScope,
        policy: OpenPolicy,
        expected_uid: u32,
    ) -> Result<Self, RendezvousError> {
        let paths = LocalSitePaths::new(runtime_base, &scope)?;
        let runtime_base_descriptor = open(runtime_base, DIRECTORY_OPEN_FLAGS, Mode::empty())
            .map_err(|error| directory_open_error(runtime_base, DirectoryRole::Base, error))?;

        let created = match policy {
            OpenPolicy::CreateIfMissing => match mkdirat(
                &runtime_base_descriptor,
                scope.as_str(),
                exact_mode(RUNTIME_DIRECTORY_MODE),
            ) {
                Ok(()) => true,
                Err(Errno::EEXIST) => false,
                Err(error) => {
                    return Err(system_error(
                        "create runtime directory",
                        &paths.runtime_directory,
                        error,
                    ));
                }
            },
            OpenPolicy::ExistingOnly => false,
        };

        let directory = openat(
            &runtime_base_descriptor,
            scope.as_str(),
            DIRECTORY_OPEN_FLAGS,
            Mode::empty(),
        )
        .map_err(|error| {
            directory_open_error(&paths.runtime_directory, DirectoryRole::Runtime, error)
        })?;
        validate_directory(&directory, &paths.runtime_directory, expected_uid, created)?;

        Ok(Self {
            paths,
            directory,
            expected_uid,
        })
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
        if !runtime_base.is_absolute() {
            return Err(RendezvousError::RuntimeBaseNotAbsolute {
                path: runtime_base.to_path_buf(),
            });
        }
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
pub(crate) enum RendezvousError {
    InvalidSiteScope,
    RuntimeBaseNotAbsolute {
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

fn validate_directory(
    descriptor: &OwnedFd,
    path: &Path,
    expected_uid: u32,
    created: bool,
) -> Result<(), RendezvousError> {
    let initial = descriptor_status(descriptor, "inspect runtime directory", path)?;
    require_type(&initial, SFlag::S_IFDIR, path, None)?;
    require_owner(&initial, path, expected_uid)?;
    if created {
        fchmod(descriptor, exact_mode(RUNTIME_DIRECTORY_MODE))
            .map_err(|error| system_error("set runtime directory mode", path, error))?;
    }
    let status = descriptor_status(descriptor, "validate runtime directory", path)?;
    require_type(&status, SFlag::S_IFDIR, path, None)?;
    require_owner(&status, path, expected_uid)?;
    require_mode(&status, path, RUNTIME_DIRECTORY_MODE)
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
    require_single_link(&initial, kind, path)?;
    if created {
        fchmod(descriptor, exact_mode(LOCK_FILE_MODE))
            .map_err(|error| system_error("set lock file mode", path, error))?;
    }
    let status = descriptor_status(descriptor, "validate lock file", path)?;
    require_type(&status, SFlag::S_IFREG, path, Some(kind))?;
    require_owner(&status, path, expected_uid)?;
    require_single_link(&status, kind, path)?;
    require_mode(&status, path, LOCK_FILE_MODE)
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

fn require_mode(status: &FileStat, path: &Path, expected: u32) -> Result<(), RendezvousError> {
    let actual = u32::from(status.st_mode) & 0o7777;
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
    status: &FileStat,
    kind: LockKind,
    path: &Path,
) -> Result<(), RendezvousError> {
    let actual = u64::from(status.st_nlink);
    if actual == 1 {
        return Ok(());
    }
    Err(RendezvousError::UnexpectedLockLinkCount {
        kind,
        path: path.to_path_buf(),
        actual,
    })
}

fn directory_open_error(path: &Path, role: DirectoryRole, error: Errno) -> RendezvousError {
    match error {
        Errno::ELOOP => RendezvousError::DirectorySymlink {
            path: path.to_path_buf(),
        },
        Errno::ENOTDIR => RendezvousError::NotDirectory {
            path: path.to_path_buf(),
        },
        _ => system_error(
            match role {
                DirectoryRole::Base => "open runtime base",
                DirectoryRole::Runtime => "open runtime directory",
            },
            path,
            error,
        ),
    }
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
mod tests;
