//! The launcher's relaunch protocol from the TUI's side: whether the launcher that started this
//! process announced it, and the status the TUI ends with under it. Statuses 75 and 76 exist only
//! there; the launcher returns neither to its caller. A macOS TUI that no launcher started replaces
//! itself with the active version instead, in the same process and terminal.

use crate::cli;
use crate::local_activation::StartupFailure;
use crate::localization::Locale;
use crate::release_update::{Launcher, depart};

/// The status that tells an announcing launcher this version could not open its Session, so it
/// starts the version it replaced.
const FAILED_START_STATUS: u8 = 76;

/// How a TUI invocation ended: closed, left for a newer version, or failed to start.
pub(crate) type TuiOutcome = Result<Option<String>, StartupFailure>;

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
            #[cfg(target_os = "macos")]
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
            if counts_against_version(failure) && launcher() == Launcher::Announcing {
                output.status = FAILED_START_STATUS;
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

/// Announcing when `ORCHBEE_RELAUNCH_V1` holds the process id of this process's parent, so a value
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
/// start, and the TUI then leaves as it would without one.
#[cfg(target_os = "macos")]
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
    let _not_started = std::process::Command::new(program)
        .args(std::env::args_os().skip(1))
        .exec();
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
