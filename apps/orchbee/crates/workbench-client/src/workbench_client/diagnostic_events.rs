//! The Workbench Client's diagnostic log records: its connection retiring on a failure, so a
//! disconnect leaves a trace, and a connection step held a second or longer, so typing that waited
//! names what it waited behind. A retirement names the failure, the kind of command it interrupted
//! and the source location that closed the mailbox, by directory, file stem and line; a slow step
//! names its command and counts bytes. No record holds terminal content, input or paths.
use super::mailbox::ClosedSite;
use crate::workbench_state::CarrierFailure;
use orchbee_local_runtime::{LogLevel, LogValue, log_event};
use std::time::Duration;

/// The connection worker runs one step at a time, and typed bytes queue behind the step in hand.
/// A step held this long is recorded.
pub(super) const SLOW_STEP: Duration = Duration::from_secs(1);

/// Records one connection step that took `SLOW_STEP` or longer: its command, the input bytes it
/// carried, the input bytes queued behind it when it ended, how long it took and whether it ended
/// without a failure.
pub(super) fn step_slow(
    command: &'static str,
    bytes: usize,
    queued_input_bytes: usize,
    elapsed: Duration,
    succeeded: bool,
) {
    let count = |value: usize| LogValue::Count(u64::try_from(value).unwrap_or(u64::MAX));
    log_event(
        LogLevel::Warning,
        "client-step-slow",
        &[
            ("command", LogValue::Token(command)),
            ("inputBytes", count(bytes)),
            ("queuedInputBytes", count(queued_input_bytes)),
            (
                "milliseconds",
                LogValue::Count(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)),
            ),
            ("succeeded", LogValue::Flag(succeeded)),
        ],
    );
}

/// Records the failure that retired the connection, the command the worker held then, and where
/// the mailbox closed: the mailbox check that hit the failure, or the Client step that decided it. A
/// process exit's close writes nothing.
pub(super) fn connection_retired(failure: CarrierFailure, site: ClosedSite) {
    let Some(failure) = failure_token(failure) else {
        return;
    };
    let (directory, file) = source(site.location.file());
    let mut fields = vec![("failure", LogValue::Token(failure))];
    if let Some(command) = site.command {
        fields.push(("command", LogValue::Token(command)));
    }
    fields.extend([
        ("sourceDirectory", LogValue::Token(directory)),
        ("sourceFile", LogValue::Token(file)),
        ("line", LogValue::Count(u64::from(site.location.line()))),
    ]);
    log_event(LogLevel::Error, "client-retired", &fields);
}

const fn failure_token(failure: CarrierFailure) -> Option<&'static str> {
    Some(match failure {
        CarrierFailure::LaunchRequestTooLarge => "launch-request-too-large",
        CarrierFailure::AuthorityDivergence => "authority-divergence",
        CarrierFailure::Deadline => "deadline",
        CarrierFailure::IdentityExhausted => "identity-exhausted",
        CarrierFailure::OutcomeUnknown => "outcome-unknown",
        CarrierFailure::Protocol => "protocol",
        CarrierFailure::Transport => "transport",
        CarrierFailure::WorkerExited => "worker-exited",
        CarrierFailure::Closed => return None,
    })
}

/// The parent directory and stem of a source file, which the log grammar admits as tokens where it
/// would refuse the path.
fn source(path: &'static str) -> (&'static str, &'static str) {
    let mut parts = path.rsplit(['/', '\\']);
    let file = parts.next().unwrap_or(path);
    let directory = parts.next().unwrap_or_default();
    (directory, file.strip_suffix(".rs").unwrap_or(file))
}
