use orchbee_workbench_protocol::*;
use serde_json::{Value, json};
fn report() -> Value {
    json!({"protocolVersion":1,"requestId":"00000000000000000000000000000001","sessionId":"00000000000000000000000000000002","action":{"kind":"report","panelId":"00000000000000000000000000000003","expectedBinding":{"workerId":"00000000000000000000000000000004","terminalId":"00000000000000000000000000000005"},"source":"hook:example","patch":{"title":"标题 😀","displayAgent":"辅助","stateLabels":{"working":"忙碌"},"tokens":{"summary":"value"}},"sequence":"18446744073709551615","ttlMilliseconds":86400000}})
}
fn admitted(value: Value) -> bool {
    serde_json::from_value::<PanelMetadataRequest>(value).is_ok_and(|v| v.validate().is_ok())
}
#[test]
fn metadata_contract_checks_whole_report_and_rejects_duplicate_map_members() {
    let raw = report();
    assert!(admitted(raw.clone()));
    let value: PanelMetadataRequest = serde_json::from_value(raw).unwrap();
    assert_eq!(
        serde_json::from_slice::<PanelMetadataRequest>(&serde_json::to_vec(&value).unwrap())
            .unwrap(),
        value
    );
    for (path, value) in [
        ("/action/patch/title", json!(" a ")),
        ("/action/patch/displayAgent", json!("a\nb")),
        ("/action/patch/stateLabels", json!({"success":"x"})),
        ("/action/patch/clearTitle", json!(true)),
        ("/action/patch/clearDisplayAgent", json!(true)),
        ("/action/patch/clearStateLabels", json!(true)),
        ("/action/patch", json!({})),
        ("/action/patch", json!({"agent":"codex"})),
        ("/action/patch/tokens", json!({})),
        ("/action/source", json!("bad source")),
        ("/action/ttlMilliseconds", json!(0)),
        ("/action/ttlMilliseconds", json!(86400001)),
        ("/action/sequence", json!("18446744073709551616")),
        ("/action/expectedBinding/terminalId", json!("0".repeat(32))),
    ] {
        let mut bad = report();
        if path.ends_with("clearTitle")
            || path.ends_with("clearDisplayAgent")
            || path.ends_with("clearStateLabels")
        {
            bad["action"]["patch"][path.split('/').next_back().unwrap()] = value;
        } else {
            *bad.pointer_mut(path).unwrap() = value;
        }
        assert!(!admitted(bad), "{path}");
    }
    for (patch, key, value) in [
        ("stateLabels", "working", "忙碌"),
        ("tokens", "summary", "value"),
    ] {
        let raw = serde_json::to_string(&report()).unwrap().replace(
            &format!("\"{key}\":\"{value}\""),
            &format!("\"{key}\":\"old\",\"{key}\":\"{value}\""),
        );
        assert!(
            serde_json::from_str::<PanelMetadataRequest>(&raw).is_err(),
            "{patch}"
        );
    }
    for clear in ["clearTitle", "clearDisplayAgent", "clearStateLabels"] {
        let mut raw = report();
        raw["action"]["patch"] = json!({clear:true});
        assert!(admitted(raw));
    }
}
fn result() -> Value {
    json!({"protocolVersion":1,"requestId":"00000000000000000000000000000001","sessionId":"00000000000000000000000000000002","outcome":{"kind":"read","workbenchRevision":"1","metadataRevision":"2","panels":[{"panelId":"00000000000000000000000000000003","binding":{"workerId":"00000000000000000000000000000004","terminalId":"00000000000000000000000000000005"},"tokens":{},"presentations":[{"source":"source","agent":null,"appliesToSource":null,"title":{"value":"x","order":"2"},"displayAgent":null,"stateLabels":{}}]}]}})
}
#[test]
fn metadata_read_contract_is_closed_bounded_correlated_and_preserves_nulls() {
    let doc = result();
    let parsed: PanelMetadataResult = serde_json::from_value(doc).unwrap();
    parsed.validate().unwrap();
    let mut request = report();
    request["action"] = json!({"kind":"read","panelIds":["00000000000000000000000000000003"]});
    let request: PanelMetadataRequest = serde_json::from_value(request).unwrap();
    request.validate().unwrap();
    assert!(parsed.answers(&request));
    for path in ["agent", "appliesToSource", "title", "displayAgent"] {
        let mut bad = result();
        bad["outcome"]["panels"][0]["presentations"][0]
            .as_object_mut()
            .unwrap()
            .remove(path);
        assert!(
            serde_json::from_value::<PanelMetadataResult>(bad).is_err(),
            "{path}"
        );
    }
    for (pointer, v) in [
        ("/outcome/panels/0/binding", Value::Null),
        ("/outcome/panels/0/presentations/0/title/order", json!("0")),
        ("/outcome/panels/0/presentations/0/title/order", json!("3")),
    ] {
        let mut bad = result();
        *bad.pointer_mut(pointer).unwrap() = v;
        let parsed: PanelMetadataResult = serde_json::from_value(bad).unwrap();
        assert!(parsed.validate().is_err());
    }
    for count in [2, 5] {
        let mut bad = result();
        let row = bad["outcome"]["panels"][0].clone();
        bad["outcome"]["panels"] = json!(vec![row; count]);
        assert!(
            serde_json::from_value::<PanelMetadataResult>(bad)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    let mut bad = result();
    let source = bad["outcome"]["panels"][0]["presentations"][0].clone();
    bad["outcome"]["panels"][0]["presentations"]
        .as_array_mut()
        .unwrap()
        .push(source);
    assert!(
        serde_json::from_value::<PanelMetadataResult>(bad.clone())
            .unwrap()
            .validate()
            .is_err()
    );
    bad["outcome"]["panels"][0]["presentations"][1]["source"] = json!("other");
    assert!(
        serde_json::from_value::<PanelMetadataResult>(bad)
            .unwrap()
            .validate()
            .is_err(),
        "different sources cannot own one write order"
    );
    let mut bad = result();
    bad["requestId"] = json!("00000000000000000000000000000009");
    assert!(
        !serde_json::from_value::<PanelMetadataResult>(bad)
            .unwrap()
            .answers(&request)
    );
    let mut empty = serde_json::to_value(request).unwrap();
    empty["action"]["panelIds"] = json!([]);
    assert!(admitted(empty.clone()));
    empty["action"]["panelIds"] = json!(vec!["00000000000000000000000000000003"; 2]);
    assert!(!admitted(empty));
}

#[test]
fn panel_metadata_shared_corpus_and_carrier_bounds() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-panel-metadata-conformance-v1.json"
    ))
    .unwrap();
    for (group, expected) in [("valid", true), ("invalid", false)] {
        for case in corpus[group].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let mut frame = vec![];
            if case["direction"] == "request" {
                let parsed = parse_workbench_request(&bytes);
                assert_eq!(parsed.is_ok(), expected, "{}", case["name"]);
                if let Ok(request) = parsed {
                    write_workbench_request(&mut frame, &request).unwrap();
                    assert_eq!(&frame[6..8], &33_u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_request(&mut frame.as_slice()).unwrap(),
                        request
                    );
                    assert!(read_workbench_response(&mut frame.as_slice()).is_err());
                }
            } else {
                let parsed = parse_workbench_response(&bytes);
                assert_eq!(parsed.is_ok(), expected, "{}", case["name"]);
                if let Ok(response) = parsed {
                    write_workbench_response(&mut frame, &response).unwrap();
                    assert_eq!(&frame[6..8], &34_u16.to_be_bytes());
                    assert_eq!(
                        read_workbench_response(&mut frame.as_slice()).unwrap(),
                        response
                    );
                    assert!(read_workbench_request(&mut frame.as_slice()).is_err());
                }
            }
        }
    }
    for (kind, maximum) in [
        (33_u16, MAXIMUM_ENCODED_INTENT_BYTES),
        (34, MAXIMUM_ENCODED_PANEL_METADATA_RESULT_BYTES),
    ] {
        let mut header = b"OBWP\0\x01".to_vec();
        header.extend(kind.to_be_bytes());
        header.extend(u32::try_from(maximum + 1).unwrap().to_be_bytes());
        let mut cursor = std::io::Cursor::new(header);
        let result = if kind == 33 {
            read_workbench_request(&mut cursor).map(|_| ())
        } else {
            read_workbench_response(&mut cursor).map(|_| ())
        };
        assert!(matches!(result, Err(WorkbenchWireError::Contract(_))));
        assert_eq!(cursor.position(), 12);
    }
}

#[test]
fn maximum_unicode_sources_and_rows_fit_the_carrier_budget() {
    let mut raw = result();
    let field = json!({"value":"😀".repeat(80),"order":"32"});
    raw["outcome"]["metadataRevision"] = json!("32");
    let mut record = raw["outcome"]["panels"][0]["presentations"][0].clone();
    record["agent"] = json!("😀".repeat(80));
    record["appliesToSource"] = json!("s".repeat(80));
    record["title"] = field.clone();
    record["displayAgent"] = field.clone();
    record["stateLabels"] =
        json!({"idle":field,"working":field,"blocked":field,"done":field,"unknown":field});
    let mut panel = raw["outcome"]["panels"][0].clone();
    panel["tokens"] = json!(
        (0..32)
            .map(|n| (format!("k{n:031}"), "😀".repeat(80)))
            .collect::<std::collections::BTreeMap<_, _>>()
    );
    panel["presentations"] = json!(
        (0..32)
            .map(|n| {
                let mut r = record.clone();
                r["source"] = json!(format!("s{n:079}"));
                for path in [
                    "/title/order",
                    "/displayAgent/order",
                    "/stateLabels/idle/order",
                    "/stateLabels/working/order",
                    "/stateLabels/blocked/order",
                    "/stateLabels/done/order",
                    "/stateLabels/unknown/order",
                ] {
                    *r.pointer_mut(path).unwrap() = json!((n + 1).to_string());
                }
                r
            })
            .collect::<Vec<_>>()
    );
    raw["outcome"]["panels"] = json!(
        (0..4)
            .map(|n| {
                let mut p = panel.clone();
                p["panelId"] = json!(format!("{:032x}", n + 10));
                p
            })
            .collect::<Vec<_>>()
    );
    let result: PanelMetadataResult = serde_json::from_value(raw).unwrap();
    let response = WorkbenchResponse::PanelMetadata(result);
    let bytes = serialize_workbench_response(&response).unwrap();
    assert!(bytes.len() > 128 * 1024 && bytes.len() <= MAXIMUM_ENCODED_PANEL_METADATA_RESULT_BYTES);
    assert_eq!(parse_workbench_response(&bytes).unwrap(), response);
}

#[test]
fn presentation_reads_answer_only_exact_order_and_request_session_identity() {
    let ids: Vec<_> = (1..=2)
        .map(|n| TerminalPresentationId::parse(&format!("{n:032x}")).unwrap())
        .collect();
    let panel = WorkbenchPanelId::parse(&"3".repeat(32)).unwrap();
    let request = PanelMetadataRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&"4".repeat(32)).unwrap(),
        session_id: SessionId::parse(&"5".repeat(32)).unwrap(),
        action: PanelMetadataAction::ReadPresentations {
            presentation_ids: ids.clone(),
        },
    };
    let mut result = PanelMetadataResult {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        session_id: request.session_id.clone(),
        outcome: PanelMetadataOutcome::PresentationsRead {
            workbench_revision: WorkbenchRevision::new(9),
            metadata_revision: WorkbenchRevision::new(0),
            presentations: ids
                .iter()
                .map(|id| PresentationMetadataRow {
                    presentation_id: id.clone(),
                    panel: PanelMetadataRow {
                        panel_id: panel.clone(),
                        binding: None,
                        tokens: MetadataTokens::default(),
                        presentations: vec![],
                    },
                })
                .collect(),
        },
    };
    result.validate().unwrap();
    assert!(result.answers(&request));
    if let PanelMetadataOutcome::PresentationsRead { presentations, .. } = &mut result.outcome {
        presentations.reverse();
    }
    assert!(!result.answers(&request));
    if let PanelMetadataOutcome::PresentationsRead { presentations, .. } = &mut result.outcome {
        presentations.reverse();
    }
    result.session_id = SessionId::parse(&"6".repeat(32)).unwrap();
    assert!(!result.answers(&request));
    result.session_id = request.session_id.clone();
    result.request_id = WorkbenchRequestId::parse(&"7".repeat(32)).unwrap();
    assert!(!result.answers(&request));
}
