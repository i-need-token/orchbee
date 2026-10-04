use orchbee_workbench_protocol::{
    PanelNode, WorkbenchProtocolDocument, parse_snapshot, serialize_document,
};
use serde_json::{Value, json};

#[test]
fn directional_ratios_retain_exact_identity_through_numeric_json() {
    use orchbee_workbench_protocol::{
        ClientPanelDirection, ClientResizeAmount, WorkbenchSplitRatio,
    };
    // The production decoder once rounded 0.54 - 0.05 from
    // 0.49000000000000005 to 0.49, making a committed adjustment look stale.
    // Workspace dev dependencies hid that failure by enabling float_roundtrip.
    for percentage in 10..=90 {
        let before = WorkbenchSplitRatio::new(f64::from(percentage) / 100.0).unwrap();
        for amount in [0.01, 0.05] {
            for direction in [
                ClientPanelDirection::Left,
                ClientPanelDirection::Right,
                ClientPanelDirection::Up,
                ClientPanelDirection::Down,
            ] {
                let ratio = ClientResizeAmount::new(amount)
                    .unwrap()
                    .apply(before, direction);
                let encoded = serde_json::to_vec(&ratio).unwrap();
                let decoded: WorkbenchSplitRatio = serde_json::from_slice(&encoded).unwrap();
                assert_eq!(decoded, ratio, "{percentage}% {amount} {direction:?}");
                assert!(
                    serde_json::from_slice::<Value>(&encoded)
                        .unwrap()
                        .is_number()
                );
            }
        }
    }
}

fn document() -> Value {
    serde_json::from_str::<Value>(include_str!(
        "../../../schemas/workbench/v1/workbench-conformance-v1.json"
    ))
    .unwrap()["documents"]["snapshot-full-r2"]
        .clone()
}
fn root(document: &mut Value) -> &mut Value {
    &mut document["terminalContexts"][1]["tabs"][0]["root"]
}

#[test]
fn nondefault_canonical_ratio_survives_decode_and_projection_roundtrip() {
    let mut value = document();
    root(&mut value)["ratio"] = json!(0.7);
    let snapshot = parse_snapshot(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(matches!(
        snapshot.terminal_contexts[1].tabs[0].root,
        PanelNode::Split { .. }
    ));
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap();
    let mut actual: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(root(&mut actual)["ratio"], json!(0.7));
}

#[test]
fn equal_default_is_omitted_and_legacy_documents_remain_readable() {
    let old = document();
    let expected = parse_snapshot(&serde_json::to_vec(&old).unwrap()).unwrap();
    let mut explicit = old;
    root(&mut explicit)["ratio"] = json!(0.5);
    let actual = parse_snapshot(&serde_json::to_vec(&explicit).unwrap()).unwrap();
    assert_eq!(actual, expected);
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(actual)).unwrap();
    let mut value: Value = serde_json::from_slice(&encoded).unwrap();
    assert!(root(&mut value).get("ratio").is_none());
}

#[test]
fn ratio_rejects_null_wrong_types_and_values_outside_supported_domain() {
    for invalid in [
        json!(null),
        json!("0.5"),
        json!(0.099),
        json!(0.901),
        json!(-1),
        json!(1),
    ] {
        let mut value = document();
        root(&mut value)["ratio"] = invalid.clone();
        assert!(
            parse_snapshot(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{invalid}"
        );
    }
    for valid in [0.1, 0.1001, 0.4999, 0.5, 0.5001, 0.8999, 0.9] {
        let mut value = document();
        root(&mut value)["ratio"] = json!(valid);
        assert!(
            parse_snapshot(&serde_json::to_vec(&value).unwrap()).is_ok(),
            "{valid}"
        );
    }
}

#[test]
fn mutation_requires_bounded_ratios_ordered_identity_and_roundtrips_canonical_result() {
    use orchbee_workbench_protocol::{
        WorkbenchAction, WorkbenchCommandOutcome, WorkbenchRequest, WorkbenchResponse,
        parse_workbench_request, parse_workbench_response,
    };
    let id = |value: u64| format!("{value:032x}");
    let request = json!({"kind":"workbench.command","protocolVersion":1,"requestId":id(1),
        "siteId":id(2),"expectedRevision":"3","action":{"kind":"layout.set-split-ratio",
        "terminalContextId":id(4),"tabId":id(5),"split":{"splitId":id(6),
        "firstSlotId":id(7),"secondSlotId":id(8)},"previousRatio":0.5,"ratio":0.7}});
    let parsed = parse_workbench_request(&serde_json::to_vec(&request).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), request);
    assert!(
        matches!(parsed, WorkbenchRequest::Command(command) if matches!(command.action, WorkbenchAction::SplitRatioSet { .. }))
    );
    for field in ["ratio", "previousRatio"] {
        for invalid in [json!(null), json!("0.7"), json!(0.0999), json!(0.9001)] {
            let mut corrupt = request.clone();
            corrupt["action"][field] = invalid;
            assert!(parse_workbench_request(&serde_json::to_vec(&corrupt).unwrap()).is_err());
        }
    }
    let mut alias = request.clone();
    alias["action"]["split"]["secondSlotId"] = json!(id(7));
    assert!(parse_workbench_request(&serde_json::to_vec(&alias).unwrap()).is_err());
    let result = json!({"kind":"workbench.command.result","protocolVersion":1,"requestId":id(1),
        "outcome":{"kind":"split-ratio-configured","revision":"4"}});
    let parsed = parse_workbench_response(&serde_json::to_vec(&result).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), result);
    assert!(
        matches!(parsed, WorkbenchResponse::Command(result) if matches!(result.outcome, WorkbenchCommandOutcome::SplitRatioConfigured { .. }))
    );
}
