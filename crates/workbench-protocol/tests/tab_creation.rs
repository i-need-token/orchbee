//! GUI differential placement plus capacity and source-metadata preservation at the shared owner.
use orchbee_workbench_protocol::*;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    name: String,
    tabs: Vec<Value>,
    groups: Vec<Value>,
    operation: Option<TabCreationPlacement>,
    created_tab_id: WorkbenchTabId,
    created_group_id: Option<WorkbenchTabGroupId>,
    expected: Value,
}
#[derive(Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

fn context(case: &Case) -> TerminalContext {
    serde_json::from_value(json!({
        "terminalContextId": format!("{:032x}", 700),
        "groups": case.groups.iter().map(|group| json!({"groupId":group["id"],"label":group["title"],"color":group["color"]})).collect::<Vec<_>>(),
        "tabs": case.tabs.iter().enumerate().map(|(index, tab)| json!({
            "tabId":tab["id"],"label":tab["title"],"pinned":tab["pinned"],"groupId":tab["group"],"automaticLabel":index==1,
            "root":{"kind":"panel","panelId":format!("{:032x}",1000+index),"childPresentations":[{"presentationId":format!("{:032x}",2000+index),"label":"child","binding":null}],"primaryPresentation":{"presentationId":format!("{:032x}",3000+index),"label":"live title","customLabel":"custom panel label","binding":null}}
        })).collect::<Vec<_>>()
    })).unwrap()
}
fn cases() -> Vec<Case> {
    serde_json::from_str::<Corpus>(include_str!(
        "../../../schemas/workbench/v1/tab-creation-conformance-v1.json"
    ))
    .unwrap()
    .cases
}
fn fresh(tab_id: WorkbenchTabId) -> NewTerminalTab {
    NewTerminalTab {
        tab_id,
        panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 810)).unwrap(),
        presentation_id: TerminalPresentationId::parse(&format!("{:032x}", 811)).unwrap(),
        label: WorkbenchLabel::parse("New Tab").unwrap(),
        automatic_label: true,
        binding: PresentationBinding {
            worker_id: WorkerId::parse(&format!("{:032x}", 812)).unwrap(),
            terminal_id: TerminalId::parse(&format!("{:032x}", 813)).unwrap(),
        },
    }
}

#[test]
fn gui_corpus_placement_preserves_existing_trees_and_creates_one_fresh_terminal() {
    for case in cases() {
        let baseline = context(&case);
        let candidate = create_terminal_tab(
            &baseline,
            case.operation.as_ref(),
            fresh(case.created_tab_id.clone()),
            case.created_group_id.as_ref(),
        )
        .unwrap();
        let actual = json!({
            "tabs": candidate.tabs.iter().map(|tab|json!({"id":tab.tab_id,"title":tab.label,"group":tab.group_id,"pinned":tab.pinned})).collect::<Vec<_>>(),
            "groups":candidate.groups.iter().map(|group|json!({"id":group.group_id,"title":group.label,"color":group.color})).collect::<Vec<_>>(),
        });
        assert_eq!(actual["tabs"], case.expected["tabs"], "{}", case.name);
        assert_eq!(actual["groups"], case.expected["groups"], "{}", case.name);
        for prior in &baseline.tabs {
            assert!(candidate.tabs.contains(prior));
        }
        let created = candidate
            .tabs
            .iter()
            .find(|tab| tab.tab_id == case.created_tab_id)
            .unwrap();
        let PanelNode::Panel {
            primary_presentation,
            child_presentations,
            ..
        } = &created.root
        else {
            panic!("fresh single Panel")
        };
        assert!(child_presentations.is_empty());
        assert_eq!(primary_presentation.label, created.label);
        assert_eq!(primary_presentation.custom_label, None);
        assert!(primary_presentation.binding.is_some());
        if let Some(TabCreationPlacement::Duplicate { tab_id }) = &case.operation {
            let source = baseline
                .tabs
                .iter()
                .find(|tab| &tab.tab_id == tab_id)
                .unwrap();
            assert_eq!(created.automatic_label, source.automatic_label);
            assert_ne!(created.root, source.root);
        }
    }
}

#[test]
fn missing_group_source_and_capacity_refuse_before_insertion() {
    let case = cases().remove(0);
    let mut baseline = context(&case);
    for placement in [
        TabCreationPlacement::InGroup {
            group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 999)).unwrap(),
        },
        TabCreationPlacement::Duplicate {
            tab_id: case.created_tab_id.clone(),
        },
        TabCreationPlacement::AfterTab {
            tab_id: case.created_tab_id.clone(),
        },
    ] {
        assert!(
            create_terminal_tab(
                &baseline,
                Some(&placement),
                fresh(case.created_tab_id.clone()),
                None
            )
            .is_err()
        );
        assert_eq!(baseline, context(&case));
    }
    baseline
        .tabs
        .resize(MAXIMUM_TABS_PER_CONTEXT, baseline.tabs[0].clone());
    assert_eq!(
        validate_tab_creation(&baseline, None),
        Err(WorkbenchRejectionReason::ResourceLimit)
    );
    baseline.tabs.truncate(1);
    baseline.groups = (0..MAXIMUM_TAB_GROUPS_PER_CONTEXT)
        .map(|index| TabGroup {
            group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 10000 + index)).unwrap(),
            label: String::new(),
            color: TabGroupColor::Grey,
        })
        .collect();
    baseline.tabs = baseline
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let mut tab = baseline.tabs[0].clone();
            tab.tab_id = WorkbenchTabId::parse(&format!("{:032x}", 20000 + index)).unwrap();
            tab.pinned = false;
            tab.group_id = Some(group.group_id.clone());
            tab
        })
        .collect();
    assert_eq!(
        validate_tab_creation(&baseline, Some(&TabCreationPlacement::NewGroup {})),
        Err(WorkbenchRejectionReason::ResourceLimit)
    );
}
