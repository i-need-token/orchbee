//! Compose Client-local attention with canonical Agent facts without changing either owner.
use super::{Output, failure_output};
use crate::{
    invocation::{AgentTarget, OutputFormat, QueryCommand},
    localization::Locale,
};
use orchbee_workbench_protocol::WorkbenchClientId;

#[cfg(windows)]
pub(in crate::cli) mod windows;

pub(super) fn run(
    command: &QueryCommand,
    target: &AgentTarget,
    client: Option<&WorkbenchClientId>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    #[cfg(windows)]
    {
        windows::run(command, target, client, format, locale)
    }
    #[cfg(not(windows))]
    {
        let _ = (command, target, client);
        failure_output(
            super::Failure::new(super::FailureKind::ServiceUnavailable).details(
                super::stage_reason_details("platform", "unsupported-platform"),
            ),
            Some(format),
            locale,
        )
    }
}

pub(super) fn list(command: &QueryCommand, format: OutputFormat, locale: Locale) -> Output {
    #[cfg(windows)]
    {
        windows::list::run(command, format, locale)
    }
    #[cfg(not(windows))]
    {
        let _ = command;
        failure_output(
            super::Failure::new(super::FailureKind::ServiceUnavailable).details(
                super::stage_reason_details("platform", "unsupported-platform"),
            ),
            Some(format),
            locale,
        )
    }
}
