//! Submits bounded Workspace report reads or writes through the admitted Server and preserves
//! exact correlated observations. It does not change canonical Workspace membership or own
//! report persistence.

use super::{Answer, Observation};
use crate::{
    cli::{
        Failure, FailureKind, Output,
        console_interrupt::{ArmedSignals, Finish, SignalWatch},
        failure_output, query,
    },
    invocation::{OutputFormat, Resource, WorkspaceMetadataCommand},
    local_client::workbench::metadata_exchange::MetadataExchangeReason,
    localization::Locale,
};
use orchbee_workbench_protocol::{WorkspaceMetadataAction, WorkspaceMetadataRequest};
use std::time::{Duration, Instant};
pub(super) fn run(
    command: &WorkspaceMetadataCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
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
    command: &WorkspaceMetadataCommand,
    signals: &mut ArmedSignals,
    deadline: Instant,
) -> Result<Answer, Failure> {
    signals.check_before(deadline)?;
    let (mut client, _, snapshot) = query::snapshot_session_until(deadline)?;
    signals.check_before(deadline)?;
    let workspace = super::super::results::resolve_or_fail(
        snapshot.navigation.workspaces.iter().map(|workspace| {
            (
                workspace.workspace_id.as_str(),
                workspace.label.as_str(),
                workspace,
            )
        }),
        &command.target,
        Resource::Workspace,
    )?;
    let request = WorkspaceMetadataRequest {
        protocol_version: snapshot.protocol_version,
        request_id: query::request_id()?,
        session_id: snapshot.session_id,
        action: WorkspaceMetadataAction::Report {
            workspace_id: workspace.workspace_id.clone(),
            source: command.source.clone(),
            tokens: command.tokens.clone(),
            sequence: command.sequence,
            ttl_milliseconds: command.ttl_milliseconds,
        },
    };
    signals.check_before(deadline)?;
    let result = client.workspace_metadata(request.clone(), deadline, signals.poll_fd());
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
        workspace_id: workspace.workspace_id.clone(),
        observation,
        interrupted: false,
        cleanup_failed: false,
    })
}
