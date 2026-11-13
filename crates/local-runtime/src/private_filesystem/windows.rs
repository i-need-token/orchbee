//! Handle-based Windows runtime admission. Open ancestors without delete sharing
//! so an admitted path cannot be redirected by renaming one of its components.
use crate::{AdmissionRefusal, RefusalReason};
use std::{
    fmt,
    fs::{File, OpenOptions},
    io,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
};
use windows_permissions::{
    LocalBox, SecurityDescriptor, Sid,
    constants::{AceType, SeObjectType, SecurityInformation},
    wrappers,
};

const REPARSE_POINT: u32 = 0x400;
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const BACKUP_SEMANTICS: u32 = 0x0200_0000;

pub(crate) fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "unsafe native Windows runtime object",
    )
}

/// A refusal made from a handle alone, before any caller has named the refused object.
#[derive(Debug)]
struct Unnamed(RefusalReason);

impl fmt::Display for Unnamed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("unsafe native Windows runtime object")
    }
}

impl std::error::Error for Unnamed {}

fn unnamed(reason: RefusalReason) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, Unnamed(reason))
}

/// Names `path` as the object of a refusal made from its handle alone, so the error becomes an
/// [`AdmissionRefusal`]; any other error is returned unchanged.
pub fn naming(error: io::Error, path: &Path) -> io::Error {
    match error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<Unnamed>())
    {
        Some(Unnamed(reason)) => AdmissionRefusal::io(*reason, path),
        None => error,
    }
}

/// One access-list entry as private admission reads it.
enum Entry<'a, T: ?Sized> {
    /// Allows access to this principal.
    Allow(&'a T),
    /// Denies access, which can only take access away.
    Deny,
    /// Any other kind, which admission cannot judge.
    Other,
}

/// The private rule over an object's owner and access list. `trusted` is the token user, SYSTEM and
/// Administrators. Any of the three may own the object: SYSTEM and the members of Administrators
/// can take ownership of any object and rewrite its access list whatever it says, so an object
/// they own is no more open than one the user owns, and an elevated session's new objects belong
/// to Administrators by Windows' default where an ordinary session's belong to the user. Every
/// allow entry must name one of the three, so no other account may own the object or reach it; a
/// missing access list, which grants everyone everything, and an entry of another kind are refused.
fn admit<T: PartialEq + ?Sized>(
    owner: Option<&T>,
    access: Option<&[Entry<'_, T>]>,
    trusted: &[&T],
) -> Result<(), RefusalReason> {
    let is_trusted = |principal: &T| trusted.iter().any(|each| **each == *principal);
    if !owner.is_some_and(is_trusted) {
        return Err(RefusalReason::OtherOwner);
    }
    for entry in access.ok_or(RefusalReason::OthersCanChange)? {
        match entry {
            Entry::Deny => {}
            Entry::Allow(principal) if is_trusted(principal) => {}
            Entry::Allow(_) => return Err(RefusalReason::OthersCanRead),
            Entry::Other => return Err(RefusalReason::AccessControlList),
        }
    }
    Ok(())
}

/// Requires the private rule of [`admit`]: an owner and allow entries naming only the token user,
/// SYSTEM and Administrators. A refusal names no object until a caller passes it to [`naming`].
pub fn require_private(file: &File) -> io::Result<()> {
    let descriptor = wrappers::GetSecurityInfo(
        file,
        SeObjectType::SE_FILE_OBJECT,
        SecurityInformation::Owner | SecurityInformation::Dacl,
    )?;
    let user: LocalBox<Sid> = orchbee_local_ipc::current_user_sid()?.parse()?;
    let system: LocalBox<Sid> = "S-1-5-18".parse()?;
    let admins: LocalBox<Sid> = "S-1-5-32-544".parse()?;
    let access: Option<Vec<Entry<'_, Sid>>> = descriptor.dacl().map(|acl| {
        (0..acl.len())
            .map(|index| match acl.get_ace(index) {
                Some(ace) if ace.ace_type() == AceType::ACCESS_DENIED_ACE_TYPE => Entry::Deny,
                Some(ace) if ace.ace_type() == AceType::ACCESS_ALLOWED_ACE_TYPE => {
                    ace.sid().map_or(Entry::Other, Entry::Allow)
                }
                _ => Entry::Other,
            })
            .collect()
    });
    admit(
        descriptor.owner(),
        access.as_deref(),
        &[&*user, &*system, &*admins],
    )
    .map_err(unnamed)
}

/// Protects an already private, newly created object without repairing an unsafe existing ACL.
pub fn protect_new(file: &mut File) -> io::Result<()> {
    require_private(file)?;
    let descriptor: LocalBox<SecurityDescriptor> = format!(
        "D:P(A;OICI;FA;;;{})(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)",
        orchbee_local_ipc::current_user_sid()?
    )
    .parse()?;
    wrappers::SetSecurityInfo(
        file,
        SeObjectType::SE_FILE_OBJECT,
        SecurityInformation::Dacl | SecurityInformation::ProtectedDacl,
        None,
        None,
        descriptor.dacl(),
        None,
    )?;
    require_private(file)
}

/// Pins a non-reparse directory; callers separately admit its ACL or protect their new leaf.
pub fn open_directory(path: &Path, writable_acl: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .share_mode(3)
        .custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS);
    if writable_acl {
        options.access_mode(0x0006_0001);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if metadata.file_attributes() & REPARSE_POINT != 0 {
        return Err(AdmissionRefusal::io(RefusalReason::SymbolicLink, path));
    }
    if !metadata.is_dir() {
        return Err(denied());
    }
    Ok(file)
}

/// Pins every ancestor of an exact absolute path and requires the final directory to be private.
pub fn open_tree(path: &Path) -> io::Result<Vec<File>> {
    let handles = open_ancestors(path)?;
    require_private(handles.last().ok_or_else(denied)?).map_err(|error| naming(error, path))?;
    Ok(handles)
}

/// Refuses a relative path or one that spells a `.` or `..` component.
pub(crate) fn require_exact_absolute(path: &Path) -> io::Result<()> {
    if path.as_os_str().to_str().is_none_or(|text| {
        text.split(['\\', '/'])
            .any(|part| matches!(part, "." | ".."))
    }) || !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(())
}

/// Pins every component of an exact absolute path without admitting the final directory's ACL.
/// The caller admits or protects its own directory below it, as an environment data root below a
/// home directory whose ACL other principals may share.
pub(crate) fn open_ancestors(path: &Path) -> io::Result<Vec<File>> {
    require_exact_absolute(path)?;
    let mut prefix = PathBuf::new();
    let mut handles = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(value) => {
                if !matches!(
                    value.kind(),
                    std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
                ) {
                    return Err(denied());
                }
                prefix.push(component.as_os_str());
            }
            Component::RootDir => {
                prefix.push(component.as_os_str());
                handles.push(open_directory(&prefix, false)?);
            }
            Component::Normal(value) => {
                prefix.push(value);
                handles.push(open_directory(&prefix, false)?);
            }
            _ => return Err(io::ErrorKind::InvalidInput.into()),
        }
    }
    Ok(handles)
}

/// Opens a private regular single-link file; creation is exclusive and never replaces a leaf.
pub fn open_file(path: &Path, create: bool, write: bool) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(write)
        .create_new(create)
        .share_mode(3)
        .custom_flags(OPEN_REPARSE_POINT)
        .open(path)?;
    require_regular_private(&file).map_err(|error| naming(error, path))?;
    Ok(file)
}

/// Pins a complete frame against concurrent write/delete handles until its copy is finished.
pub fn open_snapshot_file(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(OPEN_REPARSE_POINT)
        .open(path)?;
    require_regular_private(&file).map_err(|error| naming(error, path))?;
    Ok(file)
}

fn require_regular_private(file: &File) -> io::Result<()> {
    let metadata = file.metadata()?;
    if metadata.file_attributes() & REPARSE_POINT != 0 {
        return Err(unnamed(RefusalReason::SymbolicLink));
    }
    if !metadata.is_file() || winapi_util::file::information(file)?.number_of_links() != 1 {
        return Err(denied());
    }
    require_private(file)
}

/// Moves a flushed staging file over `target`, replacing it atomically. Windows refuses to replace
/// a file that another handle holds open, with access denied even when that handle shares delete
/// access, so the move outlasts a reader's momentary hold.
pub(crate) fn replace(staged: &Path, target: &Path) -> io::Result<()> {
    let (staged, target) = (text(staged)?, text(target)?);
    super::outlast_momentary_hold(
        |outcome: &winsafe::SysResult<()>| {
            outcome.as_ref().is_err_and(|error| {
                *error == winsafe::co::ERROR::ACCESS_DENIED
                    || *error == winsafe::co::ERROR::SHARING_VIOLATION
            })
        },
        || {
            winsafe::MoveFileEx(
                staged,
                Some(target),
                winsafe::co::MOVEFILE::REPLACE_EXISTING,
            )
        },
    )
    .map_err(io::Error::other)
}

/// A sharing violation: another handle holds the object without sharing the access asked for.
pub(super) fn held(error: &io::Error) -> bool {
    error.raw_os_error() == Some(32)
}

/// Moves `source` to the absent `target`, never over an existing object, and returns only once
/// the move is on disk. An existing `target` answers `AlreadyExists`.
pub(crate) fn move_aside(source: &Path, target: &Path) -> io::Result<()> {
    winsafe::MoveFileEx(
        text(source)?,
        Some(text(target)?),
        winsafe::co::MOVEFILE::WRITE_THROUGH,
    )
    .map_err(|error| io::Error::from_raw_os_error(error.raw() as i32))
}

fn text(path: &Path) -> io::Result<&str> {
    path.to_str()
        .ok_or_else(|| io::ErrorKind::InvalidInput.into())
}

/// Retains the exact volume and file identity of an already admitted Windows handle.
pub fn file_identity(file: &File) -> io::Result<(u64, u64)> {
    let value = winapi_util::file::information(file)?;
    Ok((value.volume_serial_number(), value.file_index()))
}

#[cfg(test)]
mod tests;
