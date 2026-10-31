//! One canonical leaf exchange followed by independently acknowledged source focus.
use super::{Answer, exchange, selection};
use crate::{
    cli::{
        Failure, FailureKind, Output,
        caller_context::PanelTarget,
        console_interrupt::{ArmedSignals, Finish},
        failure_output, query,
    },
    invocation::{
        ClientNavigationTarget, OutputFormat, PanelSwapCommand, PanelSwapDestination, Target,
    },
    local_client::{
        LocalClient, WorkbenchPanelPlacementNotSubmitted, WorkbenchPanelPlacementPrecommit,
        WorkbenchPanelPlacementRequest, WorkbenchPanelPlacementSubmission,
    },
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientPanelDirection, ClientViewAction, ClientViewAvailability, ClientViewControl,
    ClientViewControlStatus, ClientViewIntent, ClientViewOutcome, ClientViewPhase,
    ClientViewRecord, ClientViewSelection, ProtocolVersion, SessionId, Snapshot, WorkbenchAction,
    WorkbenchClientId, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchPanelId,
    WorkbenchRequestId, WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId,
};
use std::{
    thread,
    time::{Duration, Instant},
};

mod receipt;
use receipt::{Canonical, Panel, Receipt};

pub(super) fn run(command: &PanelSwapCommand, format: OutputFormat, locale: Locale) -> Output {
    let deadline = Instant::now() + Duration::from_millis(u64::from(command.timeout_ms));
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
    let mut receipt = None;
    let mut result = execute(command, deadline, &mut signals, &mut receipt);
    let mut finalization = None;
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            result = Err(Failure::new(FailureKind::Cancelled));
        }
        Err(_) => result = Err(Failure::new(FailureKind::InternalFailure)),
    }
    let mut output = if let Some(mut receipt) = receipt {
        if let Err(error) = result {
            receipt.failure = Some(error.kind);
        }
        receipt.emit(format, locale)
    } else {
        failure_output(
            result
                .err()
                .unwrap_or_else(|| Failure::new(FailureKind::InternalFailure)),
            Some(format),
            locale,
        )
    };
    output.finalization = finalization;
    output
}

fn check_wait(signals: &mut ArmedSignals, deadline: Instant) -> Result<(), Failure> {
    if signals
        .take()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?
        .is_some()
    {
        Err(Failure::new(FailureKind::Cancelled))
    } else if Instant::now() >= deadline {
        Err(Failure::new(FailureKind::Timeout))
    } else {
        Ok(())
    }
}

fn panel(
    target: &Option<Target>,
    client: &ClientViewRecord,
    caller: Option<&Target>,
    snapshot: &Snapshot,
) -> Result<Panel, Failure> {
    let intent = selection::intent(
        &ClientNavigationTarget::Panel(target.clone()),
        client,
        caller,
        snapshot,
    )?;
    let ClientViewIntent::Panel {
        terminal_context_id,
        tab_id,
        panel_id,
    } = intent
    else {
        return Err(Failure::new(FailureKind::InternalFailure));
    };
    Ok(Panel {
        terminal_context_id,
        tab_id,
        panel_id,
    })
}

fn execute(
    command: &PanelSwapCommand,
    deadline: Instant,
    signals: &mut ArmedSignals,
    output: &mut Option<Receipt>,
) -> Result<(), Failure> {
    check_wait(signals, deadline)?;
    if command.client.is_none()
        && let (Some(source), PanelSwapDestination::Pair(target)) =
            (&command.source, &command.destination)
    {
        return execute_shared(source, target, deadline, signals, output);
    }
    let caller = if command.client.is_none() {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
    } else {
        None
    };
    let (mut client, _, snapshot) = query::snapshot_session_until(deadline)?;
    let caller = caller.map(|caller| caller.resolve(&snapshot)).transpose()?;
    check_wait(signals, deadline)?;
    let reply = exchange::exchange(
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
    let selected = selection::select(&clients, command.client.as_ref(), caller.as_ref())?;
    if selected.view.phase != ClientViewPhase::Ready {
        return Err(Failure::new(FailureKind::ServiceUnavailable));
    }
    let source = panel(&command.source, selected, caller.as_ref(), &snapshot)?;
    *output = Some(Receipt {
        session_id: snapshot.session_id.clone(),
        client_id: Some(selected.client_id.clone()),
        base_revision: snapshot.revision,
        source,
        target: None,
        direction: None,
        layout_operation_id: None,
        layout_revision: None,
        request_id: None,
        canonical: Canonical::NotSubmitted,
        focus: None,
        stage: "resolution",
        reason: None,
        failure: None,
    });
    let receipt = output
        .as_mut()
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    match &command.destination {
        PanelSwapDestination::Pair(target) => {
            let target = panel(&Some(target.clone()), selected, None, &snapshot)?;
            let reason = if !receipt.source.same_tab(&target) {
                Some("cross-tab")
            } else if receipt.source.panel_id == target.panel_id {
                Some("same-panel")
            } else {
                None
            };
            receipt.target = Some(target);
            if let Some(reason) = reason {
                return unchanged(receipt, reason);
            }
        }
        PanelSwapDestination::Direction(direction) => {
            receipt.direction = Some(*direction);
            if !resolve_neighbor(
                &mut client,
                selected,
                *direction,
                receipt,
                deadline,
                signals,
            )? {
                return unchanged(receipt, "no-neighbor");
            }
        }
    }
    check_wait(signals, deadline)?;
    receipt.stage = "exchange";
    if !exchange(&mut client, receipt, deadline, signals)? {
        return Ok(());
    }
    receipt.stage = "client-observation";
    let Canonical::Committed { revision } = receipt.canonical else {
        return Err(Failure::new(FailureKind::InternalFailure));
    };
    let current = observe_client(
        &mut client,
        receipt,
        &selected.view.selection,
        revision,
        deadline,
        signals,
    )?;
    check_wait(signals, deadline)?;
    receipt.stage = "focus";
    let control = ClientViewControl {
        operation_id: query::request_id()?,
        client_id: receipt
            .client_id
            .clone()
            .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?,
        expected_view_revision: current.view_revision,
        minimum_workbench_revision: revision,
        intent: receipt.source.intent(),
    };
    receipt.focus = Some(exchange::submit_control(
        &mut client,
        &receipt.session_id,
        control,
        deadline,
        signals,
    )?);
    if receipt.focused() {
        receipt.stage = "complete";
    }
    Ok(())
}

fn unchanged(receipt: &mut Receipt, reason: &'static str) -> Result<(), Failure> {
    receipt.canonical = Canonical::Unchanged {
        revision: receipt.base_revision,
    };
    receipt.reason = Some(reason);
    receipt.stage = "complete";
    Ok(())
}

fn execute_shared(
    source: &Target,
    target: &Target,
    deadline: Instant,
    signals: &mut ArmedSignals,
    output: &mut Option<Receipt>,
) -> Result<(), Failure> {
    use crate::cli::layout_topology as structure;
    use crate::invocation::ClientLayoutTarget;
    let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
    check_wait(signals, deadline)?;
    let resolve = |target: &Target| {
        structure::resolve(&snapshot, &ClientLayoutTarget::Panel(Some(target.clone()))).map(
            |resolved| Panel {
                terminal_context_id: resolved.context.terminal_context_id.clone(),
                tab_id: resolved.tab.tab_id.clone(),
                panel_id: resolved.panel,
            },
        )
    };
    let source = resolve(source)?;
    let target = resolve(target)?;
    *output = Some(Receipt {
        session_id: snapshot.session_id.clone(),
        client_id: None,
        base_revision: snapshot.revision,
        source,
        target: Some(target),
        direction: None,
        layout_operation_id: None,
        layout_revision: None,
        request_id: None,
        canonical: Canonical::NotSubmitted,
        focus: None,
        stage: "resolution",
        reason: None,
        failure: None,
    });
    let receipt = output
        .as_mut()
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    let target = receipt
        .target
        .as_ref()
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    if !receipt.source.same_tab(target) {
        return unchanged(receipt, "cross-tab");
    }
    if receipt.source.panel_id == target.panel_id {
        return unchanged(receipt, "same-panel");
    }
    check_wait(signals, deadline)?;
    receipt.stage = "exchange";
    if exchange(&mut client, receipt, deadline, signals)? {
        receipt.stage = "complete";
    }
    Ok(())
}

fn resolve_neighbor(
    client: &mut LocalClient,
    selected: &ClientViewRecord,
    direction: ClientPanelDirection,
    receipt: &mut Receipt,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<bool, Failure> {
    let operation_id = query::request_id()?;
    receipt.layout_operation_id = Some(operation_id.clone());
    let control = ClientViewControl {
        operation_id,
        client_id: selected.client_id.clone(),
        expected_view_revision: selected.view_revision,
        minimum_workbench_revision: receipt.base_revision,
        intent: ClientViewIntent::Layout {
            terminal_context_id: receipt.source.terminal_context_id.clone(),
            tab_id: receipt.source.tab_id.clone(),
            panel_id: receipt.source.panel_id.clone(),
        },
    };
    let answer = exchange::submit_control(client, &receipt.session_id, control, deadline, signals)?;
    let layout = answer.reply.as_ref().and_then(|reply| {
        let ClientViewOutcome::Operation { operation } = &reply.outcome else {
            return None;
        };
        let ClientViewControlStatus::LayoutRead { layout, .. } = &operation.status else {
            return None;
        };
        Some(layout)
    });
    let Some(layout) = layout.filter(|_| answer.exit_code() == 0) else {
        receipt.reason = Some("layout-unconfirmed");
        return Err(Failure::new(match answer.exit_code() {
            130 => FailureKind::Cancelled,
            7 => FailureKind::Timeout,
            8 => FailureKind::OutcomeUnknown,
            4 => FailureKind::ServiceUnavailable,
            5 => FailureKind::TransportLoss,
            _ => FailureKind::Denied,
        }));
    };
    if layout.workbench_revision != receipt.base_revision {
        receipt.reason = Some("layout-revision-changed");
        return Err(Failure::new(FailureKind::Denied));
    }
    receipt.layout_revision = Some(layout.workbench_revision);
    if !layout.source_available {
        receipt.reason = Some("source-unavailable");
        return Err(Failure::new(FailureKind::NotFound));
    }
    let Some(target) = layout.neighbors.get(direction) else {
        return Ok(false);
    };
    receipt.target = Some(Panel {
        terminal_context_id: receipt.source.terminal_context_id.clone(),
        tab_id: receipt.source.tab_id.clone(),
        panel_id: target.clone(),
    });
    Ok(true)
}

fn exchange(
    client: &mut LocalClient,
    receipt: &mut Receipt,
    deadline: Instant,
    signals: &ArmedSignals,
) -> Result<bool, Failure> {
    let target = receipt
        .target
        .as_ref()
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    let request_id = query::request_id()?;
    receipt.request_id = Some(request_id.clone());
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id,
        session_id: receipt.session_id.clone(),
        expected_revision: receipt.base_revision,
        action: WorkbenchAction::PanelSwap {
            terminal_context_id: receipt.source.terminal_context_id.clone(),
            tab_id: receipt.source.tab_id.clone(),
            panel_id: receipt.source.panel_id.clone(),
            target_panel_id: target.panel_id.clone(),
        },
    };
    let request = WorkbenchPanelPlacementRequest::try_from(command)
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    match client.submit_workbench_panel_placement_with_wake(request, deadline, signals.poll_fd()) {
        WorkbenchPanelPlacementSubmission::Committed(commit) => {
            receipt.canonical = Canonical::Committed {
                revision: commit.revision,
            };
            Ok(true)
        }
        WorkbenchPanelPlacementSubmission::Unchanged { revision } => {
            receipt.canonical = Canonical::Unchanged { revision };
            receipt.stage = "complete";
            Ok(false)
        }
        WorkbenchPanelPlacementSubmission::OutcomeUnknown { .. } => {
            receipt.canonical = Canonical::Unknown;
            Ok(false)
        }
        WorkbenchPanelPlacementSubmission::DefinitelyNotSubmitted(reason) => {
            Err(Failure::new(match reason {
                WorkbenchPanelPlacementNotSubmitted::RequestNotSubmitted(error)
                    if error.kind() == std::io::ErrorKind::TimedOut =>
                {
                    FailureKind::Timeout
                }
                WorkbenchPanelPlacementNotSubmitted::RequestEncoding(_) => {
                    FailureKind::InternalFailure
                }
                _ => FailureKind::ServiceUnavailable,
            }))
        }
        WorkbenchPanelPlacementSubmission::DefinitePrecommit(precommit) => {
            let (outcome, kind) = match precommit {
                WorkbenchPanelPlacementPrecommit::Denied => {
                    (WorkbenchCommandOutcome::Denied {}, FailureKind::Denied)
                }
                WorkbenchPanelPlacementPrecommit::Rejected { revision, reason } => (
                    WorkbenchCommandOutcome::Rejected { revision, reason },
                    FailureKind::Rejected,
                ),
                WorkbenchPanelPlacementPrecommit::Unavailable { reason } => (
                    WorkbenchCommandOutcome::Unavailable { reason },
                    FailureKind::ServiceUnavailable,
                ),
            };
            receipt.canonical = Canonical::Refused {
                outcome: Box::new(outcome),
            };
            receipt.failure = Some(kind);
            Ok(false)
        }
    }
}

fn observe_client(
    client: &mut LocalClient,
    receipt: &mut Receipt,
    before: &Option<ClientViewSelection>,
    minimum: WorkbenchRevision,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<ClientViewRecord, Failure> {
    loop {
        check_wait(signals, deadline)?;
        let reply = exchange::exchange(
            client,
            &receipt.session_id,
            ClientViewAction::Get {
                client_id: receipt
                    .client_id
                    .clone()
                    .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?,
            },
            deadline,
            signals,
        )?;
        let observed = match reply.outcome {
            ClientViewOutcome::Found { client } => *client,
            ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
            ClientViewOutcome::NotFound {} | ClientViewOutcome::Unavailable { .. } => {
                return Err(Failure::new(FailureKind::ServiceUnavailable));
            }
            ClientViewOutcome::Synchronized { .. }
            | ClientViewOutcome::Listed { .. }
            | ClientViewOutcome::Operation { .. }
            | ClientViewOutcome::Conflict { .. } => {
                return Err(Failure::new(FailureKind::ProtocolInvalid));
            }
        };
        if observed.availability != ClientViewAvailability::Current
            || observed.view.phase == ClientViewPhase::Unavailable
        {
            receipt.reason = Some("client-unavailable");
            return Err(Failure::new(FailureKind::ServiceUnavailable));
        }
        if &observed.view.selection != before {
            receipt.reason = Some("client-selection-changed");
            return Err(Failure::new(FailureKind::Denied));
        }
        if observed.view.phase == ClientViewPhase::Ready
            && observed.view.workbench_revision >= minimum
        {
            return Ok(observed);
        }
        thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}
