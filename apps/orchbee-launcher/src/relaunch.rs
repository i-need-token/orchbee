//! The relaunch protocol's decisions: which outcome of a Client start is a failed start or join, and
//! what the launcher does after each start. They read no clock and start nothing, so the npm
//! launcher's copy of the same rule is tested the same way.
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// The variable through which the launcher announces the protocol to every Client it starts: the
/// launcher's own process id in decimal. A Client uses the protocol's statuses only when that process
/// is its parent, so a stale inherited value does nothing.
pub(crate) const PROTOCOL_VARIABLE: &str = "ORCHBEE_RELAUNCH_V1";

/// The Client restored the console and asks to be started again on the store's active version.
const RELAUNCH: i32 = 75;
/// The Client could not join its Session; without the protocol it would have ended with
/// `STARTUP_FAILURE`, which the launcher then returns in its place.
const JOIN_FAILED: i32 = 76;
/// The status of a TUI whose startup failed, under `docs/cli/output.md`.
const STARTUP_FAILURE: i32 = 1;
/// Rust's status for a panic that unwinds out of `main`; the CLI exit codes never use it.
const PANIC: i32 = 101;
/// `STATUS_CONTROL_C_EXIT`: Ctrl+C, Ctrl+Break or a console close ended the Client.
const CONSOLE_EXIT: u32 = 0xC000_013A;
/// The lowest error-severity `NTSTATUS`: the Client did not end by its own choice, as when its
/// loader failed or it crashed.
const ERROR_SEVERITY: u32 = 0xC000_0000;

/// At most this many relaunches within `RELAUNCH_WINDOW`.
pub(crate) const RELAUNCH_LIMIT: usize = 3;
const RELAUNCH_WINDOW: Duration = Duration::from_secs(10);

/// What one start of a Client came to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Outcome {
    /// No Client process started: its file is missing or cannot run, or the pointer names none.
    NotStarted,
    /// The Client ended with this exit code; on Windows an `NTSTATUS` reads as a negative number.
    Exited(i32),
}

/// How the launcher reads an outcome, after every start it makes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Verdict {
    /// The Client asks to be started again on the active version.
    Relaunch,
    /// The Client failed to start or to join: it never ran, it said it could not join, it
    /// panicked, or Windows ended it with an error status other than a console exit.
    Failed,
    /// Any other end, failures the Client reports with a CLI exit code included.
    Normal,
}

/// Reads an outcome. A failed start or join is told from a normal exit by status alone: the Client
/// reports a failed join with `JOIN_FAILED`, which it uses only under this launcher's protocol,
/// and the rest are statuses no CLI exit code uses.
pub(crate) fn judge(outcome: Outcome) -> Verdict {
    let Outcome::Exited(code) = outcome else {
        return Verdict::Failed;
    };
    let status = code.cast_unsigned();
    match code {
        RELAUNCH => Verdict::Relaunch,
        JOIN_FAILED | PANIC => Verdict::Failed,
        _ if status >= ERROR_SEVERITY && status != CONSOLE_EXIT => Verdict::Failed,
        _ => Verdict::Normal,
    }
}

/// Which start the outcome came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Start {
    /// The launcher's first start, of the active version.
    First,
    /// A start of the active version after a Client asked for a relaunch.
    Relaunch,
    /// A start of the version a failed relaunch replaced.
    Return,
}

/// What the launcher does after a start.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Next {
    /// Ends with this exit status.
    End(i32),
    /// Ends as a Client that cannot start.
    Unavailable,
    /// Reads the pointer again and starts the active version with the same command line.
    Relaunch,
    /// Starts the version the failed relaunch replaced, with the same command line.
    Return,
}

/// The step after `start` came to `outcome`, given the instants of the earlier relaunches. A
/// relaunch whose new Client fails to start or to join returns to the version it replaced; after a
/// first start or a return, a failure ends with the Client's status. A fourth relaunch inside ten
/// seconds is refused, and the launcher ends with the Client's status.
pub(crate) fn next(start: Start, outcome: Outcome, relaunches: &[Instant], now: Instant) -> Next {
    match (judge(outcome), outcome) {
        (Verdict::Relaunch, _) => {
            let recent = relaunches
                .iter()
                .filter(|&&at| now.saturating_duration_since(at) < RELAUNCH_WINDOW)
                .count();
            if recent < RELAUNCH_LIMIT {
                Next::Relaunch
            } else {
                Next::End(RELAUNCH)
            }
        }
        (Verdict::Failed, _) if start == Start::Relaunch => Next::Return,
        (_, Outcome::NotStarted) => Next::Unavailable,
        (_, Outcome::Exited(JOIN_FAILED)) => Next::End(STARTUP_FAILURE),
        (_, Outcome::Exited(code)) => Next::End(code),
    }
}
