//! CLI waiting is separate from the Server's retained Worktree operation.
#[cfg(any(windows, test))]
use super::output;
use super::{Failure, FailureKind, Output, failure_output};
#[cfg(any(windows, test))]
use crate::invocation::WorktreeCommand;
use crate::{
    invocation::{OutputFormat, ParsedWorktreeCommand},
    localization::Locale,
};
#[cfg(any(windows, test))]
use orchbee_workbench_protocol::{
    SiteId, WorkbenchRequestId, WorkbenchTerminalOutcome, WorkbenchWorktreeOutcome,
    WorkbenchWorktreeResult, WorktreeOperationStatus,
};
#[cfg(any(windows, test))]
use serde_json::json;

#[cfg(windows)]
mod configured_path;
#[cfg(windows)]
mod focus_windows;
#[cfg(any(windows, test))]
mod post_open;
#[cfg(any(windows, test))]
mod selection;
#[cfg(windows)]
mod windows;

#[cfg(any(windows, test))]
#[derive(Debug)]
enum Observation {
    Server { reply: WorkbenchWorktreeResult },
    Unavailable { reason: &'static str },
    Unknown { reason: &'static str },
}
#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Wait {
    Detached,
    Cancelled,
    TimedOut,
    TransportLost,
    ObservationFailed,
}
#[cfg(any(windows, test))]
struct Answer {
    operation_id: WorkbenchRequestId,
    observation: Observation,
    wait: Option<Wait>,
    post_open: Option<post_open::Receipt>,
}
#[cfg(any(windows, test))]
impl Answer {
    fn document(&self) -> serde_json::Value {
        let observation = match &self.observation {
            Observation::Server { reply } => json!({"kind":"server","reply":reply}),
            Observation::Unavailable { reason } => json!({"kind":"unavailable","reason":reason}),
            Observation::Unknown { reason } => json!({"kind":"unknown","reason":reason}),
        };
        let wait = self.wait.map(|wait| match wait {
            Wait::Detached => "detached",
            Wait::Cancelled => "cancelled",
            Wait::TimedOut => "timed-out",
            Wait::TransportLost => "transport-lost",
            Wait::ObservationFailed => "observation-failed",
        });
        let mut result = json!({"operationId":self.operation_id,"observation":observation,"wait":wait,"postOpen":null});
        if let Some(receipt) = &self.post_open {
            result["postOpen"] = receipt.document();
        }
        result
    }
    fn pending(&self) -> bool {
        matches!(&self.observation, Observation::Server { reply } if matches!(&reply.outcome,
            WorkbenchWorktreeOutcome::Available {status, ..} if matches!(status.as_ref(), WorktreeOperationStatus::Pending {..})))
    }
    fn exit_code(&self) -> u8 {
        let canonical = self.canonical_exit_code();
        if canonical != 0 {
            canonical
        } else {
            self.post_open
                .as_ref()
                .map_or(0, post_open::Receipt::exit_code)
        }
    }
    fn canonical_exit_code(&self) -> u8 {
        match self.wait {
            Some(Wait::Cancelled) => return 130,
            Some(Wait::TimedOut) => return 7,
            Some(Wait::TransportLost) => return 5,
            Some(Wait::ObservationFailed) => return 1,
            _ => {}
        }
        match &self.observation {
            Observation::Unknown { .. } => 8,
            Observation::Unavailable { .. } => 4,
            Observation::Server { reply } => match &reply.outcome {
                WorkbenchWorktreeOutcome::Available { status, .. } => match status.as_ref() {
                    WorktreeOperationStatus::Opened {
                        terminal_outcome, ..
                    } => match terminal_outcome {
                        None | Some(WorkbenchTerminalOutcome::Started { .. }) => 0,
                        Some(WorkbenchTerminalOutcome::OutcomeUnknown {}) => 8,
                        Some(WorkbenchTerminalOutcome::Unavailable { .. }) => 4,
                        Some(WorkbenchTerminalOutcome::Failed { .. }) => 3,
                    },
                    WorktreeOperationStatus::Pending { .. }
                    | WorktreeOperationStatus::Removed { .. } => 0,
                    WorktreeOperationStatus::Failed {
                        reason, progress, ..
                    } => {
                        if progress
                            .restoration
                            .as_ref()
                            .is_some_and(|receipt| receipt.unknown > 0)
                        {
                            return 8;
                        }
                        use orchbee_workbench_protocol::WorktreeOperationFailure as F;
                        match reason {
                            F::PhysicalUnknown {} | F::TransportUnknown {} => 8,
                            F::WorkerUnavailable {}
                            | F::WorkerBusy {}
                            | F::WorkerCapacity {}
                            | F::Unsupported {}
                            | F::SiteStopping {} => 4,
                            F::Internal {} => 1,
                            _ => 3,
                        }
                    }
                },
                WorkbenchWorktreeOutcome::NotFound {}
                | WorkbenchWorktreeOutcome::Conflict {}
                | WorkbenchWorktreeOutcome::Denied {} => 3,
                WorkbenchWorktreeOutcome::Unavailable { .. } => 4,
            },
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn run(command: ParsedWorktreeCommand, format: OutputFormat, locale: Locale) -> Output {
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

#[cfg(windows)]
pub(crate) fn run(command: ParsedWorktreeCommand, format: OutputFormat, locale: Locale) -> Output {
    let command =
        match command.into_resolved(format, |name| std::env::var_os(name), std::env::current_dir) {
            Ok(command) => command,
            Err(failure) => return super::parse_failure(failure, locale),
        };
    let command = &command;
    let operation_id = match command {
        WorktreeCommand::Start(command) => command
            .operation_id
            .clone()
            .map(Ok)
            .unwrap_or_else(super::query::request_id),
        WorktreeCommand::Status { operation_id } | WorktreeCommand::Resume { operation_id, .. } => {
            Ok(operation_id.clone())
        }
    };
    let operation_id = match operation_id {
        Ok(id) => id,
        Err(failure) => return failure_output(failure, Some(format), locale),
    };
    windows::run(command, operation_id, format, locale)
}

#[cfg(any(windows, test))]
fn emit(site_id: Option<&SiteId>, answer: Answer, format: OutputFormat, locale: Locale) -> Output {
    let status = answer.exit_code();
    let result = answer.document();
    let encoded = match format {
        OutputFormat::Json => {
            output::encode_document(&json!({"schema":"orchbee.worktree.operation","version":1,
            "source":{"kind":"local-worktree","siteId":site_id},"result":result}))
        }
        OutputFormat::Text => output::encode_text(&locale.cli_worktree_result(&result)),
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
