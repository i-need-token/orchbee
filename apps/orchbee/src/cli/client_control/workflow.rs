//! Submits exact admitted Client controls and observes their correlated monotonic status under
//! one cancellable deadline. It does not activate an OS window or undo an admitted control
//! when the caller stops waiting.

use super::{
    Answer, Wait, emit,
    exchange::{exchange, exchange_and_wait, interrupted},
    selection,
};
use crate::{
    cli::{
        Failure, FailureKind, Output,
        caller_context::PanelTarget,
        console_interrupt::{ArmedSignals, Finish},
        failure_output, query,
    },
    invocation::{ClientCommand, ClientNavigationTarget, OutputFormat},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewControl, ClientViewOutcome, ClientViewPhase,
};
use std::time::{Duration, Instant};

pub(super) fn run(command: &ClientCommand, format: OutputFormat, locale: Locale) -> Output {
    if let ClientCommand::Navigate {
        target,
        client: None,
        operation_id,
        timeout_ms,
        ..
    } = command
        && crate::cli::layouts::structure::explicit(target)
    {
        return crate::cli::layouts::structure::run(
            target,
            operation_id.as_ref(),
            *timeout_ms,
            format,
            locale,
        );
    }
    if let ClientCommand::Swap(command) = command {
        return super::swap::run(command, format, locale);
    }
    let timeout = match command {
        ClientCommand::Navigate { timeout_ms, .. } => *timeout_ms,
        _ => 5_000,
    };
    let deadline = Instant::now() + Duration::from_millis(u64::from(timeout));
    let caller = if matches!(command, ClientCommand::Navigate { client: None, .. }) {
        match PanelTarget::prepare_optional(|name| std::env::var_os(name)) {
            Ok(caller) => caller,
            Err(error) => return failure_output(error, Some(format), locale),
        }
    } else {
        None
    };
    let operation_id = match command {
        ClientCommand::Navigate { operation_id, .. } => match operation_id
            .clone()
            .map(Ok)
            .unwrap_or_else(query::request_id)
        {
            Ok(id) => Some(id),
            Err(error) => return failure_output(error, Some(format), locale),
        },
        ClientCommand::ControlStatus(id) => Some(id.clone()),
        _ => None,
    };
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
    let mut answer = Answer {
        status_only: matches!(command, ClientCommand::ControlStatus(_)),
        operation_id,
        session_id: None,
        reply: None,
        unknown: false,
        wait: None,
    };
    let result = observe(command, caller, deadline, &mut signals, &mut answer);
    let mut finalization = None;
    let result = match signals.finish() {
        Ok(Finish::Clear) => result,
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            if result.is_err() || answer.pending() || answer.reply.is_none() {
                answer.wait = Some(Wait::Cancelled);
            }
            Ok(())
        }
        Err(_) => Err(Failure::new(FailureKind::InternalFailure)),
    };
    let result = if result.is_err() && (answer.reply.is_some() || answer.unknown) {
        answer.wait = Some(Wait::ObservationFailed);
        Ok(())
    } else {
        result
    };
    let mut output = match result {
        Ok(()) if answer.operation_id.is_none() && answer.reply.is_none() => failure_output(
            Failure::new(if answer.wait == Some(Wait::Cancelled) {
                FailureKind::Cancelled
            } else {
                FailureKind::Timeout
            }),
            Some(format),
            locale,
        ),
        Ok(()) => match command {
            ClientCommand::Navigate {
                target: ClientNavigationTarget::Layout { query, .. },
                ..
            } => super::layout::emit(&answer, *query, format, locale),
            _ => emit(&answer, format, locale),
        },
        Err(error) => failure_output(error, Some(format), locale),
    };
    output.finalization = finalization;
    output
}
fn observe(
    command: &ClientCommand,
    caller: Option<PanelTarget>,
    deadline: Instant,
    signals: &mut ArmedSignals,
    answer: &mut Answer,
) -> Result<(), Failure> {
    if interrupted(answer, signals, deadline)? {
        return Ok(());
    }
    let (mut client, _, snapshot) = query::snapshot_session_until(deadline)?;
    answer.session_id = Some(snapshot.session_id.clone());
    if interrupted(answer, signals, deadline)? {
        return Ok(());
    }
    let action = match command {
        ClientCommand::Swap(_) => return Err(Failure::new(FailureKind::InternalFailure)),
        ClientCommand::List => ClientViewAction::List {},
        ClientCommand::Get(client_id) => ClientViewAction::Get {
            client_id: client_id.clone(),
        },
        ClientCommand::ControlStatus(operation_id) => ClientViewAction::Read {
            operation_id: operation_id.clone(),
        },
        ClientCommand::Navigate {
            target,
            client: explicit,
            ..
        } => {
            let caller = caller.map(|caller| caller.resolve(&snapshot)).transpose()?;
            let reply = exchange(
                &mut client,
                &snapshot.session_id,
                ClientViewAction::List {},
                deadline,
                signals,
            )?;
            let clients = match reply.outcome {
                ClientViewOutcome::Listed { clients } => clients,
                ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
                ClientViewOutcome::Unavailable { .. } => {
                    return Err(Failure::new(FailureKind::ServiceUnavailable));
                }
                ClientViewOutcome::Synchronized { .. }
                | ClientViewOutcome::Found { .. }
                | ClientViewOutcome::Operation { .. }
                | ClientViewOutcome::NotFound { .. }
                | ClientViewOutcome::Conflict { .. } => {
                    return Err(Failure::new(FailureKind::ProtocolInvalid));
                }
            };
            let selected = selection::select(&clients, explicit.as_ref(), caller.as_ref())?;
            let intent = if let ClientNavigationTarget::Agent(agent) = target {
                if !matches!(
                    selected.view.phase,
                    ClientViewPhase::Ready | ClientViewPhase::Busy
                ) {
                    return Err(selection::failure(
                        FailureKind::ServiceUnavailable,
                        "client-not-ready",
                        vec![selected.client_id.to_string()],
                    ));
                }
                if interrupted(answer, signals, deadline)? {
                    return Ok(());
                }
                let result = query::agent_focus_target(
                    &mut client,
                    &snapshot,
                    agent,
                    deadline,
                    signals.poll_fd(),
                );
                // Resolve/Get faults can carry provisional cancellation or expiry;
                // the existing signal/deadline owner settles them before projection.
                if interrupted(answer, signals, deadline)? {
                    return Ok(());
                }
                result?
            } else if let ClientNavigationTarget::AgentAttention(agent) = target {
                let result = query::agent_attention_target(
                    &mut client,
                    &snapshot,
                    agent,
                    deadline,
                    signals.poll_fd(),
                );
                if interrupted(answer, signals, deadline)? {
                    return Ok(());
                }
                result?
            } else {
                selection::intent(target, selected, caller.as_ref(), &snapshot)?
            };
            let operation_id = answer
                .operation_id
                .clone()
                .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
            ClientViewAction::Submit {
                command: Box::new(ClientViewControl {
                    operation_id,
                    client_id: selected.client_id.clone(),
                    expected_view_revision: selected.view_revision,
                    minimum_workbench_revision: snapshot.revision,
                    intent,
                }),
            }
        }
    };
    if interrupted(answer, signals, deadline)? {
        return Ok(());
    }
    exchange_and_wait(
        &mut client,
        &snapshot.session_id,
        action,
        matches!(command, ClientCommand::Navigate { detach: false, .. }),
        deadline,
        signals,
        answer,
    )
}
