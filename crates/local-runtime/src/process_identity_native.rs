//! The reviewed macOS reads of a process's start time and program file through libproc, which a
//! registration names a process by (`proposals/platform/host-handover.md`, H5 design). The crate
//! README's safety contract lists every invariant the unsafe blocks below rely on.
use nix::libc;
use std::{ffi::OsString, io, mem::MaybeUninit, os::unix::ffi::OsStringExt, path::PathBuf};

/// What a process id names now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Started {
    /// No live process has the id, or its process has ended and waits as a zombie to be reaped.
    Gone,
    /// A live process has the id and started at this time, in microseconds since the epoch.
    At(u64),
    /// A process of another account has the id, so its start time cannot be read.
    Denied,
}

/// When the process `pid` started, read with `proc_pidinfo` and `PROC_PIDTBSDINFO`.
pub(crate) fn started(pid: u32) -> io::Result<Started> {
    let pid =
        libc::c_int::try_from(pid).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let size = libc::c_int::try_from(size_of::<libc::proc_bsdinfo>())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
    let mut information = MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    // SAFETY: the buffer is zeroed storage of exactly `size` bytes owned here, and the kernel writes
    // at most that many; every bit pattern is a valid `proc_bsdinfo`, which holds only integers.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            information.as_mut_ptr().cast(),
            size,
        )
    };
    if written <= 0 {
        let error = io::Error::last_os_error();
        return match error.raw_os_error() {
            Some(libc::ESRCH) => Ok(Started::Gone),
            Some(libc::EPERM) => Ok(Started::Denied),
            _ => Err(error),
        };
    }
    if written != size {
        return Err(io::ErrorKind::InvalidData.into());
    }
    // SAFETY: the zeroed buffer is fully written, and any bit pattern is valid.
    let information = unsafe { information.assume_init() };
    if information.pbi_status == libc::SZOMB {
        return Ok(Started::Gone);
    }
    let start = information
        .pbi_start_tvsec
        .checked_mul(1_000_000)
        .and_then(|seconds| seconds.checked_add(information.pbi_start_tvusec))
        .filter(|start| *start != 0)
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
    Ok(Started::At(start))
}

/// The file name of the program the process `pid` runs, as [`program_path`] reads it.
pub(crate) fn program_file_name(pid: u32) -> Option<OsString> {
    program_path(pid)?.file_name().map(ToOwned::to_owned)
}

/// The full path of the program the process `pid` runs, read with `proc_pidpath`, which needs no
/// right to that process; `None` when no process has the id or the path cannot be read.
pub(crate) fn program_path(pid: u32) -> Option<PathBuf> {
    let pid = libc::c_int::try_from(pid).ok()?;
    let capacity = usize::try_from(libc::PROC_PIDPATHINFO_MAXSIZE).ok()?;
    let mut path = vec![0_u8; capacity];
    let length = u32::try_from(capacity).ok()?;
    // SAFETY: `path` is a writable buffer of `length` bytes owned here; the call writes at most
    // that many and returns how many it wrote.
    let written = unsafe { libc::proc_pidpath(pid, path.as_mut_ptr().cast(), length) };
    let written = usize::try_from(written)
        .ok()
        .filter(|written| *written > 0)?;
    path.truncate(written.min(capacity));
    Some(PathBuf::from(OsString::from_vec(path)))
}

/// The IDs of the child processes of `parent`, ended ones still waiting to be reaped included, read
/// with `proc_listchildpids`.
pub(crate) fn children(parent: u32) -> io::Result<Vec<u32>> {
    let parent =
        libc::pid_t::try_from(parent).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let mut capacity = 64_usize;
    loop {
        let mut pids = vec![0 as libc::pid_t; capacity];
        let bytes = libc::c_int::try_from(capacity * size_of::<libc::pid_t>())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        // SAFETY: `pids` is an owned, writable buffer of `bytes` bytes; the call writes at most that
        // many and returns how many IDs it wrote.
        let listed = unsafe { libc::proc_listchildpids(parent, pids.as_mut_ptr().cast(), bytes) };
        let listed = usize::try_from(listed).map_err(|_| io::Error::last_os_error())?;
        // A full buffer may have left children out, so a larger one reads them again.
        if listed >= capacity && capacity < MAXIMUM_CHILDREN {
            capacity *= 4;
            continue;
        }
        pids.truncate(listed.min(capacity));
        return Ok(pids
            .into_iter()
            .filter_map(|pid| u32::try_from(pid).ok())
            .filter(|pid| *pid != 0)
            .collect());
    }
}

/// The most child IDs one read lists.
const MAXIMUM_CHILDREN: usize = 4096;

#[cfg(test)]
#[deny(unsafe_code)]
mod tests;
