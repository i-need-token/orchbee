//! A display-data report is submitted once; its acknowledgement survives late cancellation.
#[cfg(any(windows, test))]
use super::output;
use super::{Failure, FailureKind, Output, failure_output};
use crate::{
    invocation::{OutputFormat, PanelMetadataCommand},
    localization::Locale,
};
#[cfg(windows)]
mod windows;
#[cfg(any(windows, test))]
use orchbee_workbench_protocol::{
    PanelMetadataFailure, PanelMetadataOutcome, PanelMetadataResult, SiteId, WorkbenchPanelId,
    WorkbenchRequestId, WorkbenchResponse,
};
#[cfg(any(windows, test))]
#[derive(Debug)]
enum Observation {
    Server(PanelMetadataResult),
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
                serde_json::json!({"kind":"server", "reply":WorkbenchResponse::PanelMetadata(reply.clone())})
            }
            Observation::Unknown(reason) => serde_json::json!({"kind":"unknown", "reason":reason}),
        };
        let value = serde_json::json!({"schema":"orchbee.panel.metadata", "version":1,
            "requestId":self.request_id,"siteId":self.site_id,"panelId":self.panel_id,"presentationId":self.presentation_id,
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
                PanelMetadataOutcome::Reported { .. }
                | PanelMetadataOutcome::PresentationReported { .. } => 0,
                PanelMetadataOutcome::Read { .. }
                | PanelMetadataOutcome::PresentationsRead { .. } => 5,
                PanelMetadataOutcome::Failed { reason } => match reason {
                    PanelMetadataFailure::Denied
                    | PanelMetadataFailure::NotFound
                    | PanelMetadataFailure::Unbound
                    | PanelMetadataFailure::BindingChanged
                    | PanelMetadataFailure::SiteMismatch
                    | PanelMetadataFailure::TokenLimit
                    | PanelMetadataFailure::SourceLimit
                    | PanelMetadataFailure::ResourceLimit => 3,
                    PanelMetadataFailure::Unavailable => 4,
                    PanelMetadataFailure::InvalidRequest => 5,
                    PanelMetadataFailure::RevisionExhausted
                    | PanelMetadataFailure::DeadlineOverflow
                    | PanelMetadataFailure::ResultTooLarge => 1,
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
                let mut text = locale.cli_panel_metadata(
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
pub(crate) fn run(command: &PanelMetadataCommand, format: OutputFormat, locale: Locale) -> Output {
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
