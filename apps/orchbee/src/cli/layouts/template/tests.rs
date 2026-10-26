use super::*;
use orchbee_client_server_protocol::MAXIMUM_LAUNCH_SELECTION_BYTES;
use serde_json::{Value, json};

fn leaf(name: &str) -> Value {
    json!({"type":"panel","label":name})
}
fn split(first: Value, second: Value) -> Value {
    json!({"type":"split","direction":"right","ratio":0.3,"first":first,"second":second})
}
fn parse_value(value: Value) -> Result<Node, InputError> {
    parse(&serde_json::to_vec(&value).expect("fixture"))
}

#[test]
fn exports_and_templates_keep_literal_launch_values_without_reusing_ids() {
    let root = json!({"type":"panel","panelId":"5".repeat(32),"label":" 中文 ",
        "cwd":"C:\\含 空格", "command":["pwsh","-Command","$x='literal'",""],
        "env":{"LAYOUT_EMPTY":"","LAYOUT_LITERAL":"$(not executed)"}});
    let node = parse_value(root.clone()).expect("Panel template");
    let Node::Panel(leaf) = &node else {
        panic!("leaf")
    };
    assert_eq!(
        leaf.primary.label.custom_label().expect("manual").as_str(),
        "中文"
    );
    assert_eq!(leaf.primary.command.as_ref().expect("argv")[3], "");
    assert_eq!(leaf.primary.environment.entries()[1].1, "$(not executed)");
    assert_eq!(
        parse_value(json!({"workspaceId":"7".repeat(32), "tabId":"4".repeat(32),
        "zoomed":true,"focusedPanelId":"5".repeat(32),"root":root}))
        .expect("description"),
        node
    );
    let export = json!({"schema":"orchbee.layout.export","version":1,
        "source":{"kind":"layout-observation","scope":"client","siteId":"1".repeat(32),"revision":"1","clientId":"2".repeat(32),"viewRevision":"1"},
        "result":{"layout":{"workspaceId":"7".repeat(32),"terminalContextId":"3".repeat(32),"tabId":"4".repeat(32),
            "focusedPanelId":null,"zoomed":true,"root":{"type":"panel","panelId":"5".repeat(32),
                "rightClick":"orchbee","children":[],"primary":{"presentationId":"6".repeat(32),
                    "name":"Retained","label":null,"cwd":null,"command":null,"env":{},"start":false}}}}});
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend(serde_json::to_vec(&export).expect("JSON"));
    let plan = parse(&bytes)
        .expect("current native envelope and BOM")
        .plan();
    assert_eq!(plan.leaves[0].primary.name.as_str(), "Retained");
    assert!(!plan.leaves[0].primary.start);
    for version in [2, 3, 99] {
        let mut invalid = export.clone();
        invalid["version"] = json!(version);
        assert_eq!(
            parse_value(invalid),
            Err(InputError::UnsupportedVersion(version))
        );
    }
    for (parent, field) in [
        ("/source", "scope"),
        ("/source", "clientId"),
        ("/result/layout", "workspaceId"),
        ("/result/layout", "zoomed"),
        ("/result/layout/root", "children"),
        ("/result/layout/root/primary", "label"),
        ("/result/layout/root/primary", "cwd"),
        ("/result/layout/root/primary", "command"),
    ] {
        let mut invalid = export.clone();
        invalid
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            parse_value(invalid),
            Err(InputError::InvalidLayout),
            "{field}"
        );
    }
    let mut missing_workspace = export.clone();
    missing_workspace["result"]["layout"]["workspaceId"] = Value::Null;
    assert_eq!(
        parse_value(missing_workspace),
        Err(InputError::InvalidLayout)
    );
    let mut invalid = export;
    invalid["result"]["layout"]["root"] = root;
    assert_eq!(parse_value(invalid), Err(InputError::InvalidLayout));
}

#[test]
fn nested_plan_creates_each_leaf_once_and_preserves_order_paths_and_ratios() {
    let mut right = split(leaf("C"), leaf("D"));
    right["direction"] = json!("down");
    right["ratio"] = json!(2.0);
    let plan = parse_value(split(split(leaf("A"), leaf("B")), right))
        .expect("tree")
        .plan();
    assert_eq!(
        plan.leaves
            .iter()
            .map(|leaf| leaf.primary.label.custom_label().expect("name").as_str())
            .collect::<Vec<_>>(),
        ["A", "C", "B", "D"]
    );
    assert_eq!(
        plan.splits
            .iter()
            .map(|split| (split.source, split.created, split.path.indices()))
            .collect::<Vec<_>>(),
        [
            (0, 1, [].as_slice()),
            (0, 2, [0].as_slice()),
            (1, 3, [1].as_slice())
        ]
    );
    assert_eq!(
        plan.splits[2].direction,
        WorkbenchPanelSplitDirection::Below
    );
    assert_eq!(plan.splits[2].ratio.value(), 0.9);
    assert_eq!(plan.splits[0].ratio.value(), 0.3);
}

#[test]
fn every_leaf_is_validated_before_a_plan_escapes() {
    for bad in [
        json!({"type":"panel","command":[]}),
        json!({"type":"panel","command":[""]}),
        json!({"type":"panel","command":["pwsh",null]}),
        json!({"type":"panel","command":["pwsh","\u{0}"]}),
        json!({"type":"panel","cwd":""}),
        json!({"type":"panel","env":{"PATH":"a","path":"b"}}),
        json!({"type":"panel","env":{"ORCHBEE_SESSION":"x"}}),
        json!({"type":"panel","env":null}),
        json!({"type":"panel","unknown":true}),
        json!({"type":"panel","focus":true}),
        json!({"type":"split","direction":"diagonal","ratio":0.5,"first":leaf("x"),"second":leaf("y")}),
    ] {
        assert_eq!(
            parse_value(split(leaf("valid"), bad)),
            Err(InputError::InvalidLayout)
        );
    }
    for bytes in [
        br#"{"type":"panel","type":"split"}"#.as_slice(),
        br#"{"root":{"type":"panel","env":{"X":"first","X":"last"}}}"#,
        br#"{"type":"split","direction":"right","ratio":0.5,"first":{"type":"panel"},"second":{"type":"panel","label":"a","label":"b"}}"#,
        br#"{"type":"panel"} {}"#,
    ] {
        assert_eq!(parse(bytes),Err(InputError::InvalidLayout));
    }
}

#[test]
fn limits_cover_total_bytes_leaf_count_depth_and_launch_contracts() {
    fn balanced(count: usize) -> Value {
        if count == 1 {
            leaf("x")
        } else {
            split(balanced(count / 2), balanced(count - count / 2))
        }
    }
    assert_eq!(
        parse_value(balanced(24)).expect("24").plan().leaves.len(),
        24
    );
    assert_eq!(parse_value(balanced(25)), Err(InputError::TooLarge));
    let mut chain = leaf("x");
    for _ in 1..16 {
        chain = split(chain, leaf("x"));
    }
    assert!(parse_value(chain.clone()).is_ok());
    assert_eq!(
        parse_value(split(chain, leaf("x"))),
        Err(InputError::TooLarge)
    );
    let mut bytes = serde_json::to_vec(&leaf("x")).expect("JSON");
    bytes.resize(MAX_BYTES, b' ');
    assert!(parse(&bytes).is_ok());
    bytes.push(b' ');
    assert_eq!(parse(&bytes), Err(InputError::TooLarge));
    let mut command = vec!["pwsh".to_owned()];
    command.resize(129, String::new());
    assert!(parse_value(json!({"type":"panel","command":command})).is_ok());
    command.push(String::new());
    assert_eq!(
        parse_value(json!({"type":"panel","command":command})),
        Err(InputError::InvalidLayout)
    );
    assert!(
        parse_value(json!({"type":"panel","command":["x".repeat(MAXIMUM_LAUNCH_SELECTION_BYTES)]}))
            .is_ok()
    );
    assert_eq!(
        parse_value(
            json!({"type":"panel","command":["x".repeat(MAXIMUM_LAUNCH_SELECTION_BYTES + 1)]})
        ),
        Err(InputError::InvalidLayout)
    );
}

#[test]
fn structured_members_preserve_unbound_intent_and_validate_all_children_before_effects() {
    let member = json!({"name":"Shell","label":"Manual","start":false,"env":{}});
    let input = json!({"type":"panel","rightClick":"panel","primary":member,
        "children":[{"name":"Agent","start":true,"command":["pwsh","","literal"],"env":{"CHILD_VALUE":"secret"}}]});
    let plan = parse_value(input.clone()).unwrap().plan();
    assert!(!plan.leaves[0].primary.start);
    assert!(plan.leaves[0].children[0].start);
    assert_eq!(
        plan.leaves[0].children[0].environment.entries()[0].1,
        "secret"
    );
    assert_eq!(plan.leaves[0].right_click, PanelRightClickTarget::Panel);
    for (path, value) in [
        ("/primary/command", json!(["pwsh"])),
        ("/primary/cwd", json!("C:\\work")),
        ("/primary/env", json!({"EXTRA":"value"})),
        ("/children/0/start", json!(false)),
        ("/children/0/unknown", json!(true)),
        ("/command", json!(["pwsh"])),
        ("/rightClick", json!("both")),
    ] {
        let mut bad = input.clone();
        let (parent, key) = path.rsplit_once('/').unwrap();
        bad.pointer_mut(parent).unwrap()[key] = value;
        assert_eq!(parse_value(bad), Err(InputError::InvalidLayout), "{path}");
    }
    let raw = br#"{"type":"panel","primary":{"name":"x","start":false},"children":[{"name":"x","start":true,"start":false}]}"#;
    assert_eq!(parse(raw), Err(InputError::InvalidLayout));
    let mut max = input;
    max["children"] = json!(vec![json!({"name":"Empty","start":false}); 16]);
    assert_eq!(
        parse_value(max.clone()).unwrap().plan().leaves[0]
            .children
            .len(),
        16
    );
    max["children"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"Extra","start":false}));
    assert_eq!(parse_value(max), Err(InputError::TooLarge));
}
