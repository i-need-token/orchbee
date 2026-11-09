//! A display-data report is submitted once; its acknowledgement survives late cancellation.
use super::output;
use super::{Failure, FailureKind, Output, failure_output};
use crate::cli_text::CliText;
use crate::{
    invocation::{OutputFormat, PanelAgentCommand},
    localization::Locale,
};
mod workflow;
use orchbee_workbench_protocol::{
    PanelAgentFailure, PanelAgentOutcome, PanelAgentResult, SessionId, WorkbenchPanelId,
    WorkbenchRequestId, WorkbenchResponse,
};
#[derive(Debug)]
enum Observation {
    Server(PanelAgentResult),
    Unknown(&'static str),
}
#[derive(Debug)]
struct Answer {
    request_id: WorkbenchRequestId,
    session_id: SessionId,
    panel_id: WorkbenchPanelId,
    presentation_id: Option<orchbee_workbench_protocol::TerminalPresentationId>,
    observation: Observation,
    interrupted: bool,
    cleanup_failed: bool,
}
impl Answer {
    fn document(&self) -> serde_json::Value {
        let observation = match &self.observation {
            Observation::Server(reply) => {
                serde_json::json!({"kind":"server", "reply":WorkbenchResponse::PanelAgent(reply.clone())})
            }
            Observation::Unknown(reason) => serde_json::json!({"kind":"unknown", "reason":reason}),
        };
        let value = serde_json::json!({"schema":"orchbee.panel.agent-report", "version":1,
            "requestId":self.request_id,"sessionId":self.session_id,"panelId":self.panel_id,"presentationId":self.presentation_id,
            "observation":observation,"interrupted":self.interrupted,"cleanupFailed":self.cleanup_failed});
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
                | PanelAgentOutcome::Identified { .. }
                | PanelAgentOutcome::Observed { .. }
                | PanelAgentOutcome::States { .. }
                | PanelAgentOutcome::Titles { .. }
                | PanelAgentOutcome::PresentationTitles { .. } => 5,
                PanelAgentOutcome::Failed { reason } => match reason {
                    PanelAgentFailure::Denied
                    | PanelAgentFailure::NotFound
                    | PanelAgentFailure::Unbound
                    | PanelAgentFailure::BindingChanged
                    | PanelAgentFailure::SessionMismatch
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
                let mut text = CliText::new(locale).cli_panel_agent(
                    self.request_id.as_str(),
                    self.panel_id.as_str(),
                    outcome,
                    unknown,
                );
                if let Some(id) = &self.presentation_id {
                    text.push_str(&format!(
                        "{}: {}\n",
                        CliText::new(locale).cli_report_presentation_label(),
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
                    CliText::new(locale).cli_failure(&Failure::new(FailureKind::InternalFailure))
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
    if let Err(failure) = super::execution::admit(super::execution::Capability::Reports) {
        return failure_output(failure, Some(format), locale);
    }
    workflow::run(command, format, locale)
}

#[cfg(test)]
mod tests;
