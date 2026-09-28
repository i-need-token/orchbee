//! Installed manifests and retained reload receipts; transport loss never replays a mutation.
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::{
    invocation::{ManifestCommand, OutputFormat},
    localization::Locale,
};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use orchbee_workbench_protocol::{
    AgentManifestsAction, AgentManifestsFailure, AgentManifestsOperation, AgentManifestsOutcome,
    AgentManifestsResult, SiteId, WorkbenchRequestId, WorkbenchResponse,
};
#[cfg(windows)]
enum Observation {
    Server(AgentManifestsResult),
    Unknown(&'static str),
}
#[cfg(windows)]
struct Answer {
    request_id: WorkbenchRequestId,
    site_id: SiteId,
    action: AgentManifestsAction,
    observation: Observation,
    interrupted: bool,
    cleanup_failed: bool,
}
#[cfg(windows)]
impl Answer {
    fn document(&self) -> serde_json::Value {
        let observation = match &self.observation {
            Observation::Server(reply) => {
                serde_json::json!({"kind":"server","reply":WorkbenchResponse::AgentManifests(reply.clone())})
            }
            Observation::Unknown(reason) => serde_json::json!({"kind":"unknown","reason":reason}),
        };
        let extended = matches!(self.action, AgentManifestsAction::Update { .. })
            || match &self.observation {
                Observation::Server(reply) => match &reply.outcome {
                    AgentManifestsOutcome::Current { snapshot } => snapshot.maintenance.is_some(),
                    AgentManifestsOutcome::Operation {
                        status:
                            AgentManifestsOperation::Updated { .. }
                            | AgentManifestsOperation::Unknown { .. },
                        ..
                    } => true,
                    AgentManifestsOutcome::Failed {
                        reason:
                            AgentManifestsFailure::Conflict | AgentManifestsFailure::UpdateUnavailable,
                    } => true,
                    _ => false,
                },
                _ => false,
            };
        serde_json::json!({"schema":"orchbee.site.agent-manifests","version":if extended {2} else {1},"requestId":self.request_id,"siteId":self.site_id,"action":self.action,"observation":observation,"interrupted":self.interrupted,"cleanupFailed":self.cleanup_failed})
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
                | AgentManifestsFailure::SiteMismatch
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
            Failure::new(FailureKind::ServiceUnavailable),
            Some(format),
            locale,
        )
    }
}
