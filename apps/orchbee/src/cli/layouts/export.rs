//! One bounded read-only join. Temporary viewers never take terminal control.
use super::{failure, model};
use crate::{
    cli::{
        Failure, FailureKind,
        caller_context::PanelTarget,
        client_control::{exchange as control, selection},
        console_interrupt::{ArmedSignals, SignalWatch},
        query,
    },
    invocation::{ClientLayoutTarget, LayoutExportCommand, Target},
    local_client::LocalClient,
};
use orchbee_client_server_protocol::{
    TerminalId, TerminalLaunchReadOutcome, TerminalLaunchReadRequest, TerminalLaunchState,
};
use orchbee_workbench_protocol::{
    ClientPanelLayout, ClientViewAction, ClientViewControl, ClientViewControlStatus,
    ClientViewIntent, ClientViewOutcome, ClientViewPhase, ClientViewRecord, Snapshot,
    WorkbenchDirectoryPath,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Instant};

pub(super) fn execute(
    command: &LayoutExportCommand,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Value, Failure> {
    signals.check_before(deadline)?;
    let implicit = matches!(command.target, ClientLayoutTarget::Panel(None));
    let caller = if implicit && command.client.is_none() {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
    } else {
        None
    };
    let session = query::stream_snapshot(deadline, signals.poll_fd());
    signals.check_before(deadline)?;
    let (mut client, snapshot) = session?;
    let caller = caller.map(|caller| caller.resolve(&snapshot)).transpose()?;
    observe(command, &mut client, &snapshot, caller, deadline, signals)
}

pub(super) fn from_snapshot(
    command: &LayoutExportCommand,
    client: &mut LocalClient,
    snapshot: &Snapshot,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Value, Failure> {
    observe(command, client, snapshot, None, deadline, signals)
}

fn observe(
    command: &LayoutExportCommand,
    client: &mut LocalClient,
    snapshot: &Snapshot,
    caller: Option<Target>,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Value, Failure> {
    let implicit = matches!(command.target, ClientLayoutTarget::Panel(None));
    signals.check_before(deadline)?;
    let selected = if command.client.is_some() || implicit {
        let observed = control::exchange(
            client,
            &snapshot.session_id,
            ClientViewAction::List {},
            deadline,
            signals,
        )?;
        let clients = match observed.outcome {
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
            return Err(failure(FailureKind::ServiceUnavailable, "client-not-ready"));
        }
        Some(selected.clone())
    } else {
        None
    };
    let target = target(
        snapshot,
        &command.target,
        caller.as_ref(),
        selected.as_ref(),
    )?;
    let context = target.context;
    let tab = target.tab;
    let leaves =
        model::leaves(&tab.root).map_err(|reason| failure(FailureKind::CheckFailed, reason))?;
    let layout = selected
        .as_ref()
        .map(|selected| read_layout(client, snapshot, selected, &target, deadline, signals))
        .transpose()?;
    let mut facts = BTreeMap::new();
    for (panel_id, presentation) in leaves {
        signals.check_before(deadline)?;
        let Some(binding) = presentation.binding.as_ref() else {
            continue;
        };
        let request = TerminalLaunchReadRequest::new(
            query::obcs_request_id()?,
            TerminalId::parse(binding.terminal_id.as_str())
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
        );
        let read =
            client.submit_terminal_launch_read_cancellable(request, deadline, signals.poll_fd());
        signals.check_before(deadline)?;
        let result = read.map_err(query::classify_launch_read)?;
        let TerminalLaunchReadOutcome::Found {
            session_id,
            workbench_revision,
            terminal,
            launch,
            launch_selection,
            ..
        } = result.outcome()
        else {
            return Err(failure(
                FailureKind::ServiceUnavailable,
                "layout-unavailable",
            ));
        };
        let route = terminal.bound_to();
        if session_id.as_str() != snapshot.session_id.as_str()
            || *workbench_revision != snapshot.revision.value()
            || terminal.terminal_id().as_str() != binding.terminal_id.as_str()
            || terminal.worker_id().as_str() != binding.worker_id.as_str()
            || route.presentation_id().as_str() != presentation.presentation_id.as_str()
            || route.panel_id().as_str() != panel_id.as_str()
            || route.tab_id().as_str() != tab.tab_id.as_str()
            || route.terminal_context_id().as_str() != context.terminal_context_id.as_str()
        {
            return Err(failure(FailureKind::Denied, "terminal-binding-changed"));
        }
        if launch.state() == TerminalLaunchState::NotTracked {
            return Err(failure(FailureKind::ServiceUnavailable, "launch-untracked"));
        }
        let (_, processes) = query::read_terminal_processes_cancellable(
            client,
            deadline,
            snapshot,
            &Target::Id(binding.terminal_id.to_string()),
            signals.poll_fd(),
        )?;
        signals.check_before(deadline)?;
        let cwd = processes
            .info
            .processes
            .iter()
            .find(|process| process.pid == processes.info.root_pid)
            .and_then(|process| process.current_directory.as_ref())
            .cloned();
        if let Some(path) = &cwd {
            WorkbenchDirectoryPath::parse(path)
                .map_err(|_| failure(FailureKind::ProtocolInvalid, "layout-unavailable"))?;
        }
        facts.insert(
            presentation.presentation_id.clone(),
            model::Leaf {
                cwd,
                launch: launch_selection.as_deref().cloned(),
            },
        );
    }
    let root = match &layout {
        Some(layout) => model::tree(&tab.root, &layout.splits, &facts),
        None => model::shared_tree(&tab.root, &facts),
    }
    .map_err(|reason| failure(FailureKind::Denied, reason))?;
    let after = query::refresh_snapshot(
        client,
        &snapshot.session_id,
        snapshot.revision,
        deadline,
        signals,
    )?;
    if after.revision != snapshot.revision {
        return Err(failure(FailureKind::Denied, "layout-changed"));
    }
    let workspace = snapshot
        .navigation
        .workspaces
        .iter()
        .find(|workspace| workspace.terminal_context_id == context.terminal_context_id)
        .ok_or_else(|| failure(FailureKind::ProtocolInvalid, "workspace-unavailable"))?;
    let document = json!({"schema":"orchbee.layout.export","version":1,
        "source":{"kind":"layout-observation","scope":if selected.is_some() {"client"} else {"shared"},"sessionId":snapshot.session_id,"revision":snapshot.revision,
            "clientId":selected.as_ref().map(|client| &client.client_id),"viewRevision":selected.as_ref().map(|client| client.view_revision)},
        "result":{"layout":{"workspaceId":&workspace.workspace_id,
            "terminalContextId":context.terminal_context_id,"tabId":tab.tab_id,
            "maximized":layout.as_ref().map(|layout| layout.zoomed),"focusedPanelId":layout.as_ref().and_then(|layout| layout.focused_panel_id.as_ref()),"root":root}}});
    if !crate::cli::output::encode_document(&document)
        .is_ok_and(|bytes| bytes.len() <= super::template::MAX_BYTES)
    {
        return Err(Failure::new(FailureKind::OutputTooLarge));
    }
    Ok(document)
}

type LayoutTarget<'a> = crate::cli::layout_topology::Resolved<'a>;

fn target<'a>(
    snapshot: &'a Snapshot,
    target: &ClientLayoutTarget,
    caller: Option<&Target>,
    selected: Option<&ClientViewRecord>,
) -> Result<LayoutTarget<'a>, Failure> {
    if matches!(target, ClientLayoutTarget::Panel(None)) {
        let target = caller.cloned().map(Ok).unwrap_or_else(|| {
            selection::current_panel(
                selected.ok_or_else(|| failure(FailureKind::NotFound, "client-not-ready"))?,
                snapshot,
            )
        })?;
        crate::cli::layout_topology::resolve(snapshot, &ClientLayoutTarget::Panel(Some(target)))
    } else {
        crate::cli::layout_topology::resolve(snapshot, target)
    }
}

fn read_layout(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    selected: &ClientViewRecord,
    target: &LayoutTarget<'_>,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<ClientPanelLayout, Failure> {
    let context = target.context;
    let tab = target.tab;
    let panel = &target.panel;
    let answer = control::submit_control(
        client,
        &snapshot.session_id,
        ClientViewControl {
            operation_id: query::request_id()?,
            client_id: selected.client_id.clone(),
            expected_view_revision: selected.view_revision,
            minimum_workbench_revision: snapshot.revision,
            intent: ClientViewIntent::Layout {
                terminal_context_id: context.terminal_context_id.clone(),
                tab_id: tab.tab_id.clone(),
                panel_id: panel.clone(),
            },
        },
        deadline,
        signals,
    )?;
    signals.check_before(deadline)?;
    if answer.exit_code() != 0 {
        let kind = match answer.exit_code() {
            130 => FailureKind::Cancelled,
            7 => FailureKind::Timeout,
            4 => FailureKind::ServiceUnavailable,
            5 | 8 => FailureKind::TransportLoss,
            _ => FailureKind::Denied,
        };
        return Err(failure(kind, "layout-unavailable"));
    }
    let Some(reply) = answer.reply else {
        return Err(failure(FailureKind::TransportLoss, "layout-unavailable"));
    };
    let ClientViewOutcome::Operation { operation } = reply.outcome else {
        return Err(failure(FailureKind::Denied, "layout-unavailable"));
    };
    let ClientViewControlStatus::LayoutRead { layout, .. } = operation.status else {
        return Err(failure(FailureKind::Denied, "layout-unavailable"));
    };
    if layout.workbench_revision != snapshot.revision
        || layout.terminal_context_id != context.terminal_context_id
        || layout.tab_id != tab.tab_id
        || layout.source_panel_id != *panel
    {
        return Err(failure(FailureKind::Denied, "layout-changed"));
    }
    Ok(*layout)
}
