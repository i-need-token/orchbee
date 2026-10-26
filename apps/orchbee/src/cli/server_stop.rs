//! Session stop receipts remain inspectable across Client disconnection and Server exit.
use super::output;
use super::{Failure, FailureKind, Output, failure_output};
use crate::{
    invocation::{OutputFormat, ServerStopCommand},
    localization::Locale,
};
use orchbee_local_client_protocol::StopId;
use serde_json::{Value, json};

mod reply;
#[cfg(windows)]
mod windows;
mod workflow;
#[cfg(windows)]
use windows as platform;
#[cfg(not(windows))]
mod unix;
use reply::{Answer, Observation, render};
#[cfg(not(windows))]
use unix as platform;

/// Chooses one request identity and returns a bounded stop/status result without Session activation.
pub(crate) fn run(command: &ServerStopCommand, format: OutputFormat, locale: Locale) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::ServerStop) {
        return failure_output(failure, Some(format), locale);
    }
    let id = match command {
        ServerStopCommand::Status { id } | ServerStopCommand::Stop { id: Some(id), .. } => *id,
        ServerStopCommand::Stop { id: None, .. } => {
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
    workflow::run(command, id, format, locale)
}

fn id_text(id: StopId) -> String {
    id.bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

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
            &json!({"schema":"orchbee.server.stop","version":1,"source":super::config::local_runtime_source(),"result":result}),
        ),
        OutputFormat::Text => output::encode_text(&locale.cli_server_stop_result(
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
