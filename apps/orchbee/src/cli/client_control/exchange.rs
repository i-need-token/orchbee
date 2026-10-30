//! One Client control submission followed by correlated, monotonic retained reads.
//! Cancellation and transport loss preserve the independently known submission facts.
use super::{Answer, Wait};
use crate::{
    cli::{Failure, FailureKind, console_interrupt::SignalWatch, query},
    local_client::{LocalClient, workbench::client_view::ClientViewExchangeError},
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewControl, ClientViewControlStatus, ClientViewOutcome,
    ProtocolVersion, SessionId, WorkbenchClientViewRequest, WorkbenchClientViewResult,
};
use std::{
    thread,
    time::{Duration, Instant},
};
pub(in crate::cli) fn submit_control(
    client: &mut LocalClient,
    session_id: &SessionId,
    command: ClientViewControl,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<Answer, Failure> {
    let mut answer = Answer {
        status_only: false,
        operation_id: Some(command.operation_id.clone()),
        session_id: Some(session_id.clone()),
        reply: None,
        unknown: false,
        wait: None,
    };
    if !interrupted(&mut answer, signals, deadline)? {
        let result = exchange_and_wait(
            client,
            session_id,
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

pub(super) fn exchange_and_wait(
    client: &mut LocalClient,
    session_id: &SessionId,
    action: ClientViewAction,
    wait: bool,
    deadline: Instant,
    signals: &mut impl SignalWatch,
    answer: &mut Answer,
) -> Result<(), Failure> {
    // Every subsequent exchange is a retained Read. A lost Submit is never replayed.
    let submitted = matches!(action, ClientViewAction::Submit { .. });
    match exchange(client, session_id, action, deadline, signals) {
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
            session_id,
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

/// Observe an already submitted operation without repeating its mutation.
pub(in crate::cli) fn wait_submitted(
    client: &mut LocalClient,
    reply: WorkbenchClientViewResult,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<Answer, Failure> {
    let ClientViewOutcome::Operation { operation } = &reply.outcome else {
        return Err(Failure::new(FailureKind::ProtocolInvalid));
    };
    let id = operation.command.operation_id.clone();
    let command = operation.command.clone();
    let session = reply.session_id.clone();
    let mut answer = Answer {
        status_only: false,
        operation_id: Some(id.clone()),
        session_id: Some(session.clone()),
        reply: Some(reply),
        unknown: false,
        wait: None,
    };
    if answer.pending() {
        exchange_and_wait(
            client,
            &session,
            ClientViewAction::Read { operation_id: id },
            true,
            deadline,
            signals,
            &mut answer,
        )?;
    }
    answer.retain_matching_command(&command);
    Ok(answer)
}
fn verify_attention(
    client: &mut LocalClient,
    reply: &WorkbenchClientViewResult,
    deadline: Instant,
    signals: &impl SignalWatch,
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
            ClientViewControlStatus::PopupOpened { .. }
            | ClientViewControlStatus::PopupClosed { .. }
            | ClientViewControlStatus::PresentationRead { .. }
            | ClientViewControlStatus::ViewportRead { .. }
            | ClientViewControlStatus::NavigationRead { .. }
            | ClientViewControlStatus::TerminalAttentionRead { .. }
            | ClientViewControlStatus::AttentionRead { .. }
            | ClientViewControlStatus::LayoutAdjusted { .. }
            | ClientViewControlStatus::DirectionFocused { .. }
            | ClientViewControlStatus::DirectionUnchanged { .. }
            | ClientViewControlStatus::LayoutRead { .. }
            | ClientViewControlStatus::Applied { .. }
            | ClientViewControlStatus::ZoomUnchanged { .. }
            | ClientViewControlStatus::ZoomToggled { .. }
            | ClientViewControlStatus::Rejected { .. }
            | ClientViewControlStatus::Incomplete { .. } => previous.status == next.status,
        }
}
pub(in crate::cli) fn exchange(
    client: &mut LocalClient,
    session_id: &SessionId,
    action: ClientViewAction,
    deadline: Instant,
    signals: &impl SignalWatch,
) -> Result<WorkbenchClientViewResult, Failure> {
    let request = WorkbenchClientViewRequest {
        protocol_version: ProtocolVersion,
        request_id: query::request_id()?,
        session_id: session_id.clone(),
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
pub(super) fn interrupted(
    answer: &mut Answer,
    signals: &mut impl SignalWatch,
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

/// The failure class of one finished Client view submission: its process exit code, refined by
/// the typed outcome for a rejected observation step.
pub(in crate::cli) fn submission_failure(
    code: u8,
    outcome: Option<&ClientViewOutcome>,
) -> Option<FailureKind> {
    Some(match code {
        0 => return None,
        3 => match outcome {
            Some(ClientViewOutcome::Denied {}) => FailureKind::Denied,
            Some(ClientViewOutcome::NotFound {}) => FailureKind::NotFound,
            // This step consumes its requested ClientView receipt class; other replies cannot confirm that observation.
            _ => FailureKind::Rejected,
        },
        4 => FailureKind::ServiceUnavailable,
        5 | 8 => FailureKind::TransportLoss,
        7 => FailureKind::Timeout,
        130 => FailureKind::Cancelled,
        _ => FailureKind::ProtocolInvalid,
    })
}
