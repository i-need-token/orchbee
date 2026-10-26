//! Read-only local build and Server metadata, separate from Workbench status.
use super::{Failure, FailureKind, Output, failure_output};
use crate::{
    invocation::{OutputFormat, RuntimeStatusScope},
    localization::Locale,
};

#[cfg(windows)]
mod projection;
#[cfg(windows)]
mod windows;

pub(crate) fn run(scope: RuntimeStatusScope, format: OutputFormat, locale: Locale) -> Output {
    #[cfg(windows)]
    {
        windows::run(scope, format, locale)
    }
    #[cfg(not(windows))]
    {
        let _ = scope;
        failure_output(
            Failure::new(FailureKind::ServiceUnavailable).details(super::stage_reason_details(
                "platform",
                "unsupported-platform",
            )),
            Some(format),
            locale,
        )
    }
}

#[cfg(windows)]
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
    result.stderr = locale.cli_runtime_status_failure(kind.as_str());
    result
}
