//! Installed manifests and retained reload receipts; transport loss never replays a mutation.
#[cfg(windows)]
use super::output;
use super::{Failure, FailureKind, Output, failure_output};
use crate::{
    invocation::{ManifestCommand, OutputFormat},
    localization::Locale,
};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use orchbee_workbench_protocol::{
    AgentManifestsAction, AgentManifestsFailure, AgentManifestsOperation, AgentManifestsOutcome,
    AgentManifestsResult, SessionId, WorkbenchRequestId, WorkbenchResponse,
};
#[cfg(windows)]
enum Observation {
    Server(Box<AgentManifestsResult>),
    Unknown(&'static str),
}
#[cfg(windows)]
struct Answer {
    request_id: WorkbenchRequestId,
    session_id: SessionId,
    action: AgentManifestsAction,
    observation: Observation,
    interrupted: bool,
    cleanup_failed: bool,
}
#[cfg(windows)]
impl Answer {
    fn document(&self) -> serde_json::Value {
        let mut observation = match &self.observation {
            Observation::Server(reply) => {
                serde_json::json!({"kind":"server","reply":WorkbenchResponse::AgentManifests((**reply).clone())})
            }
            Observation::Unknown(reason) => serde_json::json!({"kind":"unknown","reason":reason}),
        };
        if let Some(snapshot) = observation
            .get_mut("reply")
            .and_then(|reply| reply.get_mut("outcome"))
            .and_then(|outcome| outcome.get_mut("snapshot"))
            .and_then(serde_json::Value::as_object_mut)
        {
            let maintenance = snapshot
                .entry("maintenance")
                .or_insert(serde_json::Value::Null);
            if let Some(maintenance) = maintenance.as_object_mut() {
                maintenance
                    .entry("automatic")
                    .or_insert(serde_json::Value::Null);
            }
        }
        serde_json::json!({"schema":"orchbee.server.agent-manifests","version":1,"requestId":self.request_id,"sessionId":self.session_id,"action":self.action,"observation":observation,"interrupted":self.interrupted,"cleanupFailed":self.cleanup_failed})
    }
    fn exit_code(&self) -> u8 {
        let failure = match &self.observation {
            Observation::Unknown(_) => {
                return if matches!(
                    self.action,
                    AgentManifestsAction::Reload { .. } | AgentManifestsAction::Update { .. }
                ) {
                    8
                } else {
                    4
                };
            }
            Observation::Server(reply) => match &reply.outcome {
                AgentManifestsOutcome::Operation {
                    status: AgentManifestsOperation::Unknown { .. },
                    ..
                } => return 8,
                AgentManifestsOutcome::Operation {
                    status: AgentManifestsOperation::Updated { report, activation },
                    ..
                } => {
                    if self.cleanup_failed {
                        return 1;
                    }
                    return if report.successful()
                        && matches!(
                            activation,
                            orchbee_workbench_protocol::AgentManifestActivation::Applied { .. }
                        ) {
                        0
                    } else if report.failure
                        == Some(orchbee_workbench_protocol::AgentManifestUpdateFailure::Cancelled)
                    {
                        7
                    } else {
                        1
                    };
                }
                AgentManifestsOutcome::Current { .. }
                | AgentManifestsOutcome::Operation {
                    status: AgentManifestsOperation::Applied { .. },
                    ..
                } => None,
                AgentManifestsOutcome::Operation {
                    status: AgentManifestsOperation::Pending {},
                    ..
                } => return 8,
                AgentManifestsOutcome::Failed { reason }
                | AgentManifestsOutcome::Operation {
                    status: AgentManifestsOperation::Failed { reason },
                    ..
                } => Some(*reason),
            },
        };
        if self.cleanup_failed {
            return 1;
        }
        match failure {
            None => 0,
            Some(
                AgentManifestsFailure::Denied
                | AgentManifestsFailure::SessionMismatch
                | AgentManifestsFailure::Busy
                | AgentManifestsFailure::ResourceLimit
                | AgentManifestsFailure::NotFound
                | AgentManifestsFailure::Conflict
                | AgentManifestsFailure::UpdateUnavailable,
            ) => 3,
            Some(AgentManifestsFailure::Unavailable) => 4,
            Some(AgentManifestsFailure::Cancelled) => 7,
            Some(AgentManifestsFailure::LoadFailed | AgentManifestsFailure::RevisionExhausted) => 1,
        }
    }
    fn emit(&self, format: OutputFormat, locale: Locale) -> Output {
        let encoded = match format {
            OutputFormat::Json => output::encode_document(&self.document()),
            OutputFormat::Text => {
                let (outcome, unknown) = match &self.observation {
                    Observation::Server(r) => (Some(&r.outcome), None),
                    Observation::Unknown(r) => (None, Some(*r)),
                };
                let id = match &self.action {
                    AgentManifestsAction::Current {} => None,
                    AgentManifestsAction::Reload { operation_id }
                    | AgentManifestsAction::Update { operation_id }
                    | AgentManifestsAction::Operation { operation_id } => {
                        Some(operation_id.as_str())
                    }
                };
                output::encode_text(&locale.cli_agent_manifests(id, outcome, unknown))
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
pub(crate) fn run(command: &ManifestCommand, format: OutputFormat, locale: Locale) -> Output {
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
