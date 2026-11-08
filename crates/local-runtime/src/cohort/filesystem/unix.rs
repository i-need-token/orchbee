//! POSIX cohort files are opened relative to an admitted private directory descriptor.
use crate::{
    cohort::CohortArtifact,
    rendezvous::{open_runtime_base, validate_directory},
};
use nix::{
    fcntl::{OFlag, openat},
    sys::stat::{Mode, SFlag, fstat},
    unistd::geteuid,
};
use std::{fs::File, io, os::fd::OwnedFd, path::Path};
pub(crate) fn open_directory(path: &Path) -> io::Result<OwnedFd> {
    let descriptor = open_runtime_base(path).map_err(io::Error::other)?;
    validate_directory(&descriptor, path, geteuid().as_raw(), None).map_err(io::Error::other)?;
    Ok(descriptor)
}
// POSIX cohorts keep the shipped names until ORC-328.
pub(crate) const fn has_release_names() -> bool {
    true
}
pub(crate) fn artifact_file_name(artifact: CohortArtifact) -> String {
    match artifact {
        CohortArtifact::Client => "orchbee".to_owned(),
        CohortArtifact::Server => "orchbee-server".to_owned(),
        CohortArtifact::Worker => "orchbee-worker".to_owned(),
        CohortArtifact::TerminalHost => "orchbee-terminal-host".to_owned(),
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
        || status.st_uid != geteuid().as_raw()
        || status.st_nlink != 1
        || status.st_mode & 0o7022 != 0
        || (executable && status.st_mode & 0o100 == 0)
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe native cohort artifact",
        ));
    }
    #[cfg(target_os = "linux")]
    crate::private_filesystem::linux::require_private(&fd)?;
    #[cfg(target_os = "macos")]
    crate::private_filesystem::macos::require_private(&fd, _path)?;
    Ok(File::from(fd))
}
