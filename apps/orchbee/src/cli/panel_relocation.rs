//! Retained terminal relocation with atomic directory binding and acknowledged Client effects.
mod receipt;
mod targeting;
mod view;
use super::{
    Failure, FailureKind, Output,
    caller_context::PanelTarget,
    client_control::{selection, windows as control},
    failure_output, mutation, query, results,
    source::{ArmedSignals, Finish, SignalWatch},
};
use crate::{
    invocation::{
        OutputFormat, PanelRelocationCommand, PanelRelocationDestination, Target, user_path_windows,
    },
    local_client::LocalClient,
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientSplitIdentity, ClientViewAction, ClientViewIntent, ClientViewOutcome, ClientViewPhase,
    ClientViewRecord, Snapshot, WorkbenchAction, WorkbenchDirectoryPath, WorkbenchLabel,
    WorkbenchPanelRelocateDestination, WorkbenchPanelSlotId, WorkbenchSplitId, WorkbenchTabId,
};
use receipt::{Receipt, applied};
use std::time::{Duration, Instant};
use targeting::Panel;

fn internal() -> Failure {
    Failure::new(FailureKind::InternalFailure)
}
pub(crate) fn run(
    command: &PanelRelocationCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let deadline = Instant::now() + Duration::from_millis(u64::from(command.timeout_ms));
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => return failure_output(internal(), Some(format), locale),
    };
    let mut receipt = None;
    let mut result = execute(command, deadline, &mut signals, &mut receipt, locale);
    let mut finalization = None;
    let mut cancelled = false;
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            cancelled = true;
            if !matches!(&result, Err(error) if error.kind == FailureKind::OutcomeUnknown) {
                result = Err(Failure::new(FailureKind::Cancelled));
            }
        }
        Err(_) => {
            if !matches!(&result, Err(error) if error.kind == FailureKind::OutcomeUnknown) {
                result = Err(internal());
            }
        }
    }
    let mut output = if let Some(mut receipt) = receipt {
        if let Err(failure) = result {
            receipt.failure = Some(failure);
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

fn select_client(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    command: &PanelRelocationCommand,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Option<ClientViewRecord>, Failure> {
    let caller = if command.client.is_none() {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
            .map(|caller| caller.resolve(snapshot))
            .transpose()?
    } else {
        None
    };
    let reply = control::exchange(
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
    if clients.is_empty() && command.client.is_none() && !command.focus && command.ratio().is_none()
    {
        return Ok(None);
    }
    let selected = selection::select(&clients, command.client.as_ref(), caller.as_ref())?;
    if selected.view.phase != ClientViewPhase::Ready {
        return Err(Failure::new(FailureKind::ServiceUnavailable));
    }
    Ok(Some(selected.clone()))
}

fn execute(
    command: &PanelRelocationCommand,
    deadline: Instant,
    signals: &mut ArmedSignals,
    output: &mut Option<Receipt>,
    locale: Locale,
) -> Result<(), Failure> {
    signals.check_before(deadline)?;
    let (mut client, _, mut snapshot) = query::snapshot_session_until(deadline)?;
    let source = targeting::panel(&snapshot, &command.source)?;
    let selected = select_client(&mut client, &snapshot, command, deadline, signals)?;
    *output = Some(Receipt {
        session_id: snapshot.session_id.clone(),
        base_revision: snapshot.revision,
        source: source.clone(),
        client_id: selected.as_ref().map(|client| client.client_id.clone()),
        requested_focus: command.focus,
        requested_ratio: command.ratio(),
        directory_path: None,
        directory_source: None,
        movement: None,
        directory: None,
        ratio: None,
        focus: None,
        source_layout: None,
        target_layout: None,
        stage: "preflight",
        reason: None,
        failure: None,
    });
    let receipt = output.as_mut().ok_or_else(internal)?;
    if let Some(selected) = &selected {
        let current = view::observe(&mut client, selected, &snapshot, deadline, signals)?;
        receipt.source_layout = Some(view::layout(
            &mut client,
            &current,
            &snapshot,
            &source,
            deadline,
            signals,
        )?);
        if receipt
            .source_layout
            .as_ref()
            .is_some_and(|layout| layout.zoomed)
        {
            return unchanged(receipt, "zoomed-tab");
        }
    }
    let destination = match &command.destination {
        PanelRelocationDestination::Tab {
            tab,
            panel,
            direction,
            ..
        } => {
            let mut target = targeting::tab(&snapshot, tab)?;
            if let Some(selected) = &selected {
                let current = view::observe(&mut client, selected, &snapshot, deadline, signals)?;
                let layout =
                    view::layout(&mut client, &current, &snapshot, &target, deadline, signals)?;
                if let Some(focused) = &layout.focused_panel_id {
                    target.panel_id = focused.clone();
                }
                receipt.target_layout = Some(layout);
                if receipt
                    .target_layout
                    .as_ref()
                    .is_some_and(|layout| layout.zoomed)
                {
                    return unchanged(receipt, "zoomed-tab");
                }
            }
            if let Some(panel) = panel {
                let explicit = targeting::panel(&snapshot, panel)?;
                if !target.same_tab(&explicit) {
                    return Err(Failure::new(FailureKind::NotFound));
                }
                target = explicit;
            }
            if !target.exists(&snapshot) {
                return Err(Failure::new(FailureKind::NotFound));
            }
            WorkbenchPanelRelocateDestination::Tab {
                terminal_context_id: Some(target.terminal_context_id),
                tab_id: target.tab_id,
                target_panel_id: Some(target.panel_id),
                direction: *direction,
            }
        }
        PanelRelocationDestination::NewTab { context, label } => {
            WorkbenchPanelRelocateDestination::NewTab {
                terminal_context_id: Some(targeting::context(
                    &snapshot,
                    context.as_ref(),
                    &source,
                )?),
                label: label.clone(),
            }
        }
        PanelRelocationDestination::NewWorkspace {
            project,
            label,
            tab_label,
        } => {
            let project = project
                .as_ref()
                .map(|target| {
                    super::results::resolve_project(&snapshot, target)
                        .map(|project| project.project_id.clone())
                })
                .transpose()?;
            let (path, origin) =
                source_directory(&mut client, &snapshot, &source, deadline, signals)?;
            let label = label
                .clone()
                .unwrap_or_else(|| automatic_label(&path, locale));
            receipt.directory_path = Some(path);
            receipt.directory_source = Some(origin);
            if !source.exists(&snapshot) {
                return Err(Failure::new(FailureKind::NotFound));
            }
            WorkbenchPanelRelocateDestination::NewWorkspace {
                project_id: project,
                primary_directory_id: None,
                primary_directory_path: receipt.directory_path.clone(),
                label,
                tab_label: tab_label.clone(),
            }
        }
    };
    // Re-observe after preparation and before the one canonical transfer. View and topology are
    // separate owners; post-transfer checks retain a commit if Client publication later fails.
    if let Some(selected) = &selected {
        let current = view::observe(&mut client, selected, &snapshot, deadline, signals)?;
        let layout = view::layout(&mut client, &current, &snapshot, &source, deadline, signals)?;
        if layout.zoomed {
            return Err(Failure::new(FailureKind::Denied));
        }
        if let WorkbenchPanelRelocateDestination::Tab {
            terminal_context_id: Some(context),
            tab_id,
            target_panel_id: Some(panel),
            ..
        } = &destination
        {
            let target = Panel {
                terminal_context_id: context.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel.clone(),
            };
            let layout =
                view::layout(&mut client, &current, &snapshot, &target, deadline, signals)?;
            let implicit = matches!(
                command.destination,
                PanelRelocationDestination::Tab { panel: None, .. }
            );
            if layout.zoomed
                || (implicit
                    && layout
                        .focused_panel_id
                        .as_ref()
                        .is_some_and(|focused| focused != panel))
            {
                return Err(Failure::new(FailureKind::Denied));
            }
        }
    }
    receipt.stage = "move";
    signals.check_before(deadline)?;
    let action = match &destination {
        WorkbenchPanelRelocateDestination::Tab {
            terminal_context_id: Some(context),
            tab_id,
            target_panel_id: Some(panel),
            direction,
        } if context == &source.terminal_context_id && tab_id == &source.tab_id => {
            WorkbenchAction::PanelMove {
                terminal_context_id: context.clone(),
                tab_id: tab_id.clone(),
                panel_id: source.panel_id.clone(),
                target_panel_id: panel.clone(),
                direction: *direction,
            }
        }
        _ => WorkbenchAction::PanelRelocate {
            terminal_context_id: source.terminal_context_id.clone(),
            tab_id: source.tab_id.clone(),
            panel_id: source.panel_id.clone(),
            destination,
        },
    };
    let within_tab = matches!(action, WorkbenchAction::PanelMove { .. });
    receipt.movement = Some(mutation::submit_action(
        &mut client,
        &snapshot,
        action,
        deadline,
        signals.poll_fd(),
    )?);
    let movement = receipt.movement.as_ref().ok_or_else(internal)?;
    let revision = movement.committed_revision().ok_or_else(internal)?;
    let target = Panel {
        terminal_context_id: movement
            .transfer()
            .map(|transfer| transfer.target_terminal_context_id.clone())
            .unwrap_or_else(|| source.terminal_context_id.clone()),
        tab_id: WorkbenchTabId::parse(
            movement
                .id_value(if within_tab { "tabId" } else { "targetTabId" })
                .ok_or_else(internal)?,
        )
        .map_err(|_| internal())?,
        panel_id: source.panel_id.clone(),
    };
    snapshot = query::refresh_snapshot(
        &mut client,
        &snapshot.session_id,
        revision,
        deadline,
        signals,
    )?;
    if !target.exists(&snapshot) {
        return Err(Failure::new(FailureKind::NotFound));
    }
    if let Some(path) = &receipt.directory_path {
        let terminal =
            query::primary_terminal(&snapshot, &Target::Id(target.panel_id.to_string()))?;
        let terminal = results::terminal_select(&snapshot, &terminal)?;
        let transfer = receipt
            .movement
            .as_ref()
            .and_then(|movement| movement.transfer())
            .ok_or_else(internal)?;
        let workspace_id = transfer
            .created_workspace_id
            .as_ref()
            .ok_or_else(internal)?;
        let directory_id = transfer
            .created_directory_id
            .as_ref()
            .ok_or_else(internal)?;
        let directory = snapshot
            .navigation
            .workspace_directory(workspace_id)
            .filter(|directory| {
                &directory.directory_id == directory_id
                    && &directory.path == path
                    && directory.worker_id.as_str() == terminal.worker_id
            })
            .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?;
        receipt.directory = Some(directory.clone());
    }
    if let Some(selected) = &selected {
        if let Some(ratio) = command.ratio() {
            receipt.stage = "ratio";
            let movement = receipt.movement.as_ref().ok_or_else(internal)?;
            let get = |name| movement.id_value(name).ok_or_else(internal);
            let identity = if within_tab {
                // Placement receipts do not carry the new split identity. Only the exact
                // committed snapshot can establish it without following a concurrent move.
                if snapshot.revision != revision {
                    return Err(Failure::new(FailureKind::Denied));
                }
                view::parent_split(&snapshot, &target)?
            } else {
                ClientSplitIdentity {
                    split_id: WorkbenchSplitId::parse(get("splitId")?).map_err(|_| internal())?,
                    first_slot_id: WorkbenchPanelSlotId::parse(get("firstSlotId")?)
                        .map_err(|_| internal())?,
                    second_slot_id: WorkbenchPanelSlotId::parse(get("secondSlotId")?)
                        .map_err(|_| internal())?,
                }
            };
            let path = view::split_path(&snapshot, &target, &identity)?;
            let current = view::observe(&mut client, selected, &snapshot, deadline, signals)?;
            receipt.ratio = Some(view::submit(
                &mut client,
                &current,
                &snapshot,
                ClientViewIntent::SetSplitRatio {
                    terminal_context_id: target.terminal_context_id.clone(),
                    tab_id: target.tab_id.clone(),
                    path,
                    ratio,
                    expected_split: Some(identity),
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
            receipt.stage = "focus";
            let current = view::observe(&mut client, selected, &snapshot, deadline, signals)?;
            receipt.focus = Some(view::submit(
                &mut client,
                &current,
                &snapshot,
                ClientViewIntent::Panel {
                    terminal_context_id: target.terminal_context_id.clone(),
                    tab_id: target.tab_id.clone(),
                    panel_id: target.panel_id.clone(),
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
        receipt.stage = "layouts";
        // The final focus intentionally changes selection, so its confirmed view becomes the
        // observation baseline; a plain Get still verifies Ready/minimum through the layout reply.
        let current = if command.focus {
            focused_client(&mut client, selected, &snapshot, &target, deadline, signals)?
        } else {
            view::observe(&mut client, selected, &snapshot, deadline, signals)?
        };
        receipt.target_layout = Some(view::layout(
            &mut client,
            &current,
            &snapshot,
            &target,
            deadline,
            signals,
        )?);
        receipt.source_layout = if let Some(context) = snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == source.terminal_context_id)
            && let Some(tab) = context.tabs.iter().find(|tab| tab.tab_id == source.tab_id)
            && let Some(panel) = results::panel_ids_in(&tab.root).first()
        {
            let remaining = Panel {
                panel_id: (*panel).clone(),
                ..source
            };
            Some(view::layout(
                &mut client,
                &current,
                &snapshot,
                &remaining,
                deadline,
                signals,
            )?)
        } else {
            None
        };
    }
    signals.check_before(deadline)?;
    receipt.stage = "complete";
    Ok(())
}
fn unchanged(receipt: &mut Receipt, reason: &'static str) -> Result<(), Failure> {
    receipt.reason = Some(reason);
    receipt.stage = "complete";
    Ok(())
}
fn source_directory(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    source: &Panel,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(WorkbenchDirectoryPath, &'static str), Failure> {
    let terminal = query::primary_terminal(snapshot, &Target::Id(source.panel_id.to_string()))?;
    let (_, info) = query::read_terminal_processes_cancellable(
        client,
        deadline,
        snapshot,
        &terminal,
        signals.poll_fd(),
    )?;
    signals.check_before(deadline)?;
    let root = info
        .info
        .processes
        .iter()
        .find(|process| process.pid == info.info.root_pid)
        .ok_or_else(|| Failure::new(FailureKind::ServiceUnavailable))?;
    if let Some(path) = &root.current_directory {
        if !std::path::Path::new(path).is_absolute() {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
        Ok((
            WorkbenchDirectoryPath::parse(path)
                .map_err(|_| Failure::new(FailureKind::ProtocolInvalid))?,
            "terminal-process",
        ))
    } else {
        Ok((
            user_path_windows::normalize(".", |name| std::env::var_os(name), std::env::current_dir)
                .map_err(|_| Failure::new(FailureKind::ServiceUnavailable))?,
            "current",
        ))
    }
}
fn automatic_label(path: &WorkbenchDirectoryPath, locale: Locale) -> WorkbenchLabel {
    let name = std::path::Path::new(path.as_str())
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(locale.cli_default_workspace_label());
    WorkbenchLabel::parse(&name.chars().take(512).collect::<String>()).unwrap_or_else(|_| {
        WorkbenchLabel::parse(locale.cli_default_workspace_label())
            .expect("localized default label")
    })
}
fn focused_client(
    client: &mut LocalClient,
    original: &ClientViewRecord,
    snapshot: &Snapshot,
    target: &Panel,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<ClientViewRecord, Failure> {
    let reply = control::exchange(
        client,
        &snapshot.session_id,
        ClientViewAction::Get {
            client_id: original.client_id.clone(),
        },
        deadline,
        signals,
    )?;
    let ClientViewOutcome::Found { client } = reply.outcome else {
        return Err(Failure::new(FailureKind::ServiceUnavailable));
    };
    if client.availability != orchbee_workbench_protocol::ClientViewAvailability::Current
        || client.view.phase != ClientViewPhase::Ready
        || client.view.workbench_revision < snapshot.revision
        || !client.view.selection.as_ref().is_some_and(|selection| {
            selection.terminal_context_id == target.terminal_context_id
                && selection.tab_id.as_ref() == Some(&target.tab_id)
                && selection.panel_id.as_ref() == Some(&target.panel_id)
        })
    {
        return Err(Failure::new(FailureKind::Denied));
    }
    Ok(*client)
}
