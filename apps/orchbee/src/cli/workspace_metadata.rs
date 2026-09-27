//! A display-data report is submitted once; its acknowledgement survives late cancellation.
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::{
    invocation::{OutputFormat, WorkspaceMetadataCommand},
    localization::Locale,
};
#[cfg(windows)]
mod windows;
#[cfg(any(windows, test))]
use orchbee_workbench_protocol::{
    SiteId, WorkbenchRequestId, WorkbenchResponse, WorkbenchWorkspaceId, WorkspaceMetadataFailure,
    WorkspaceMetadataOutcome, WorkspaceMetadataResult,
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
    site_id: SiteId,
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
            "requestId":self.request_id,"siteId":self.site_id,"workspaceId":self.workspace_id,
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
                    | WorkspaceMetadataFailure::SiteMismatch
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
            Failure::new(FailureKind::ServiceUnavailable),
            Some(format),
            locale,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchbee_workbench_protocol::{
        MetadataReportDisposition, ProtocolVersion, WorkbenchRevision,
    };
    #[test]
    fn workspace_metadata_receipts_survive_late_interrupt_and_validate_the_public_schema() {
        let mut answer = Answer {
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
            site_id: SiteId::parse(&format!("{:032x}", 2)).unwrap(),
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 3)).unwrap(),
            observation: Observation::Unknown("response-deadline"),
            interrupted: true,
            cleanup_failed: true,
        };
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("schemas/orchbee.workspace.metadata.json")).unwrap();
        let validator = jsonschema::draft202012::new(&schema).unwrap();
        assert_eq!(answer.exit_code(), 8);
        assert!(validator.is_valid(&answer.document()));
        answer.observation = Observation::Server(WorkspaceMetadataResult {
            protocol_version: ProtocolVersion,
            request_id: answer.request_id.clone(),
            site_id: answer.site_id.clone(),
            outcome: WorkspaceMetadataOutcome::Reported {
                workspace_id: answer.workspace_id.clone(),
                workbench_revision: WorkbenchRevision::new(5),
                metadata_revision: WorkbenchRevision::new(9),
                disposition: MetadataReportDisposition::Applied,
            },
        });
        assert_eq!(answer.exit_code(), 1);
        answer.cleanup_failed = false;
        assert_eq!(answer.exit_code(), 0);
        assert!(validator.is_valid(&answer.document()));
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let emitted = answer.emit(OutputFormat::Text, locale);
            assert_eq!(emitted.status, 0);
            assert!(emitted.stderr.is_empty());
            assert!(!emitted.stdout.contains(&0x1b));
            assert!(
                String::from_utf8(emitted.stdout)
                    .unwrap()
                    .contains(answer.request_id.as_str())
            );
        }
        let mut invalid = answer.document();
        invalid["observation"]["reply"]["outcome"]["disposition"] = serde_json::json!("invented");
        assert!(!validator.is_valid(&invalid));
    }
}
