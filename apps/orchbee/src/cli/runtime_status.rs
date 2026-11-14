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

pub(in crate::cli) use projection::{process_lines, processes_document};

pub(crate) fn run(scope: RuntimeStatusScope, format: OutputFormat, locale: Locale) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::RuntimeStatus) {
        return failure_output(failure, Some(format), locale);
    }
    workflow::run(scope, format, locale)
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
            Failure::new(FailureKind::Denied)
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
    // A refusal keeps the shared copy that names the older side or the other environment.
    if kind != FailureKind::Incompatible {
        result.stderr = CliText::new(locale).cli_runtime_status_failure(kind.as_str());
    }
    result
}
