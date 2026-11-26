//! `server restart`: hands this Session to a new Server and Worker of this build while every
//! terminal keeps running.
mod reply;
mod workflow;

use super::{Failure, FailureKind, Output, failure_output};
use crate::cli::console_interrupt::{ArmedSignals, Finish};
use crate::{invocation::OutputFormat, localization::Locale};
use std::time::{Duration, Instant};
use workflow::restart;

/// How long one `server restart` waits for its outcome.
const DEADLINE: Duration = Duration::from_secs(150);

/// How one `server restart` ended, before a cancelled wait is classified.
pub(super) enum Restart {
    /// The new Server serves the Session; every terminal kept running.
    Restarted,
    /// Nothing changed; the reason says why.
    Refused(Refusal),
    /// The running Server cannot hand its terminals to a new Server.
    SessionTooOld,
    /// The restart failed in `phase`; the running Server keeps serving, or, for `Open`, no Server
    /// serves and the next start brings the Session back with every terminal.
    Failed(FailedPhase),
    /// The fixed deadline elapsed; the restart may still finish by itself.
    TimedOut,
}

/// Why a restart was refused before it changed anything.
pub(super) enum Refusal {
    NotRunning,
    Busy,
    BackgroundPrograms,
    Clients,
    WorkerUnavailable,
    Host,
    Records,
}

/// Where a restart failed.
pub(super) enum FailedPhase {
    /// The new Server could not be started.
    Start,
    /// The new Server ended in its Check, before it started its Worker.
    Check,
    /// The restart ended after the new Server started its Worker; the running Server keeps the
    /// Session or took it back.
    Takeover,
    /// No Server serves the Session.
    Open,
}

/// `server restart` from the command line: one restart workflow under one fixed deadline. Ctrl+C
/// stops the wait only; the restart continues by itself.
pub(crate) fn run(format: OutputFormat, locale: Locale) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::ServerRestart) {
        return failure_output(failure, Some(format), locale);
    }
    let mut signals = match ArmedSignals::arm_interrupts_or_inherit() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let outcome = restart(Instant::now() + DEADLINE, &mut signals);
    let (interrupted, finalization) = match signals.finish() {
        Ok(Finish::Clear) => (false, None),
        Ok(Finish::Cancelled { finalization, .. }) => (true, Some(finalization)),
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let mut output = reply::render(outcome, interrupted, format, locale);
    output.finalization = finalization;
    output
}
