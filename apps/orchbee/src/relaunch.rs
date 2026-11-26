//! The launcher's relaunch protocol from the TUI's side: whether the launcher that started this
//! process announced it, and the status the TUI ends with under it. Statuses 75 and 76 exist only
//! there; the launcher returns neither to its caller. A macOS or Linux TUI that no launcher started
//! replaces itself with the active version instead, in the same process and terminal, and records its own
//! failed start or join for the fallback as a launcher would.

use crate::cli;
use crate::local_activation::StartupFailure;
use crate::localization::Locale;
use crate::release_update::{Launcher, depart};

/// The status that tells an announcing launcher this version could not open its Session, so it
/// starts the version it replaced.
const FAILED_START_STATUS: u8 = 76;

/// How a TUI invocation ended: closed, left for a newer version, or failed to start.
pub(crate) type TuiOutcome = Result<Option<String>, StartupFailure>;

/// How long a failure record may hold the TUI, as a launcher waits for it, and how often it looks.
#[cfg(unix)]
const RECORD_BOUND: std::time::Duration = std::time::Duration::from_secs(5);
#[cfg(unix)]
const RECORD_POLL: std::time::Duration = std::time::Duration::from_millis(50);

/// Runs a TUI invocation. A macOS or Linux TUI that no launcher started records its own panic, as a
/// launcher records a Client that ends with Rust's panic status, then goes on unwinding, so the
/// process still ends with that status.
pub(crate) fn run_tui(run: impl FnOnce() -> TuiOutcome) -> TuiOutcome {
    #[cfg(unix)]
    if launcher() == Launcher::Absent {
        return std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)).unwrap_or_else(
            |panic| {
                record_own_failure();
                std::panic::resume_unwind(panic)
            },
        );
    }
    run()
}

/// The process result of a TUI invocation, for `orchbee` and `orchbee session attach` alike.
pub(crate) fn tui_output(outcome: &TuiOutcome, locale: Locale) -> cli::Output {
    match outcome {
        Ok(None) => cli::Output {
            status: 0,
            stdout: Vec::new(),
            stderr: String::new(),
            finalization: None,
        },
        Ok(Some(version)) => {
            let launcher = launcher();
            #[cfg(unix)]
            if launcher == Launcher::Absent {
                replace_in_place();
            }
            let departure = depart(version, launcher, locale);
            cli::Output {
                status: departure.status,
                stdout: Vec::new(),
                stderr: departure.line.unwrap_or_default(),
                finalization: None,
            }
        }
        Err(failure) => {
            let mut output = cli::startup_failure(failure, locale);
            if counts_against_version(failure) {
                match launcher() {
                    Launcher::Announcing => output.status = FAILED_START_STATUS,
                    #[cfg(unix)]
                    Launcher::Absent => record_own_failure(),
                    #[cfg(windows)]
                    Launcher::Absent => {}
                }
            }
            output
        }
    }
}

/// Whether a startup failure means this version cannot open the Session: its own files fail the
/// cohort check, or the running Session is older than it can join. Every other failure, such as a
/// console, deadline, or environment fault, says nothing about the version and stays status 1.
const fn counts_against_version(failure: &StartupFailure) -> bool {
    matches!(
        failure,
        StartupFailure::Cohort | StartupFailure::SessionTooOld(_)
    )
}

/// Announcing when `ORCHBEE_RELAUNCH` holds the process id of this process's parent, so a value
/// a later process inherited does nothing.
fn launcher() -> Launcher {
    let announced = std::env::var_os(orchbee_local_runtime::RELAUNCH_PROTOCOL_VARIABLE)
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|value| value.parse::<u32>().ok());
    if announced.is_some() && announced == parent_process_id() {
        Launcher::Announcing
    } else {
        Launcher::Absent
    }
}

/// The parent process id.
#[cfg(unix)]
fn parent_process_id() -> Option<u32> {
    Some(std::os::unix::process::parent_id())
}

/// Replaces this process with the store's active Client, read again as a launcher reads it, with
/// the same arguments and environment, so the shell or carrier that started this TUI waits on the
/// same process and the new version opens in the same terminal. The replacement starts with no
/// blocked signal, as a start from a shell does. It returns only when the replacement could not
/// start, which it records as a launcher records a Client that did not start, and the TUI then
/// leaves as it would without one.
#[cfg(unix)]
fn replace_in_place() {
    use std::os::unix::process::CommandExt;

    let Ok(root) = orchbee_local_runtime::release_store_root(|name| std::env::var_os(name)) else {
        return;
    };
    let Ok(Some(active)) = orchbee_local_runtime::active_release(&root) else {
        return;
    };
    let program = active
        .directory
        .join(orchbee_local_runtime::CohortArtifact::Client.file_name());
    if nix::sys::signal::SigSet::empty().thread_set_mask().is_err() {
        return;
    }
    let _not_started = std::process::Command::new(&program)
        .args(std::env::args_os().skip(1))
        .exec();
    record_failure(&program);
}

/// Records this TUI's own failed start or join; see [`record_failure`].
#[cfg(unix)]
fn record_own_failure() {
    if let Ok(client) = orchbee_local_runtime::current_executable() {
        record_failure(&client);
    }
}

/// Records that the Client at `client` failed to start or to join, as a launcher records a Client
/// it started: when the pointer of the store `client` runs from, read again now, still makes its
/// version active through a counting move that may fall back, the previous version's Client runs
/// the Client-failure helper with null standard streams, and this process waits for it at most
/// five seconds. A Client outside a store version folder records nothing.
#[cfg(unix)]
fn record_failure(client: &std::path::Path) {
    use std::{io::Read, process::Stdio};

    let Some(folder) = client
        .parent()
        .filter(|folder| orchbee_local_runtime::is_release_version_directory(folder))
    else {
        return;
    };
    let Some(root) = folder.parent().and_then(std::path::Path::parent) else {
        return;
    };
    let mut pointer = Vec::new();
    let limit = orchbee_local_runtime::RELEASE_POINTER_LIMIT as u64;
    let read = std::fs::File::open(root.join(orchbee_local_runtime::RELEASE_POINTER))
        .and_then(|file| file.take(limit + 1).read_to_end(&mut pointer));
    let pointer = (read.is_ok() && pointer.len() as u64 <= limit).then_some(pointer);
    let Some((helper, version)) =
        orchbee_release_records::client_failure_helper(client, pointer.as_deref())
    else {
        return;
    };
    let Ok(mut child) = std::process::Command::new(helper)
        .arg(orchbee_local_runtime::CLIENT_FAILURE_HELPER)
        .arg(version)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    let give_up = std::time::Instant::now() + RECORD_BOUND;
    while matches!(child.try_wait(), Ok(None)) && std::time::Instant::now() < give_up {
        std::thread::sleep(RECORD_POLL);
    }
}

/// The parent process id from a process snapshot.
#[cfg(windows)]
fn parent_process_id() -> Option<u32> {
    use winsafe::{self as w, co};

    let own = std::process::id();
    let mut snapshot =
        w::HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None).ok()?;
    snapshot
        .iter_processes()
        .filter_map(Result::ok)
        .find(|entry| entry.th32ProcessID == own)
        .map(|entry| entry.th32ParentProcessID)
}
