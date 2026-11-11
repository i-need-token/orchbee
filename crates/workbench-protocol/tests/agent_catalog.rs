use orchbee_workbench_protocol::*;
#[test]
fn agent_catalog_shared_corpus_and_carrier_kinds() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-agent-catalog-conformance-v1.json"
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
    let request = parse_workbench_request(&document("refresh")).unwrap();
    let response = parse_workbench_response(&document("complete-catalog")).unwrap();
    let mut bytes = Vec::new();
    write_workbench_request(&mut bytes, &request).unwrap();
    assert_eq!(&bytes[6..8], &75_u16.to_be_bytes());
    assert_eq!(
        read_workbench_request(&mut bytes.as_slice()).unwrap(),
        request
    );
    bytes.clear();
    write_workbench_response(&mut bytes, &response).unwrap();
    assert_eq!(&bytes[6..8], &76_u16.to_be_bytes());
    assert_eq!(
        read_workbench_response(&mut bytes.as_slice()).unwrap(),
        response
    );
}
