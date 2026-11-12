use super::*;
use serde_json::json;
fn id(n: u128) -> String {
    format!("{n:032x}")
}
fn snapshot() -> Snapshot {
    serde_json::from_value(json!({"protocolVersion":1,"sessionId":id(1),"revision":"3",
        "navigation":{ "projects":[{"projectId":id(3), "label":"project"}], "directories":[{"directoryId":id(4),"workerId":id(5),"path":"C:/repo"}], "workspaces":[{"workspaceId":id(2),"terminalContextId":id(2),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"},{"workspaceId":id(6), "label":"workspace", "primaryDirectoryId":id(4), "terminalContextId":id(7), "projectId":id(3)}]},
        "terminalContexts":[{"terminalContextId":id(2),"groups":[],"tabs":[]},
        {"terminalContextId":id(7),"groups":[],"tabs":[{"tabId":id(8),"label":"tab","groupId":null,"pinned":false,
        "root":{"kind":"panel","panelId":id(9),"primaryPresentation":{"customLabel":null,"presentationId":id(10),"label":"primary","binding":null},"childPresentations":[]}}]}]})).unwrap()
}
fn record() -> ClientViewRecord {
    serde_json::from_value(json!({"clientId":id(11),"heartbeat":"1","viewRevision":"2","availability":"current",
        "view":{"workbenchRevision":"3","phase":"busy","selection":{"terminalContextId":id(7),"tabId":id(8),"panelId":id(9),"zoomed":false,"panelCount":1}}})).unwrap()
}
#[test]
fn canonical_selection_infers_workspace_tab_and_panel_without_using_display_overrides() {
    let (context, panel) = infer(&snapshot(), Some(&record()), None).unwrap();
    assert_eq!(context.workspace_id, Some(id(6)));
    assert_eq!(context.workspace_label.as_deref(), Some("workspace"));
    assert_eq!(context.workspace_cwd.as_deref(), Some("C:/repo"));
    assert_eq!(context.tab_label.as_deref(), Some("tab"));
    assert_eq!(context.focused_panel_id, Some(id(9)));
    assert_eq!(panel.unwrap().0, Target::Id(id(9)));
    let mut wrong = record();
    wrong.view.selection.as_mut().unwrap().terminal_context_id =
        orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&id(2)).unwrap();
    assert!(infer(&snapshot(), Some(&wrong), None).is_err());
    wrong = record();
    wrong.view.selection.as_mut().unwrap().panel_id = None;
    assert!(infer(&snapshot(), Some(&wrong), None).is_err());
}
#[test]
fn empty_workspace_and_global_invocations_have_no_invented_member() {
    let mut snapshot = snapshot();
    snapshot.terminal_contexts[1].tabs.clear();
    let mut client = record();
    let view = client.view.selection.as_mut().unwrap();
    view.tab_id = None;
    view.panel_id = None;
    view.panel_count = 0;
    let (context, member) = infer(&snapshot, Some(&client), None).unwrap();
    assert_eq!(context.workspace_id, Some(id(6)));
    assert!(member.is_none());
    assert!(context.tab_id.is_none());
    let (global, member) = infer(&snapshot, None, None).unwrap();
    assert_eq!(global, PluginInvocationContext::default());
    assert!(member.is_none());
}
#[test]
fn runtime_context_uses_observed_cwd_and_unambiguous_member_agent() {
    let mut context = PluginInvocationContext {
        workspace_cwd: Some("C:/repo".into()),
        ..Default::default()
    };
    let mut document = json!({"panel":{"cwd":"C:/repo/subdir","agentStatus":"working","agentSessionIds":["current"]},
        "agents":{"sessions":[{"session":{"sessionId":"other","providerId":"wrong"}},
            {"session":{"sessionId":"current","providerId":"codex"}}]}});
    enrich(&mut context, &document);
    assert_eq!(context.workspace_cwd.as_deref(), Some("C:/repo/subdir"));
    assert_eq!(context.focused_panel_agent.as_deref(), Some("codex"));
    assert_eq!(context.focused_panel_status.as_deref(), Some("working"));
    document["panel"]["agentSessionIds"] = json!(["current", "other"]);
    // Each enrichment starts from a fresh inferred context.
    let mut fresh = PluginInvocationContext::default();
    enrich(&mut fresh, &document);
    assert!(fresh.focused_panel_agent.is_none());
    document["panel"]["reportedAgent"] = json!({"view":{"agent":"Team Agent","state":"blocked"}});
    enrich(&mut fresh, &document);
    assert_eq!(fresh.focused_panel_agent.as_deref(), Some("Team Agent"));
    assert_eq!(fresh.focused_panel_status.as_deref(), Some("blocked"));
}

#[test]
fn headless_destination_keeps_canonical_workspace_without_guessing_tab_or_focus() {
    let snapshot = snapshot();
    let context = infer_context(
        &snapshot,
        &snapshot.terminal_contexts[1].terminal_context_id,
    )
    .unwrap();
    assert_eq!(context.workspace_id, Some(id(6)));
    assert_eq!(context.workspace_label.as_deref(), Some("workspace"));
    assert_eq!(context.workspace_cwd.as_deref(), Some("C:/repo"));
    assert!(
        context.tab_id.is_none()
            && context.focused_panel_id.is_none()
            && context.focused_panel_cwd.is_none()
            && context.focused_panel_agent.is_none()
    );
    let unknown = WorkbenchTerminalContextId::parse(&id(999)).unwrap();
    assert!(infer_context(&snapshot, &unknown).is_err());
}
