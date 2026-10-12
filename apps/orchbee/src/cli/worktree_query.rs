//! Source-based listing has its own envelope; the registered-directory format is stable.
use super::{Failure, FailureKind, Output, failure_output};
use crate::{
    invocation::{OutputFormat, ParsedWorktreeQueryCommand},
    localization::Locale,
};

#[cfg(windows)]
mod windows;

pub(crate) fn run(
    command: ParsedWorktreeQueryCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    #[cfg(windows)]
    {
        let command = match command.into_resolved(
            format,
            |name| std::env::var_os(name),
            std::env::current_dir,
        ) {
            Ok(command) => command,
            Err(failure) => return super::parse_failure(failure, locale),
        };
        windows::run(&command, format, locale)
    }
    #[cfg(not(windows))]
    {
        let _ = command;
        failure_output(
            Failure::new(FailureKind::ServiceUnavailable),
            Some(format),
            locale,
        )
    }
}

#[cfg(any(windows, test))]
fn emit(
    result: orchbee_workbench_protocol::WorkbenchWorktreeQueryResult,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    use orchbee_workbench_protocol::{
        WorkbenchWorktreeQueryOutcome as O, WorktreeQueryFailure as F,
    };
    let status = match &result.outcome {
        O::Found { .. } => 0,
        O::Failed { reason } => match reason {
            F::NotFound {} | F::Denied {} | F::SourceChanged {} | F::GitFailed { .. } => 3,
            F::Unsupported {} | F::Unavailable { .. } | F::SpawnUnavailable { .. } => 4,
            F::TransportLoss {} | F::MalformedListing {} => 5,
            F::Timeout {} => 7,
            F::OwnerIo { .. } | F::SpawnFailed { .. } | F::TooLarge { .. } => 1,
        },
    };
    let encoded = match format {
        OutputFormat::Json => super::output::encode_document(&serde_json::json!({
            "schema":"orchbee.worktree.query", "version":2, "result":result,
        })),
        OutputFormat::Text => {
            super::output::encode_text(&locale.cli_worktree_query(&result.outcome))
        }
    };
    match encoded {
        Ok(stdout) => Output {
            status,
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(_) => failure_output(
            Failure::new(FailureKind::OutputTooLarge),
            Some(format),
            locale,
        ),
    }
}

#[cfg(test)]
mod tests;
