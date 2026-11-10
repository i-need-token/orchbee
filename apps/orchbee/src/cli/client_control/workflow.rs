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
        console_interrupt::{ArmedSignals, Finish, SignalWatch},
        failure_output, query,
        stale_retry::{Ended, StaleRetry},
    },
    invocation::{ClientCommand, ClientNavigationTarget, OutputFormat},
    local_client::LocalClient,
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewControl, ClientViewIntent, ClientViewOutcome, ClientViewPhase,
    ClientViewSequence, Snapshot, WorkbenchClientId,
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
        && crate::cli::layout_topology::explicit(target)
    {
        return crate::cli::layout_topology::run(
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
            operation_id,
            detach,
            ..
        } => {
            let navigation = Navigation {
                target,
                explicit: explicit.as_ref(),
                caller: caller.as_ref(),
                wait: !detach,
            };
            // A caller-named operation identity is the caller's handle; it is never replaced.
            let retry = StaleRetry::new(operation_id.is_none());
            return navigate(
                &mut client,
                snapshot,
                &navigation,
                retry,
                deadline,
                signals,
                answer,
            );
        }
    };
    if interrupted(answer, signals, deadline)? {
        return Ok(());
    }
    exchange_and_wait(
        &mut client,
        &snapshot.session_id,
        action,
        false,
        deadline,
        signals,
        answer,
    )
}

/// What one navigation names, resolved again by each attempt.
struct Navigation<'a> {
    target: &'a ClientNavigationTarget,
    explicit: Option<&'a WorkbenchClientId>,
    caller: Option<&'a PanelTarget>,
    wait: bool,
}

/// The exact Client control one attempt resolved: the Client, the view revision it fences and the
/// intent. The Client and intent are the identity a retry must find again.
type Resolved = (WorkbenchClientId, ClientViewSequence, ClientViewIntent);

/// Resolve and submit one navigation under the stale-view retry (ORC-413). A control refused
/// before any action as `stale-view`, or an Agent read the Workbench outran, reads a fresh
/// snapshot and resolves the Client and intent again under the original deadline, at most
/// `stale_retry::RETRIES` times. A refused control is submitted again, under a fresh operation
/// identity, only when its Client and intent are unchanged; otherwise the last refusal stays the
/// answer. An unknown, incomplete or still pending outcome is never submitted again.
fn navigate(
    client: &mut LocalClient,
    mut snapshot: Snapshot,
    navigation: &Navigation<'_>,
    mut retry: StaleRetry<(WorkbenchClientId, ClientViewIntent)>,
    deadline: Instant,
    signals: &mut ArmedSignals,
    answer: &mut Answer,
) -> Result<(), Failure> {
    loop {
        let refused = answer.reply.is_some();
        // A retry keeps the refusal it holds: its reads record no expiry in the answer, and a
        // captured signal still settles through the signal owner.
        let mut scratch = Answer {
            status_only: false,
            operation_id: None,
            session_id: None,
            reply: None,
            unknown: false,
            wait: None,
        };
        let resolution = resolve(
            client,
            &snapshot,
            navigation,
            deadline,
            signals,
            if refused { &mut scratch } else { &mut *answer },
        );
        let (client_id, view_revision, intent) = match resolution {
            Ok(Some(resolved)) => resolved,
            Ok(None) => return Ok(()),
            Err(failure) => {
                if retry.reread(Ended::resolution(&failure))
                    && let Ok(fresh) = query::refresh_snapshot(
                        client,
                        &snapshot.session_id,
                        snapshot.revision,
                        deadline,
                        signals,
                    )
                {
                    snapshot = fresh;
                    continue;
                }
                return if refused { Ok(()) } else { Err(failure) };
            }
        };
        if !retry.submit((client_id.clone(), intent.clone())) {
            return Ok(());
        }
        if refused {
            if signals.check_before(deadline).is_err() {
                return Ok(());
            }
            answer.operation_id = Some(query::request_id()?);
            answer.reply = None;
        } else if interrupted(answer, signals, deadline)? {
            return Ok(());
        }
        let operation_id = answer
            .operation_id
            .clone()
            .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
        let action = ClientViewAction::Submit {
            command: Box::new(ClientViewControl {
                operation_id,
                client_id,
                expected_view_revision: view_revision,
                minimum_workbench_revision: snapshot.revision,
                intent,
            }),
        };
        exchange_and_wait(
            client,
            &snapshot.session_id,
            action,
            navigation.wait,
            deadline,
            signals,
            answer,
        )?;
        let ended = if answer.stale_refusal() {
            Ended::Stale
        } else {
            Ended::Settled
        };
        if !retry.reread(ended) {
            return Ok(());
        }
        match query::refresh_snapshot(
            client,
            &snapshot.session_id,
            snapshot.revision,
            deadline,
            signals,
        ) {
            Ok(fresh) => snapshot = fresh,
            Err(_) => return Ok(()),
        }
    }
}

/// Resolve the Client and intent one navigation names against `snapshot`. `None` is an
/// interruption that `answer` records.
fn resolve(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    navigation: &Navigation<'_>,
    deadline: Instant,
    signals: &mut ArmedSignals,
    answer: &mut Answer,
) -> Result<Option<Resolved>, Failure> {
    let caller = navigation
        .caller
        .map(|caller| caller.resolve(snapshot))
        .transpose()?;
    let reply = exchange(
        client,
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
    let selected = selection::select(&clients, navigation.explicit, caller.as_ref())?;
    let intent = if let ClientNavigationTarget::Agent(agent) = navigation.target {
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
            return Ok(None);
        }
        let result =
            query::agent_focus_target(client, snapshot, agent, deadline, signals.poll_fd());
        // Resolve/Get faults can carry provisional cancellation or expiry;
        // the existing signal/deadline owner settles them before projection.
        if interrupted(answer, signals, deadline)? {
            return Ok(None);
        }
        result?
    } else if let ClientNavigationTarget::AgentAttention(agent) = navigation.target {
        let result =
            query::agent_attention_target(client, snapshot, agent, deadline, signals.poll_fd());
        if interrupted(answer, signals, deadline)? {
            return Ok(None);
        }
        result?
    } else {
        selection::intent(navigation.target, selected, caller.as_ref(), snapshot)?
    };
    Ok(Some((
        selected.client_id.clone(),
        selected.view_revision,
        intent,
    )))
}
