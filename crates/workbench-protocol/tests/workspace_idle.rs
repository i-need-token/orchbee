use orchbee_workbench_protocol::*;

#[test]
fn workspace_idle_shared_corpus_carrier_kinds_and_bound() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-workspace-idle-conformance-v1.json"
    ))
    .unwrap();
    for (key, expected) in [("valid", true), ("invalid", false)] {
        for case in corpus[key].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let actual = if case["direction"] == "request" {
                parse_workbench_request(&bytes).is_ok()
            } else {
                parse_workbench_response(&bytes).is_ok()
            };
            assert_eq!(actual, expected, "{}", case["name"]);
        }
    }
    let document = |name: &str| {
        let case = corpus["valid"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == name)
            .unwrap();
        serde_json::to_vec(&case["document"]).unwrap()
    };
    let request = parse_workbench_request(&document("read")).unwrap();
    let response = parse_workbench_response(&document("busy")).unwrap();
    let mut bytes = Vec::new();
    write_workbench_request(&mut bytes, &request).unwrap();
    assert_eq!(&bytes[6..8], &81_u16.to_be_bytes());
    assert_eq!(
        read_workbench_request(&mut bytes.as_slice()).unwrap(),
        request
    );
    bytes.clear();
    write_workbench_response(&mut bytes, &response).unwrap();
    assert_eq!(&bytes[6..8], &82_u16.to_be_bytes());
    assert_eq!(
        read_workbench_response(&mut bytes.as_slice()).unwrap(),
        response
    );

    // The longest result the protocol admits is exactly the declared bound, and it travels.
    let WorkbenchResponse::WorkspaceIdle(mut longest) = response else {
        panic!("the busy case is a Workspace idle result");
    };
    let id = |kind: u128, index: usize| format!("{:032x}", (kind << 64) | (index as u128 + 1));
    longest.outcome = WorkspaceIdleOutcome::Busy {
        members: (0..MAXIMUM_PANEL_NODES_PER_CONTEXT)
            .map(|index| {
                serde_json::from_value(serde_json::json!({
                    "tabId": id(1, index),
                    "panelId": id(2, index),
                    "presentationId": id(3, index),
                    "reason": "unknown",
                }))
                .unwrap()
            })
            .collect(),
    };
    let longest = WorkbenchResponse::WorkspaceIdle(longest);
    let encoded = serialize_workbench_response(&longest).unwrap();
    assert_eq!(encoded.len(), MAXIMUM_ENCODED_WORKSPACE_IDLE_RESULT_BYTES);
    bytes.clear();
    write_workbench_response(&mut bytes, &longest).unwrap();
    assert_eq!(
        read_workbench_response(&mut bytes.as_slice()).unwrap(),
        longest
    );
}
