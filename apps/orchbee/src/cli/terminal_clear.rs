//! Existing-Site, exact-target, one-shot clear; submitted uncertainty is never replayed.
use super::{
    Failure, FailureKind, Output, failure_output, output, query, results,
    source::{ArmedSignals, Finish, SignalWatch},
};
use crate::{
    invocation::{OutputFormat, Target},
    local_client::SubmissionError,
    localization::Locale,
};
use orchbee_client_server_protocol::{
    TerminalControlAction, TerminalControlOutcome, TerminalControlRequest, TerminalViewerAction,
    TerminalViewerRequest,
};
use serde_json::json;
use std::time::{Duration, Instant};

pub(crate) fn run(target: &Target, format: OutputFormat, locale: Locale) -> Output {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let mut result = execute(target, format, locale, deadline, &mut signals)
        .unwrap_or_else(|failure| failure_output(failure, Some(format), locale));
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled { finalization, .. }) => {
            result.status = 130;
            result.finalization = Some(finalization);
        }
        Err(_) => result.status = 1,
    }
    result
}
fn execute(
    target: &Target,
    format: OutputFormat,
    locale: Locale,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Output, Failure> {
    signals.check_before(deadline)?;
    let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
    let source = results::snapshot_source(&snapshot);
    let (terminal, viewer, _, _, _) = query::attach_viewer_cancellable(
        &mut client,
        deadline,
        &snapshot,
        target,
        signals.poll_fd(),
    )
    .map_err(|failure| failure.source(source.clone()))?;
    let operation = query::obcs_request_id()?;
    let effect = (|| {
        signals.check_before(deadline)?;
        let request = TerminalControlRequest::new(
            operation.clone(),
            TerminalControlAction::Clear {
                attachment_id: viewer.clone(),
            },
        );
        match client.submit_terminal_control_cancellable(request, Some(deadline), signals.poll_fd())
        {
            Ok(result) => Ok(result.into_outcome()),
            Err(SubmissionError::OutcomeUnknown { .. }) => {
                Ok(TerminalControlOutcome::OutcomeUnknown {})
            }
            Err(error) => Err(query::classify_viewer(error)),
        }
    })();
    if let Ok(id) = query::obcs_request_id() {
        let _ = client.submit_terminal_viewer_cancellable(
            TerminalViewerRequest::new(
                id,
                TerminalViewerAction::Detach {
                    attachment_id: viewer,
                },
            ),
            Some(deadline),
            signals.poll_fd(),
        );
    }
    drop(client);
    let outcome = effect.map_err(|failure| failure.source(source.clone()))?;
    let status = match outcome {
        TerminalControlOutcome::Cleared {
            cleanup_confirmed: true,
            ..
        } => 0,
        TerminalControlOutcome::Cleared {
            cleanup_confirmed: false,
            ..
        }
        | TerminalControlOutcome::OutcomeUnknown {} => 8,
        TerminalControlOutcome::Rejected { .. } => 3,
        TerminalControlOutcome::Unavailable { .. } => 4,
        TerminalControlOutcome::Acquired { .. }
        | TerminalControlOutcome::InputAccepted {}
        | TerminalControlOutcome::Resized { .. }
        | TerminalControlOutcome::Pasted { .. }
        | TerminalControlOutcome::HostColorsAccepted {} => {
            return Err(Failure::new(FailureKind::ProtocolInvalid).source(source));
        }
    };
    let stdout = if format == OutputFormat::Json {
        output::encode_document(
            &json!({"schema":"orchbee.terminal.clear","version":1,"source":source,
            "result":{"terminal":results::terminal_row_json(&terminal),"operationId":operation.as_str(),"outcome":outcome}}),
        )
        .map_err(|_| Failure::new(FailureKind::OutputTooLarge))?
    } else {
        locale
            .cli_terminal_clear(&terminal.terminal_id, &outcome)
            .into_bytes()
    };
    Ok(Output {
        status,
        stdout,
        stderr: String::new(),
        finalization: None,
    })
}

#[cfg(test)]
mod tests;
