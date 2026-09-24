//! Pure routing over one canonical snapshot and one bounded Client observation.
use crate::{
    cli::{Failure, FailureKind, results},
    invocation::{ClientNavigationTarget, Resource, Target, ZoomMode},
};
use orchbee_workbench_protocol::{
    ClientViewAvailability, ClientViewIntent, ClientViewRecord, Snapshot, WorkbenchClientId,
};
use serde_json::{Map, json};

pub(super) fn failure(kind: FailureKind, reason: &'static str, candidates: Vec<String>) -> Failure {
    let mut failure = Failure::new(kind).details(Map::from_iter([
        ("stage".into(), json!("client-selection")),
        ("reason".into(), json!(reason)),
        ("candidates".into(), json!(candidates)),
    ]));
    failure.failure_schema = Some("orchbee.cli.failure.client-control");
    failure
}
pub(super) fn select<'a>(
    clients: &'a [ClientViewRecord],
    explicit: Option<&WorkbenchClientId>,
    caller: Option<&Target>,
) -> Result<&'a ClientViewRecord, Failure> {
    if let Some(id) = explicit {
        let client = clients
            .iter()
            .find(|client| &client.client_id == id)
            .ok_or_else(|| failure(FailureKind::NotFound, "client-not-found", vec![]))?;
        if client.availability != ClientViewAvailability::Current {
            return Err(failure(
                FailureKind::ServiceUnavailable,
                "client-stale",
                vec![id.to_string()],
            ));
        }
        return Ok(client);
    }
    let candidates: Vec<_> = clients
        .iter()
        .filter(|client| {
            client.availability == ClientViewAvailability::Current
                && caller.is_none_or(|caller| {
                    client.view.selection.as_ref().is_some_and(|selection| {
                        selection
                            .panel_id
                            .as_ref()
                            .is_some_and(|panel| panel.as_str() == caller.text())
                    })
                })
        })
        .collect();
    match candidates.as_slice() {
        [client] => Ok(client),
        [] => Err(failure(FailureKind::NotFound, "client-not-found", vec![])),
        _ => Err(failure(
            FailureKind::Ambiguous,
            "client-ambiguous",
            candidates
                .iter()
                .map(|client| client.client_id.to_string())
                .collect(),
        )),
    }
}
pub(super) fn intent(
    target: &ClientNavigationTarget,
    client: &ClientViewRecord,
    caller: Option<&Target>,
    snapshot: &Snapshot,
) -> Result<ClientViewIntent, Failure> {
    match target {
        ClientNavigationTarget::Workspace(target) => {
            let workspace = results::resolve_or_fail(
                snapshot.navigation.projects.iter().flat_map(|project| {
                    project.workspaces.iter().map(|workspace| {
                        (
                            workspace.workspace_id.as_str(),
                            workspace.label.as_str(),
                            workspace,
                        )
                    })
                }),
                target,
                Resource::Workspace,
            )?;
            Ok(ClientViewIntent::Context {
                terminal_context_id: workspace.terminal_context_id.clone(),
            })
        }
        ClientNavigationTarget::Tab(target) => {
            let (context, tab) = results::resolve_or_fail(
                snapshot.terminal_contexts.iter().flat_map(|context| {
                    context
                        .tabs
                        .iter()
                        .map(move |tab| (tab.tab_id.as_str(), tab.label.as_str(), (context, tab)))
                }),
                target,
                Resource::Tab,
            )?;
            Ok(ClientViewIntent::Tab {
                terminal_context_id: context.terminal_context_id.clone(),
                tab_id: tab.tab_id.clone(),
            })
        }
        ClientNavigationTarget::Panel(explicit)
        | ClientNavigationTarget::Zoom {
            target: explicit, ..
        } => {
            let current = client
                .view
                .selection
                .as_ref()
                .and_then(|selection| selection.panel_id.as_ref())
                .map(|panel| Target::Id(panel.to_string()));
            let target_panel = explicit
                .as_ref()
                .or(caller)
                .or(current.as_ref())
                .ok_or_else(|| {
                    failure(
                        FailureKind::NotFound,
                        "client-panel-unavailable",
                        vec![client.client_id.to_string()],
                    )
                })?;
            let (panel, context, tab) = results::resolve_or_fail(
                results::panel_entries(snapshot)
                    .into_iter()
                    .map(|(panel, context, tab)| (panel.as_str(), "", (panel, context, tab))),
                target_panel,
                Resource::Panel,
            )?;
            let context_id = context.terminal_context_id.clone();
            let tab_id = tab.tab_id.clone();
            let panel_id = panel.clone();
            if let ClientNavigationTarget::Zoom { mode, .. } = target {
                if *mode == ZoomMode::Toggle {
                    return Ok(ClientViewIntent::ToggleZoom {
                        terminal_context_id: context_id,
                        tab_id,
                        panel_id,
                    });
                }
                let enabled = *mode == ZoomMode::On;
                Ok(ClientViewIntent::Zoom {
                    terminal_context_id: context_id,
                    tab_id,
                    panel_id,
                    enabled,
                })
            } else {
                Ok(ClientViewIntent::Panel {
                    terminal_context_id: context_id,
                    tab_id,
                    panel_id,
                })
            }
        }
    }
}
