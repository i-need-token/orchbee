use orchbee_workbench_protocol::*;
#[test]
fn window_title_matches_shared_corpus_and_distinct_carrier_kinds() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-window-title-conformance-v1.json"
    ))
    .unwrap();
    for (name, accepted) in [("valid", true), ("invalid", false)] {
        for case in corpus[name].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let result = if case["direction"] == "request" {
                parse_workbench_request(&bytes).map(|request| {
                    let mut wire = Vec::new();
                    write_workbench_request(&mut wire, &request).unwrap();
                    assert_eq!(&wire[6..8], &43_u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_request(&mut wire.as_slice()).unwrap(),
                        request
                    );
                })
            } else {
                parse_workbench_response(&bytes).map(|result| {
                    let mut wire = Vec::new();
                    write_workbench_response(&mut wire, &result).unwrap();
                    assert_eq!(&wire[6..8], &44_u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_response(&mut wire.as_slice()).unwrap(),
                        result
                    );
                })
            };
            assert_eq!(result.is_ok(), accepted, "{}", case["name"]);
        }
    }
}

#[test]
fn window_title_normalizes_controls_then_scalar_limit_then_trim() {
    assert_eq!(
        WindowTitle::normalize("  a\tb  中\x1b\n c ")
            .unwrap()
            .as_str(),
        "ab  中 c"
    );
    assert!(WindowTitle::normalize(" \t\u{0085}").is_err());
    assert_eq!(
        WindowTitle::normalize(&"😀".repeat(201))
            .unwrap()
            .as_str()
            .chars()
            .count(),
        200
    );
    assert!(WindowTitle::normalize(&format!("{}x", " ".repeat(200))).is_err());
}

#[test]
fn window_title_rejects_unpaired_surrogate_on_the_wire() {
    assert!(parse_workbench_request(br#"{"kind":"workbench.window-title","protocolVersion":1,"requestId":"11111111111111111111111111111111","siteId":"22222222222222222222222222222222","action":{"kind":"set","title":"\ud800"}}"#).is_err());
}
