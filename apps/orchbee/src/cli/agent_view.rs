//! Compose Client-local attention with canonical Agent facts without changing either owner.
use super::{Output, failure_output};
use crate::{
    invocation::{AgentTarget, OutputFormat, QueryCommand},
    localization::Locale,
};
use orchbee_workbench_protocol::WorkbenchClientId;

#[cfg(windows)]
mod windows;

pub(super) fn run(
    command: &QueryCommand,
    target: &AgentTarget,
    client: &WorkbenchClientId,
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
            super::Failure::new(super::FailureKind::ServiceUnavailable),
            Some(format),
            locale,
        )
    }
}
