//! Agent startup composes a name reservation, one terminal submission and
//! canonical readiness. Every failure retains the independently known effects.

use super::{Failure, FailureKind, Output, failure_output};
use crate::{
    invocation::{AgentStartCommand, OutputFormat},
    localization::Locale,
};

#[cfg(windows)]
mod command;
#[cfg(windows)]
mod windows;

pub(crate) fn run(command: &AgentStartCommand, format: OutputFormat, locale: Locale) -> Output {
    #[cfg(windows)]
    {
        windows::run(command, format, locale)
    }
    #[cfg(not(windows))]
    {
        let _ = command;
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
