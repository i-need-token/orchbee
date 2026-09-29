use super::{
    receipt::{Receipt, applied},
    targeting::{self, Created},
};
use crate::{
    cli::{
        Failure, FailureKind, Output,
        caller_context::PanelTarget,
        client_control::{selection, windows as control},
        failure_output, mutation, query,
        source::{ArmedSignals, Finish, SignalWatch},
    },
    invocation::{CreationCommand, CreationKind, OutputFormat},
    local_client::LocalClient,
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientSplitIdentity, ClientSplitPath, ClientViewAction, ClientViewAvailability,
    ClientViewControl, ClientViewIntent, ClientViewOutcome, ClientViewPhase, ClientViewRecord,
    PanelNode, ProtocolVersion, SiteId, Snapshot, SnapshotReadIntent, WorkbenchPanelId,
    WorkbenchPanelSlotId, WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchSplitId,
    WorkbenchTabId, WorkbenchTerminalContextId,
};
use std::{
    thread,
    time::{Duration, Instant},
};

pub(super) fn run(command: &CreationCommand, format: OutputFormat, locale: Locale) -> Output {
    let settings = match crate::cli::configuration::load() {
        Ok(settings) => settings,
        Err(error) => return failure_output(error.failure(), Some(format), locale),
    };
    let mut configured = command.clone();
    if configured.options.plugin_pane.is_none() {
        settings.apply_directory(&mut configured.directory, &mut configured.options);
    }
    let command = &configured;

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
    let mut cancelled = false;
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            cancelled = true;
            // A cancellation exit must not erase an uncertain canonical submission.
            if !matches!(&result, Err(error) if error.kind == FailureKind::OutcomeUnknown) {
                result = Err(Failure::new(FailureKind::Cancelled));
            }
        }
        Err(_) => {
            if !matches!(&result, Err(error) if error.kind == FailureKind::OutcomeUnknown) {
                result = Err(Failure::new(FailureKind::InternalFailure));
            }
        }
    }
    let mut output = if let Some(mut receipt) = receipt {
        if let Err(error) = result {
            receipt.failure = Some(error.kind);
        }
        receipt.emit(format, locale)
    } else {
        failure_output(result.err().unwrap_or_else(internal), Some(format), locale)
    };
    if cancelled {
        output.status = 130;
    }
    output.finalization = finalization;
    output
}
fn internal() -> Failure {
    Failure::new(FailureKind::InternalFailure)
}
pub(super) fn check(signals: &mut ArmedSignals, deadline: Instant) -> Result<(), Failure> {
    if signals.take().map_err(|_| internal())?.is_some() {
        Err(Failure::new(FailureKind::Cancelled))
    } else if Instant::now() >= deadline {
        Err(Failure::new(FailureKind::Timeout))
    } else {
        Ok(())
    }
}

fn execute(
    command: &CreationCommand,
    deadline: Instant,
    signals: &mut ArmedSignals,
    output: &mut Option<Receipt>,
) -> Result<(), Failure> {
    check(signals, deadline)?;
    let ratio = match command.kind {
        CreationKind::Split { ratio, .. } => ratio,
        _ => None,
    };
    let needs_client = command.client.is_some() || command.focus || ratio.is_some();
    let caller = if command.client.is_none() && (needs_client || targeting::needs_source(command)) {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
    } else {
        None
    };
    let (mut client, _, snapshot) = query::snapshot_session_until(deadline)?;
    let caller = caller.map(|caller| caller.resolve(&snapshot)).transpose()?;
    check(signals, deadline)?;
    let selected = if needs_client || (targeting::needs_source(command) && caller.is_none()) {
        let reply = control::exchange(
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
        let selected =
            selection::select(&clients, command.client.as_ref(), caller.as_ref())?.clone();
        if selected.view.phase != ClientViewPhase::Ready {
            return Err(Failure::new(FailureKind::ServiceUnavailable));
        }
        Some(selected)
    } else {
        None
    };
    let mut canonical = targeting::resolve(command, &snapshot, caller.as_ref(), selected.as_ref())?;
    let directory = super::directory::resolve(
        command,
        &mut canonical,
        &snapshot,
        (caller.as_ref(), selected.as_ref()),
        &mut client,
        deadline,
        signals,
    )?;
    check(signals, deadline)?;
    let canonical = mutation::submit_creation(
        &mut client,
        &snapshot,
        &canonical,
        deadline,
        signals.poll_fd(),
    )?;
    *output = Some(Receipt {
        directory,
        canonical,
        client_id: selected.as_ref().map(|client| client.client_id.clone()),
        requested_ratio: ratio,
        requested_focus: command.focus,
        ratio: None,
        focus: None,
        stage: "created",
        failure: None,
    });
    let receipt = output.as_mut().ok_or_else(internal)?;
    check(signals, deadline)?;
    if ratio.is_none() && !command.focus {
        receipt.stage = "complete";
        return Ok(());
    }
    let selected = selected.as_ref().ok_or_else(internal)?;
    let revision = receipt
        .canonical
        .committed_revision()
        .ok_or_else(internal)?;
    let get = |name| receipt.canonical.id_value(name).ok_or_else(internal);
    let created = Created {
        context: WorkbenchTerminalContextId::parse(get("terminalContextId")?)
            .map_err(|_| internal())?,
        tab: WorkbenchTabId::parse(get("tabId")?).map_err(|_| internal())?,
        panel: WorkbenchPanelId::parse(get(
            if matches!(command.kind, CreationKind::Split { .. }) {
                "newPanelId"
            } else {
                "panelId"
            },
        )?)
        .map_err(|_| internal())?,
        split: matches!(command.kind, CreationKind::Split { .. }),
    };
    if let Some(ratio) = ratio {
        let identity = ClientSplitIdentity {
            split_id: WorkbenchSplitId::parse(get("splitId")?).map_err(|_| internal())?,
            first_slot_id: WorkbenchPanelSlotId::parse(get("firstSlotId")?)
                .map_err(|_| internal())?,
            second_slot_id: WorkbenchPanelSlotId::parse(get("secondSlotId")?)
                .map_err(|_| internal())?,
        };
        receipt.stage = "ratio-resolution";
        let fresh = read_snapshot(&mut client, &snapshot.site_id, revision, deadline, signals)?;
        let tree = fresh
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == created.context)
            .and_then(|context| context.tabs.iter().find(|tab| tab.tab_id == created.tab))
            .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
        let path = split_path(&tree.root, &identity, &mut vec![])
            .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
        let current = observe(
            &mut client,
            &snapshot.site_id,
            selected,
            &created,
            fresh.revision,
            deadline,
            signals,
        )?;
        receipt.stage = "ratio";
        receipt.ratio = Some(control::submit_control(
            &mut client,
            &snapshot.site_id,
            ClientViewControl {
                operation_id: query::request_id()?,
                client_id: selected.client_id.clone(),
                expected_view_revision: current.view_revision,
                minimum_workbench_revision: fresh.revision,
                intent: ClientViewIntent::SetSplitRatio {
                    terminal_context_id: created.context.clone(),
                    tab_id: created.tab.clone(),
                    path,
                    ratio,
                    expected_split: Some(identity),
                },
            },
            deadline,
            signals,
        )?);
        if !receipt
            .ratio
            .as_ref()
            .is_some_and(|answer| applied(answer, true))
        {
            return Ok(());
        }
    }
    if command.focus {
        receipt.stage = "focus-observation";
        let current = observe(
            &mut client,
            &snapshot.site_id,
            selected,
            &created,
            revision,
            deadline,
            signals,
        )?;
        check(signals, deadline)?;
        receipt.stage = "focus";
        receipt.focus = Some(control::submit_control(
            &mut client,
            &snapshot.site_id,
            ClientViewControl {
                operation_id: query::request_id()?,
                client_id: selected.client_id.clone(),
                expected_view_revision: current.view_revision,
                minimum_workbench_revision: revision,
                intent: ClientViewIntent::Panel {
                    terminal_context_id: created.context,
                    tab_id: created.tab,
                    panel_id: created.panel,
                },
            },
            deadline,
            signals,
        )?);
        if !receipt
            .focus
            .as_ref()
            .is_some_and(|answer| applied(answer, false))
        {
            return Ok(());
        }
    }
    receipt.stage = "complete";
    Ok(())
}

fn read_snapshot(
    client: &mut LocalClient,
    site: &SiteId,
    minimum: WorkbenchRevision,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Snapshot, Failure> {
    check(signals, deadline)?;
    let result = client.submit_workbench_snapshot_read_before(
        SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: query::request_id()?,
        },
        deadline,
    );
    check(signals, deadline)?;
    match result
        .map_err(|_| Failure::new(FailureKind::ServiceUnavailable))?
        .outcome
    {
        WorkbenchSnapshotOutcome::Available { snapshot }
            if &snapshot.site_id == site && snapshot.revision >= minimum =>
        {
            Ok(snapshot)
        }
        WorkbenchSnapshotOutcome::Available { .. } => {
            Err(Failure::new(FailureKind::ProtocolInvalid))
        }
        WorkbenchSnapshotOutcome::Denied {} => Err(Failure::new(FailureKind::Denied)),
        WorkbenchSnapshotOutcome::Unavailable { .. } => {
            Err(Failure::new(FailureKind::ServiceUnavailable))
        }
    }
}

fn split_path(
    node: &PanelNode,
    identity: &ClientSplitIdentity,
    path: &mut Vec<u8>,
) -> Option<ClientSplitPath> {
    let PanelNode::Split {
        split_id, children, ..
    } = node
    else {
        return None;
    };
    if *split_id == identity.split_id {
        return (children[0].slot_id == identity.first_slot_id
            && children[1].slot_id == identity.second_slot_id)
            .then(|| ClientSplitPath::new(path.clone()).ok())
            .flatten();
    }
    for (index, child) in children.iter().enumerate() {
        path.push(u8::try_from(index).ok()?);
        let found = split_path(&child.node, identity, path);
        path.pop();
        if found.is_some() {
            return found;
        }
    }
    None
}

fn observe(
    client: &mut LocalClient,
    site: &SiteId,
    original: &ClientViewRecord,
    created: &Created,
    minimum: WorkbenchRevision,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<ClientViewRecord, Failure> {
    loop {
        check(signals, deadline)?;
        let reply = control::exchange(
            client,
            site,
            ClientViewAction::Get {
                client_id: original.client_id.clone(),
            },
            deadline,
            signals,
        )?;
        let current = match reply.outcome {
            ClientViewOutcome::Found { client } => *client,
            ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
            ClientViewOutcome::NotFound {} | ClientViewOutcome::Unavailable { .. } => {
                return Err(Failure::new(FailureKind::ServiceUnavailable));
            }
            _ => return Err(Failure::new(FailureKind::ProtocolInvalid)),
        };
        if current.availability != ClientViewAvailability::Current
            || current.view.phase == ClientViewPhase::Unavailable
        {
            return Err(Failure::new(FailureKind::ServiceUnavailable));
        }
        let can_refresh_empty = current.view.workbench_revision >= original.view.workbench_revision
            && targeting::unchanged_empty_selection(
                &original.view.selection,
                &current.view.selection,
            );
        if current.view.phase == ClientViewPhase::Ready
            && (current.view.workbench_revision >= minimum || can_refresh_empty)
        {
            if !targeting::unchanged_selection(
                &original.view.selection,
                &current.view.selection,
                created,
            ) {
                return Err(Failure::new(FailureKind::Denied));
            }
            return Ok(current);
        }
        thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}
