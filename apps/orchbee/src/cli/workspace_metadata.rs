//! A display-data report is submitted once; its acknowledgement survives late cancellation.
#[cfg(any(windows, test))]
use super::output;
use super::{Failure, FailureKind, Output, failure_output};
use crate::{
    invocation::{OutputFormat, WorkspaceMetadataCommand},
    localization::Locale,
};
#[cfg(windows)]
mod windows;
#[cfg(any(windows, test))]
use orchbee_workbench_protocol::{
    SessionId, WorkbenchRequestId, WorkbenchResponse, WorkbenchWorkspaceId,
    WorkspaceMetadataFailure, WorkspaceMetadataOutcome, WorkspaceMetadataResult,
};
#[cfg(any(windows, test))]
#[derive(Debug)]
enum Observation {
    Server(WorkspaceMetadataResult),
    Unknown(&'static str),
}
#[cfg(any(windows, test))]
#[derive(Debug)]
struct Answer {
    request_id: WorkbenchRequestId,
    session_id: SessionId,
    workspace_id: WorkbenchWorkspaceId,
    observation: Observation,
    interrupted: bool,
    cleanup_failed: bool,
}
#[cfg(any(windows, test))]
impl Answer {
    fn document(&self) -> serde_json::Value {
        let observation = match &self.observation {
            Observation::Server(reply) => {
                serde_json::json!({"kind":"server", "reply":WorkbenchResponse::WorkspaceMetadata(reply.clone())})
            }
            Observation::Unknown(reason) => serde_json::json!({"kind":"unknown", "reason":reason}),
        };
        serde_json::json!({"schema":"orchbee.workspace.metadata", "version":1,
            "requestId":self.request_id,"sessionId":self.session_id,"workspaceId":self.workspace_id,
            "observation":observation,"interrupted":self.interrupted,"cleanupFailed":self.cleanup_failed})
    }
    fn exit_code(&self) -> u8 {
        if matches!(self.observation, Observation::Unknown(_)) {
            return 8;
        }
        if self.cleanup_failed {
            return 1;
        }
        match &self.observation {
            Observation::Unknown(_) => 8,
            Observation::Server(reply) => match reply.outcome {
                WorkspaceMetadataOutcome::Reported { .. } => 0,
                WorkspaceMetadataOutcome::Read { .. } => 5,
                WorkspaceMetadataOutcome::Failed { reason } => match reason {
                    WorkspaceMetadataFailure::Denied
                    | WorkspaceMetadataFailure::NotFound
                    | WorkspaceMetadataFailure::SessionMismatch
                    | WorkspaceMetadataFailure::TokenLimit
                    | WorkspaceMetadataFailure::SequenceSourceLimit
                    | WorkspaceMetadataFailure::ResourceLimit => 3,
                    WorkspaceMetadataFailure::Unavailable => 4,
                    WorkspaceMetadataFailure::InvalidRequest => 5,
                    WorkspaceMetadataFailure::RevisionExhausted
                    | WorkspaceMetadataFailure::DeadlineOverflow
                    | WorkspaceMetadataFailure::ResultTooLarge => 1,
                },
            },
        }
    }
    fn emit(&self, format: OutputFormat, locale: Locale) -> Output {
        let encoded = match format {
            OutputFormat::Json => output::encode_document(&self.document()),
            OutputFormat::Text => {
                let (outcome, unknown) = match &self.observation {
                    Observation::Server(reply) => (Some(&reply.outcome), None),
                    Observation::Unknown(reason) => (None, Some(*reason)),
                };
                output::encode_text(&locale.cli_workspace_metadata(
                    self.request_id.as_str(),
                    self.workspace_id.as_str(),
                    outcome,
                    unknown,
                ))
            }
        };
        match encoded {
            Ok(stdout) => Output {
                status: self.exit_code(),
                stdout,
                stderr: if self.cleanup_failed {
                    locale.cli_failure(&Failure::new(FailureKind::InternalFailure))
                } else {
                    String::new()
                },
                finalization: None,
            },
            Err(_) => failure_output(
                Failure::new(FailureKind::OutputTooLarge),
                Some(format),
                locale,
            ),
        }
    }
}
pub(crate) fn run(
    command: &WorkspaceMetadataCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    #[cfg(windows)]
    {
        windows::run(command, format, locale)
    }
    #[cfg(not(windows))]
    {
        let _ = command;
        failure_output(
            Failure::new(FailureKind::ServiceUnavailable).details(
                crate::cli::stage_reason_details("platform", "unsupported-platform"),
            ),
            Some(format),
            locale,
        )
    }
}

#[cfg(test)]
mod tests;
