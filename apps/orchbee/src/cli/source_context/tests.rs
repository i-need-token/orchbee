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
