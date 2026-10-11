use super::*;
use crate::cli::workspace_focus::unchanged;
use orchbee_workbench_protocol::ClientViewRecord;
#[test]
fn revision_and_derived_count_refresh_do_not_override_navigation_guard() {
    let mut original: ClientViewRecord = serde_json::from_value(json!({"clientId":"00000000000000000000000000000001","heartbeat":"1","viewRevision":"1","availability":"current","view":{"phase":"ready","workbenchRevision":"2","selection":{"terminalContextId":"00000000000000000000000000000002","tabId":"00000000000000000000000000000004","panelId":"00000000000000000000000000000005","zoomed":false,"panelCount":1}}})).unwrap();
    let mut fresh = original.clone();
    fresh.view_revision = serde_json::from_value(json!("3")).unwrap();
    fresh.view.workbench_revision = orchbee_workbench_protocol::WorkbenchRevision::new(3);
    fresh.view.selection.as_mut().unwrap().panel_count = 2;
    assert!(unchanged(&original, &fresh));
    fresh.view.selection.as_mut().unwrap().zoomed = true;
    assert!(!unchanged(&original, &fresh));
    fresh.view.selection.as_mut().unwrap().zoomed = false;
    original
        .view
        .selection
        .as_mut()
        .unwrap()
        .terminal_context_id = orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(
        "00000000000000000000000000000003",
    )
    .unwrap();
    assert!(!unchanged(&original, &fresh));
}
