use super::*;
#[test]
fn semantic_events_parser_owns_repeatable_filters_zero_deadline_and_rejects_scope_leakage() {
    use crate::invocation::{Request, parse_request};
    let parse = |args: &[&str]| parse_request(args.iter().map(std::ffi::OsString::from));
    let Request::Events { command, format } = parse(&[
        "events",
        "subscribe",
        "--event=tab.renamed",
        "--event",
        "pane.exited",
        "--timeout",
        "0",
    ])
    .unwrap() else {
        panic!()
    };
    assert_eq!(command.events, [Kind::TabRenamed, Kind::PaneExited]);
    assert_eq!(command.timeout_ms, Some(0));
    assert_eq!(format, OutputFormat::Json);
    assert!(parse(&["events", "wait"]).is_err());
    assert!(
        parse(&[
            "events",
            "wait",
            "--event",
            "tab.renamed",
            "--event",
            "pane.exited"
        ])
        .is_err()
    );
    assert!(parse(&["command", "list", "--filter", "{}"]).is_err());
    assert!(parse(&["events","wait","--filter",r#"{"kind":"event","event":"tab.renamed","scope":{"kind":"all"},"condition":{"kind":"any"},"program":"untrusted"}"#]).is_err());
    assert!(
        parse(&[
            "events",
            "wait",
            "--event",
            "pane.output_matched",
            "--panel",
            &"5".repeat(32),
            "--match",
            "ready",
            "--source",
            "visible"
        ])
        .is_err()
    );
}
#[test]
fn semantic_events_retained_tracker_rejects_duplicate_probe_or_history_after_scan_advance() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/workbench/v1/workbench-events-conformance-v1.json"
    ))
    .unwrap();
    let mut reply: SemanticEventsResult = serde_json::from_value({
        let mut value = corpus["valid"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == "real terminal settlement")
            .unwrap()["document"]
            .clone();
        value.as_object_mut().unwrap().remove("kind");
        value
    })
    .unwrap();
    let Outcome::Matched { cursor, .. } = &reply.outcome else {
        panic!()
    };
    let mut tracker = Tracker {
        cursor: SemanticEventCursor {
            epoch: cursor.epoch.clone(),
            sequence: WorkbenchRevision::new(0),
        },
        probe_sequence: WorkbenchRevision::new(0),
    };
    tracker.observe(&reply).unwrap();
    assert!(tracker.observe(&reply).is_err());
    let Outcome::Matched { event, .. } = &mut reply.outcome else {
        panic!()
    };
    event.position = SemanticEventPosition::Probe {
        subscription_id: orchbee_workbench_protocol::WorkbenchRequestId::parse(&format!(
            "{:032x}",
            3
        ))
        .unwrap(),
        sequence: WorkbenchRevision::new(u64::MAX),
    };
    tracker.observe(&reply).unwrap();
    assert!(tracker.observe(&reply).is_err());
}
