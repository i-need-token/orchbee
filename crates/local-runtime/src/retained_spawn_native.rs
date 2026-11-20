//! The reviewed macOS adapter that starts a retained child through `posix_spawn` with
//! `POSIX_SPAWN_CLOEXEC_DEFAULT`, so the child receives only the standard streams its file actions
//! open, and that reaps the child. It also clears the child's signal mask and resets `SIGPIPE` to
//! its default action. The standard library's spawn does not clear the mask (checked on macOS
//! with Rust 1.97 through both its `posix_spawn` and its fork path), so a child it starts while a
//! command's signal owner is armed keeps that owner's blocked signals. macOS marks a new socket or pipe close-on-exec only after
//! creating it, so without that flag a descriptor another thread is creating at that moment would
//! reach the child and outlive this process's own use of it. [`crate::retained_child`], the hub
//! carrier's launch, the Worker's configured command and a Session start's detached child, through
//! [`crate::command_child`], are the only callers; the crate README's safety contract lists every
//! invariant the unsafe blocks below rely on.
use nix::libc;
use std::{
    ffi::{CStr, CString, OsStr, OsString},
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, BorrowedFd},
        unix::ffi::OsStrExt,
    },
    path::Path,
    ptr,
};

unsafe extern "C" {
    // macOS 10.15 and later. The libc crate does not declare it for Apple targets.
    fn posix_spawn_file_actions_addchdir_np(
        actions: *mut libc::posix_spawn_file_actions_t,
        path: *const libc::c_char,
    ) -> libc::c_int;
}

const NULL_DEVICE: &CStr = c"/dev/null";

/// Starts `executable` with `arguments` after it in its argument vector, exactly `environment`,
/// `directory` as its working directory, an empty signal mask and `SIGPIPE` at its default action,
/// whatever this thread blocks or ignores. Its three standard streams are the descriptors in
/// `standard`, each at least 3 and duplicated onto 0 to 2 in order, or `/dev/null` when it is
/// `None`. With `process_group`, the child leads a new process group whose identifier is its own.
/// It inherits no other descriptor. Returns the child's process identifier; the caller reaps it
/// through [`wait`].
pub(crate) fn spawn(
    executable: &Path,
    arguments: &[&OsStr],
    environment: &[(OsString, OsString)],
    directory: &Path,
    standard: Option<[BorrowedFd<'_>; 3]>,
    process_group: bool,
) -> io::Result<u32> {
    let program = c_string(executable.as_os_str().as_bytes())?;
    let arguments = arguments
        .iter()
        .map(|argument| c_string(argument.as_bytes()))
        .collect::<io::Result<Vec<_>>>()?;
    let environment = environment
        .iter()
        .map(|(name, value)| {
            let mut entry = Vec::with_capacity(name.len() + value.len() + 1);
            entry.extend_from_slice(name.as_bytes());
            entry.push(b'=');
            entry.extend_from_slice(value.as_bytes());
            c_string(&entry)
        })
        .collect::<io::Result<Vec<_>>>()?;
    let directory = c_string(directory.as_os_str().as_bytes())?;
    let argument_vector = pointers(std::iter::once(&program).chain(&arguments));
    let environment_vector = pointers(&environment);

    let mut actions = FileActions::new()?;
    match standard {
        None => {
            actions.open_null(libc::STDIN_FILENO, libc::O_RDONLY)?;
            actions.open_null(libc::STDOUT_FILENO, libc::O_WRONLY)?;
            actions.open_null(libc::STDERR_FILENO, libc::O_WRONLY)?;
        }
        Some([input, output, errors]) => {
            actions.duplicate(input, libc::STDIN_FILENO)?;
            actions.duplicate(output, libc::STDOUT_FILENO)?;
            actions.duplicate(errors, libc::STDERR_FILENO)?;
        }
    }
    actions.change_directory(&directory)?;
    let mut attributes = Attributes::new()?;
    attributes.empty_signal_mask()?;
    attributes.default_pipe_signal()?;
    let mut flags = libc::POSIX_SPAWN_CLOEXEC_DEFAULT
        | libc::POSIX_SPAWN_SETSIGMASK
        | libc::POSIX_SPAWN_SETSIGDEF;
    if process_group {
        attributes.lead_process_group()?;
        flags |= libc::POSIX_SPAWN_SETPGROUP;
    }
    attributes.set_flags(flags)?;

    let mut process: libc::pid_t = 0;
    // SAFETY: `program` and every string the two vectors point to outlive the call; both vectors
    // end with a null pointer; `actions` and `attributes` are initialized and not yet destroyed;
    // `process` is writable. posix_spawn keeps none of these pointers after it returns.
    let result = unsafe {
        libc::posix_spawn(
            &raw mut process,
            program.as_ptr(),
            actions.as_ptr(),
            attributes.as_ptr(),
            argument_vector.as_ptr(),
            environment_vector.as_ptr(),
        )
    };
    status(result)?;
    u32::try_from(process)
        .map_err(|_| io::Error::other("posix_spawn returned an invalid process identifier"))
}

/// The raw wait status of `process` once it has ended, or `None` while it runs when `block` is
/// false. `process` must be a child this process spawned and has not yet reaped; the caller keeps
/// the status, because a reaped identifier may name another process later.
pub(crate) fn wait(process: u32, block: bool) -> io::Result<Option<libc::c_int>> {
    let process =
        libc::pid_t::try_from(process).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let options = if block { 0 } else { libc::WNOHANG };
    loop {
        let mut raw_status: libc::c_int = 0;
        // SAFETY: `raw_status` is writable for the call, and `process` names a direct child.
        match unsafe { libc::waitpid(process, &raw mut raw_status, options) } {
            0 => return Ok(None),
            -1 => {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error);
                }
            }
            _ => return Ok(Some(raw_status)),
        }
    }
}

/// The spawn file actions, destroyed exactly once when dropped. On macOS the object is an opaque
/// pointer, so moving this owner moves only that pointer.
struct FileActions(libc::posix_spawn_file_actions_t);

impl FileActions {
    fn new() -> io::Result<Self> {
        let mut actions = MaybeUninit::<libc::posix_spawn_file_actions_t>::uninit();
        // SAFETY: init writes an initialized object through a pointer to storage owned here.
        status(unsafe { libc::posix_spawn_file_actions_init(actions.as_mut_ptr()) })?;
        // SAFETY: a successful init initialized `actions`.
        Ok(Self(unsafe { actions.assume_init() }))
    }

    fn open_null(&mut self, descriptor: libc::c_int, flags: libc::c_int) -> io::Result<()> {
        // SAFETY: the object is initialized and not destroyed; the path is a static NUL-terminated
        // string, which the call copies.
        status(unsafe {
            libc::posix_spawn_file_actions_addopen(
                &raw mut self.0,
                descriptor,
                NULL_DEVICE.as_ptr(),
                flags,
                0,
            )
        })
    }

    /// Duplicates `source` onto `target` in the child, where the copy is not close-on-exec.
    fn duplicate(&mut self, source: BorrowedFd<'_>, target: libc::c_int) -> io::Result<()> {
        // SAFETY: the object is initialized and not destroyed. The action records only the number
        // of `source`, which the caller keeps open until the spawn that uses this action returns.
        status(unsafe {
            libc::posix_spawn_file_actions_adddup2(&raw mut self.0, source.as_raw_fd(), target)
        })
    }

    fn change_directory(&mut self, directory: &CStr) -> io::Result<()> {
        // SAFETY: the object is initialized and not destroyed; `directory` is NUL-terminated and
        // the call copies it.
        status(unsafe { posix_spawn_file_actions_addchdir_np(&raw mut self.0, directory.as_ptr()) })
    }

    fn as_ptr(&self) -> *const libc::posix_spawn_file_actions_t {
        &raw const self.0
    }
}

impl Drop for FileActions {
    fn drop(&mut self) {
        // SAFETY: the object was initialized by `new` and this is its only destruction.
        unsafe {
            libc::posix_spawn_file_actions_destroy(&raw mut self.0);
        }
    }
}

/// The spawn attributes, destroyed exactly once when dropped. On macOS the object is an opaque
/// pointer, so moving this owner moves only that pointer.
struct Attributes(libc::posix_spawnattr_t);

impl Attributes {
    fn new() -> io::Result<Self> {
        let mut attributes = MaybeUninit::<libc::posix_spawnattr_t>::uninit();
        // SAFETY: init writes an initialized object through a pointer to storage owned here.
        status(unsafe { libc::posix_spawnattr_init(attributes.as_mut_ptr()) })?;
        // SAFETY: a successful init initialized `attributes`.
        Ok(Self(unsafe { attributes.assume_init() }))
    }

    /// Gives the child an empty signal mask, as the standard library's spawn does, so a signal the
    /// spawning thread blocks, such as `SIGTERM` around a signal-handling thread, still reaches it;
    /// the flag `POSIX_SPAWN_SETSIGMASK` applies it.
    fn empty_signal_mask(&mut self) -> io::Result<()> {
        let mut signals = MaybeUninit::<libc::sigset_t>::uninit();
        // SAFETY: both calls receive a pointer to a signal set owned here; sigemptyset initializes
        // it before posix_spawnattr_setsigmask reads it, and that call copies it.
        unsafe {
            if libc::sigemptyset(signals.as_mut_ptr()) != 0 {
                return Err(io::Error::last_os_error());
            }
            status(libc::posix_spawnattr_setsigmask(
                &raw mut self.0,
                signals.as_ptr(),
            ))
        }
    }

    /// Resets `SIGPIPE` to its default action in the child, as the standard library's spawn does;
    /// the flag `POSIX_SPAWN_SETSIGDEF` applies it.
    fn default_pipe_signal(&mut self) -> io::Result<()> {
        let mut signals = MaybeUninit::<libc::sigset_t>::uninit();
        // SAFETY: each call receives a pointer to a signal set owned here; sigemptyset initializes
        // it before sigaddset and posix_spawnattr_setsigdefault read it, and the last copies it.
        unsafe {
            if libc::sigemptyset(signals.as_mut_ptr()) != 0
                || libc::sigaddset(signals.as_mut_ptr(), libc::SIGPIPE) != 0
            {
                return Err(io::Error::last_os_error());
            }
            status(libc::posix_spawnattr_setsigdefault(
                &raw mut self.0,
                signals.as_ptr(),
            ))
        }
    }

    /// Makes the child the leader of a new process group, whose identifier is its own; the flag
    /// `POSIX_SPAWN_SETPGROUP` applies it.
    fn lead_process_group(&mut self) -> io::Result<()> {
        // SAFETY: the object is initialized and not destroyed; group 0 names the child itself.
        status(unsafe { libc::posix_spawnattr_setpgroup(&raw mut self.0, 0) })
    }

    fn set_flags(&mut self, flags: libc::c_int) -> io::Result<()> {
        let flags = libc::c_short::try_from(flags)
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        // SAFETY: the object is initialized and not destroyed.
        status(unsafe { libc::posix_spawnattr_setflags(&raw mut self.0, flags) })
    }

    fn as_ptr(&self) -> *const libc::posix_spawnattr_t {
        &raw const self.0
    }
}

impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: the object was initialized by `new` and this is its only destruction.
        unsafe {
            libc::posix_spawnattr_destroy(&raw mut self.0);
        }
    }
}

fn c_string(bytes: &[u8]) -> io::Result<CString> {
    CString::new(bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "a spawn string holds a NUL byte",
        )
    })
}

/// A null-terminated vector of pointers into `strings`, which must outlive every use of it.
fn pointers<'a>(strings: impl IntoIterator<Item = &'a CString>) -> Vec<*mut libc::c_char> {
    strings
        .into_iter()
        .map(|string| string.as_ptr().cast_mut())
        .chain(std::iter::once(ptr::null_mut()))
        .collect()
}

/// A posix_spawn family result: zero, or the error number itself.
fn status(result: libc::c_int) -> io::Result<()> {
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(result))
    }
}

#[cfg(test)]
#[deny(unsafe_code)]
mod tests;
