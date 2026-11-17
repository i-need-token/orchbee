//! Store start: a release or test Client outside the release store that finds no Session asks the
//! store's active version to start it, through that version's public `server start`, and then joins
//! through its own activation exactly as before. The store's Client starts the Session's programs
//! from its own version folder, so a window's files stay replaceable while its terminals run, and a
//! newer store never loses the start to an older Client. Whatever the store's Client answers, the
//! caller's own activation stays the only verdict: it joins the Session that Client started, reports
//! the refusal it meets, or starts its own Server as before. Each start decision writes one
//! `session-start-source` record with tokens and counts only. Windows and macOS run it; Linux keeps
//! starting its own cohort.
//!
//! On macOS the window application's bundled Client is the only installer a copy dragged from the
//! disk image has. Where no store version is active, it first installs its own release directory
//! with its public `install`, the route Setup and install.sh take, which also links the command,
//! makes the window store, and hands the start to the version it installed; one `store-install`
//! record says how that went. An install that fails or outlasts its share of the deadline
//! changes nothing about this start: the Client starts its own cohort as before and tries again
//! at the next start.
use super::{ActivationFailure, StartupFailure};
use crate::local_client::CLIENT_VERSION;
use orchbee_local_ipc::ChildOutputReader;
use orchbee_local_runtime::{
    ActiveRelease, CohortArtifact, EnvironmentKind, LogLevel, LogValue,
    ModeCheckedLocalSessionRendezvous, RendezvousError, SessionScope, private_filesystem,
};
#[cfg(target_os = "macos")]
use std::os::unix::process::CommandExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    cmp::Ordering,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

/// The most standard output kept from the store's Client; its `server start` document is far
/// smaller. Output past the bound is read and dropped, so the Client never blocks on a full pipe.
const OUTPUT_LIMIT: usize = 64 * 1024;
/// Further attempts after a pointer read fails other than by absence, and the wait before each:
/// a replacement holds the pointer for an instant while it renames the new one into place.
const POINTER_RETRIES: u32 = 3;
const POINTER_RETRY_PAUSE: Duration = Duration::from_millis(20);
/// How often the wait for the store's Client checks its exit and the deadline.
const POLL: Duration = Duration::from_millis(10);
/// `CREATE_NO_WINDOW`: the store's Client runs without a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// The part of the start deadline an install of this Client's own release directory leaves for
/// the store Client's start and this Client's join; the install may use the rest.
const INSTALL_RESERVE: Duration = Duration::from_secs(8);
/// The `error` codes an `orchbee.install` document names (`apps/orchbee/src/cli/schemas/
/// orchbee.install.json`), as the tokens its record carries; any other code, or another document,
/// records `other`.
const INSTALL_ERRORS: [&str; 23] = [
    "development-build",
    "not-release-directory",
    "signing-keys-unavailable",
    "signature-untrusted",
    "signature-invalid",
    "manifest-invalid",
    "download",
    "checksum",
    "storage",
    "install-location",
    "busy",
    "cohort-invalid",
    "version-conflict",
    "downgrade-refused",
    "version-held",
    "other-architecture",
    "smoke-failed",
    "activation",
    "activation-unknown",
    "cancelled",
    "timeout",
    "physical-unknown",
    "internal",
];

/// Why a Session start stays with this Client's own cohort; each token names the reason in the log.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Own {
    /// This Client runs from a store version folder, so it is the store's Client and never hands a
    /// start on: a Client it starts cannot start another.
    StoreClient,
    /// A development build always starts its staged cohort.
    Development,
    NoStoreRoot,
    /// No pointer, or bytes that are no pointer record: no active version.
    NoPointer,
    PointerUnreadable,
    /// The active version is lower than this Client, which never starts a Server older than itself.
    StoreLower,
    ClientMissing,
}

impl Own {
    const fn token(self) -> &'static str {
        match self {
            Self::StoreClient => "store-client",
            Self::Development => "development",
            Self::NoStoreRoot => "no-store-root",
            Self::NoPointer => "no-pointer",
            Self::PointerUnreadable => "pointer-unreadable",
            Self::StoreLower => "store-lower",
            Self::ClientMissing => "client-missing",
        }
    }

    /// The ordinary starts of an environment without a store are not warnings.
    const fn level(self) -> LogLevel {
        match self {
            Self::StoreClient | Self::Development | Self::NoStoreRoot | Self::NoPointer => {
                LogLevel::Info
            }
            Self::PointerUnreadable | Self::StoreLower | Self::ClientMissing => LogLevel::Warning,
        }
    }
}

/// Which cohort starts the Session: the store Client at this absolute path, the version this Client
/// installs first, or this Client's own.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Decision {
    Store(PathBuf),
    /// No store version is active and this Client may install its own release directory: the
    /// Session then starts as the store's start rule decides afterwards.
    Install,
    Own(Own),
}

/// The store start rule over the facts it reads, in order: the build's environment; this Client's
/// own folder, which may be a store version folder; the store root; the active version the store's
/// pointer names; this Client's version; and whether the store's Client is present in that
/// version's folder. A fact is read only when every earlier one allows the store, so a development
/// build or a store Client reads nothing. Without an active version, `installs` says whether this
/// Client installs its own release directory first, as a macOS window application's bundled
/// Client does.
pub(super) fn decide(
    kind: EnvironmentKind,
    own_directory: &Path,
    own_version: &str,
    installs: bool,
    store_root: impl FnOnce() -> io::Result<PathBuf>,
    active: impl FnOnce(&Path) -> io::Result<Option<ActiveRelease>>,
    client_present: impl FnOnce(&Path) -> bool,
) -> Decision {
    if kind == EnvironmentKind::Development {
        return Decision::Own(Own::Development);
    }
    if orchbee_local_runtime::is_release_version_directory(own_directory) {
        return Decision::Own(Own::StoreClient);
    }
    let Ok(root) = store_root() else {
        return Decision::Own(Own::NoStoreRoot);
    };
    let release = match active(&root) {
        Ok(Some(release)) => release,
        Ok(None) if installs => return Decision::Install,
        Ok(None) => return Decision::Own(Own::NoPointer),
        Err(_) => return Decision::Own(Own::PointerUnreadable),
    };
    if orchbee_local_runtime::compare_product_versions(&release.version, own_version)
        == Ordering::Less
    {
        return Decision::Own(Own::StoreLower);
    }
    // The Client's file name carries this build's environment stem, so a build only ever names a
    // Client of its own environment, in the store its own stem's root holds.
    let client = release.directory.join(CohortArtifact::Client.file_name());
    if !client_present(&client) {
        return Decision::Own(Own::ClientMissing);
    }
    Decision::Store(client)
}

/// Before this Client's own activation: when the selected Session's Client endpoint is absent and
/// the rule chooses the store, runs the store's Client and waits for it until `deadline`. Answers
/// whether that Client reports it started the Session. At the deadline the answer is `TimedOut` and
/// the store's Client keeps running.
pub(super) fn start_if_absent(
    base: &Path,
    scope: &str,
    explicit: Option<&str>,
    own_client: &Path,
    deadline: Instant,
) -> Result<bool, ActivationFailure> {
    if !session_absent(base, scope) {
        return Ok(false);
    }
    let own_directory = own_client.parent().unwrap_or(own_client);
    let decide_now = |installs: bool| {
        decide(
            orchbee_local_runtime::environment_kind(),
            own_directory,
            CLIENT_VERSION,
            installs,
            || orchbee_local_runtime::release_store_root(|name| std::env::var_os(name)),
            read_active,
            |client| admit(client).is_ok(),
        )
    };
    let installs = cfg!(target_os = "macos")
        && orchbee_local_runtime::bundle_of_cohort(own_directory).is_some();
    let decision = match decide_now(installs) {
        // The rule runs again over the store the install left, and never installs twice.
        Decision::Install if install_own(own_client, deadline) => decide_now(false),
        decided => decided,
    };
    let client = match decision {
        Decision::Store(client) => client,
        Decision::Own(reason) => return Ok(own(reason)),
        // The install did not complete, so the store still has no active version.
        Decision::Install => return Ok(own(Own::NoPointer)),
    };
    match run(&client, explicit, deadline) {
        Run::Exited(status, output) if status.success() => {
            record(LogLevel::Info, "store", None, None);
            Ok(reports_started(&output))
        }
        Run::Exited(status, _) => {
            let code = status.code().map(|code| u64::from(code.cast_unsigned()));
            record(LogLevel::Warning, "own", Some("store-start-failed"), code);
            Ok(false)
        }
        Run::Failed => {
            record(LogLevel::Warning, "own", Some("store-start-failed"), None);
            Ok(false)
        }
        Run::Deadline => {
            record(LogLevel::Info, "store", None, None);
            Err(ActivationFailure::bare(StartupFailure::TimedOut))
        }
    }
}

/// Whether the Session has no Client endpoint: its runtime folder is absent, or present without the
/// endpoint. Any other observation leaves the start to the caller's activation, which reports it.
/// The folder's handles close before the store's Client runs.
fn session_absent(base: &Path, scope: &str) -> bool {
    let Ok(scope) = SessionScope::parse(scope) else {
        return false;
    };
    match ModeCheckedLocalSessionRendezvous::validate_mode_checked(base, scope) {
        Ok(session) => matches!(session.client_socket_exists(), Ok(false)),
        Err(RendezvousError::System { source, .. }) => source.kind() == io::ErrorKind::NotFound,
        Err(_) => false,
    }
}

/// The store's active version, read again after a failure other than absence.
fn read_active(root: &Path) -> io::Result<Option<ActiveRelease>> {
    let mut attempt = 0;
    loop {
        match orchbee_local_runtime::active_release(root) {
            Err(_) if attempt < POINTER_RETRIES => {
                attempt += 1;
                thread::sleep(POINTER_RETRY_PAUSE);
            }
            read => return read,
        }
    }
}

/// Pins every folder down to the Client's version folder, which must be private, and the Client,
/// which must be a private regular file with one link. On Windows no folder on the path can be
/// renamed or replaced by a link while the handles live; on macOS the private rule keeps every
/// other account from doing so. Either way the program started is the admitted file inside
/// `<store root>/versions/<version>/`.
fn admit(client: &Path) -> io::Result<Vec<File>> {
    let directory = client.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let mut pins = private_filesystem::open_tree(directory)?;
    pins.push(private_filesystem::open_file(client, false, false)?);
    Ok(pins)
}

enum Run {
    Exited(ExitStatus, Vec<u8>),
    /// The Client could not be admitted, started or watched.
    Failed,
    Deadline,
}

/// Runs `[--session <name>] server start --format json` with the store's Client, pinned while it
/// starts, as [`wait_for`] runs a command.
fn run(client: &Path, explicit: Option<&str>, deadline: Instant) -> Run {
    let Ok(pins) = admit(client) else {
        return Run::Failed;
    };
    let mut command = Command::new(client);
    if let Some(name) = explicit {
        command.args(["--session", name]);
    }
    command.args(["server", "start", "--format", "json"]);
    wait_for(command, pins, deadline)
}

/// Runs this Client's own `install --format json`, as [`wait_for`] runs a command, and
/// waits for it until the deadline leaves [`INSTALL_RESERVE`]. Answers whether it exited 0 with an
/// `orchbee.install` document that reports the version activated, and writes one `store-install`
/// record: `installed`; `failed` with the document's `error` token and the exit `status`, or with
/// `start-failed` when it could not be started or watched; or `unfinished` when its share of the
/// deadline ran out first, in which case it keeps running and completes on its own, as a store
/// Client's start does. Its own folder needs no admission: it is the program this process runs.
fn install_own(own_client: &Path, deadline: Instant) -> bool {
    let mut command = Command::new(own_client);
    command.args(["install", "--format", "json"]);
    let until = deadline
        .checked_sub(INSTALL_RESERVE)
        .unwrap_or_else(Instant::now);
    let mut fields = Vec::with_capacity(3);
    let installed = match wait_for(command, Vec::new(), until) {
        Run::Exited(status, output) => {
            let document = serde_json::from_slice::<serde_json::Value>(&output).ok();
            let installed = status.success()
                && document.as_ref().is_some_and(|document| {
                    document["schema"] == "orchbee.install"
                        && document["result"]["activated"] == true
                });
            if !installed {
                let error = document
                    .as_ref()
                    .and_then(|document| document["result"]["error"].as_str())
                    .and_then(|code| INSTALL_ERRORS.into_iter().find(|known| *known == code))
                    .unwrap_or("other");
                fields.push(("error", LogValue::Token(error)));
                if let Some(code) = status.code() {
                    fields.push(("status", LogValue::Count(u64::from(code.cast_unsigned()))));
                }
            }
            installed
        }
        Run::Failed => {
            fields.push(("error", LogValue::Token("start-failed")));
            false
        }
        Run::Deadline => {
            fields.insert(0, ("outcome", LogValue::Token("unfinished")));
            orchbee_local_runtime::log_event(LogLevel::Warning, "store-install", &fields);
            return false;
        }
    };
    let (level, outcome) = if installed {
        (LogLevel::Info, "installed")
    } else {
        (LogLevel::Warning, "failed")
    };
    fields.insert(0, ("outcome", LogValue::Token(outcome)));
    orchbee_local_runtime::log_event(level, "store-install", &fields);
    installed
}

/// Runs `command`, which inherits this process's environment, so it selects the same Session and
/// store, except the relaunch protocol's variable, which no process a Client starts receives, and
/// waits for it until `deadline`; `pins` hold its program in place until it exists. It gets null
/// standard input and error, a bounded standard output, and a working folder no version owns, since
/// a Session it starts outlives this Client: the system folder on Windows, where it also gets no
/// console window, and `/` on macOS, where it leads a process group of its own, so a terminal's
/// interrupt meant for this Client never reaches it.
fn wait_for(mut command: Command, pins: Vec<File>, deadline: Instant) -> Run {
    #[cfg(windows)]
    let Ok(directory) = winsafe::GetSystemDirectory() else {
        return Run::Failed;
    };
    #[cfg(target_os = "macos")]
    let directory = "/";
    let Ok((mut reader, writer)) = ChildOutputReader::pipe() else {
        return Run::Failed;
    };
    command
        .env_remove(orchbee_local_runtime::RELAUNCH_PROTOCOL_VARIABLE)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    #[cfg(target_os = "macos")]
    command.process_group(0);
    let spawned = command.spawn();
    // The command holds the parent's copy of the output pipe's writer; the image is mapped once
    // the process exists, so the pins have done their part.
    drop(command);
    drop(pins);
    let Ok(mut child) = spawned else {
        return Run::Failed;
    };
    let mut output = Vec::new();
    loop {
        let drained = drain(&mut reader, &mut output);
        match child.try_wait() {
            Ok(Some(status)) => {
                let _ = drain(&mut reader, &mut output);
                return Run::Exited(status, output);
            }
            Ok(None) => {}
            Err(_) => return Run::Failed,
        }
        if drained.is_err() {
            return Run::Failed;
        }
        if Instant::now() >= deadline {
            // Ownership transfer: the child keeps its own budget, a store Client's 20-second start
            // or an install's own deadline, and exits by itself. Dropping `child` closes only this
            // process's handle; it is never killed, since it may be the one starting the Session or
            // completing an install.
            return Run::Deadline;
        }
        thread::sleep(POLL.min(deadline.saturating_duration_since(Instant::now())));
    }
}

/// Reads what the pipe holds now, keeping at most [`OUTPUT_LIMIT`] bytes. A bounded number of reads
/// per call keeps a Client that writes without pause from starving the deadline check.
fn drain(reader: &mut impl Read, output: &mut Vec<u8>) -> io::Result<()> {
    let mut bytes = [0; 8192];
    for _ in 0..16 {
        match reader.read(&mut bytes) {
            Ok(0) => return Ok(()),
            Ok(count) => {
                let room = OUTPUT_LIMIT.saturating_sub(output.len());
                output.extend_from_slice(&bytes[..count.min(room)]);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::BrokenPipe
                ) =>
            {
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Records that the start stays with this Client's own cohort for `reason`; answers that the store
/// started nothing.
fn own(reason: Own) -> bool {
    record(reason.level(), "own", Some(reason.token()), None);
    false
}

/// Whether the store Client's `orchbee.server.start` document says it started the Session.
fn reports_started(output: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(output).is_ok_and(|document| {
        document["schema"] == "orchbee.server.start" && document["result"]["outcome"] == "started"
    })
}

fn record(
    level: LogLevel,
    source: &'static str,
    reason: Option<&'static str>,
    status: Option<u64>,
) {
    let mut fields = vec![("source", LogValue::Token(source))];
    if let Some(reason) = reason {
        fields.push(("reason", LogValue::Token(reason)));
    }
    if let Some(status) = status {
        fields.push(("status", LogValue::Count(status)));
    }
    orchbee_local_runtime::log_event(level, "session-start-source", &fields);
}

#[cfg(test)]
mod tests;
