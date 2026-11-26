//! macOS and Linux update effects: declared inputs, no-follow source admission, the system `curl`
//! and `tar` in `/usr/bin`, owned execution and the distribution capsule check. Every child starts
//! through the local runtime's `posix_spawn` launch with an empty signal mask, since the update's
//! signal owner blocks `SIGINT` and `SIGTERM` on the thread that starts it, in its own process
//! group, which settles before the result.
use super::{Context, acquire::system_proxy::Settings};
use orchbee_local_runtime::{CommandOutput, SpawnEnvironment, run_unmasked_unix_command};
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

/// The system programs a download runs, by their full paths, so a same-named program earlier on
/// `PATH`, such as one a package manager installed, never stands in for them.
const CURL: &str = "/usr/bin/curl";
const TAR: &str = "/usr/bin/tar";

/// The proxy variables curl reads for an HTTPS transfer, under both spellings it looks up, and the
/// only part of this process's environment it receives (curl(1), "ENVIRONMENT"). curl ignores an
/// upper-case `HTTP_PROXY`, and an HTTPS transfer never reads `http_proxy`.
const PROXY_VARIABLES: [&str; 6] = [
    "https_proxy",
    "HTTPS_PROXY",
    "all_proxy",
    "ALL_PROXY",
    "no_proxy",
    "NO_PROXY",
];

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

/// Raises this process's soft limit on open descriptors to its hard limit, at most `OPEN_MAX`
/// (10 240), the highest soft limit macOS grants without a system setting. A stage holds every
/// folder it creates open until it publishes, and its cleanup opens the whole stage at once; a
/// window version's application bundle holds hundreds of folders, more than the 256 a Terminal
/// shell or install.sh starts with. Only the window install calls it, and it starts no Session
/// afterwards, so no terminal inherits the raised limit.
#[cfg(target_os = "macos")]
pub(super) fn admit_bundle_folders() {
    use nix::sys::resource::{Resource, getrlimit, rlim_t, setrlimit};
    const OPEN_MAX: rlim_t = 10_240;
    if let Ok((soft, hard)) = getrlimit(Resource::RLIMIT_NOFILE) {
        let wanted = hard.min(OPEN_MAX);
        if soft < wanted {
            // A limit that cannot rise stays, and the install fails at its first refused open.
            let _raised = setrlimit(Resource::RLIMIT_NOFILE, wanted, hard);
        }
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

/// The system curl with a cleared environment holding only the [`PROXY_VARIABLES`] this process
/// sets, so no curlrc location (`HOME`, `CURL_HOME`), credential, `SSLKEYLOGFILE` or other TLS
/// setting reaches it.
pub(super) fn curl_command() -> io::Result<Command> {
    let mut command = Command::new(CURL);
    command.env_clear();
    for name in PROXY_VARIABLES {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    Ok(command)
}

/// No system proxy is read here: the proxy variables are curl's whole proxy configuration, as for
/// any command-line program.
pub(super) fn system_proxy() -> Option<Settings> {
    None
}

/// The system tar with an empty environment. Its arguments are the caller's; it receives no working
/// directory, which the `posix_spawn` launch cannot set, so the caller names the folder with `-C`.
pub(super) fn tar_command() -> io::Result<Command> {
    let mut command = Command::new(TAR);
    command.env_clear();
    Ok(command)
}

/// Runs a command of the install, the staged Client's version check or the published copy's
/// completion, with this process's environment and the command's changes to it, until it ends, the
/// run's deadline passes or Ctrl+C arrives.
pub(super) fn execute(command: Command, context: &mut Context) -> io::Result<CommandOutput> {
    Ok(run_unmasked_unix_command(
        command,
        SpawnEnvironment::Inherited,
        OUTPUT_LIMIT,
        Some(context.deadline),
        || context.interrupted(),
    ))
}

/// Like [`execute`] for a download or unpack child (`curl`, `tar`), which receives exactly the
/// environment its command sets. Unlike the Windows Job, nothing ends it when this Client is killed
/// outright; it then ends at its own limit, and the next update writes a new archive and a new
/// unpacked folder rather than the files it holds open.
pub(super) fn execute_contained(
    command: Command,
    context: &mut Context,
) -> io::Result<CommandOutput> {
    execute_capturing(command, OUTPUT_LIMIT, context)
}

/// Like [`execute_contained`], but captures up to `limit` bytes of standard output.
pub(super) fn execute_capturing(
    command: Command,
    limit: usize,
    context: &mut Context,
) -> io::Result<CommandOutput> {
    Ok(run_unmasked_unix_command(
        command,
        SpawnEnvironment::Cleared,
        limit,
        Some(context.deadline),
        || context.interrupted(),
    ))
}

/// Like [`execute_contained`] for the archive transfer, which ends at its own `deadline` rather
/// than the command's; Ctrl+C still ends it at once.
pub(super) fn execute_transfer(
    command: Command,
    deadline: Instant,
    context: &mut Context,
) -> io::Result<CommandOutput> {
    Ok(run_unmasked_unix_command(
        command,
        SpawnEnvironment::Cleared,
        OUTPUT_LIMIT,
        Some(deadline),
        || context.interrupted(),
    ))
}

pub(super) fn verify_capsule(directory: &Path, expected: &str) -> io::Result<bool> {
    Ok(orchbee_agent_integration_writer::verify_distribution_capsule(directory, expected))
}
