use super::*;
use serde_json::json;
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn selection(tab: Option<u64>, panel: Option<u64>, count: u16) -> Option<ClientViewSelection> {
    Some(serde_json::from_value(json!({"terminalContextId":id(4),"tabId":tab.map(id),"panelId":panel.map(id),"zoomed":false,"panelCount":count})).unwrap())
}
#[test]
fn selection_guard_allows_only_the_creation_count_or_first_tab_transition() {
    let mut created = Created {
        context: WorkbenchTerminalContextId::parse(&id(4)).unwrap(),
        tab: WorkbenchTabId::parse(&id(5)).unwrap(),
        panel: WorkbenchPanelId::parse(&id(20)).unwrap(),
        split: true,
    };
    let before = selection(Some(5), Some(6), 2);
    assert!(unchanged_selection(
        &before,
        &selection(Some(5), Some(6), 3),
        &created
    ));
    for after in [
        before.clone(),
        selection(Some(5), Some(6), 4),
        selection(Some(5), Some(20), 3),
        selection(Some(9), Some(6), 3),
    ] {
        assert!(!unchanged_selection(&before, &after, &created));
    }
    let mut zoomed = selection(Some(5), Some(6), 3);
    zoomed.as_mut().unwrap().zoomed = true;
    assert!(!unchanged_selection(&before, &zoomed, &created));
    created.split = false;
    let empty = selection(None, None, 0);
    assert!(unchanged_empty_selection(&empty, &empty));
    assert!(!unchanged_empty_selection(&before, &before));
    assert!(!unchanged_empty_selection(&None, &None));
    assert!(!unchanged_empty_selection(
        &empty,
        &selection(Some(5), Some(20), 1)
    ));
    assert!(unchanged_selection(&empty, &empty, &created));
    assert!(unchanged_selection(
        &empty,
        &selection(Some(5), Some(20), 1),
        &created
    ));
    assert!(!unchanged_selection(
        &empty,
        &selection(Some(5), Some(6), 1),
        &created
    ));
    assert!(!unchanged_selection(
        &empty,
        &selection(Some(9), Some(20), 1),
        &created
    ));
}
