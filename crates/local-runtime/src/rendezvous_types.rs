//! Defines validated Session scope names, the macOS and Linux socket names, and typed lock, path,
//! and lifetime observations shared by runtime admission adapters. These values perform no
//! filesystem effects and do not establish service liveness on their own.

use crate::{AdmissionRefusal, RefusalReason};
#[cfg(unix)]
use nix::sys::stat::SFlag;
use std::{error::Error, fmt, path::PathBuf};
const ACTIVATION_LOCK_NAME: &str = "activation.lock";
const SESSION_LIFETIME_LOCK_NAME: &str = "session-lifetime.lock";
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionScope(String);

impl SessionScope {
    /// A scope is the runtime folder of one Session, named by the Session, so it takes the
    /// Session name grammar.
    pub fn parse(value: &str) -> Result<Self, RendezvousError> {
        if !crate::session_name::valid(value) {
            return Err(RendezvousError::InvalidSessionScope);
        }
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LockKind {
    Activation,
    SessionLifetime,
}

impl LockKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Activation => ACTIVATION_LOCK_NAME,
            Self::SessionLifetime => SESSION_LIFETIME_LOCK_NAME,
        }
    }
}

/// One observed rendezvous entry: raw pathname facts for the caller's
/// own verdict — never a judgment about what the entry means.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryObservation {
    Absent,
    Present {
        kind: EntryKind,
        owner_matches: bool,
        mode: u32,
        size: u64,
        link_count: u64,
    },
}

/// The observed kind of one pathname entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryKind {
    Socket,
    RegularFile,
    Directory,
    Symlink,
    Other,
}

#[cfg(unix)]
impl EntryKind {
    pub(crate) fn of(mode: nix::libc::mode_t) -> Self {
        match SFlag::from_bits_truncate(mode) & SFlag::S_IFMT {
            SFlag::S_IFSOCK => Self::Socket,
            SFlag::S_IFREG => Self::RegularFile,
            SFlag::S_IFDIR => Self::Directory,
            SFlag::S_IFLNK => Self::Symlink,
            _ => Self::Other,
        }
    }
}

/// The observation result of the Session lifetime lock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionLifetimeObservation {
    Absent,
    Free,
    Held,
    Unsafe,
}

#[derive(Debug)]
pub enum RendezvousError {
    InvalidSessionScope,
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
            Self::InvalidSessionScope => formatter.write_str("invalid local Session scope"),
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

impl RendezvousError {
    /// The refusal this error states, for advice on fixing it: an owner, access, or link failure,
    /// or a refusal an admission step returned. Any other failure states none.
    pub fn refusal(&self) -> Option<AdmissionRefusal> {
        match self {
            Self::UnexpectedOwner { path, .. } => {
                Some(AdmissionRefusal::new(RefusalReason::OtherOwner, path))
            }
            Self::UnexpectedMode { path, actual, .. } if actual & 0o077 != 0 => {
                Some(AdmissionRefusal::new(RefusalReason::OthersCanRead, path))
            }
            Self::DirectorySymlink { path } | Self::LockFileSymlink { path, .. } => {
                Some(AdmissionRefusal::new(RefusalReason::SymbolicLink, path))
            }
            Self::System { source, .. } => AdmissionRefusal::of(source).cloned(),
            _ => None,
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

/// The folder that holds every environment's Linux socket folder.
#[cfg(any(all(unix, not(target_os = "macos")), test))]
pub(crate) const LINUX_SOCKET_PARENT: &str = "/tmp";
/// The macOS spelling of the same folder; `/tmp` is a link there.
#[cfg(any(target_os = "macos", test))]
pub(crate) const MACOS_SOCKET_PARENT: &str = "/private/tmp";

/// The one folder an environment's macOS or Linux sockets live in for one user,
/// `<parent>/<stem>-<uid>`. The data root holds every other runtime file.
#[cfg(any(unix, test))]
pub(crate) fn socket_root(parent: &str, stem: &str, uid: u32) -> String {
    format!("{parent}/{stem}-{uid}")
}

/// The name every socket of one Session runtime starts with: the first 32 hex digits of the
/// SHA-256 of the runtime base's canonical path, `/`, and the lowercase Session name, so its
/// length never depends on the home folder or the Session name.
#[cfg(any(unix, test))]
pub(crate) fn endpoint_id(runtime_base: &[u8], session: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"orchbee-posix-runtime-v1\0");
    hash.update(runtime_base);
    hash.update(b"/");
    hash.update(session.to_ascii_lowercase().as_bytes());
    format!("{:x}", hash.finalize())[..32].to_owned()
}

/// What one Session socket is for.
#[cfg(any(unix, test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SocketRole {
    Client,
    /// Bound before it is renamed onto the Client socket, so readiness is one atomic step.
    Pending,
    Worker,
    Host,
    /// Managed Agent observation.
    Agent,
}

/// The socket file name of one role, `<id>-<role>.sock`.
#[cfg(any(unix, test))]
pub(crate) fn socket_leaf(id: &str, role: SocketRole) -> String {
    let role = match role {
        SocketRole::Client => "client",
        SocketRole::Pending => "pending",
        SocketRole::Worker => "worker",
        SocketRole::Host => "host",
        SocketRole::Agent => "agent",
    };
    format!("{id}-{role}.sock")
}

#[cfg(test)]
mod tests;
