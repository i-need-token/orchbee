//! Starts one Unix command through the C library's `posix_spawn`, by nix's safe wrapper, so the
//! child begins with an empty signal mask and `SIGPIPE` at its default action. The standard
//! library's spawn passes the spawning thread's mask on, so a command started while that thread
//! blocks `SIGINT` and `SIGTERM` to capture them would ignore both, and its group's settlement could
//! only end it by `SIGKILL` after the grace. The root leads a new process group, as
//! [`CommandChild::spawn`](super::CommandChild::spawn) places it, and reads `/dev/null` as its
//! input. On macOS it also passes `POSIX_SPAWN_CLOEXEC_DEFAULT`, since that system sets
//! close-on-exec only after a descriptor exists, so the child receives its three standard streams
//! and no other descriptor, however many threads of this process create descriptors meanwhile.
use super::CommandChild;
use crate::CommandSpawnFailure;
use nix::{
    spawn::{PosixSpawnAttr, PosixSpawnFileActions, PosixSpawnFlags, posix_spawn},
    sys::signal::{SigSet, Signal},
};
use rustix::process::Pid;
use std::{
    ffi::{CString, OsString},
    fs::File,
    io,
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::ffi::{OsStrExt, OsStringExt},
    },
    process::Command,
};

/// The environment a command launched this way receives. The standard library does not report
/// whether `Command::env_clear` was called, so the caller states it here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpawnEnvironment {
    /// This process's environment with the variables the command sets and removes.
    Inherited,
    /// Exactly the variables the command sets, as after `Command::env_clear`.
    Cleared,
}

/// Starts `command` with its program, arguments and the `environment` it states, its standard
/// output and error on `stdout` and `stderr`. A working directory is refused, since `posix_spawn`
/// cannot set one portably; the command names its paths instead.
pub(super) fn spawn(
    command: &Command,
    environment: SpawnEnvironment,
    stdout: &OwnedFd,
    stderr: &OwnedFd,
) -> Result<CommandChild, CommandSpawnFailure> {
    let not_created = |error: io::Error| CommandSpawnFailure {
        error,
        created: false,
    };
    if command.get_current_dir().is_some() {
        return Err(not_created(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a command launched with an empty signal mask takes no working directory",
        )));
    }
    let program = c_string(command.get_program().as_bytes().to_vec()).map_err(not_created)?;
    let arguments = std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|argument| c_string(argument.as_bytes().to_vec()))
        .collect::<io::Result<Vec<_>>>()
        .map_err(not_created)?;
    let variables = variables(command, environment).map_err(not_created)?;
    // Opened here, close-on-exec, and duplicated onto the child's input, so the launch needs no
    // open action.
    let input = File::open("/dev/null").map_err(not_created)?;
    let launch = || -> nix::Result<nix::unistd::Pid> {
        let mut actions = PosixSpawnFileActions::init()?;
        actions.add_dup2(input.as_raw_fd(), 0)?;
        actions.add_dup2(stdout.as_raw_fd(), 1)?;
        actions.add_dup2(stderr.as_raw_fd(), 2)?;
        let mut attributes = PosixSpawnAttr::init()?;
        attributes.set_flags(
            PosixSpawnFlags::POSIX_SPAWN_SETPGROUP
                | PosixSpawnFlags::POSIX_SPAWN_SETSIGMASK
                | PosixSpawnFlags::POSIX_SPAWN_SETSIGDEF
                | close_other_descriptors(),
        )?;
        attributes.set_pgroup(nix::unistd::Pid::from_raw(0))?;
        attributes.set_sigmask(&SigSet::empty())?;
        let mut defaults = SigSet::empty();
        defaults.add(Signal::SIGPIPE);
        attributes.set_sigdefault(&defaults)?;
        posix_spawn(
            program.as_c_str(),
            &actions,
            &attributes,
            &arguments,
            &variables,
        )
    };
    // A failed `posix_spawn` leaves no process: macOS creates none, and the C library reaps a
    // child whose program could not run before it reports the error.
    let root = launch().map_err(|errno| not_created(errno.into()))?;
    let root = Pid::from_raw(root.as_raw()).ok_or_else(|| CommandSpawnFailure {
        error: io::Error::other("posix_spawn reported no process ID"),
        created: true,
    })?;
    Ok(CommandChild::spawned(root))
}

/// The flag that closes every descriptor but the standard streams on exec. macOS needs it, since its
/// descriptors are close-on-exec only once created; Linux creates them close-on-exec already.
fn close_other_descriptors() -> PosixSpawnFlags {
    #[cfg(target_os = "macos")]
    {
        PosixSpawnFlags::from_bits_retain(nix::libc::POSIX_SPAWN_CLOEXEC_DEFAULT)
    }
    #[cfg(not(target_os = "macos"))]
    {
        PosixSpawnFlags::empty()
    }
}

/// The variables the child receives, as name and value pairs.
pub(super) fn variable_pairs(
    command: &Command,
    environment: SpawnEnvironment,
) -> Vec<(OsString, OsString)> {
    let mut variables: Vec<(OsString, OsString)> = match environment {
        SpawnEnvironment::Inherited => std::env::vars_os().collect(),
        SpawnEnvironment::Cleared => Vec::new(),
    };
    for (name, value) in command.get_envs() {
        variables.retain(|(present, _)| present != name);
        if let Some(value) = value {
            variables.push((name.to_owned(), value.to_owned()));
        }
    }
    variables
}

/// The `NAME=value` entries the child receives.
fn variables(command: &Command, environment: SpawnEnvironment) -> io::Result<Vec<CString>> {
    variable_pairs(command, environment)
        .into_iter()
        .map(|(name, value)| {
            let mut entry = name.into_vec();
            entry.push(b'=');
            entry.extend_from_slice(value.as_bytes());
            c_string(entry)
        })
        .collect()
}

fn c_string(bytes: Vec<u8>) -> io::Result<CString> {
    CString::new(bytes).map_err(|_| io::ErrorKind::InvalidInput.into())
}
