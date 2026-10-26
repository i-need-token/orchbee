use super::*;
use serde_json::json;
fn id(n: u8) -> String {
    format!("{n:032x}")
}
#[test]
fn ready_empty_workspace_is_a_source_without_inventing_a_panel() {
    let mut snapshot: Snapshot = serde_json::from_value(json!({"protocolVersion":1,"sessionId":id(1),"revision":"2","navigation":{ "projects":[{"projectId":id(3), "label":"p"}], "directories":[], "workspaces":[{"workspaceId":id(2),"terminalContextId":id(2),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"},{"workspaceId":id(4), "label":"w", "primaryDirectoryId":null, "terminalContextId":id(5), "projectId":id(3)}]},"terminalContexts":[{"terminalContextId":id(2),"groups":[],"tabs":[]},{"terminalContextId":id(5),"groups":[],"tabs":[]}]})).unwrap();
    let mut selected: ClientViewRecord = serde_json::from_value(json!({"clientId":id(6),"heartbeat":"1","viewRevision":"1","availability":"current","view":{"workbenchRevision":"2","phase":"ready","selection":{"terminalContextId":id(5),"tabId":null,"panelId":null,"zoomed":false,"panelCount":0}}})).unwrap();
    let origin = resolve(&snapshot, None, Some(&selected)).unwrap().unwrap();
    assert_eq!(origin.workspace.text(), id(4));
    assert!(origin.panel.is_none());
    selected
        .view
        .selection
        .as_mut()
        .unwrap()
        .terminal_context_id = snapshot.navigation.workspaces[0]
        .terminal_context_id
        .clone();
    assert_eq!(
        resolve(&snapshot, None, Some(&selected))
            .unwrap()
            .unwrap()
            .workspace
            .text(),
        id(2)
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
    selected.view.selection = None;
    assert_eq!(
        resolve(&snapshot, None, Some(&selected)).unwrap_err().kind,
        FailureKind::NotFound
    );
    snapshot.navigation.workspaces.clear();
    snapshot.terminal_contexts.clear();
    assert!(resolve(&snapshot, None, Some(&selected)).unwrap().is_none());
}
