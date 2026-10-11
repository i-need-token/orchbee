//! The same independently specified outcomes run through the GUI TabModel and native transition.
use orchbee_workbench_protocol::{
    TabConfiguration, TerminalContext, WorkbenchTabGroupId, configure_tabs,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    name: String,
    tabs: Vec<Value>,
    groups: Vec<Value>,
    operation: TabConfiguration,
    created_group_id: Option<WorkbenchTabGroupId>,
    expected: Value,
}
#[derive(Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

fn context(case: &Case) -> TerminalContext {
    serde_json::from_value(json!({
        "terminalContextId": format!("{:032x}", 900),
        "groups": case.groups.iter().map(|group| json!({"groupId":group["id"],"label":group["title"],"color":group["color"]})).collect::<Vec<_>>(),
        "tabs": case.tabs.iter().enumerate().map(|(index, tab)| json!({
            "tabId":tab["id"],"label":tab["title"],"pinned":tab["pinned"],"groupId":tab["group"],"automaticLabel":index == 1,
            "root":{"kind":"panel","rightClick":"panel","panelId":format!("{:032x}",1000+index),"childPresentations":[{"presentationId":format!("{:032x}",2000+index),"label":"child","binding":null}],"primaryPresentation":{"presentationId":format!("{:032x}",3000+index),"label":"primary","customLabel":"kept","binding":{"workerId":format!("{:032x}",4000),"terminalId":format!("{:032x}",5000+index)}}}
        })).collect::<Vec<_>>()
    })).unwrap()
}

fn metadata(context: &TerminalContext) -> Value {
    json!({
        "groups":context.groups.iter().map(|group| json!({"id":group.group_id,"title":group.label,"color":group.color})).collect::<Vec<_>>(),
        "tabs":context.tabs.iter().map(|tab| json!({"id":tab.tab_id,"title":tab.label,"group":tab.group_id,"pinned":tab.pinned})).collect::<Vec<_>>()
    })
}

#[test]
fn gui_and_native_share_configuration_outcomes_and_preserve_all_terminal_data() {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/tab-configuration-conformance-v1.json"
    ))
    .unwrap();
    for case in corpus.cases {
        let baseline = context(&case);
        let candidate = configure_tabs(&baseline, &case.operation, case.created_group_id.as_ref())
            .unwrap()
            .unwrap_or_else(|| baseline.clone());
        assert_eq!(metadata(&candidate), case.expected, "{}", case.name);
        assert_eq!(baseline.tabs.len(), candidate.tabs.len());
        for tab in &candidate.tabs {
            let prior = baseline
                .tabs
                .iter()
                .find(|prior| prior.tab_id == tab.tab_id)
                .unwrap();
            assert_eq!(tab.root, prior.root, "{}", case.name);
            assert_eq!(tab.automatic_label, prior.automatic_label, "{}", case.name);
            assert_eq!(tab.label, prior.label, "{}", case.name);
        }
        assert_invariants(&candidate);
    }
}

fn assert_invariants(context: &TerminalContext) {
    let mut unpinned = false;
    let mut last = std::collections::HashMap::new();
    for (index, tab) in context.tabs.iter().enumerate() {
        assert!(!tab.pinned || (!unpinned && tab.group_id.is_none()));
        unpinned |= !tab.pinned;
        if let Some(group) = &tab.group_id {
            assert!(
                context
                    .groups
                    .iter()
                    .any(|candidate| &candidate.group_id == group)
            );
            if let Some(previous) = last.insert(group, index) {
                assert_eq!(previous + 1, index);
            }
        }
    }
}

#[test]
fn drag_selection_boundary_matrix_preserves_partition_groups_and_terminal_identity() {
    use orchbee_workbench_protocol::WorkbenchTabInsertIndex;
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/tab-configuration-conformance-v1.json"
    ))
    .unwrap();
    let baseline = context(&corpus.cases[0]);
    let original = baseline.clone();
    let groups: Vec<_> = std::iter::once(None)
        .chain(
            baseline
                .groups
                .iter()
                .map(|group| Some(group.group_id.clone())),
        )
        .collect();
    for mask in 1..(1_u16 << baseline.tabs.len()) {
        let ids: Vec<_> = baseline
            .tabs
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, tab)| tab.tab_id.clone())
            .collect();
        for boundary in 0..=baseline.tabs.len() {
            for group in &groups {
                let operation = TabConfiguration::MoveTabsByDrag {
                    tab_ids: ids.clone().try_into().unwrap(),
                    insert_index: WorkbenchTabInsertIndex::new(boundary).unwrap(),
                    group_id: group.clone(),
                };
                if let Ok(candidate) = configure_tabs(&baseline, &operation, None) {
                    let candidate = candidate.unwrap_or_else(|| baseline.clone());
                    assert_invariants(&candidate);
                    assert_eq!(candidate.tabs.len(), baseline.tabs.len());
                    for tab in &candidate.tabs {
                        let prior = baseline
                            .tabs
                            .iter()
                            .find(|prior| prior.tab_id == tab.tab_id)
                            .unwrap();
                        assert_eq!(tab.root, prior.root);
                        assert_eq!(tab.label, prior.label);
                        assert_eq!(tab.pinned, prior.pinned);
                    }
                }
                assert_eq!(
                    baseline, original,
                    "planning cannot change the caller's context"
                );
            }
        }
    }
}

#[test]
fn missing_targets_and_incompatible_new_identity_leave_baseline_untouched() {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/tab-configuration-conformance-v1.json"
    ))
    .unwrap();
    let baseline = context(&corpus.cases[0]);
    let before = baseline.clone();
    let missing = WorkbenchTabGroupId::parse(&format!("{:032x}", 99999)).unwrap();
    assert!(
        configure_tabs(
            &baseline,
            &TabConfiguration::Ungroup {
                group_id: missing.clone()
            },
            None
        )
        .is_err()
    );
    assert!(configure_tabs(&baseline, &corpus.cases[0].operation, Some(&missing)).is_err());
    assert_eq!(baseline, before);
}
