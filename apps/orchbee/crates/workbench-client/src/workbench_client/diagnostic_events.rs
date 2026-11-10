//! The Workbench Client's diagnostic log record: its connection retiring on a failure, so a
//! disconnect leaves a trace. The record names the failure, the kind of command it interrupted and
//! the source location that closed the mailbox, by directory, file stem and line; it holds no
//! terminal content, input or paths.
use super::mailbox::ClosedSite;
use crate::workbench_state::CarrierFailure;
use orchbee_local_runtime::{LogLevel, LogValue, log_event};

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
