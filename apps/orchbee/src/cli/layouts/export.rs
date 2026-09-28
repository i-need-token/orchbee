//! One bounded read-only join. Temporary viewers never take terminal control.
use super::{failure, model};
use crate::{
    cli::{
        Failure, FailureKind,
        caller_context::PanelTarget,
        client_control::{selection, windows as control},
        query, results,
        source::{ArmedSignals, SignalWatch},
    },
    invocation::{ClientLayoutTarget, LayoutExportCommand, Resource, Target},
    local_client::LocalClient,
};
use orchbee_client_server_protocol::{
    TerminalId, TerminalLaunchReadOutcome, TerminalLaunchReadRequest, TerminalLaunchState,
};
use orchbee_workbench_protocol::{
    ClientPanelLayout, ClientViewAction, ClientViewControl, ClientViewControlStatus,
    ClientViewIntent, ClientViewOutcome, ClientViewPhase, ClientViewRecord, Snapshot, Tab,
    TerminalContext, WorkbenchDirectoryPath, WorkbenchPanelId,
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
    signals.check_before(deadline)?;
    let observed = control::exchange(
        &mut client,
        &snapshot.site_id,
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
        _ => return Err(Failure::new(FailureKind::ProtocolInvalid)),
    };
    let selected = selection::select(&clients, command.client.as_ref(), caller.as_ref())?;
    if selected.view.phase != ClientViewPhase::Ready {
        return Err(failure(FailureKind::ServiceUnavailable, "client-not-ready"));
    }
    let target = target(&snapshot, &command.target, caller.as_ref(), selected)?;
    let context = target.context;
    let tab = target.tab;
    let leaves =
        model::leaves(&tab.root).map_err(|reason| failure(FailureKind::CheckFailed, reason))?;
    let layout = read_layout(&mut client, &snapshot, selected, &target, deadline, signals)?;
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
            site_id,
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
        if site_id.as_str() != snapshot.site_id.as_str()
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
            &mut client,
            deadline,
            &snapshot,
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
    let root = model::tree(&tab.root, &layout.splits, &facts)
        .map_err(|reason| failure(FailureKind::Denied, reason))?;
    let after = query::refresh_snapshot(
        &mut client,
        &snapshot.site_id,
        snapshot.revision,
        deadline,
        signals,
    )?;
    if after.revision != snapshot.revision {
        return Err(failure(FailureKind::Denied, "layout-changed"));
    }
    let workspace = snapshot
        .navigation
        .projects
        .iter()
        .flat_map(|project| &project.workspaces)
        .find(|workspace| workspace.terminal_context_id == context.terminal_context_id);
    let document = json!({"schema":"orchbee.layout.export","version":2,
        "source":{"kind":"layout-observation","siteId":snapshot.site_id,"revision":snapshot.revision,
            "clientId":selected.client_id,"viewRevision":selected.view_revision},
        "result":{"layout":{"workspaceId":workspace.map(|workspace| &workspace.workspace_id),
            "terminalContextId":context.terminal_context_id,"tabId":tab.tab_id,
            "zoomed":layout.zoomed,"focusedPanelId":layout.focused_panel_id,"root":root}}});
    if !crate::cli::output::encode_document(&document)
        .is_ok_and(|bytes| bytes.len() <= super::template::MAX_BYTES)
    {
        return Err(Failure::new(FailureKind::OutputTooLarge));
    }
    Ok(document)
}

struct LayoutTarget<'a> {
    context: &'a TerminalContext,
    tab: &'a Tab,
    panel: WorkbenchPanelId,
}

fn target<'a>(
    snapshot: &'a Snapshot,
    target: &ClientLayoutTarget,
    caller: Option<&Target>,
    selected: &ClientViewRecord,
) -> Result<LayoutTarget<'a>, Failure> {
    match target {
        ClientLayoutTarget::Tab(target) => {
            let (context, tab) = results::resolve_or_fail(
                snapshot.terminal_contexts.iter().flat_map(|context| {
                    context.tabs.iter().enumerate().map(move |(index, tab)| {
                        (
                            tab.tab_id.as_str(),
                            tab.display_label(index),
                            (context, tab),
                        )
                    })
                }),
                target,
                Resource::Tab,
            )?;
            let panel = results::panel_ids_in(&tab.root)
                .first()
                .cloned()
                .cloned()
                .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
            Ok(LayoutTarget {
                context,
                tab,
                panel,
            })
        }
        ClientLayoutTarget::Panel(explicit) => {
            let current;
            let target = if let Some(target) = explicit.as_ref().or(caller) {
                target
            } else {
                current = selection::current_panel(selected, snapshot)?;
                &current
            };
            let (id, context, tab) = results::resolve_or_fail(
                results::panel_entries(snapshot)
                    .into_iter()
                    .map(|(id, context, tab)| (id.as_str(), "", (id, context, tab))),
                target,
                Resource::Panel,
            )?;
            Ok(LayoutTarget {
                context,
                tab,
                panel: id.clone(),
            })
        }
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
        &snapshot.site_id,
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
