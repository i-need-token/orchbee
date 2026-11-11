//! POSIX cohort files are opened relative to an admitted directory descriptor. A cohort may be
//! staged by its user or installed by root, so the directory and its files belong to the effective
//! user or to root and no one else can change them.
use crate::{cohort::CohortArtifact, environment::environment_stem, rendezvous::open_runtime_base};
use nix::{
    fcntl::{OFlag, openat},
    sys::stat::{FileStat, Mode, SFlag, fstat},
    unistd::geteuid,
};
use std::{fs::File, io, os::fd::OwnedFd, path::Path};

const ROOT_UID: u32 = 0;

pub(crate) fn open_directory(path: &Path) -> io::Result<OwnedFd> {
    let descriptor = open_runtime_base(path).map_err(io::Error::other)?;
    let status = fstat(&descriptor)?;
    if SFlag::from_bits_truncate(status.st_mode) & SFlag::S_IFMT != SFlag::S_IFDIR
        || !trusted(&status)
    {
        return Err(unsafe_artifact());
    }
    Ok(descriptor)
}
/// Environments ship distinct cohort names (ORC-382); the environment owns the compiled stem.
pub(crate) const fn has_release_names() -> bool {
    crate::environment::is_release()
}
pub(crate) fn artifact_file_name(artifact: CohortArtifact) -> String {
    match artifact {
        CohortArtifact::Client => environment_stem().to_owned(),
        CohortArtifact::Server => format!("{}-server", environment_stem()),
        CohortArtifact::Worker => format!("{}-worker", environment_stem()),
        CohortArtifact::TerminalHost => format!("{}-terminal-host", environment_stem()),
    }
}
pub(crate) fn open_file(
    directory: &std::os::fd::OwnedFd,
    name: &str,
    executable: bool,
    _path: &Path,
) -> io::Result<File> {
    let fd = openat(
        directory,
        name,
        OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK,
        Mode::empty(),
    )?;
    let status = fstat(&fd)?;
    if SFlag::from_bits_truncate(status.st_mode) != SFlag::S_IFREG
        || !trusted(&status)
        || status.st_nlink != 1
        || status.st_mode & 0o7000 != 0
        || (executable && status.st_mode & 0o100 == 0)
    {
        return Err(unsafe_artifact());
    }
    #[cfg(target_os = "macos")]
    crate::private_filesystem::macos::require_private(&fd, _path)?;
    Ok(File::from(fd))
}

/// Owned by the effective user or by root, and writable by neither its group nor others.
fn trusted(status: &FileStat) -> bool {
    (status.st_uid == geteuid().as_raw() || status.st_uid == ROOT_UID)
        && status.st_mode & 0o022 == 0
}

fn unsafe_artifact() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "unsafe native cohort artifact",
    )
}
