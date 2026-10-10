//! CLI waiting is separate from the Server's retained group Close operation.
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::{
    invocation::{OutputFormat, WorkspaceGroupCloseCommand},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    SiteId, WorkbenchRequestId, WorkbenchWorkspaceGroupCloseResult, WorkspaceGroupCloseOutcome,
    WorkspaceGroupCloseStatus,
};
use serde_json::json;

#[cfg(windows)]
mod windows;

#[derive(Debug)]
enum Observation {
    Server {
        reply: WorkbenchWorkspaceGroupCloseResult,
    },
    Unavailable {
        reason: &'static str,
    },
    Unknown {
        reason: &'static str,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Wait {
    Detached,
    Cancelled,
    TimedOut,
    TransportLost,
    ObservationFailed,
}
#[derive(Debug)]
struct Answer {
    operation_id: WorkbenchRequestId,
    observation: Observation,
    wait: Option<Wait>,
}
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
        json!({"operationId":self.operation_id,"observation":observation,"wait":wait})
    }
    fn pending(&self) -> bool {
        matches!(&self.observation, Observation::Server { reply } if matches!(&reply.outcome,
            WorkspaceGroupCloseOutcome::Available {status, ..} if matches!(status, WorkspaceGroupCloseStatus::Pending {..})))
    }
    fn exit_code(&self) -> u8 {
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
                WorkspaceGroupCloseOutcome::Available { status, .. } => match status {
                    WorkspaceGroupCloseStatus::Pending {} | WorkspaceGroupCloseStatus::Closed {} => 0,
                    WorkspaceGroupCloseStatus::Failed { reason, .. } => match reason {
                        orchbee_workbench_protocol::WorkspaceGroupCloseFailure::OutcomeUnknown => 8,
                        orchbee_workbench_protocol::WorkspaceGroupCloseFailure::WorkerUnavailable | orchbee_workbench_protocol::WorkspaceGroupCloseFailure::SiteStopping => 4,
                        orchbee_workbench_protocol::WorkspaceGroupCloseFailure::Internal => 1,
                        _ => 3,
                    },
                },
                WorkspaceGroupCloseOutcome::NotFound {}
                | WorkspaceGroupCloseOutcome::Conflict {}
                | WorkspaceGroupCloseOutcome::Denied {} => 3,
                WorkspaceGroupCloseOutcome::Unavailable { .. } => 4,
            },
        }
    }
}

pub(crate) fn run(
    command: &WorkspaceGroupCloseCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let operation_id = match command {
        WorkspaceGroupCloseCommand::Start { operation_id, .. } => operation_id
            .clone()
            .map(Ok)
            .unwrap_or_else(super::query::request_id),
        WorkspaceGroupCloseCommand::Status { operation_id }
        | WorkspaceGroupCloseCommand::Resume { operation_id, .. } => Ok(operation_id.clone()),
    };
    let operation_id = match operation_id {
        Ok(id) => id,
        Err(failure) => return failure_output(failure, Some(format), locale),
    };
    #[cfg(windows)]
    {
        windows::run(command, operation_id, format, locale)
    }
    #[cfg(not(windows))]
    {
        emit(
            None,
            Answer {
                operation_id,
                observation: Observation::Unavailable {
                    reason: "unsupported-platform",
                },
                wait: None,
            },
            format,
            locale,
        )
    }
}

fn emit(site_id: Option<&SiteId>, answer: Answer, format: OutputFormat, locale: Locale) -> Output {
    let status = answer.exit_code();
    let result = answer.document();
    let encoded = match format {
        OutputFormat::Json => {
            output::encode_document(&json!({"schema":"orchbee.project.close-all","version":1,
            "source":{"kind":"local-workspace-group","siteId":site_id},"result":result}))
        }
        OutputFormat::Text => {
            output::encode_text(&locale.cli_workspace_group_close_result(&result))
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
