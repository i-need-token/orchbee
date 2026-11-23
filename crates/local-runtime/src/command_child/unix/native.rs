//! Starts one command through the reviewed native adapter on macOS, so the child receives its three
//! standard streams and no other descriptor, however many threads of this process create
//! descriptors meanwhile. The root leads a new process group, as
//! [`CommandChild::spawn`](super::CommandChild::spawn) places it, and [`spawn_detached`] starts a
//! child whose owner may leave it running.
use super::{CommandChild, SpawnEnvironment, spawn::variable_pairs};
use crate::{CommandSpawnFailure, retained_spawn_native};
use rustix::process::Pid;
use std::{
    ffi::{OsStr, OsString},
    io,
    os::fd::{AsFd, AsRawFd, OwnedFd},
    path::{Path, PathBuf},
    process::Command,
};

/// Starts `command` as the leader of its own process group, with its three `streams` as its standard
/// input, output and error, in the environment `environment` names and in the working directory it
/// sets, or this process's when it sets none, and answers the child's process identifier. A relative
/// program is refused, because the adapter starts an absolute path and searches for none.
fn start(
    command: &Command,
    environment: SpawnEnvironment,
    [input, output, errors]: [OwnedFd; 3],
) -> Result<u32, CommandSpawnFailure> {
    let not_created = |error: io::Error| CommandSpawnFailure {
        error,
        created: false,
    };
    let executable = Path::new(command.get_program());
    if executable.is_relative() {
        return Err(not_created(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a command started through the native adapter needs an absolute program path",
        )));
    }
    let directory: PathBuf = match command.get_current_dir() {
        Some(directory) => directory.to_owned(),
        None => std::env::current_dir().map_err(not_created)?,
    };
    let input = above_standard(input).map_err(not_created)?;
    let output = above_standard(output).map_err(not_created)?;
    let errors = above_standard(errors).map_err(not_created)?;
    let arguments: Vec<&OsStr> = command.get_args().collect();
    let variables: Vec<(OsString, OsString)> = variable_pairs(command, environment);
    let id = retained_spawn_native::spawn(
        executable,
        &arguments,
        &variables,
        &directory,
        Some([input.as_fd(), output.as_fd(), errors.as_fd()]),
        true,
        false,
    )
    .map_err(not_created)?;
    Ok(id)
}

/// Starts `command` as [`start`] does, as the owner of a group root that a `CommandChild` settles.
pub(super) fn spawn(
    command: &Command,
    environment: SpawnEnvironment,
    streams: [OwnedFd; 3],
) -> Result<CommandChild, CommandSpawnFailure> {
    let id = start(command, environment, streams)?;
    let root = i32::try_from(id)
        .ok()
        .and_then(Pid::from_raw)
        .ok_or_else(|| CommandSpawnFailure {
            error: io::Error::other("posix_spawn returned an invalid process identifier"),
            created: true,
        })?;
    Ok(CommandChild::spawned(root))
}

/// Starts `command` as [`spawn`] does, for a child its owner may leave running: the caller waits for
/// its identifier, and nothing signals or reaps it when the caller drops its owner.
pub(super) fn spawn_detached(
    command: &Command,
    environment: SpawnEnvironment,
    streams: [OwnedFd; 3],
) -> io::Result<u32> {
    start(command, environment, streams).map_err(|failure| failure.error)
}

/// `stream` itself when it is at least 3, or a copy of it at or above 3. The adapter duplicates the
/// streams onto 0 to 2 in order, so a source below 3 could be overwritten before it is read.
fn above_standard(stream: OwnedFd) -> io::Result<OwnedFd> {
    if stream.as_raw_fd() >= 3 {
        return Ok(stream);
    }
    rustix::io::fcntl_dupfd_cloexec(&stream, 3).map_err(io::Error::from)
}
