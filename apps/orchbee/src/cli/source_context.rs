//! A source context proves membership; it never guesses a terminal or directory.
use crate::{
    cli::{Failure, FailureKind, client_control::selection, results},
    invocation::Target,
};
use orchbee_workbench_protocol::{ClientViewPhase, ClientViewRecord, Snapshot};

#[derive(Debug)]
pub(super) struct Origin {
    pub(super) workspace: Option<Target>,
    pub(super) panel: Option<Target>,
}

pub(super) fn resolve(
    snapshot: &Snapshot,
    caller: Option<&Target>,
    selected: Option<&ClientViewRecord>,
) -> Result<Option<Origin>, Failure> {
    let (context, panel) = if let Some(caller) = caller {
        let entries = results::panel_entries(snapshot);
        let (_, context, _) = entries
            .into_iter()
            .find(|(id, _, _)| id.as_str() == caller.text())
            .ok_or_else(missing)?;
        (context.terminal_context_id.clone(), Some(caller.clone()))
    } else if let Some(selected) = selected {
        if selected.view.phase != ClientViewPhase::Ready {
            return Err(Failure::new(FailureKind::ServiceUnavailable));
        }
        let view = selected.view.selection.as_ref().ok_or_else(missing)?;
        let context = snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == view.terminal_context_id)
            .ok_or_else(missing)?;
        if let Some(tab) = &view.tab_id
            && !context
                .tabs
                .iter()
                .any(|candidate| &candidate.tab_id == tab)
        {
            return Err(missing());
        }
        let panel = view
            .panel_id
            .as_ref()
            .map(|_| selection::current_panel(selected, snapshot))
            .transpose()?;
        (view.terminal_context_id.clone(), panel)
    } else {
        return Ok(None);
    };
    let workspace = if context == snapshot.navigation.standalone_terminal_context_id {
        None
    } else {
        Some(Target::Id(
            snapshot
                .navigation
                .workspaces
                .iter()
                .find(|workspace| workspace.terminal_context_id == context)
                .ok_or_else(missing)?
                .workspace_id
                .to_string(),
        ))
    };
    Ok(Some(Origin { workspace, panel }))
}

fn missing() -> Failure {
    Failure::new(FailureKind::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn id(n: u8) -> String {
        format!("{n:032x}")
    }
    #[test]
    fn ready_empty_workspace_is_a_source_without_inventing_a_panel() {
        let snapshot: Snapshot = serde_json::from_value(json!({"protocolVersion":1,"siteId":id(1),"revision":"2","navigation":{"standaloneTerminalContextId":id(2), "projects":[{"projectId":id(3), "label":"p"}], "directories":[], "workspaces":[{"workspaceId":id(4), "label":"w", "primaryDirectoryId":null, "terminalContextId":id(5), "projectId":id(3)}]},"terminalContexts":[{"terminalContextId":id(2),"groups":[],"tabs":[]},{"terminalContextId":id(5),"groups":[],"tabs":[]}]})).unwrap();
        let mut selected: ClientViewRecord = serde_json::from_value(json!({"clientId":id(6),"heartbeat":"1","viewRevision":"1","availability":"current","view":{"workbenchRevision":"2","phase":"ready","selection":{"terminalContextId":id(5),"tabId":null,"panelId":null,"zoomed":false,"panelCount":0}}})).unwrap();
        let origin = resolve(&snapshot, None, Some(&selected)).unwrap().unwrap();
        assert_eq!(origin.workspace.unwrap().text(), id(4));
        assert!(origin.panel.is_none());
        selected
            .view
            .selection
            .as_mut()
            .unwrap()
            .terminal_context_id = snapshot.navigation.standalone_terminal_context_id.clone();
        assert!(
            resolve(&snapshot, None, Some(&selected))
                .unwrap()
                .unwrap()
                .workspace
                .is_none()
        );
        selected
            .view
            .selection
            .as_mut()
            .unwrap()
            .terminal_context_id =
            orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&id(9)).unwrap();
        assert_eq!(
            resolve(&snapshot, None, Some(&selected)).unwrap_err().kind,
            FailureKind::NotFound
        );
    }
}
