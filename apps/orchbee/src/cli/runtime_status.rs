//! Read-only local build and Server metadata, separate from Workbench status.
use super::{Failure, FailureKind, Output, failure_output};
use crate::cli_text::CliText;
use crate::{
    invocation::{OutputFormat, RuntimeStatusScope},
    localization::Locale,
};

mod projection;
mod workflow;

pub(crate) fn run(scope: RuntimeStatusScope, format: OutputFormat, locale: Locale) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::RuntimeStatus) {
        return failure_output(failure, Some(format), locale);
    }
    workflow::run(scope, format, locale)
}

fn failed(mut failure: Failure, format: OutputFormat, locale: Locale) -> Output {
    let kind = failure.kind;
    if matches!(kind, FailureKind::Timeout | FailureKind::Cancelled) {
        failure = failure.details(super::stage_details("runtime-status"));
    }
    let mut result = failure_output(
        failure.source(super::config::local_runtime_source()),
        Some(format),
        locale,
    );
    result.stderr = CliText::new(locale).cli_runtime_status_failure(kind.as_str());
    result
}
