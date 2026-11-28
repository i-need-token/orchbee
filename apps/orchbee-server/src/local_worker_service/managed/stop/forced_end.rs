//! What a macOS or Linux Session stop does once its attempt failed. A recorded stop refuses every
//! Client until the process ends, and the person who asked for it meant the terminals to end, so the
//! stop ends the Session's processes by force and completes rather than wait for a retry; only a
//! stop that cannot prove them gone, or cannot complete, ends this Server without the Stopped
//! record. It takes no capture of its own, and a Windows stop, whose failed attempt waits for a
//! retry and then becomes an `exit` that keeps every terminal, never comes here.
use super::{Failure, Owners, failure, host_absent, keeper_absent, stop_ingress};
use crate::{
    local_client_service,
    session_shutdown::{Attempt, SessionShutdown},
};
use orchbee_local_client_protocol::{StopFailure, StopPhase};
use orchbee_local_runtime::{ExactProcess, HostPresence, LogLevel, LogValue};
use std::{
    io, thread,
    time::{Duration, Instant},
};

/// How long the forced end and the completion after it may take together; ending a process by
/// force takes milliseconds.
const BUDGET: Duration = Duration::from_secs(5);
/// How long [`leave`] gives the Client listeners to close.
const LEAVE_BUDGET: Duration = Duration::from_secs(5);
const FORCED: &str = "session-stop-forced";

/// Ends the Session's processes after `failed` ended its attempt: the Worker first, since it cares
/// for the Keeper and could start another, then every Host and the Keeper, each by its recorded
/// identity with `SIGKILL`. The PTYs close with the last of them, so the kernel hangs up each
/// program still in a terminal, one that no Host may signal included. The next start reopens the
/// newest valid save, the final capture when `failed` came after it. Returns the deadline the
/// completion keeps, or the failure that leaves.
pub(super) fn end(
    owners: &mut Owners,
    shutdown: &SessionShutdown,
    attempt: Attempt,
    failed: &Failure,
) -> Result<Instant, Failure> {
    let deadline = Instant::now() + BUDGET;
    let ended = end_processes(owners, shutdown, attempt, deadline);
    record(failed, ended.is_ok());
    ended.map_err(|error| Failure {
        disposed: failed.disposed,
        ..error
    })?;
    // No Host is left to adopt.
    if let Some(kept) = &owners.checkpoint {
        kept.remove_bindings();
    }
    shutdown.phase(attempt, StopPhase::Ingress);
    // Its socket goes with the Session's other endpoints, and its thread with the process.
    let _ = stop_ingress(owners, deadline);
    Ok(deadline)
}

/// Ends the Worker, every running Host and the Keeper by force before `deadline`, then waits for
/// the Worker connection's thread, which ends with the connection.
fn end_processes(
    owners: &mut Owners,
    shutdown: &SessionShutdown,
    attempt: Attempt,
    deadline: Instant,
) -> Result<(), Failure> {
    shutdown.phase(attempt, StopPhase::WorkerExit);
    owners
        .worker
        .terminate_before(deadline)
        .map_err(|error| Failure {
            phase: StopPhase::WorkerExit,
            failure: if error.kind() == io::ErrorKind::TimedOut {
                StopFailure::Deadline
            } else {
                StopFailure::ProcessWait
            },
            disposed: 0,
        })?;
    shutdown.phase(attempt, StopPhase::HostExit);
    // Each running Host is ended even after one fails, so as many terminals close as can.
    let hosts = owners.session_record.hosts().and_then(|hosts| {
        hosts
            .into_iter()
            .filter_map(|host| match host.presence {
                HostPresence::Running(process) => process,
                _ => None,
            })
            .map(|process| ExactProcess::end_host(process, deadline))
            .fold(Ok(()), Result::and)
    });
    let keeper = orchbee_local_runtime::end_recorded_keeper(&owners.session_record, deadline);
    hosts
        .and(keeper)
        .map_err(|error| failure(StopPhase::HostExit, error))?;
    // A Host or Keeper that could not be observed, or one still being started, is not proven gone.
    if !host_absent(owners) || !keeper_absent(owners) {
        return Err(Failure {
            phase: StopPhase::HostExit,
            failure: StopFailure::Unavailable,
            disposed: 0,
        });
    }
    while !owners.carrier.is_finished() {
        if Instant::now() >= deadline {
            return Err(Failure {
                phase: StopPhase::WorkerExit,
                failure: StopFailure::Deadline,
                disposed: 0,
            });
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

/// Ends this Server without the Stopped record once its stop can neither prove the Session's
/// processes gone nor complete, so a stop always ends the process within its budgets. The next
/// start recovers the Session as after a crash once each of them is gone, and refuses while one
/// that could not be ended still runs.
pub(super) fn leave(
    mut clients: local_client_service::ManagedClients,
    shutdown: &SessionShutdown,
    failed: Failure,
) -> io::Result<()> {
    eprintln!(
        "orchbee-server diagnostic component=lifecycle operation=stop stage=leave phase={:?} failure={:?}",
        failed.phase, failed.failure
    );
    let deadline = Instant::now() + LEAVE_BUDGET;
    shutdown.end(deadline);
    clients.stop_before(deadline)?;
    Err(io::Error::other("Session stop failed"))
}

/// Records the forced end of the attempt `failed` ended, and whether it proved every process gone.
fn record(failed: &Failure, ended: bool) {
    orchbee_local_runtime::log_event(
        if ended {
            LogLevel::Warning
        } else {
            LogLevel::Error
        },
        FORCED,
        &[
            ("phase", LogValue::Token(phase_token(failed.phase))),
            ("failure", LogValue::Token(failure_token(failed.failure))),
            (
                "outcome",
                LogValue::Token(if ended { "ended" } else { "failed" }),
            ),
        ],
    );
}

/// The token `server stop` documents for `phase`.
const fn phase_token(phase: StopPhase) -> &'static str {
    match phase {
        StopPhase::Admission => "admission",
        StopPhase::BackgroundJobs => "background-jobs",
        StopPhase::Inventory => "inventory",
        StopPhase::Force => "force",
        StopPhase::Dispose => "dispose",
        StopPhase::HostExit => "host-exit",
        StopPhase::Worker => "worker",
        StopPhase::WorkerExit => "worker-exit",
        StopPhase::Ingress => "ingress",
        StopPhase::Clients => "clients",
        StopPhase::Completion => "completion",
    }
}

/// The token `server stop` documents for `failure`.
const fn failure_token(failure: StopFailure) -> &'static str {
    match failure {
        StopFailure::Deadline => "deadline",
        StopFailure::Unavailable => "unavailable",
        StopFailure::OutcomeUnknown => "outcome-unknown",
        StopFailure::Protocol => "protocol",
        StopFailure::Refused => "refused",
        StopFailure::ProcessWait => "process-wait",
        StopFailure::RecordWrite => "record-write",
    }
}
