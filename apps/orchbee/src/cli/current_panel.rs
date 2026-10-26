//! Current-Panel query composition; identity and Client records retain their existing owners.
use super::{Output, failure_output, success_output};
use crate::{
    invocation::{OutputFormat, PanelQueryTarget, QueryCommand},
    localization::Locale,
};

#[cfg(windows)]
mod windows;

pub(super) fn run(
    command: &QueryCommand,
    target: &PanelQueryTarget,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    #[cfg(windows)]
    {
        windows::run(command, target, format, locale)
    }
    #[cfg(not(windows))]
    {
        let explicit = match target {
            PanelQueryTarget::Panel(target) => Some(target),
            PanelQueryTarget::Current => None,
            PanelQueryTarget::Client(_) => {
                return failure_output(
                    super::Failure::new(super::FailureKind::ServiceUnavailable).details(
                        super::stage_reason_details("platform", "unsupported-platform"),
                    ),
                    Some(format),
                    locale,
                );
            }
        };
        let result = match command {
            QueryCommand::PanelCurrent(_) => super::caller_context::read(
                explicit,
                |name| std::env::var_os(name),
                super::query::read_snapshot,
            ),
            _ => super::query::panel_process_info(explicit),
        };
        match result {
            Ok((source, projection)) => success_output(projection, source, command, format, locale),
            Err(error) => failure_output(error, Some(format), locale),
        }
    }
}
