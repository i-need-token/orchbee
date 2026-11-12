//! Descriptor-based macOS and Linux admission. A POSIX descriptor does not stop another process
//! from renaming the path it was opened through, as a Windows handle without delete sharing does,
//! so privacy rests on ownership instead: every folder below the data root belongs to the
//! effective user and admits no one else, so no other user can rename, replace or add an entry
//! there. Symbolic links are never followed below the data root; the data root itself may be a
//! link and is walked through its canonical path.
use crate::{AdmissionRefusal, DataRoot, RefusalReason};
use nix::{
    errno::Errno,
    fcntl::{AtFlags, OFlag, open, openat},
    sys::stat::{Mode, SFlag, fchmod, fstatat},
    unistd::geteuid,
};
use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io,
    os::{
        fd::OwnedFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Component, Path, PathBuf},
};

const DIRECTORY_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;
const DIRECTORY_OPEN_FLAGS: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW);

pub(crate) fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "unsafe native runtime object",
    )
}

/// Requires an object the effective user owns that grants nobody else any access. macOS also
/// refuses an extended ACL and a volume that ignores ownership.
pub fn require_private(file: &File) -> io::Result<()> {
    require_private_at(file, &descriptor_path(file))
}

/// Sets the private mode on an object this process has just created, reads it back and admits it;
/// an existing object with another owner is never repaired. A mode that does not read back means
/// the file system does not keep permissions.
pub fn protect_new(file: &mut File) -> io::Result<()> {
    let path = descriptor_path(file);
    let metadata = file.metadata()?;
    if metadata.uid() != effective_uid() {
        return Err(AdmissionRefusal::io(RefusalReason::OtherOwner, &path));
    }
    let mode = if metadata.is_dir() {
        DIRECTORY_MODE
    } else {
        FILE_MODE
    };
    keep_mode(file, mode, &path)?;
    require_private_at(file, &path)
}

/// Opens a real directory without following a final symbolic link; callers separately admit it or
/// protect their new leaf. There is no separate ACL access to request on POSIX.
pub fn open_directory(path: &Path, _writable_acl: bool) -> io::Result<File> {
    let file = File::from(
        open(path, DIRECTORY_OPEN_FLAGS, Mode::empty())
            .map_err(|error| directory_error(None, path.as_os_str(), path, error))?,
    );
    if !file.metadata()?.is_dir() {
        return Err(denied());
    }
    Ok(file)
}

/// Opens every component of an exact absolute path and requires the final directory to be private,
/// or, for the data root itself, owned by the effective user and changeable by no one else.
pub fn open_tree(path: &Path) -> io::Result<Vec<File>> {
    let walk = walk(path)?;
    let depth = walk.handles.len() - 1;
    let last = walk.handles.last().ok_or_else(denied)?;
    match walk.root_depth {
        Some(root) if root == depth => require_data_root(last, &walk.path)?,
        // The walk already admitted every folder below the data root.
        Some(_) => {}
        None => require_private_directory(last, &walk.path)?,
    }
    Ok(walk.handles)
}

/// Refuses a relative path or one that spells a `.` or `..` component.
pub(crate) fn require_exact_absolute(path: &Path) -> io::Result<()> {
    if path
        .as_os_str()
        .to_str()
        .is_none_or(|text| text.split('/').any(|part| matches!(part, "." | "..")))
        || !path.is_absolute()
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(())
}

/// Opens every component of an exact absolute path without admitting the final directory. Folders
/// below the data root are admitted as private on the way, since no handle keeps them in place.
pub(crate) fn open_ancestors(path: &Path) -> io::Result<Vec<File>> {
    walk(path).map(|walk| walk.handles)
}

/// Opens a private regular single-link file without following a symbolic link; creation is
/// exclusive, never replaces a leaf, and reads its private mode back.
pub fn open_file(path: &Path, create: bool, write: bool) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(write)
        .create_new(create)
        .mode(FILE_MODE)
        // Never block on a FIFO placed at the name; regular files ignore the flag.
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| file_error(error, path))?;
    if create {
        if file.metadata()?.uid() != effective_uid() {
            return Err(AdmissionRefusal::io(RefusalReason::OtherOwner, path));
        }
        keep_mode(&file, FILE_MODE, path)?;
    }
    require_regular_private(&file, path)?;
    Ok(file)
}

/// Opens a complete file for copying. Writers here never change a published file in place: they
/// flush a staging file and rename it over the name, so the opened descriptor keeps reading one
/// complete version where Windows refuses write sharing instead.
pub fn open_snapshot_file(path: &Path) -> io::Result<File> {
    open_file(path, false, false)
}

/// The device and inode of an already admitted descriptor.
pub fn file_identity(file: &File) -> io::Result<(u64, u64)> {
    let metadata = file.metadata()?;
    Ok((metadata.dev(), metadata.ino()))
}

/// Renames a flushed staging file over `target` in the same folder, then flushes the folder so the
/// rename survives a power loss where the file system allows it. The rename itself is atomic: a
/// reader sees the old or the new file, never a partial one.
pub(crate) fn replace(staged: &Path, target: &Path) -> io::Result<()> {
    fs::rename(staged, target)?;
    sync_parent(target);
    Ok(())
}

/// Moves `source` to the absent `target` in the same folder and never over an existing object,
/// then flushes the folder. A file system without an exclusive rename falls back to checking the
/// name first, which the caller's writer lock keeps free of other Orchbee writers.
pub(crate) fn move_aside(source: &Path, target: &Path) -> io::Result<()> {
    use rustix::{
        fs::{CWD, RenameFlags, renameat_with},
        io::Errno as RustixErrno,
    };
    match renameat_with(CWD, source, CWD, target, RenameFlags::NOREPLACE) {
        Ok(()) => {}
        // Linux answers EINVAL or ENOSYS without renameat2 support; macOS answers ENOTSUP.
        Err(RustixErrno::INVAL | RustixErrno::NOSYS | RustixErrno::NOTSUP) => {
            match fs::symlink_metadata(target) {
                Ok(_) => return Err(io::ErrorKind::AlreadyExists.into()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    fs::rename(source, target)?
                }
                Err(error) => return Err(error),
            }
        }
        Err(error) => return Err(error.into()),
    }
    sync_parent(target);
    Ok(())
}

/// The data root rule: owned by the effective user, and no one else may change it. Others may read
/// and traverse it; everything below it is private.
pub(crate) fn require_data_root(file: &File, path: &Path) -> io::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_dir() {
        return Err(Errno::ENOTDIR.into());
    }
    if metadata.uid() != effective_uid() {
        return Err(AdmissionRefusal::io(RefusalReason::OtherOwner, path));
    }
    if metadata.mode() & 0o022 != 0 {
        return Err(AdmissionRefusal::io(RefusalReason::OthersCanChange, path));
    }
    #[cfg(target_os = "macos")]
    super::macos::require_private(file, path)?;
    Ok(())
}

/// Opened components, from `/` down to the requested folder, and the depth of the data root when
/// the path lies inside it.
struct Walk {
    handles: Vec<File>,
    root_depth: Option<usize>,
    path: PathBuf,
}

fn walk(path: &Path) -> io::Result<Walk> {
    require_exact_absolute(path)?;
    let (target, root_depth) = walked_spelling(path)?;
    let mut current = PathBuf::from("/");
    let mut descriptor = open(Path::new("/"), DIRECTORY_OPEN_FLAGS, Mode::empty())?;
    let mut handles = Vec::new();
    let mut depth = 0;
    for component in target.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                current.push(name);
                let next = File::from(
                    openat(&descriptor, name, DIRECTORY_OPEN_FLAGS, Mode::empty()).map_err(
                        |error| directory_error(Some(&descriptor), name, &current, error),
                    )?,
                );
                depth += 1;
                if root_depth.is_some_and(|root| depth > root) {
                    require_private_directory(&next, &current)?;
                }
                handles.push(File::from(std::mem::replace(
                    &mut descriptor,
                    OwnedFd::from(next),
                )));
            }
            _ => return Err(io::ErrorKind::InvalidInput.into()),
        }
    }
    handles.push(File::from(descriptor));
    Ok(Walk {
        handles,
        root_depth,
        path: current,
    })
}

/// A path inside this process's data root is walked through the root's canonical path, which also
/// gives the root's depth; any other path is walked as spelled. A missing data root is NotFound.
fn walked_spelling(path: &Path) -> io::Result<(PathBuf, Option<usize>)> {
    let Ok(root) = DataRoot::resolve(|name| std::env::var_os(name)) else {
        return Ok((path.to_owned(), None));
    };
    let Ok(relative) = path.strip_prefix(root.path()) else {
        return Ok((path.to_owned(), None));
    };
    let canonical = fs::canonicalize(root.path())?;
    let depth = canonical
        .components()
        .filter(|component| matches!(component, Component::Normal(_)))
        .count();
    Ok((canonical.join(relative), Some(depth)))
}

fn require_private_directory(file: &File, path: &Path) -> io::Result<()> {
    if !file.metadata()?.is_dir() {
        return Err(Errno::ENOTDIR.into());
    }
    require_private_at(file, path)
}

fn require_regular_private(file: &File, path: &Path) -> io::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(denied());
    }
    require_private_at(file, path)
}

fn require_private_at(file: &File, path: &Path) -> io::Result<()> {
    let metadata = file.metadata()?;
    if metadata.uid() != effective_uid() {
        return Err(AdmissionRefusal::io(RefusalReason::OtherOwner, path));
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(AdmissionRefusal::io(RefusalReason::OthersCanRead, path));
    }
    #[cfg(target_os = "macos")]
    super::macos::require_private(file, path)?;
    Ok(())
}

/// Sets `mode` on an object this process created and reads it back.
fn keep_mode(file: &File, mode: u32, path: &Path) -> io::Result<()> {
    fchmod(file, Mode::from_bits_truncate(mode as nix::libc::mode_t))?;
    if file.metadata()?.mode() & 0o7777 != mode {
        return Err(AdmissionRefusal::io(
            RefusalReason::PermissionsNotKept,
            path,
        ));
    }
    Ok(())
}

fn file_error(error: io::Error, path: &Path) -> io::Error {
    if error.raw_os_error() == Some(Errno::ELOOP as i32) {
        AdmissionRefusal::io(RefusalReason::SymbolicLink, path)
    } else {
        error
    }
}

fn directory_error(parent: Option<&OwnedFd>, name: &OsStr, path: &Path, error: Errno) -> io::Error {
    let link = match error {
        Errno::ELOOP => true,
        Errno::ENOTDIR => match parent {
            Some(parent) => {
                fstatat(parent, name, AtFlags::AT_SYMLINK_NOFOLLOW).is_ok_and(|status| {
                    SFlag::from_bits_truncate(status.st_mode) & SFlag::S_IFMT == SFlag::S_IFLNK
                })
            }
            None => fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_symlink()),
        },
        _ => false,
    };
    if link {
        AdmissionRefusal::io(RefusalReason::SymbolicLink, path)
    } else {
        error.into()
    }
}

/// Flushes the folder holding `path` after a rename. Best effort: the rename has already happened,
/// and Windows makes no stronger promise about the folder entry.
fn sync_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = File::open(parent).and_then(|folder| folder.sync_all());
    }
}

/// The path a descriptor was opened through, for refusals and the macOS ACL lookup; empty when the
/// host cannot say.
fn descriptor_path(file: &File) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let mut path = PathBuf::new();
        match nix::fcntl::fcntl(file, nix::fcntl::FcntlArg::F_GETPATH(&mut path)) {
            Ok(_) => path,
            Err(_) => PathBuf::new(),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        use std::os::fd::AsRawFd;
        fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())).unwrap_or_default()
    }
}

fn effective_uid() -> u32 {
    geteuid().as_raw()
}
