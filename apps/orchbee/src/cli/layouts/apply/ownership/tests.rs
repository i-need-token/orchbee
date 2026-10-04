use super::*;
use serde_json::{Value, json};
fn id(n: u32) -> String {
    format!("{n:032x}")
}

#[test]
fn known_shared_ratio_receipt_advances_only_its_exact_owned_split() {
    use orchbee_workbench_protocol::{
        ClientLayoutAdjustment, ClientLayoutChange, ClientSplitChange, WorkbenchRevision,
        WorkbenchSplitRatio,
    };
    let snapshot: Snapshot = orchbee_workbench_protocol::parse_snapshot(
        &serde_json::to_vec(
            &serde_json::from_str::<Value>(include_str!(
                "../../../../../../../schemas/workbench/v1/workbench-conformance-v1.json"
            ))
            .unwrap()["documents"]["snapshot-full-r2"],
        )
        .unwrap(),
    )
    .unwrap();
    let context = &snapshot.terminal_contexts[1];
    let before = &context.tabs[0];
    let PanelNode::Split {
        split_id,
        children,
        ratio,
        ..
    } = &before.root
    else {
        panic!()
    };
    let adjustment = ClientLayoutAdjustment {
        terminal_context_id: context.terminal_context_id.clone(),
        tab_id: before.tab_id.clone(),
        workbench_revision: WorkbenchRevision::new(3),
        selected: false,
        change: ClientLayoutChange::Split {
            split: ClientSplitChange {
                split_id: split_id.clone(),
                first_slot_id: children[0].slot_id.clone(),
                second_slot_id: children[1].slot_id.clone(),
                previous_ratio: *ratio,
                ratio: WorkbenchSplitRatio::new(0.7).unwrap(),
            },
        },
    };
    let candidate = with_ratio(before, &context.terminal_context_id, &adjustment).unwrap();
    let mut value = serde_json::to_value(before).unwrap();
    value["root"]["ratio"] = json!(0.7);
    assert_eq!(serde_json::to_value(candidate).unwrap(), value);
    for case in ["tab", "context", "split", "first", "second", "previous"] {
        let mut corrupt = serde_json::to_value(&adjustment).unwrap();
        match case {
            "tab" => corrupt["tabId"] = json!(id(999)),
            "context" => corrupt["terminalContextId"] = json!(id(999)),
            "previous" => corrupt["change"]["split"]["previousRatio"] = json!(0.3),
            other => {
                corrupt["change"]["split"][match other {
                    "split" => "splitId",
                    "first" => "firstSlotId",
                    _ => "secondSlotId",
                }] = json!(id(999))
            }
        }
        assert!(
            with_ratio(
                before,
                &context.terminal_context_id,
                &serde_json::from_value(corrupt).unwrap()
            )
            .is_err(),
            "{case}"
        );
    }
    assert_eq!(
        serde_json::to_value(before).unwrap()["root"].get("ratio"),
        None
    );
}
fn input() -> Value {
    json!({"type":"panel","rightClick":"panel","primary":{"name":"Empty","start":false},
    "children":[{"name":"Child","label":"Manual","start":true}]})
}
fn plan() -> crate::cli::layouts::template::Plan {
    crate::cli::layouts::template::parse(&serde_json::to_vec(&input()).unwrap())
        .unwrap()
        .plan()
}
fn tab() -> Tab {
    serde_json::from_value(json!({"tabId":id(1),"label":"Tab","pinned":false,"groupId":null,
    "root":{"kind":"panel","panelId":id(2),"rightClick":"panel","primaryPresentation":{"presentationId":id(3),"label":"Empty","binding":null},
    "childPresentations":[{"presentationId":id(4),"label":"Child","customLabel":"Manual","binding":null}]}})).unwrap()
}
#[test]
fn materialization_must_match_every_member_label_policy_and_unbound_state() {
    let expected = tab();
    let plan = plan();
    let (panels, splits) = materialized(&plan, &expected.root).unwrap();
    assert_eq!(panels, [WorkbenchPanelId::parse(&id(2)).unwrap()]);
    assert!(splits.is_empty());
    let root = serde_json::to_value(expected.root).unwrap();
    for (pointer, value) in [
        ("/rightClick", json!("orchbee")),
        ("/primaryPresentation/label", json!("Wrong")),
        ("/childPresentations/0/customLabel", json!("Changed")),
        (
            "/childPresentations/0/binding",
            json!({"terminalId":id(5),"workerId":id(6)}),
        ),
    ] {
        let mut changed = root.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(
            materialized(&plan, &serde_json::from_value(changed).unwrap()).is_err(),
            "{pointer}"
        );
    }
}
#[test]
fn binding_adopts_only_one_exact_members_binding_and_no_other_tree_change() {
    let before = tab();
    let mut value = serde_json::to_value(&before).unwrap();
    value["root"]["childPresentations"][0]["binding"] =
        json!({"terminalId":id(5),"workerId":id(6)});
    let after: Tab = serde_json::from_value(value.clone()).unwrap();
    let panel = WorkbenchPanelId::parse(&id(2)).unwrap();
    let presentation = TerminalPresentationId::parse(&id(4)).unwrap();
    assert_eq!(bind(&before, &after, &panel, &presentation).unwrap(), after);
    assert!(bind(&after, &after, &panel, &presentation).is_err());
    for (pointer, new) in [
        ("/pinned", json!(true)),
        ("/root/primaryPresentation/label", json!("Concurrent")),
        ("/root/childPresentations/0/presentationId", json!(id(9))),
        ("/root/childPresentations/0/customLabel", json!("Other")),
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(pointer).unwrap() = new;
        assert!(
            bind(
                &before,
                &serde_json::from_value(changed).unwrap(),
                &panel,
                &presentation
            )
            .is_err(),
            "{pointer}"
        );
    }
}
#[test]
fn final_observation_requires_child_order_start_intent_labels_and_policy() {
    let plan = plan();
    let panels = [WorkbenchPanelId::parse(&id(2)).unwrap()];
    let mut observed = input();
    observed["panelId"] = json!(id(2));
    assert!(matches_template(&plan, &panels, &observed));
    for (pointer, new) in [
        ("/rightClick", json!("orchbee")),
        ("/primary/start", json!(true)),
        ("/children/0/label", json!("Changed")),
        ("/children", json!([])),
        ("/panelId", json!(id(8))),
    ] {
        let mut changed = observed.clone();
        *changed.pointer_mut(pointer).unwrap() = new;
        assert!(!matches_template(&plan, &panels, &changed), "{pointer}");
    }
}
