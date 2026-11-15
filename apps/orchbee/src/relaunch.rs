//! The launcher's relaunch protocol from the TUI's side: whether the launcher that started this
//! process announced it, and the status the TUI ends with under it. Statuses 75 and 76 exist only
//! there; the launcher returns neither to its caller.

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
            let departure = depart(version, launcher(), locale);
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
    #[cfg(windows)]
    {
        let announced = std::env::var_os(orchbee_local_runtime::RELAUNCH_PROTOCOL_VARIABLE)
            .and_then(|value| value.into_string().ok())
            .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|value| value.parse::<u32>().ok());
        if announced.is_some() && announced == parent_process_id() {
            return Launcher::Announcing;
        }
    }
    Launcher::Absent
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
