use super::{Answer, Wait, emit, selection};
use crate::{
    cli::{
        Failure, FailureKind, Output,
        caller_context::PanelTarget,
        failure_output, query,
        source::{ArmedSignals, Finish, SignalWatch},
    },
    invocation::{ClientCommand, ClientNavigationTarget, OutputFormat},
    local_client::{LocalClient, workbench::client_view::ClientViewExchangeError},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewControl, ClientViewControlStatus, ClientViewOutcome,
    ClientViewPhase, ProtocolVersion, SiteId, WorkbenchClientViewRequest,
    WorkbenchClientViewResult,
};
use std::{
    thread,
    time::{Duration, Instant},
};

pub(super) fn run(command: &ClientCommand, format: OutputFormat, locale: Locale) -> Output {
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
        site_id: None,
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
    answer.site_id = Some(snapshot.site_id.clone());
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
                &snapshot.site_id,
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
                _ => return Err(Failure::new(FailureKind::ProtocolInvalid)),
            };
            let selected = selection::select(&clients, explicit.as_ref(), caller.as_ref())?;
            let intent = if let ClientNavigationTarget::Agent(agent) = target {
                if selected.view.phase != ClientViewPhase::Ready {
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
        &snapshot.site_id,
        action,
        matches!(command, ClientCommand::Navigate { detach: false, .. }),
        deadline,
        signals,
        answer,
    )
}

pub(in crate::cli) fn submit_control(
    client: &mut LocalClient,
    site_id: &SiteId,
    command: ClientViewControl,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Answer, Failure> {
    let mut answer = Answer {
        status_only: false,
        operation_id: Some(command.operation_id.clone()),
        site_id: Some(site_id.clone()),
        reply: None,
        unknown: false,
        wait: None,
    };
    if !interrupted(&mut answer, signals, deadline)? {
        let result = exchange_and_wait(
            client,
            site_id,
            ClientViewAction::Submit {
                command: Box::new(command),
            },
            true,
            deadline,
            signals,
            &mut answer,
        );
        if let Err(error) = result {
            if answer.reply.is_none() && !answer.unknown {
                return Err(error);
            }
            answer.wait = Some(Wait::ObservationFailed);
        }
    }
    Ok(answer)
}

fn exchange_and_wait(
    client: &mut LocalClient,
    site_id: &SiteId,
    action: ClientViewAction,
    wait: bool,
    deadline: Instant,
    signals: &mut ArmedSignals,
    answer: &mut Answer,
) -> Result<(), Failure> {
    // Every subsequent exchange is a retained Read. A lost Submit is never replayed.
    let submitted = matches!(action, ClientViewAction::Submit { .. });
    match exchange(client, site_id, action, deadline, signals) {
        Ok(reply) => {
            verify_attention(client, &reply, deadline, signals)?;
            answer.reply = Some(reply);
        }
        Err(error) if submitted => {
            answer.unknown = true;
            let _ = error;
            return Ok(());
        }
        Err(error) => return Err(error),
    }
    if !submitted {
        return Ok(());
    }
    if !wait {
        if answer.pending() {
            answer.wait = Some(Wait::Detached);
        }
        return Ok(());
    }
    while answer.pending() {
        if interrupted(answer, signals, deadline)? {
            return Ok(());
        }
        thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
        if interrupted(answer, signals, deadline)? {
            return Ok(());
        }
        let operation_id = answer
            .operation_id
            .clone()
            .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
        match exchange(
            client,
            site_id,
            ClientViewAction::Read { operation_id },
            deadline,
            signals,
        ) {
            Ok(reply)
                if answer
                    .reply
                    .as_ref()
                    .is_some_and(|previous| monotonic(previous, &reply)) =>
            {
                if let Err(error) = verify_attention(client, &reply, deadline, signals) {
                    answer.reply = None;
                    return Err(error);
                }
                answer.reply = Some(reply)
            }
            Ok(_) => {
                answer.wait = Some(Wait::ObservationFailed);
                return Ok(());
            }
            Err(_) => {
                if !interrupted(answer, signals, deadline)? {
                    answer.wait = Some(Wait::TransportLost);
                }
                return Ok(());
            }
        }
    }
    Ok(())
}
fn verify_attention(
    client: &mut LocalClient,
    reply: &WorkbenchClientViewResult,
    deadline: Instant,
    signals: &ArmedSignals,
) -> Result<(), Failure> {
    if let ClientViewOutcome::Operation { operation } = &reply.outcome
        && let ClientViewControlStatus::AttentionRead { attention, .. } = &operation.status
    {
        query::authenticate_attention(client, attention, deadline, signals.poll_fd())?;
    }
    if let ClientViewOutcome::Operation { operation } = &reply.outcome
        && let ClientViewControlStatus::TerminalAttentionRead { attention, .. } = &operation.status
    {
        let snapshot = query::read_agent_snapshot(client, deadline, signals.poll_fd())?;
        for entry in &attention.entries {
            let report = query::terminal_agent::read(
                client,
                &snapshot,
                &entry.binding,
                None,
                deadline,
                signals.poll_fd(),
            )?;
            if attention.report_revision > report.report_revision
                || entry
                    .state
                    .as_ref()
                    .is_some_and(|s| s.sequence > report.observation.sequence)
            {
                return Err(Failure::new(FailureKind::ProtocolInvalid));
            }
        }
    }
    Ok(())
}
fn monotonic(previous: &WorkbenchClientViewResult, next: &WorkbenchClientViewResult) -> bool {
    let (
        ClientViewOutcome::Operation {
            operation: previous,
        },
        ClientViewOutcome::Operation { operation: next },
    ) = (&previous.outcome, &next.outcome)
    else {
        return false;
    };
    previous.command == next.command
        && match previous.status {
            ClientViewControlStatus::Queued {} => true,
            ClientViewControlStatus::Delivered {} => {
                !matches!(next.status, ClientViewControlStatus::Queued {})
            }
            _ => previous.status == next.status,
        }
}
pub(in crate::cli) fn exchange(
    client: &mut LocalClient,
    site_id: &SiteId,
    action: ClientViewAction,
    deadline: Instant,
    signals: &ArmedSignals,
) -> Result<WorkbenchClientViewResult, Failure> {
    let request = WorkbenchClientViewRequest {
        protocol_version: ProtocolVersion,
        request_id: query::request_id()?,
        site_id: site_id.clone(),
        action,
    };
    client
        .exchange_client_view(
            request,
            deadline.min(Instant::now() + Duration::from_secs(2)),
            signals.poll_fd(),
        )
        .map_err(|error| {
            Failure::new(match error {
                ClientViewExchangeError::Unavailable => FailureKind::ServiceUnavailable,
                ClientViewExchangeError::Encoding => FailureKind::InternalFailure,
                ClientViewExchangeError::Protocol => FailureKind::ProtocolInvalid,
                ClientViewExchangeError::Timeout => FailureKind::Timeout,
                ClientViewExchangeError::Transport => FailureKind::TransportLoss,
            })
        })
}
fn interrupted(
    answer: &mut Answer,
    signals: &mut ArmedSignals,
    deadline: Instant,
) -> Result<bool, Failure> {
    if signals
        .take()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?
        .is_some()
    {
        answer.wait = Some(Wait::Cancelled);
        Ok(true)
    } else if Instant::now() >= deadline {
        answer.wait = Some(Wait::TimedOut);
        Ok(true)
    } else {
        Ok(false)
    }
}
