//! macOS and Linux install effects: declared inputs, no-follow source admission, the owned
//! execution of the staged Client's version check and of the published copy's completion, and the
//! distribution capsule check. No download runs here: the feed, curl and tar ports refuse without
//! observation, so `update` has nothing to fetch with.
use super::{Context, acquire::system_proxy::Settings};
use orchbee_local_runtime::CommandOutput;
use std::{
    fs::{File, OpenOptions},
    io,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

/// The output a command's result keeps, as on Windows.
const OUTPUT_LIMIT: usize = 64 * 1024;

/// No feed is read here, since no download runs.
pub(super) fn feed_source() -> Option<String> {
    None
}

/// `ORCHBEE_INSTALL_DIR`, or `~/.local/share/orchbee` for release and `~/.local/share/<stem>` for a
/// development or test build, so only release names the release store. Session start reads the
/// same root through the same owner.
pub(super) fn install_root() -> io::Result<PathBuf> {
    orchbee_local_runtime::release_store_root(|name| std::env::var_os(name))
}

/// Opens a local feed or release manifest without following a link at its name; another kind of
/// file is no manifest.
pub(super) fn open_manifest(path: &Path) -> io::Result<Option<File>> {
    let file = open_no_follow(path)?;
    Ok(file.metadata()?.is_file().then_some(file))
}

/// Opens one file of a local release source to copy it: a regular file that is not a link. A
/// source needs no owner or mode, unlike the store: nothing read from it is trusted until its copy
/// in the private stage matches the signed manifest, so a source npm unpacked anywhere installs.
pub(super) fn open_source_file(path: &Path) -> io::Result<File> {
    let file = open_no_follow(path)?;
    if file.metadata()?.is_file() {
        Ok(file)
    } else {
        Err(io::ErrorKind::InvalidInput.into())
    }
}

/// Opens `path` for reading; a link at its last component refuses, and a FIFO never blocks the open.
fn open_no_follow(path: &Path) -> io::Result<File> {
    use nix::fcntl::OFlag;
    OpenOptions::new()
        .read(true)
        .custom_flags((OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK).bits())
        .open(path)
}

pub(super) fn curl_command() -> io::Result<Command> {
    Err(io::ErrorKind::Unsupported.into())
}

/// No per-user system proxy is read here, where no download runs.
pub(super) fn system_proxy() -> Option<Settings> {
    None
}

pub(super) fn tar_command() -> io::Result<Command> {
    Err(io::ErrorKind::Unsupported.into())
}

/// Runs a command of the install, the staged Client's version check or the published copy's
/// completion, until it ends, the run's deadline passes or Ctrl+C arrives; its process group
/// settles before the result.
pub(super) fn execute(command: Command, context: &mut Context) -> io::Result<CommandOutput> {
    Ok(orchbee_local_runtime::run_unix_command(
        command,
        OUTPUT_LIMIT,
        Some(context.deadline),
        || context.interrupted(),
    ))
}

pub(super) fn execute_contained(
    _command: Command,
    _context: &mut Context,
) -> io::Result<CommandOutput> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn execute_transfer(
    _command: Command,
    _deadline: Instant,
    _context: &mut Context,
) -> io::Result<CommandOutput> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn execute_capturing(
    _command: Command,
    _limit: usize,
    _context: &mut Context,
) -> io::Result<CommandOutput> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn verify_capsule(directory: &Path, expected: &str) -> io::Result<bool> {
    Ok(orchbee_agent_integration_writer::verify_distribution_capsule(directory, expected))
}
