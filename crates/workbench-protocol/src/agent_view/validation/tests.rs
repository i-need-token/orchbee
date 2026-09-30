use super::*;
use serde_json::{Value, json};
fn valid(value: Value) -> bool {
    parse_agent_view_spec(&serde_json::to_vec(&value).unwrap()).is_ok()
}
#[test]
fn normalizes_source_label_and_default_sort() {
    let spec = parse_agent_view_spec(
        br#"{"source":"  cli.views ","label":"  a\nb  ","sort":[{"field":"attention"}]}"#,
    )
    .unwrap();
    assert_eq!(spec.source, "cli.views");
    assert_eq!(spec.label.as_deref(), Some("ab"));
    assert_eq!(spec.sort[0].order, AgentViewSortOrder::Asc);
}
#[test]
fn rejects_source_label_token_and_closed_shape() {
    for value in [
        json!({"source":""}),
        json!({"source":"bad/source"}),
        json!({"source":"中"}),
        json!({"source":"x".repeat(121)}),
        json!({"source":"x","label":"\n"}),
        json!({"source":"x","label":"中".repeat(33)}),
        json!({"source":"x","unknown":1}),
        json!({"source":"x","sort":[{"field":{"token":"bad.name"}}]}),
        json!({"source":"x","sort":[{"field":{"token":"x","extra":0}}]}),
    ] {
        assert!(!valid(value.clone()), "{value}");
    }
    assert!(valid(
        json!({"source":"x".repeat(120),"label":"中".repeat(32),"sort":[{"field":{"token":"x".repeat(32)}}]})
    ));
}
#[test]
fn field_types_and_context_pairs_are_closed() {
    for (field, value, expected) in [
        (json!("seen"), json!(true), true),
        (json!("seen"), json!("true"), false),
        (
            json!("state_change_seq"),
            json!(9_007_199_254_740_991u64),
            true,
        ),
        (
            json!("state_change_seq"),
            json!(9_007_199_254_740_992u64),
            true,
        ),
        (json!("state_change_seq"), json!(u64::MAX), true),
        (json!("state_change_seq"), json!(-1), false),
        (json!("state_change_seq"), json!(1.1), false),
        (json!("status"), json!("done"), true),
        (json!("status"), json!("succeeded"), false),
        (
            json!("workspace_id"),
            json!({"context":"current_workspace_id"}),
            true,
        ),
        (json!("tab_id"), json!({"context":"current_tab_id"}), true),
        (json!("pane_id"), json!({"context":"current_tab_id"}), false),
        (
            json!("tab_id"),
            json!({"context":"current_workspace_id"}),
            false,
        ),
        (json!({"token":"phase"}), json!(""), true),
        (json!({"token":"phase"}), json!(true), false),
    ] {
        assert_eq!(
            valid(json!({"source":"x","filter":{"op":"eq","field":field,"value":value}})),
            expected
        );
    }
}
#[test]
fn bounds_all_filter_dimensions_and_sort_count() {
    let leaf = json!({"op":"exists","field":"agent"});
    let mut tree = leaf.clone();
    for _ in 1..8 {
        tree = json!({"op":"not","filter":tree});
    }
    assert!(valid(json!({"source":"x","filter":tree})));
    assert!(!valid(
        json!({"source":"x","filter":{"op":"not","filter":tree}})
    ));
    for count in [0, 63, 64] {
        assert_eq!(
            valid(json!({"source":"x","filter":{"op":"all","filters":vec![leaf.clone();count]}})),
            count == 63
        );
    }
    for count in [0, 32, 33] {
        assert_eq!(
            valid(
                json!({"source":"x","filter":{"op":"in","field":"agent","values":vec!["a";count]}})
            ),
            count == 32
        );
    }
    for count in [8, 9] {
        assert_eq!(
            valid(json!({"source":"x","sort":vec![json!({"field":"seen"});count]})),
            count == 8
        );
    }
}
#[test]
fn spec_and_wire_have_separate_encoded_bounds() {
    let spec = parse_agent_view_spec(
        br#"{"source":"x","filter":{"op":"eq","field":"agent","value":"a"}}"#,
    )
    .unwrap();
    let mut value = serde_json::to_value(spec).unwrap();
    value["filter"]["value"] = json!("a".repeat(MAXIMUM_AGENT_VIEW_SPEC_BYTES));
    assert!(!valid(value));
    let request = AgentViewRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&"1".repeat(32)).unwrap(),
        site_id: SiteId::parse(&"2".repeat(32)).unwrap(),
        action: AgentViewAction::Clear {
            source: Some("x".into()),
        },
    };
    let mut bytes = Vec::new();
    crate::write_workbench_request(
        &mut bytes,
        &crate::WorkbenchRequest::AgentView(request.clone()),
    )
    .unwrap();
    assert_eq!(
        crate::read_workbench_request(&mut &bytes[..]).unwrap(),
        crate::WorkbenchRequest::AgentView(request.clone())
    );
    let result = AgentViewResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        outcome: AgentViewOutcome::Available {
            snapshot: AgentViewSnapshot {
                revision: WorkbenchRevision::new(0),
                spec: None,
            },
        },
    };
    assert!(result.answers(&request));
    bytes.clear();
    crate::write_workbench_response(
        &mut bytes,
        &crate::WorkbenchResponse::AgentView(result.clone()),
    )
    .unwrap();
    assert_eq!(
        crate::read_workbench_response(&mut &bytes[..]).unwrap(),
        crate::WorkbenchResponse::AgentView(result)
    );
}

#[test]
fn mutation_replies_must_describe_the_requested_effect() {
    let spec = parse_agent_view_spec(br#"{"source":"owner","label":"View"}"#).unwrap();
    let mut request = AgentViewRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&"1".repeat(32)).unwrap(),
        site_id: SiteId::parse(&"2".repeat(32)).unwrap(),
        action: AgentViewAction::Set {
            spec: Box::new(spec.clone()),
        },
    };
    let reply = |spec| AgentViewResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        site_id: request.site_id.clone(),
        outcome: AgentViewOutcome::Available {
            snapshot: AgentViewSnapshot {
                revision: WorkbenchRevision::new(1),
                spec,
            },
        },
    };
    let installed = reply(Some(spec.clone()));
    let cleared = reply(None);
    let mut different = spec.clone();
    different.source = "other".into();
    let other = reply(Some(different));
    assert!(installed.answers(&request));
    assert!(!cleared.answers(&request));
    assert!(!other.answers(&request));
    let mut invalid = spec;
    invalid.label = Some(" View ".into());
    assert!(!reply(Some(invalid)).answers(&request));
    request.action = AgentViewAction::Clear {
        source: Some(" owner ".into()),
    };
    assert!(cleared.answers(&request));
    assert!(other.answers(&request));
    assert!(!installed.answers(&request));
    request.action = AgentViewAction::Clear { source: None };
    assert!(cleared.answers(&request));
    assert!(!other.answers(&request));
    request.action = AgentViewAction::Read {};
    assert!(other.answers(&request));
    let mut unrelated = installed;
    unrelated.request_id = WorkbenchRequestId::parse(&"3".repeat(32)).unwrap();
    assert!(!unrelated.answers(&request));
}
