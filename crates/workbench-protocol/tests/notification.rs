use orchbee_workbench_protocol::*;
#[test]
fn notification_normalization_matches_fixed_vectors_and_scalar_bounds() {
    let n = Notification::normalize(
        "  a\t b\u{0085}\x1b\n c ",
        Some(" \t "),
        NotificationPosition::BottomLeft,
        NotificationSound::None,
    )
    .unwrap();
    assert_eq!(n.title, "a b c");
    assert_eq!(n.body, None);
    n.validate().unwrap();
    assert!(
        Notification::normalize(
            "\u{0085} ",
            None,
            NotificationPosition::TopLeft,
            NotificationSound::None
        )
        .is_err()
    );
    let n = Notification::normalize(
        &"😀".repeat(81),
        Some(&"中".repeat(241)),
        NotificationPosition::TopRight,
        NotificationSound::Done,
    )
    .unwrap();
    assert_eq!(n.title.chars().count(), 80);
    assert_eq!(n.title.len(), 320);
    assert_eq!(n.body.unwrap().chars().count(), 240);
}
#[test]
fn notification_matches_shared_corpus_and_distinct_carrier_kinds() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-notification-conformance-v1.json"
    ))
    .unwrap();
    for (name, accepted) in [("valid", true), ("invalid", false)] {
        for case in corpus[name].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let result = if case["direction"] == "request" {
                parse_workbench_request(&bytes).map(|request| {
                    let mut wire = Vec::new();
                    write_workbench_request(&mut wire, &request).unwrap();
                    assert_eq!(&wire[6..8], &41_u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_request(&mut wire.as_slice()).unwrap(),
                        request
                    );
                })
            } else {
                parse_workbench_response(&bytes).map(|result| {
                    let mut wire = Vec::new();
                    write_workbench_response(&mut wire, &result).unwrap();
                    assert_eq!(&wire[6..8], &42_u16.to_be_bytes());
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
