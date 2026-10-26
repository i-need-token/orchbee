//! Site stop receipts remain inspectable across Client disconnection and Server exit.
#[cfg(windows)]
use super::output;
use super::{Failure, FailureKind, Output, failure_output};
use crate::{
    invocation::{OutputFormat, SiteStopCommand},
    localization::Locale,
};
#[cfg(windows)]
use orchbee_local_client_protocol::StopId;
#[cfg(windows)]
use serde_json::{Value, json};

#[cfg(windows)]
mod reply;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use reply::{Answer, Observation, render};

#[cfg(not(windows))]
pub(crate) fn run(command: &SiteStopCommand, format: OutputFormat, locale: Locale) -> Output {
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

/// Chooses one request identity and returns a bounded stop/status result without Site activation.
#[cfg(windows)]
pub(crate) fn run(command: &SiteStopCommand, format: OutputFormat, locale: Locale) -> Output {
    let id = match command {
        SiteStopCommand::Status { id } | SiteStopCommand::Stop { id: Some(id), .. } => *id,
        SiteStopCommand::Stop { id: None, .. } => {
            let mut bytes = [0; 16];
            if getrandom::fill(&mut bytes).is_err() || bytes == [0; 16] {
                return failure_output(
                    Failure::new(FailureKind::InternalFailure),
                    Some(format),
                    locale,
                );
            }
            StopId::new(bytes).expect("validated entropy")
        }
    };
    windows::run(command, id, format, locale)
}

#[cfg(windows)]
fn id_text(id: StopId) -> String {
    id.bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(windows)]
fn emit(
    id: StopId,
    result: Value,
    status: u8,
    wait: Option<&str>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let encoded = match format {
        OutputFormat::Json => output::encode_document(
            &json!({"schema":"orchbee.site.stop","version":1,"source":super::config::local_runtime_source(),"result":result}),
        ),
        OutputFormat::Text => output::encode_text(&locale.cli_site_stop_result(
            result["outcome"].as_str().expect("outcome"),
            &id_text(id),
            wait,
        )),
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
