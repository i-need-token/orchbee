use super::*;

#[test]
fn current_stream_envelopes_validate_without_reopening_historical_event_names() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/orchbee.events.stream.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/workbench/v1/workbench-events-conformance-v1.json"
    ))
    .unwrap();
    let source = json!({"kind":"server","sessionId":"1".repeat(32),"revision":"1"});
    let mut kinds = std::collections::BTreeSet::new();
    for case in corpus["valid"].as_array().unwrap() {
        let mut value = case["document"].clone();
        value.as_object_mut().unwrap().remove("kind");
        let Ok(reply) = serde_json::from_value::<SemanticEventsResult>(value) else {
            continue;
        };
        let kind = match reply.outcome {
            Outcome::Opened { .. } => "opened",
            Outcome::Matched { .. } => "event",
            Outcome::Gap { .. } => "gap",
            _ => continue,
        };
        kinds.insert(kind);
        let mut document = json!({"schema":"orchbee.events.stream","version":1,
            "kind":kind,"source":source,"result":public_reply(&reply).unwrap()});
        validator.validate(&document).unwrap();
        if let Outcome::Matched { event, .. } = &reply.outcome
            && event.event.as_str().starts_with("pane.")
        {
            document["result"] = json!(reply);
            assert!(!validator.is_valid(&document));
        }
    }
    let closed = json!({"schema":"orchbee.events.stream","version":1,"kind":"closed",
        "source":source,"reason":"completed","cleanup":"closed"});
    validator.validate(&closed).unwrap();
    assert_eq!(
        kinds,
        std::collections::BTreeSet::from(["opened", "event", "gap"])
    );
}

#[test]
fn organization_events_have_one_nullable_project_location_in_both_machine_envelopes() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/workbench/v1/workbench-events-conformance-v1.json"
    ))
    .unwrap();
    for name in ["ungrouped-workspace-event", "grouped-workspace-event"] {
        let mut reply = corpus["valid"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == name)
            .unwrap()["document"]
            .clone();
        reply.as_object_mut().unwrap().remove("kind");
        let reply: SemanticEventsResult = serde_json::from_value(reply).unwrap();
        for (schema_id, document_schema) in [
            (
                "orchbee.events.stream",
                include_str!("../schemas/orchbee.events.stream.json"),
            ),
            (
                "orchbee.events.wait",
                include_str!("../schemas/orchbee.events.wait.json"),
            ),
        ] {
            let schema: serde_json::Value = serde_json::from_str(document_schema).unwrap();
            let validator = jsonschema::validator_for(&schema).unwrap();
            let mut document = json!({"schema":schema_id,"version":1,
                "source":{"kind":"server","sessionId":reply.session_id,"revision":"1"},"result":reply});
            if schema_id == "orchbee.events.stream" {
                document["kind"] = json!("event");
            } else {
                document["cleanup"] = json!("closed");
            }
            validator.validate(&document).unwrap();
            let mut wrong = document.clone();
            wrong["result"]["outcome"]["events"][0]["data"]["projectId"] = json!(null);
            assert!(!validator.is_valid(&wrong));
            wrong = document.clone();
            wrong["result"]["outcome"]["events"][0]["data"]["workspace"]
                .as_object_mut()
                .unwrap()
                .remove("projectId");
            assert!(!validator.is_valid(&wrong));
        }
    }
}

#[test]
fn semantic_events_parser_owns_repeatable_filters_zero_deadline_and_rejects_scope_leakage() {
    use crate::invocation::{Request, parse_request};
    let parse = |args: &[&str]| parse_request(args.iter().map(std::ffi::OsString::from));
    let Request::Events { command, format } = parse(&[
        "events",
        "subscribe",
        "--event=tab.renamed",
        "--event",
        "panel.exited",
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
            "panel.exited"
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
            "panel.output_matched",
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

/// Every event shape in the conformance corpus renders as one text line led
/// by the receipt time and the event name; the help names the closed set.
#[test]
fn subscribe_text_renders_each_event_as_one_line_and_help_lists_every_name() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../schemas/workbench/v1/workbench-events-conformance-v1.json"
    ))
    .unwrap();
    let mut rendered = Vec::new();
    for case in corpus["valid"].as_array().unwrap() {
        let mut value = case["document"].clone();
        value.as_object_mut().unwrap().remove("kind");
        let Ok(reply) = serde_json::from_value::<SemanticEventsResult>(value) else {
            continue;
        };
        if let Outcome::Matched { event, .. } = &reply.outcome {
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let line = text::record("event", &reply.outcome, "23:14:02Z", locale);
                assert!(
                    line.starts_with(&format!(
                        "23:14:02Z  {}  ",
                        crate::invocation::events::public_name(event.event)
                    )),
                    "{line}"
                );
                assert_eq!(line.lines().count(), 1, "{line}");
            }
            rendered.push(text::record(
                "event",
                &reply.outcome,
                "23:14:02Z",
                Locale::EnUs,
            ));
        }
    }
    assert!(
        rendered
            .iter()
            .any(|line| line.contains("panel.agent_status_changed")),
        "{rendered:?}"
    );
    assert_eq!(
        text::clock(std::time::UNIX_EPOCH + Duration::from_secs(86_400 + 3_661)),
        "01:01:01Z"
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let help = locale
            .events_help(crate::invocation::CommandPath::EventsSubscribe)
            .notes;
        for kind in Kind::ALL {
            assert!(
                help.contains(&crate::invocation::events::public_name(*kind)),
                "{}",
                kind.as_str()
            );
        }
        assert_ne!(locale.events_subscribe_status(7), locale.events_status(7));
    }
}
