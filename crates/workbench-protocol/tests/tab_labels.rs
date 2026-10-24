use orchbee_workbench_protocol::{Tab, WorkbenchAction};
use serde_json::json;

fn tab_value() -> serde_json::Value {
    json!({
        "tabId": "1".repeat(32), "label":"7", "groupId":null, "pinned":false,
        "root":{"kind":"panel","panelId":"2".repeat(32),"childPresentations":[],
            "primaryPresentation":{"customLabel":null,"presentationId":"3".repeat(32),"label":"7","binding":null}}
    })
}

#[test]
fn legacy_numeric_names_stay_fixed_and_automatic_names_follow_only_context_position() {
    let value = tab_value();
    let fixed: Tab = serde_json::from_value(value.clone()).unwrap();
    assert!(!fixed.automatic_label);
    assert_eq!(serde_json::to_value(&fixed).unwrap(), value);
    for position in 0..256 {
        assert_eq!(fixed.display_label(position), "7");
        let automatic = Tab {
            automatic_label: true,
            ..fixed.clone()
        };
        assert_eq!(
            automatic.display_label(position),
            (position + 1).to_string()
        );
        assert_eq!(automatic.label, fixed.label);
        assert_eq!(automatic.root, fixed.root);
    }
    let mut explicit_false = value;
    explicit_false["automaticLabel"] = json!(false);
    assert_eq!(
        serde_json::from_value::<Tab>(explicit_false).unwrap(),
        fixed
    );
}

#[test]
fn naming_mode_is_strict_boolean_in_both_state_and_creation_intent() {
    let command = json!({"kind":"tab.create","terminalContextId":"4".repeat(32),
        "label":"7","geometry":{"columns":80,"rows":24,"cellPixelWidth":0,"cellPixelHeight":0}});
    for automatic in [false, true] {
        let mut value = command.clone();
        value["automaticLabel"] = json!(automatic);
        let parsed: WorkbenchAction = serde_json::from_value(value).unwrap();
        assert!(
            matches!(parsed, WorkbenchAction::TabCreate {automatic_label, ..} if automatic_label == automatic)
        );
        let encoded = serde_json::to_value(parsed).unwrap();
        assert_eq!(
            encoded.get("automaticLabel"),
            automatic.then_some(&json!(true))
        );
    }
    for invalid in [json!(null), json!(0), json!("true"), json!([]), json!({})] {
        let mut value = tab_value();
        value["automaticLabel"] = invalid.clone();
        assert!(serde_json::from_value::<Tab>(value).is_err());
        let mut value = command.clone();
        value["automaticLabel"] = invalid;
        assert!(serde_json::from_value::<WorkbenchAction>(value).is_err());
    }
}
