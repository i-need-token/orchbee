use super::*;
use crate::invocation::Target;

fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn snapshot() -> Snapshot {
    let panel = |n| {
        json!({"kind":"panel","panelId":id(n),
        "primaryPresentation":{"customLabel":null,"presentationId":id(n+100),"label":"shell","binding":null},
        "childPresentations":[]})
    };
    serde_json::from_value(json!({
        "protocolVersion":1,"siteId":id(2),"revision":"12",
        "navigation":{"projects":[],"directories":[],"workspaces":[{"workspaceId":id(4),"terminalContextId":id(4),"projectId":null,"primaryDirectoryId":null,"label":"Workspace"}]},
        "terminalContexts":[{"terminalContextId":id(4),"groups":[],"tabs":[{
            "tabId":id(5),"label":"work","pinned":false,"groupId":null,"root":{
                "kind":"split","splitId":id(50),"orientation":"horizontal","ratio":0.3,
                "children":[{"slotId":id(51),"node":panel(6)},{"slotId":id(52),"node":panel(8)}]
            }
        }]}]
    })).unwrap()
}

#[test]
fn explicit_structure_and_ratio_resolution_need_no_client_state() {
    let snapshot = snapshot();
    let tab = resolve(
        &snapshot,
        &ClientLayoutTarget::Tab(Target::Label("work".into())),
    )
    .unwrap();
    let panel = resolve(
        &snapshot,
        &ClientLayoutTarget::Panel(Some(Target::Id(id(8)))),
    )
    .unwrap();
    assert_eq!(tab.tab.tab_id, panel.tab.tab_id);
    assert_eq!(panel.panel.as_str(), id(8));
    let (identity, previous) =
        split(&tab.tab.root, &ClientSplitPath::new(vec![]).unwrap()).unwrap();
    assert_eq!(identity.split_id.as_str(), id(50));
    assert_eq!(identity.first_slot_id.as_str(), id(51));
    assert_eq!(identity.second_slot_id.as_str(), id(52));
    assert_eq!(previous.value(), 0.3);
    assert!(split(&tab.tab.root, &ClientSplitPath::new(vec![0]).unwrap()).is_err());
    assert!(resolve(&snapshot, &ClientLayoutTarget::Panel(None)).is_err());
}

#[test]
fn geometry_and_implicit_selection_stay_client_owned() {
    for query in [
        LayoutQuery::Edges,
        LayoutQuery::Neighbor(orchbee_workbench_protocol::ClientPanelDirection::Left),
    ] {
        assert!(!explicit(&ClientNavigationTarget::Layout {
            target: Some(Target::Id(id(6))),
            query
        }));
    }
    assert!(!explicit(&ClientNavigationTarget::Layout {
        target: None,
        query: LayoutQuery::Layout
    }));
    assert!(explicit(&ClientNavigationTarget::Layout {
        target: Some(Target::Id(id(6))),
        query: LayoutQuery::Layout
    }));
}
