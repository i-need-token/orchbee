use super::*;
use serde_json::{Value, json};
fn id(n: u32) -> String {
    format!("{n:032x}")
}
fn panel(n: u32) -> Value {
    json!({"kind":"panel","panelId":id(n),"childPresentations":[],"primaryPresentation":{"presentationId":id(n+10),"label":"shell","binding":{"terminalId":id(n+20),"workerId":id(n+30)}}})
}
fn split(first: Value, second: Value) -> Value {
    json!({"kind":"split","orientation":"horizontal","splitId":id(50),"children":[{"slotId":id(51),"node":first},{"slotId":id(52),"node":second}]})
}
fn node(value: Value) -> PanelNode {
    serde_json::from_value(value).unwrap()
}
fn identity() -> ClientSplitIdentity {
    serde_json::from_value(json!({"splitId":id(50),"firstSlotId":id(51),"secondSlotId":id(52)}))
        .unwrap()
}
#[test]
fn graft_only_adopts_the_acknowledged_new_leaf_and_preserves_the_old_binding() {
    let source = WorkbenchPanelId::parse(&id(1)).unwrap();
    let created = WorkbenchPanelId::parse(&id(2)).unwrap();
    let before = node(panel(1));
    let after = node(split(panel(1), panel(2)));
    let mut expected = before.clone();
    graft(
        &mut expected,
        &after,
        &source,
        &created,
        &identity(),
        WorkbenchPanelSplitDirection::Right,
        &id(12),
    )
    .unwrap();
    assert_eq!(expected, after);
    let original = split(panel(1), panel(2));
    for pointer in [
        "/splitId",
        "/children/0/slotId",
        "/children/1/slotId",
        "/children/0/node/primaryPresentation/binding/terminalId",
        "/children/1/node/panelId",
        "/children/1/node/primaryPresentation/presentationId",
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(pointer).unwrap() = json!(id(99));
        assert!(
            graft(
                &mut before.clone(),
                &node(changed),
                &source,
                &created,
                &identity(),
                WorkbenchPanelSplitDirection::Right,
                &id(12)
            )
            .is_err(),
            "{pointer}"
        );
    }
    for (key, value) in [
        ("rightClick", json!("panel")),
        (
            "childPresentations",
            json!([panel(9)["primaryPresentation"].clone()]),
        ),
    ] {
        let mut changed = original.clone();
        changed["children"][1]["node"][key] = value;
        assert!(
            graft(
                &mut before.clone(),
                &node(changed),
                &source,
                &created,
                &identity(),
                WorkbenchPanelSplitDirection::Right,
                &id(12)
            )
            .is_err()
        );
    }
}
#[test]
fn labels_change_only_the_requested_presentation() {
    let mut expected = node(split(panel(1), panel(2)));
    label(
        &mut expected,
        &WorkbenchPanelId::parse(&id(2)).unwrap(),
        &orchbee_workbench_protocol::PanelLabelUpdate::parse(Some("中文 label")).unwrap(),
    )
    .unwrap();
    let mut desired = split(panel(1), panel(2));
    desired["children"][1]["node"]["primaryPresentation"]["customLabel"] = json!("中文 label");
    assert_eq!(expected, node(desired));
    assert!(
        label(
            &mut expected,
            &WorkbenchPanelId::parse(&id(90)).unwrap(),
            &Default::default()
        )
        .is_err()
    );
}
#[test]
fn final_observation_rejects_foreign_ratios_labels_and_leaf_identity() {
    let input = json!({"type":"split","direction":"right","ratio":0.3,"first":{"type":"panel","label":"one"},"second":{"type":"panel"}});
    let plan = crate::cli::layouts::template::parse(input.to_string().as_bytes())
        .unwrap()
        .plan();
    let panels = [
        WorkbenchPanelId::parse(&id(1)).unwrap(),
        WorkbenchPanelId::parse(&id(2)).unwrap(),
    ];
    let mut output = input.clone();
    output["first"]["panelId"] = json!(id(1));
    output["second"]["panelId"] = json!(id(2));
    assert!(matches_template(&plan, &panels, &output));
    for (pointer, value) in [
        ("/ratio", json!(0.7)),
        ("/first/label", json!("other")),
        ("/second/panelId", json!(id(1))),
        ("/direction", json!("down")),
    ] {
        let mut changed = output.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert!(!matches_template(&plan, &panels, &changed), "{pointer}");
    }
}
