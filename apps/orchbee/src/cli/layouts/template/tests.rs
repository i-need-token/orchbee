use super::*;
use serde_json::{Value, json};

fn leaf(name: &str) -> Value {
    json!({"type":"panel","label":name})
}
fn split(first: Value, second: Value) -> Value {
    json!({"type":"split","direction":"right","ratio":0.3,"first":first,"second":second})
}
fn parse_value(value: Value) -> Result<Node, &'static str> {
    parse(&serde_json::to_vec(&value).expect("fixture"))
}

#[test]
fn export_and_herdr_templates_keep_literal_launch_values_without_reusing_ids() {
    let root = json!({"type":"pane","pane_id":"old:1:2","label":" 中文 ",
        "cwd":"C:\\含 空格", "command":["pwsh","-Command","$x='literal'",""],
        "env":{"LAYOUT_EMPTY":"","LAYOUT_LITERAL":"$(not executed)"}});
    let node = parse_value(root.clone()).expect("Herdr pane");
    let Node::Panel(leaf) = &node else {
        panic!("leaf")
    };
    assert_eq!(leaf.label.custom_label().expect("manual").as_str(), "中文");
    assert_eq!(leaf.command.as_ref().expect("argv")[3], "");
    assert_eq!(leaf.environment.entries()[1].1, "$(not executed)");
    assert_eq!(
        parse_value(json!({"workspace_id":"old:1", "tab_id":"old:1:2",
        "zoomed":true,"focused_pane_id":"ignored","root":root}))
        .expect("description"),
        node
    );
    let export = json!({"schema":"orchbee.layout.export","version":1,
        "source":{"kind":"layout-observation","siteId":"old","revision":"1","clientId":"old","viewRevision":"1"},
        "result":{"layout":{"workspaceId":null,"terminalContextId":"old","tabId":"old",
            "focusedPanelId":null,"zoomed":true,"root":root}}});
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend(serde_json::to_vec(&export).expect("JSON"));
    assert_eq!(parse(&bytes).expect("native envelope and BOM"), node);
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
            .map(|leaf| leaf.label.custom_label().expect("name").as_str())
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
        json!({"type":"panel","env":{"ORCHBEE_SITE":"x"}}),
        json!({"type":"panel","env":null}),
        json!({"type":"panel","panelId":"one","pane_id":"two"}),
        json!({"type":"panel","focus":true}),
        json!({"type":"split","direction":"diagonal","ratio":0.5,"first":leaf("x"),"second":leaf("y")}),
    ] {
        assert_eq!(
            parse_value(split(leaf("valid"), bad)),
            Err("invalid-layout")
        );
    }
    for bytes in [
        br#"{"type":"panel","type":"pane"}"#.as_slice(),
        br#"{"root":{"type":"panel","env":{"X":"first","X":"last"}}}"#,
        br#"{"type":"split","direction":"right","ratio":0.5,"first":{"type":"panel"},"second":{"type":"panel","label":"a","label":"b"}}"#,
        br#"{"type":"panel"} {}"#,
    ] {
        assert_eq!(parse(bytes),Err("invalid-layout"));
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
    assert_eq!(parse_value(balanced(25)), Err("layout-too-large"));
    let mut chain = leaf("x");
    for _ in 1..16 {
        chain = split(chain, leaf("x"));
    }
    assert!(parse_value(chain.clone()).is_ok());
    assert_eq!(
        parse_value(split(chain, leaf("x"))),
        Err("layout-too-large")
    );
    let mut bytes = serde_json::to_vec(&leaf("x")).expect("JSON");
    bytes.resize(MAX_BYTES, b' ');
    assert!(parse(&bytes).is_ok());
    bytes.push(b' ');
    assert_eq!(parse(&bytes), Err("layout-too-large"));
    let mut command = vec!["pwsh".to_owned()];
    command.resize(129, String::new());
    assert!(parse_value(json!({"type":"panel","command":command})).is_ok());
    command.push(String::new());
    assert_eq!(
        parse_value(json!({"type":"panel","command":command})),
        Err("invalid-layout")
    );
    assert!(
        parse_value(json!({"type":"panel","command":["x".repeat(MAXIMUM_LAUNCH_SELECTION_BYTES)]}))
            .is_ok()
    );
    assert_eq!(
        parse_value(
            json!({"type":"panel","command":["x".repeat(MAXIMUM_LAUNCH_SELECTION_BYTES + 1)]})
        ),
        Err("invalid-layout")
    );
}
