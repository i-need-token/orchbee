use super::*;
use serde_json::json;
fn id(n: u8) -> String {
    format!("{n:032x}")
}
fn fixture() -> Snapshot {
    let tab = |t, p| {
        json!({"tabId":id(t),"label":"shell","pinned":false,"groupId":null,
            "root":{"kind":"panel","panelId":id(p),"primaryPresentation":{"customLabel":null,"presentationId":id(p+40),"label":"shell","binding":null},"childPresentations":[]}})
    };
    serde_json::from_value(json!({"protocolVersion":1,"siteId":id(1),"revision":"2",
            "navigation":{ "projects":[{"projectId":id(10), "label":"Project"}], "directories":[], "workspaces":[{"workspaceId":id(2),"terminalContextId":id(2),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"},{"workspaceId":id(11), "label":"Work", "terminalContextId":id(12), "primaryDirectoryId":null, "projectId":id(10)}]},
            "terminalContexts":[{"terminalContextId":id(2),"groups":[],"tabs":[tab(3,4),tab(5,6)]},
                {"terminalContextId":id(12),"groups":[],"tabs":[tab(13,14)]}]})).unwrap()
}
fn selected(c: u8, t: u8, p: u8) -> ClientViewSelection {
    ClientViewSelection {
        terminal_context_id: WorkbenchTerminalContextId::parse(&id(c)).unwrap(),
        tab_id: Some(WorkbenchTabId::parse(&id(t)).unwrap()),
        panel_id: Some(WorkbenchPanelId::parse(&id(p)).unwrap()),
        zoomed: false,
        panel_count: 1,
    }
}
#[test]
fn panel_relocation_resolves_across_workspaces_and_rejects_ambiguous_tabs() {
    let snapshot = fixture();
    let source = panel(&snapshot, &Target::Id(id(4))).unwrap();
    let target = tab(&snapshot, &Target::Id(id(13))).unwrap();
    assert!(!source.same_tab(&target));
    assert!(target.exists(&snapshot));
    assert_eq!(
        context(
            &snapshot,
            Some(&TabContext::Workspace(Target::Label("Work".into()))),
            &source
        )
        .unwrap(),
        target.terminal_context_id
    );
    assert_eq!(
        tab(&snapshot, &Target::Label("shell".into()))
            .unwrap_err()
            .kind,
        FailureKind::Ambiguous
    );
    assert!(
        !Panel {
            tab_id: source.tab_id,
            ..target
        }
        .exists(&snapshot)
    );
}
#[test]
fn panel_relocation_preserves_unaffected_selection_and_restricts_removal_repair() {
    let mut snapshot = fixture();
    let before = selected(2, 3, 4);
    assert!(follows_original(Some(&before), Some(&before), &snapshot));
    let other = selected(2, 5, 6);
    assert!(!follows_original(Some(&before), Some(&other), &snapshot));
    let mut zoomed = before.clone();
    zoomed.zoomed = true;
    assert!(!follows_original(Some(&before), Some(&zoomed), &snapshot));
    snapshot.terminal_contexts[0].tabs.remove(0);
    assert!(follows_original(Some(&before), Some(&other), &snapshot));
    assert!(!follows_original(
        Some(&before),
        Some(&selected(12, 13, 14)),
        &snapshot
    ));
    let workspace_before = selected(12, 13, 14);
    snapshot.terminal_contexts.remove(1);
    snapshot.navigation.workspaces.clear();
    assert!(follows_original(
        Some(&workspace_before),
        Some(&other),
        &snapshot
    ));
    assert!(!follows_original(
        Some(&workspace_before),
        Some(&workspace_before),
        &snapshot
    ));
    snapshot.terminal_contexts[0].tabs.clear();
    let empty = ClientViewSelection {
        tab_id: None,
        panel_id: None,
        panel_count: 0,
        ..other
    };
    assert!(unchanged_empty_selection(None, None));
    assert!(unchanged_empty_selection(Some(&empty), Some(&empty)));
    assert!(!unchanged_empty_selection(Some(&before), Some(&before)));
    assert!(!unchanged_empty_selection(Some(&empty), None));
    let other_empty = ClientViewSelection {
        terminal_context_id: workspace_before.terminal_context_id.clone(),
        ..empty.clone()
    };
    assert!(!unchanged_empty_selection(Some(&empty), Some(&other_empty)));
    assert!(follows_original(Some(&before), Some(&empty), &snapshot));
    assert!(follows_original(
        Some(&workspace_before),
        Some(&empty),
        &snapshot
    ));
}
