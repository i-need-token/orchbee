use super::{Answer, Observation};
use crate::{
    cli::{
        Failure, FailureKind, Output, failure_output, query,
        source::{ArmedSignals, Finish, SignalWatch},
    },
    invocation::{ManifestCommand, OutputFormat},
    local_client::workbench::metadata_exchange::MetadataExchangeReason,
    localization::Locale,
};
use orchbee_workbench_protocol::{
    AgentManifestsAction, AgentManifestsOperation, AgentManifestsOutcome, AgentManifestsRequest,
};
use std::time::{Duration, Instant};
pub(super) fn run(command: &ManifestCommand, format: OutputFormat, locale: Locale) -> Output {
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
    command: &ManifestCommand,
    signals: &mut ArmedSignals,
    deadline: Instant,
) -> Result<Answer, Failure> {
    signals.check_before(deadline)?;
    let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
    let action = match command {
        ManifestCommand::Update { operation_id } => AgentManifestsAction::Update {
            operation_id: match operation_id {
                Some(id) => id.clone(),
                None => query::request_id()?,
            },
        },
        ManifestCommand::Current => AgentManifestsAction::Current {},
        ManifestCommand::Reload { operation_id } => AgentManifestsAction::Reload {
            operation_id: match operation_id {
                Some(id) => id.clone(),
                None => query::request_id()?,
            },
        },
        ManifestCommand::Status { operation_id } => AgentManifestsAction::Operation {
            operation_id: operation_id.clone(),
        },
    };
    let mut request = AgentManifestsRequest {
        protocol_version: snapshot.protocol_version,
        request_id: query::request_id()?,
        site_id: snapshot.site_id,
        action: action.clone(),
    };
    signals.check_before(deadline)?;
    let reply = client.agent_manifests(request.clone(), deadline, signals.poll_fd());
    let observation = match reply {
        Ok(reply) => Observation::Server(Box::new(reply)),
        Err(error) if error.submitted => Observation::Unknown(error.reason.as_str()),
        Err(error) => {
            signals.check_before(deadline)?;
            return Err(Failure::new(match error.reason {
                MetadataExchangeReason::SessionUnavailable => FailureKind::ServiceUnavailable,
                MetadataExchangeReason::Encoding => FailureKind::InternalFailure,
                MetadataExchangeReason::Deadline => FailureKind::Timeout,
                _ => FailureKind::TransportLoss,
            }));
        }
    };
    let mut answer = Answer {
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        action,
        observation,
        interrupted: false,
        cleanup_failed: false,
    };
    // Once admitted, cancellation retains the last receipt. Each subsequent exchange is status only.
    while matches!(
        answer.action,
        AgentManifestsAction::Reload { .. } | AgentManifestsAction::Update { .. }
    ) && matches!(&answer.observation, Observation::Server(reply) if matches!(reply.outcome, AgentManifestsOutcome::Operation { status:AgentManifestsOperation::Pending {}, .. }))
    {
        if signals.check_before(deadline).is_err() {
            break;
        }
        std::thread::sleep(
            Duration::from_millis(25).min(deadline.saturating_duration_since(Instant::now())),
        );
        if signals.check_before(deadline).is_err() {
            break;
        }
        let (AgentManifestsAction::Reload { operation_id }
        | AgentManifestsAction::Update { operation_id }) = &answer.action
        else {
            break;
        };
        let Ok(id) = query::request_id() else {
            answer.cleanup_failed = true;
            break;
        };
        request.request_id = id;
        request.action = AgentManifestsAction::Operation {
            operation_id: operation_id.clone(),
        };
        match client.agent_manifests(request.clone(), deadline, signals.poll_fd()) {
            Ok(reply) => {
                // Status queries still belong to the original mutation kind.
                let expected = AgentManifestsRequest {
                    action: answer.action.clone(),
                    ..request.clone()
                };
                if !reply.answers(&expected) {
                    break;
                }
                answer.request_id = request.request_id.clone();
                answer.observation = Observation::Server(Box::new(reply));
            }
            Err(_) => break, // The pending receipt remains true even when its status cannot be refreshed.
        }
    }
    Ok(answer)
}
