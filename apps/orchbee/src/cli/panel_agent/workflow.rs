//! Executes bounded exact-target Panel Agent report requests through the Server and maps
//! correlated outcomes to CLI observations. It does not interpret terminal output as a report
//! or manufacture process proof on the Client.

use super::{Answer, Observation};
use crate::{
    cli::{
        Failure, FailureKind, Output,
        console_interrupt::{ArmedSignals, Finish, SignalWatch},
        failure_output, query,
    },
    invocation::{OutputFormat, PanelAgentCommand},
    local_client::MetadataExchangeReason,
    localization::Locale,
};
use orchbee_workbench_protocol::PanelAgentRequest;
use std::time::{Duration, Instant};
pub(super) fn run(command: &PanelAgentCommand, format: OutputFormat, locale: Locale) -> Output {
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
    let mut result = submit(command, &mut signals, deadline);
    let mut finalization = None;
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            match &mut result {
                Ok(answer) => answer.interrupted = true,
                Err(_) => result = Err(Failure::new(FailureKind::Cancelled)),
            }
        }
        Err(_) => match &mut result {
            Ok(answer) => answer.cleanup_failed = true,
            Err(_) => result = Err(Failure::new(FailureKind::InternalFailure)),
        },
    }
    let mut output = match result {
        Ok(answer) => answer.emit(format, locale),
        Err(failure) => failure_output(failure, Some(format), locale),
    };
    output.finalization = finalization;
    output
}
fn submit(
    command: &PanelAgentCommand,
    signals: &mut ArmedSignals,
    deadline: Instant,
) -> Result<Answer, Failure> {
    signals.check_before(deadline)?;
    let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
    signals.check_before(deadline)?;
    let (panel_id, binding) = crate::cli::report_target::resolve(
        &snapshot,
        &command.target,
        command.presentation_id.as_ref(),
    )?;
    let request = PanelAgentRequest {
        protocol_version: snapshot.protocol_version,
        request_id: query::request_id()?,
        session_id: snapshot.session_id,
        action: command.action(panel_id.clone(), binding),
    };
    signals.check_before(deadline)?;
    let result = client.panel_agent(request.clone(), deadline, signals.poll_fd());
    // After handoff, never replace a receipt or unknown outcome with timeout/cancellation.
    let observation = match result {
        Ok(reply) => Observation::Server(reply),
        Err(error) if error.submitted => Observation::Unknown(error.reason.as_str()),
        Err(error) => {
            signals.check_before(deadline)?;
            return Err(Failure::new(match error.reason {
                MetadataExchangeReason::ConnectionUnavailable => FailureKind::ServiceUnavailable,
                MetadataExchangeReason::Encoding => FailureKind::InternalFailure,
                MetadataExchangeReason::Deadline => FailureKind::Timeout,
                _ => FailureKind::TransportLoss,
            }));
        }
    };
    Ok(Answer {
        request_id: request.request_id,
        session_id: request.session_id,
        panel_id: panel_id.clone(),
        presentation_id: command.presentation_id.clone(),
        observation,
        interrupted: false,
        cleanup_failed: false,
    })
}
