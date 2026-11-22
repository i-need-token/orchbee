//! Read-only local build and Server metadata, separate from Workbench status.
use super::{Failure, FailureKind, Output, failure_output};
use crate::cli_text::CliText;
use crate::{
    invocation::{OutputFormat, RuntimeStatusScope},
    localization::Locale,
};
use orchbee_local_client_protocol::ConnectClientError;

mod projection;
mod workflow;

/// Reads the `restart` member for a Session's `query` answer: the restart tier the release store's
/// active version gives it. The update command owns the store's manifests, so the caller passes it.
pub(crate) type RestartTier =
    fn(&orchbee_connection_handshake::QueryResult) -> Option<serde_json::Value>;

pub(in crate::cli) use projection::{process_lines, processes_document};

/// `doctor`'s installation facts: what `status client` reports, as its result object.
pub(in crate::cli) fn client_facts(
    deadline: std::time::Instant,
) -> Result<serde_json::Value, Failure> {
    workflow::client(deadline).map(|report| report.result())
}

/// `doctor`'s Session facts: what `status server` reports, as its result object.
pub(in crate::cli) fn server_facts(
    deadline: std::time::Instant,
    signals: &super::console_interrupt::ArmedSignals,
    restart: RestartTier,
) -> Result<serde_json::Value, Failure> {
    workflow::server(deadline, signals, restart).map(|report| report.result())
}

pub(crate) fn run(
    scope: RuntimeStatusScope,
    format: OutputFormat,
    locale: Locale,
    restart: RestartTier,
) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::RuntimeStatus) {
        return failure_output(failure, Some(format), locale);
    }
    workflow::run(scope, format, locale, restart)
}

/// Classifies a failed handshake `query`: a Session that predates the handshake, or belongs to
/// another environment, is incompatible rather than absent.
pub(in crate::cli) fn classify_query(error: ConnectClientError) -> Failure {
    match error {
        ConnectClientError::ConnectFailed(ref io) | ConnectClientError::Io(ref io)
            if io.kind() == std::io::ErrorKind::TimedOut =>
        {
            Failure::new(FailureKind::Timeout)
        }
        ConnectClientError::ConnectFailed(ref io) | ConnectClientError::Io(ref io)
            if io.kind() == std::io::ErrorKind::PermissionDenied =>
        {
            Failure::new(FailureKind::Denied).elevated_session_from(&error)
        }
        other => super::query::classify_connect(
            crate::local_client::LocalClientConnectError::Admission(other),
        ),
    }
}

fn failed(mut failure: Failure, format: OutputFormat, locale: Locale) -> Output {
    let kind = failure.kind;
    if matches!(kind, FailureKind::Timeout | FailureKind::Cancelled) {
        failure = failure.details(super::stage_details("runtime-status"));
    }
    let mut result = failure_output(
        failure.source(crate::cli_outcome::local_runtime_source()),
        Some(format),
        locale,
    );
    // A refusal keeps the shared copy that names the older side or the other environment, or a
    // Session started as administrator.
    if !matches!(kind, FailureKind::Incompatible | FailureKind::Denied) {
        result.stderr = CliText::new(locale).cli_runtime_status_failure(kind.as_str());
    }
    result
}
