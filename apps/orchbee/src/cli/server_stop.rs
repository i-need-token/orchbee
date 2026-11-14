//! Session stop receipts remain inspectable across Client disconnection and Server exit.
use super::output;
use super::{Failure, FailureKind, Output, failure_output};
use crate::cli_text::CliText;
use crate::{
    invocation::{OutputFormat, ServerStopCommand},
    localization::Locale,
};
use orchbee_local_client_protocol::{ConnectClientError, StopId};
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

/// Submission truth survives native exchange failure so the workflow never replays a mutation.
struct ExchangeFailure {
    failure: ConnectClientError,
    submitted: bool,
}

/// `server stop` or `server status` from the command line; a stop first passes destructive
/// admission, which may prompt for the Session name and logs the request.
pub(crate) fn run_command(
    command: &ServerStopCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if let ServerStopCommand::Stop { confirm, .. } = command
        && let Err(refusal) = super::destructive_request::admit(
            super::destructive_request::DestructiveCommand::ServerStop,
            *confirm,
            None,
            format,
            locale,
        )
    {
        return *refusal;
    }
    run(command, format, locale)
}

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

/// How `uninstall` found one Session after asking it to stop.
pub(in crate::cli) enum SessionEnd {
    /// The Session completed its stop, or ended through `exit` with no terminal open.
    Stopped,
    /// The Session did not end; the token names why, as `server stop` reports it.
    Failed(&'static str),
}

/// Stops the Session whose runtime directory is `runtime` exactly as `session stop` does, for
/// `uninstall`, which has already admitted the request: a Session of this build stops through its
/// own stop request, and one another build started ends only when it holds no terminal.
pub(in crate::cli) fn stop_session(
    runtime: &std::path::Path,
    deadline: std::time::Instant,
    signals: &mut super::console_interrupt::ArmedSignals,
) -> SessionEnd {
    let mut bytes = [0; 16];
    if getrandom::fill(&mut bytes).is_err() {
        return SessionEnd::Failed("entropy");
    }
    let Ok(id) = StopId::new(bytes) else {
        return SessionEnd::Failed("entropy");
    };
    let Ok(session) =
        orchbee_local_runtime::ModeCheckedLocalSessionRendezvous::from_runtime_directory(runtime)
    else {
        return SessionEnd::Failed("runtime");
    };
    let mut answer = Answer {
        id,
        observation: Observation::Unavailable("runtime"),
        wait: None,
        query: false,
    };
    workflow::observe_session(
        &session,
        orchbee_local_client_protocol::StopAction::Start,
        false,
        deadline,
        &mut answer,
        signals,
    );
    match (answer.observation, answer.wait) {
        (_, Some(wait)) => SessionEnd::Failed(wait),
        (Observation::Stopped | Observation::Exited, None) => SessionEnd::Stopped,
        (Observation::Unavailable(reason) | Observation::Unknown(reason), None) => {
            SessionEnd::Failed(reason)
        }
        (Observation::Server(_), None) => SessionEnd::Failed("stop-failed"),
    }
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
            &json!({"schema":"orchbee.server.stop","version":1,"source":crate::cli_outcome::local_runtime_source(),"result":result}),
        ),
        OutputFormat::Text => output::encode_text(&CliText::new(locale).cli_server_stop_result(
            result["outcome"].as_str().expect("outcome"),
            result["reason"].as_str(),
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
