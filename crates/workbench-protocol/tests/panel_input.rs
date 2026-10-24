use orchbee_workbench_protocol::{PanelNode, PanelRightClickTarget};
use serde_json::json;

#[test]
fn panel_policy_has_a_closed_domain_and_an_omitted_default() {
    let original = json!({
        "kind":"panel", "panelId":"11111111111111111111111111111111",
        "childPresentations":[], "primaryPresentation":{"customLabel":null,
            "binding":null,"label":"terminal","presentationId":"22222222222222222222222222222222"
        }
    });
    let parsed: PanelNode = serde_json::from_value(original.clone()).unwrap();
    assert!(matches!(
        parsed,
        PanelNode::Panel {
            right_click: PanelRightClickTarget::Orchbee,
            ..
        }
    ));
    assert_eq!(serde_json::to_value(&parsed).unwrap(), original);
    for policy in ["orchbee", "panel"] {
        let mut supplied = original.clone();
        supplied["rightClick"] = json!(policy);
        let parsed: PanelNode = serde_json::from_value(supplied.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(parsed).unwrap(),
            if policy == "orchbee" {
                original.clone()
            } else {
                supplied
            }
        );
    }
    for invalid in [
        json!(null),
        json!("pane"),
        json!("herdr"),
        json!(""),
        json!(false),
        json!(1),
    ] {
        let mut supplied = original.clone();
        supplied["rightClick"] = invalid;
        assert!(serde_json::from_value::<PanelNode>(supplied).is_err());
    }
}
