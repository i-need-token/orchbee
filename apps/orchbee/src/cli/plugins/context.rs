//! Infer display context from canonical membership and corroborated passive observations.
use crate::{
    cli::{
        Failure, FailureKind,
        caller_context::PanelTarget,
        client_control::{selection, windows::exchange},
        current_presentation::{self, Origin, Resolved},
        query, results,
        source::ArmedSignals,
        workbench_query, workbench_worktrees,
    },
    invocation::{QueryCommand, Target},
    local_client::LocalClient,
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewAvailability, ClientViewOutcome, ClientViewPhase, ClientViewRecord,
    PluginInvocationContext, Snapshot, TerminalId, WorkbenchClientId, WorkbenchPanelId,
};
use std::time::Instant;

pub(super) fn resolve(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    explicit: Option<&WorkbenchClientId>,
    provided: PluginInvocationContext,
    operation_id: &str,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<PluginInvocationContext, Failure> {
    signals.check_before(deadline)?;
    let caller = if explicit.is_none() {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
            .map(|caller| caller.resolve_exact(snapshot))
            .transpose()?
    } else {
        None
    };
    let selected = if caller.is_none() {
        let response = exchange(
            client,
            &snapshot.site_id,
            ClientViewAction::List {},
            deadline,
            signals,
        )?;
        let ClientViewOutcome::Listed { clients } = response.outcome else {
            return Err(Failure::new(
                if matches!(response.outcome, ClientViewOutcome::Denied {}) {
                    FailureKind::Denied
                } else {
                    FailureKind::ServiceUnavailable
                },
            ));
        };
        if explicit.is_none()
            && !clients
                .iter()
                .any(|c| c.availability == ClientViewAvailability::Current)
        {
            None
        } else {
            Some(selection::select(&clients, explicit, None)?.clone())
        }
    } else {
        None
    };
    let (mut context, panel) = infer(snapshot, selected.as_ref(), caller)?;
    context.invocation_source = Some("cli".into());
    context.correlation_id = Some(operation_id.into());
    if let Some(id) = &context.workspace_id {
        let projection = results::project(
            &QueryCommand::WorkspaceGet(Target::Id(id.clone())),
            snapshot,
        )?;
        if let Some(worktrees) =
            workbench_worktrees::read(client, snapshot, &projection, deadline, signals)?
        {
            context.worktree = worktrees.plugin_context(id)?;
        }
    }
    if let Some((target, terminal)) = panel {
        let projection = results::project(&QueryCommand::PanelGet(target), snapshot)?;
        let results::Projection::PanelGet(panel) = &projection else {
            return Err(invalid());
        };
        let origin = if let Some(record) = &selected {
            let id = WorkbenchPanelId::parse(&panel.row.panel_id).map_err(|_| invalid())?;
            Origin::Client(Box::new(current_presentation::read_with_receipt(
                client, snapshot, record, &id, deadline, signals,
            )?))
        } else {
            terminal.map_or(Origin::Primary, Origin::Caller)
        };
        let member = Resolved::new(panel, origin)?;
        let (_, document) = workbench_query::collect(
            client,
            snapshot,
            projection,
            selected,
            Some(member),
            deadline,
            signals,
        )?;
        enrich(&mut context, &document.result());
    } else {
        // Even an empty Workspace is an observation, never a pending switch intent.
        let fresh = query::refresh_snapshot(
            client,
            &snapshot.site_id,
            snapshot.revision,
            deadline,
            signals,
        )?;
        if fresh.revision != snapshot.revision {
            return Err(crate::cli::workbench_agents::failure("workbench-changed"));
        }
        if let Some(original) = selected {
            let reply = exchange(
                client,
                &snapshot.site_id,
                ClientViewAction::Get {
                    client_id: original.client_id.clone(),
                },
                deadline,
                signals,
            )?;
            if !matches!(reply.outcome, ClientViewOutcome::Found { client }
                if client.availability == ClientViewAvailability::Current
                    && client.view_revision == original.view_revision && client.view == original.view)
            {
                return Err(crate::cli::workbench_agents::failure("client-changed"));
            }
        }
    }
    signals.check_before(deadline)?;
    let context = context.with_overrides(provided);
    context
        .validate()
        .map_err(|_| Failure::new(FailureKind::Rejected))?;
    Ok(context)
}

type CurrentPanel = (Target, Option<TerminalId>);
fn invalid() -> Failure {
    Failure::new(FailureKind::ProtocolInvalid)
}

fn infer(
    snapshot: &Snapshot,
    selected: Option<&ClientViewRecord>,
    caller: Option<CurrentPanel>,
) -> Result<(PluginInvocationContext, Option<CurrentPanel>), Failure> {
    let mut result = PluginInvocationContext::default();
    let (context_id, tab_id, panel) = if let Some(caller) = caller {
        let (id, context, tab) = results::panel_entries(snapshot)
            .into_iter()
            .find(|(id, _, _)| id.as_str() == caller.0.text())
            .ok_or_else(invalid)?;
        result.focused_pane_id = Some(id.to_string());
        (
            &context.terminal_context_id,
            Some(&tab.tab_id),
            Some(caller),
        )
    } else if let Some(record) = selected {
        if !matches!(
            record.view.phase,
            ClientViewPhase::Ready | ClientViewPhase::Busy
        ) {
            return Err(Failure::new(FailureKind::ServiceUnavailable));
        }
        let Some(view) = &record.view.selection else {
            return Ok((result, None));
        };
        let panel = if view.panel_id.is_some() {
            let panel = selection::observed_panel(record, snapshot)?;
            result.focused_pane_id = Some(panel.text().to_owned());
            Some((panel, None))
        } else {
            None
        };
        (&view.terminal_context_id, view.tab_id.as_ref(), panel)
    } else {
        return Ok((result, None));
    };
    let context = snapshot
        .terminal_contexts
        .iter()
        .find(|context| &context.terminal_context_id == context_id)
        .ok_or_else(invalid)?;
    match tab_id {
        Some(id) => {
            let tab = context
                .tabs
                .iter()
                .find(|tab| &tab.tab_id == id)
                .ok_or_else(invalid)?;
            if panel.is_none() {
                return Err(invalid());
            }
            result.tab_id = Some(id.to_string());
            result.tab_label = Some(tab.label.clone());
        }
        None if !context.tabs.is_empty() || panel.is_some() => return Err(invalid()),
        None => {}
    }
    if let Some((project, workspace)) = snapshot.navigation.projects.iter().find_map(|project| {
        project
            .workspaces
            .iter()
            .find(|workspace| &workspace.terminal_context_id == context_id)
            .map(|workspace| (project, workspace))
    }) {
        result.workspace_id = Some(workspace.workspace_id.to_string());
        result.workspace_label = Some(workspace.label.clone());
        result.workspace_cwd = workspace.primary_directory_id.as_ref().and_then(|id| {
            project
                .directories
                .iter()
                .find(|directory| &directory.directory_id == id)
                .map(|directory| directory.path.as_str().to_owned())
        });
    }
    Ok((result, panel))
}

fn enrich(context: &mut PluginInvocationContext, document: &serde_json::Value) {
    let panel = &document["panel"];
    context.focused_pane_cwd = panel["cwd"].as_str().map(String::from);
    context.workspace_cwd = context
        .focused_pane_cwd
        .clone()
        .or(context.workspace_cwd.take());
    let report = &panel["reportedAgent"]["view"];
    context.focused_pane_status = report["state"]
        .as_str()
        .filter(|state| *state != "unknown")
        .or_else(|| panel["agentStatus"].as_str())
        .map(String::from);
    context.focused_pane_agent = report["agent"].as_str().map(String::from);
    if context.focused_pane_agent.is_some() {
        return;
    }
    let Some(ids) = panel["agentSessionIds"].as_array() else {
        return;
    };
    let [id] = ids.as_slice() else {
        return;
    };
    context.focused_pane_agent = document["agents"]["sessions"]
        .as_array()
        .and_then(|sessions| {
            sessions
                .iter()
                .find(|session| session["session"]["sessionId"] == *id)
        })
        .and_then(|session| session["session"]["providerId"].as_str())
        .map(String::from);
}

#[cfg(test)]
mod tests;
