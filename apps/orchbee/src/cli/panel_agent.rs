//! A display-data report is submitted once; its acknowledgement survives late cancellation.
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::{
    invocation::{OutputFormat, PanelAgentCommand},
    localization::Locale,
};
#[cfg(windows)]
mod windows;
#[cfg(any(windows, test))]
use orchbee_workbench_protocol::{
    PanelAgentFailure, PanelAgentOutcome, PanelAgentResult, SiteId, WorkbenchPanelId,
    WorkbenchRequestId, WorkbenchResponse,
};
#[cfg(any(windows, test))]
#[derive(Debug)]
enum Observation {
    Server(PanelAgentResult),
    Unknown(&'static str),
}
#[cfg(any(windows, test))]
#[derive(Debug)]
struct Answer {
    request_id: WorkbenchRequestId,
    site_id: SiteId,
    panel_id: WorkbenchPanelId,
    presentation_id: Option<orchbee_workbench_protocol::TerminalPresentationId>,
    observation: Observation,
    interrupted: bool,
    cleanup_failed: bool,
}
#[cfg(any(windows, test))]
impl Answer {
    fn document(&self) -> serde_json::Value {
        let observation = match &self.observation {
            Observation::Server(reply) => {
                serde_json::json!({"kind":"server", "reply":WorkbenchResponse::PanelAgent(reply.clone())})
            }
            Observation::Unknown(reason) => serde_json::json!({"kind":"unknown", "reason":reason}),
        };
        let mut value = serde_json::json!({"schema":"orchbee.panel.agent-report", "version":if self.presentation_id.is_some() {2} else {1},
            "requestId":self.request_id,"siteId":self.site_id,"panelId":self.panel_id,
            "observation":observation,"interrupted":self.interrupted,"cleanupFailed":self.cleanup_failed});
        if let Some(id) = &self.presentation_id {
            value["presentationId"] = serde_json::json!(id);
        }
        value
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
                PanelAgentOutcome::Reported { .. }
                | PanelAgentOutcome::PresentationReported { .. } => 0,
                PanelAgentOutcome::Read { .. }
                | PanelAgentOutcome::PresentationsRead { .. }
                | PanelAgentOutcome::Explained { .. }
                | PanelAgentOutcome::Observed { .. }
                | PanelAgentOutcome::States { .. } => 5,
                PanelAgentOutcome::Failed { reason } => match reason {
                    PanelAgentFailure::Denied
                    | PanelAgentFailure::NotFound
                    | PanelAgentFailure::Unbound
                    | PanelAgentFailure::BindingChanged
                    | PanelAgentFailure::SiteMismatch
                    | PanelAgentFailure::HistoryLimit
                    | PanelAgentFailure::SourceLimit
                    | PanelAgentFailure::ResourceLimit => 3,
                    PanelAgentFailure::Unavailable => 4,
                    PanelAgentFailure::InvalidRequest => 5,
                    PanelAgentFailure::RevisionExhausted
                    | PanelAgentFailure::ClockOverflow
                    | PanelAgentFailure::ResultTooLarge => 1,
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
                let mut text = locale.cli_panel_agent(
                    self.request_id.as_str(),
                    self.panel_id.as_str(),
                    outcome,
                    unknown,
                );
                if let Some(id) = &self.presentation_id {
                    text.push_str(&format!(
                        "{}: {}\n",
                        locale.cli_report_presentation_label(),
                        id
                    ));
                }
                output::encode_text(&text)
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
pub(crate) fn run(command: &PanelAgentCommand, format: OutputFormat, locale: Locale) -> Output {
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
    use orchbee_workbench_protocol::{PanelAgentDisposition, ProtocolVersion, WorkbenchRevision};
    #[test]
    fn panel_agent_receipts_survive_late_interrupt_and_validate_the_public_schema() {
        let mut answer = Answer {
            presentation_id: None,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap(),
            site_id: SiteId::parse(&format!("{:032x}", 2)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 3)).unwrap(),
            observation: Observation::Unknown("response-deadline"),
            interrupted: true,
            cleanup_failed: true,
        };
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("schemas/orchbee.panel.agent-report.json")).unwrap();
        let validator = jsonschema::draft202012::new(&schema).unwrap();
        assert_eq!(answer.exit_code(), 8);
        assert!(validator.is_valid(&answer.document()));
        answer.observation = Observation::Server(PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: answer.request_id.clone(),
            site_id: answer.site_id.clone(),
            outcome: PanelAgentOutcome::Reported {
                panel_id: answer.panel_id.clone(),
                binding: orchbee_workbench_protocol::PresentationBinding {
                    worker_id: orchbee_workbench_protocol::WorkerId::parse(&format!("{:032x}", 4))
                        .unwrap(),
                    terminal_id: orchbee_workbench_protocol::TerminalId::parse(&format!(
                        "{:032x}",
                        5
                    ))
                    .unwrap(),
                },
                workbench_revision: WorkbenchRevision::new(5),
                report_revision: WorkbenchRevision::new(9),
                disposition: PanelAgentDisposition::Applied,
                changed: true,
                view: Box::new(orchbee_workbench_protocol::PanelAgentView {
                    agent: None,
                    state: orchbee_workbench_protocol::PanelAgentState::Unknown,
                    authority_source: None,
                    message: None,
                    session: None,
                }),
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
        for disposition in [
            PanelAgentDisposition::Applied,
            PanelAgentDisposition::Ignored,
            PanelAgentDisposition::Deferred,
        ] {
            if let Observation::Server(reply) = &mut answer.observation
                && let PanelAgentOutcome::Reported {
                    disposition: actual,
                    ..
                } = &mut reply.outcome
            {
                *actual = disposition;
            }
            assert_eq!(answer.exit_code(), 0);
            assert!(validator.is_valid(&answer.document()));
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let text = answer.emit(OutputFormat::Text, locale);
                assert_eq!(text.status, 0);
                assert!(!text.stdout.contains(&0x1b));
            }
        }
        let mut invalid = answer.document();
        invalid["observation"]["reply"]["outcome"]["disposition"] = serde_json::json!("invented");
        assert!(!validator.is_valid(&invalid));
    }
}
