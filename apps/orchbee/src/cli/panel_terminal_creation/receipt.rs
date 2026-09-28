use crate::{
    cli::{Failure, FailureKind, Output, failure_output, output},
    invocation::OutputFormat,
    localization::Locale,
};
use orchbee_workbench_protocol::{
    WorkbenchCommandOutcome as Outcome, WorkbenchTerminalOutcome as Terminal,
};
use serde_json::Value;
pub(super) fn failure(outcome: &Outcome) -> Option<FailureKind> {
    match outcome {
        Outcome::PanelTerminalPending { .. }
        | Outcome::PanelTerminalCreated {
            terminal_outcome: Terminal::Started { .. },
            ..
        } => None,
        Outcome::PanelTerminalCreated {
            terminal_outcome: Terminal::OutcomeUnknown {},
            ..
        } => Some(FailureKind::OutcomeUnknown),
        Outcome::PanelTerminalCreated {
            terminal_outcome: Terminal::Unavailable { .. },
            ..
        }
        | Outcome::Unavailable { .. } => Some(FailureKind::ServiceUnavailable),
        Outcome::PanelTerminalCreated {
            terminal_outcome: Terminal::Failed { .. },
            ..
        }
        | Outcome::Rejected { .. } => Some(FailureKind::Rejected),
        Outcome::Denied {} => Some(FailureKind::Denied),
        Outcome::LaunchProfileUnavailable {} => Some(FailureKind::NotFound),
        Outcome::LaunchProfileAmbiguous { .. } => Some(FailureKind::Ambiguous),
        _ => Some(FailureKind::ProtocolInvalid),
    }
}
pub(super) fn emit(
    document: &Value,
    failure: Option<FailureKind>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let status = failure.map_or(0, FailureKind::exit_code);
    let bytes = match format {
        OutputFormat::Json => output::encode_document(document),
        OutputFormat::Text => {
            let mut text = locale.cli_panel_terminal_create(document);
            if let Some(kind) = failure {
                text.push_str(&locale.cli_failure(&Failure::new(kind)));
            }
            output::encode_text(&text)
        }
    };
    match bytes {
        Ok(bytes) if status != 0 && format == OutputFormat::Text => Output {
            status,
            stdout: vec![],
            stderr: String::from_utf8(bytes).expect("UTF-8 output"),
            finalization: None,
        },
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
