//! macOS and Linux effects inside the data root: the default runtime base, `internal/runtime`,
//! which activation creates before admitting it, and private folders a writer in another crate
//! creates. The data root may be a symbolic link and only its owner may change it; everything below
//! it is the owner's alone. Nothing is repaired: a folder that breaks a rule is refused with the
//! item at fault.
use super::{DataRoot, HOME_VARIABLE};
use crate::{AdmissionRefusal, RefusalReason};
use nix::{
    errno::Errno,
    fcntl::{AtFlags, OFlag, openat},
    sys::stat::{Mode, SFlag, fchmod, fstat, fstatat, mkdirat},
    unistd::geteuid,
};
use std::{
    ffi::{OsStr, OsString},
    fs::{DirBuilder, File},
    io,
    os::{
        fd::OwnedFd,
        unix::fs::{DirBuilderExt, MetadataExt},
    },
    path::{Component, Path, PathBuf},
};

const PRIVATE_MODE: u32 = 0o700;
const DIRECTORY_OPEN_FLAGS: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW);

/// Creates this environment's default runtime base below its data root and returns it through the
/// data root's canonical path, the spelling activation admits and every socket name derives from.
pub fn prepare_default_runtime_base(env: impl Fn(&str) -> Option<OsString>) -> io::Result<PathBuf> {
    let (canonical, root) = open_data_root(&env)?;
    let internal = private_child(&root, OsStr::new("internal"), &canonical.join("internal"))?;
    let runtime = canonical.join("internal").join("runtime");
    private_child(&internal, OsStr::new("runtime"), &runtime)?;
    Ok(runtime)
}

/// Creates a directory inside this environment's data root, and every missing directory between
/// them, as private, for a writer in another crate that then places its own files there.
pub fn create_private_directory(path: &Path) -> io::Result<Vec<File>> {
    let env = |name: &str| std::env::var_os(name);
    let root_path = DataRoot::resolve(env)
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?
        .path()
        .to_path_buf();
    let relative = path
        .strip_prefix(&root_path)
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let (mut current_path, root) = open_data_root(&env)?;
    let mut opened = vec![root];
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(io::ErrorKind::InvalidInput.into());
        };
        current_path.push(name);
        let parent = opened.last().ok_or(io::ErrorKind::InvalidInput)?;
        let child = private_child(parent, name, &current_path)?;
        opened.push(child);
    }
    Ok(opened.into_iter().map(File::from).collect())
}

/// Creates the data root when it is missing, as 0700 with any missing folder above it, and admits
/// it: owned by the effective user and writable by no one else. The root and its ancestors may be
/// symbolic links, so it is opened through its canonical path. A root under the home folder also
/// needs the home folder to be the effective user's, so `sudo` that keeps the invoking user's home
/// never leaves root's files there.
fn open_data_root(env: &impl Fn(&str) -> Option<OsString>) -> io::Result<(PathBuf, OwnedFd)> {
    let root = DataRoot::resolve(env).map_err(|_| io::Error::from(io::ErrorKind::NotFound))?;
    let root_path = root.path();
    if env(HOME_VARIABLE).is_none()
        && let Some(home) = root_path.parent()
        && std::fs::metadata(home)?.uid() != effective_uid()
    {
        return Err(AdmissionRefusal::io(RefusalReason::OtherOwner, home));
    }
    let created = match std::fs::symlink_metadata(root_path) {
        Ok(_) => false,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match DirBuilder::new()
                .recursive(true)
                .mode(PRIVATE_MODE)
                .create(root_path)
            {
                Ok(()) => true,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
                Err(error) => return Err(error),
            }
        }
        Err(error) => return Err(error),
    };
    let canonical = std::fs::canonicalize(root_path)?;
    let descriptor =
        crate::rendezvous::open_runtime_base(&canonical).map_err(|error| {
            match error.refusal() {
                Some(refusal) => io::Error::new(io::ErrorKind::PermissionDenied, refusal),
                None => io::Error::other(error),
            }
        })?;
    let status = fstat(&descriptor)?;
    if status.st_uid != effective_uid() {
        return Err(AdmissionRefusal::io(RefusalReason::OtherOwner, root_path));
    }
    if created {
        require_kept_mode(&descriptor, root_path)?;
    } else if status.st_mode & 0o022 != 0 {
        require_mode_change_kept(&descriptor, status.st_mode, 0o022, root_path)?;
        return Err(AdmissionRefusal::io(
            RefusalReason::OthersCanChange,
            root_path,
        ));
    }
    Ok((canonical, descriptor))
}

/// Creates `name` below an admitted folder as 0700 when it is missing and admits it as private:
/// a real folder, owned by the effective user, with no access for anyone else.
fn private_child(parent: &OwnedFd, name: &OsStr, path: &Path) -> io::Result<OwnedFd> {
    let created = match mkdirat(parent, name, Mode::from_bits_truncate(0o700)) {
        Ok(()) => true,
        Err(Errno::EEXIST) => false,
        Err(error) => return Err(error.into()),
    };
    let descriptor = match openat(parent, name, DIRECTORY_OPEN_FLAGS, Mode::empty()) {
        Ok(descriptor) => descriptor,
        Err(Errno::ELOOP) => return Err(AdmissionRefusal::io(RefusalReason::SymbolicLink, path)),
        Err(Errno::ENOTDIR) if is_symlink(parent, name) => {
            return Err(AdmissionRefusal::io(RefusalReason::SymbolicLink, path));
        }
        Err(error) => return Err(error.into()),
    };
    let status = fstat(&descriptor)?;
    if SFlag::from_bits_truncate(status.st_mode) & SFlag::S_IFMT != SFlag::S_IFDIR {
        return Err(Errno::ENOTDIR.into());
    }
    if status.st_uid != effective_uid() {
        return Err(AdmissionRefusal::io(RefusalReason::OtherOwner, path));
    }
    if created {
        require_kept_mode(&descriptor, path)?;
    } else if status.st_mode & 0o077 != 0 {
        require_mode_change_kept(&descriptor, status.st_mode, 0o077, path)?;
        return Err(AdmissionRefusal::io(RefusalReason::OthersCanRead, path));
    }
    #[cfg(target_os = "macos")]
    crate::private_filesystem::macos::require_private(&descriptor, path)?;
    Ok(descriptor)
}

/// Sets 0700 on a folder Orchbee has just created and reads it back; a file system that does not
/// keep the mode cannot keep the data private.
fn require_kept_mode(descriptor: &OwnedFd, path: &Path) -> io::Result<()> {
    fchmod(descriptor, Mode::from_bits_truncate(0o700))?;
    if permission_bits(fstat(descriptor)?.st_mode) != PRIVATE_MODE {
        return Err(AdmissionRefusal::io(
            RefusalReason::PermissionsNotKept,
            path,
        ));
    }
    Ok(())
}

/// Asks an existing folder whose mode lets others in whether its file system keeps modes at all,
/// before that mode is judged: clears `open_bits` on the open folder, reads the mode back and puts
/// the original mode back, so nothing is repaired. A file system that ignores the change cannot
/// keep the data private whatever bits it shows, so it is refused as not keeping permissions, not
/// with `chmod` advice that cannot take effect there.
fn require_mode_change_kept(
    descriptor: &OwnedFd,
    mode: impl Into<u32>,
    open_bits: u32,
    path: &Path,
) -> io::Result<()> {
    let original = permission_bits(mode);
    let closed = original & !open_bits;
    fchmod(descriptor, Mode::from_bits_truncate(closed))?;
    let read_back = permission_bits(fstat(descriptor)?.st_mode);
    fchmod(descriptor, Mode::from_bits_truncate(original))?;
    if read_back != closed {
        return Err(AdmissionRefusal::io(
            RefusalReason::PermissionsNotKept,
            path,
        ));
    }
    Ok(())
}

fn is_symlink(parent: &OwnedFd, name: &OsStr) -> bool {
    matches!(
        fstatat(parent, name, AtFlags::AT_SYMLINK_NOFOLLOW),
        Ok(status) if SFlag::from_bits_truncate(status.st_mode) & SFlag::S_IFMT == SFlag::S_IFLNK
    )
}

// Native stat field widths differ across Unix platforms; normalize losslessly.
fn permission_bits(mode: impl Into<u32>) -> u32 {
    mode.into() & 0o7777
}

fn effective_uid() -> u32 {
    geteuid().as_raw()
}
