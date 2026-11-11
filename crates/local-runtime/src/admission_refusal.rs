//! Typed refusals of a macOS or Linux data folder or socket folder that fails the ownership,
//! permission, or file-lock rules. Each names the item at fault so the startup surface can say how
//! to fix it; nothing here inspects or repairs the file system.
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

/// Why a folder or file was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefusalReason {
    /// Another user owns it, as when `sudo` keeps the invoking user's home.
    OtherOwner,
    /// Other users can write it.
    OthersCanChange,
    /// Other users have some access to a private item.
    OthersCanRead,
    /// The file system did not keep the mode Orchbee set, or ignores ownership.
    PermissionsNotKept,
    /// The file system refused a file lock.
    LocksUnsupported,
    /// A symbolic link where only real folders and files are admitted.
    SymbolicLink,
    /// A macOS access control list that can let other users in.
    AccessControlList,
    /// The socket folder belongs to another user or is open to other users.
    UnsafeSocketFolder,
}

/// One refusal and the item at fault.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionRefusal {
    pub reason: RefusalReason,
    pub path: PathBuf,
}

impl AdmissionRefusal {
    pub fn new(reason: RefusalReason, path: &Path) -> Self {
        Self {
            reason,
            path: path.to_path_buf(),
        }
    }

    /// The refusal as the I/O error an admission step returns.
    #[cfg(unix)]
    pub(crate) fn io(reason: RefusalReason, path: &Path) -> std::io::Error {
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            Self::new(reason, path),
        )
    }

    /// The refusal an I/O error carries, if it carries one.
    pub fn of(error: &std::io::Error) -> Option<&Self> {
        error.get_ref()?.downcast_ref()
    }
}

impl fmt::Display for AdmissionRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self.reason {
            RefusalReason::OtherOwner => "owned by another user",
            RefusalReason::OthersCanChange => "writable by other users",
            RefusalReason::OthersCanRead => "accessible to other users",
            RefusalReason::PermissionsNotKept => "file system does not keep permissions",
            RefusalReason::LocksUnsupported => "file system does not support file locks",
            RefusalReason::SymbolicLink => "symbolic link",
            RefusalReason::AccessControlList => "extended access control list",
            RefusalReason::UnsafeSocketFolder => "unsafe socket folder",
        };
        write!(formatter, "{reason}: {}", self.path.display())
    }
}

impl Error for AdmissionRefusal {}
