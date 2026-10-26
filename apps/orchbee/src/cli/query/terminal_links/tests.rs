use super::*;
use crate::cli_text::CliText;
use crate::terminal_links::LinkRegion;
#[test]
fn link_activation_receipt_requires_truthful_handled_and_closed_known_or_unknown_identity() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../schemas/orchbee.terminal.link-activate.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let id = "11111111111111111111111111111111";
    let base = json!({"schema":"orchbee.terminal.link-activate","version":1,"source":{"kind":"workbench-snapshot","sessionId":id,"revision":"1"},"result":{"terminal":{"terminalId":id,"workerId":id,"boundTo":{"presentationId":id,"panelId":id,"tabId":id,"terminalContextId":id}},"contentRevision":"0","contentGeneration":"1","hostEpoch":id,"origin":{"row":0,"col":0},"offsetFromBottom":"0","viewportStartRow":"0","kind":"none","url":null,"regions":[],"handled":false,"activation":{"state":"no-link"}}});
    validator.validate(&base).unwrap();
    for state in ["no-handler", "started", "failed", "unknown"] {
        let mut doc = base.clone();
        doc["result"]["kind"] = json!("explicit");
        doc["result"]["url"] = json!("custom://example");
        let mut activation = json!({"state":state});
        if state != "no-handler" {
            activation["operationId"] = json!(id);
            activation["pluginId"] = json!("tools");
            activation["handlerId"] = json!("handler");
            activation["actionId"] = json!("open");
        }
        match state {
            "started" => {
                activation["worker"] = json!({"workerId":id,"generation":"1"});
                activation["jobId"] = json!(id);
                activation["existing"] = json!(false);
                doc["result"]["handled"] = json!(true);
            }
            "failed" => activation["reason"] = json!("disabled"),
            "unknown" => activation["worker"] = serde_json::Value::Null,
            _ => {}
        }
        doc["result"]["activation"] = activation;
        validator.validate(&doc).unwrap();
        let mut wrong = doc.clone();
        wrong["result"]["handled"] = json!(state != "started");
        assert!(!validator.is_valid(&wrong));
        wrong = doc.clone();
        wrong["result"]["activation"]["extra"] = json!(true);
        assert!(!validator.is_valid(&wrong));
        if state != "no-handler" {
            doc["result"]["activation"]
                .as_object_mut()
                .unwrap()
                .remove("operationId");
            assert!(!validator.is_valid(&doc));
        }
    }
    let mut read = base;
    read["schema"] = json!("orchbee.terminal.link-resolve");
    let old: serde_json::Value = serde_json::from_str(include_str!(
        "../../schemas/orchbee.terminal.link-resolve.json"
    ))
    .unwrap();
    assert!(!jsonschema::validator_for(&old).unwrap().is_valid(&read));
}
#[test]
fn link_activation_text_distinguishes_admission_refusal_and_unknown_in_both_locales() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for state in ["no-link", "no-handler", "started", "failed", "unknown"] {
            let text = CliText::new(locale)
                .cli_terminal_link_activation(state, Some("operation\u{1b}[31m"));
            assert!(!text.contains('\u{1b}'));
            assert!(text.contains("operation"));
            if state == "unknown" {
                assert!(text.contains(if locale == Locale::EnUs {
                    "do not retry"
                } else {
                    "不要重试"
                }));
            }
        }
    }
}
#[test]
fn link_resolve_receipt_is_closed_precise_and_distinguishes_explicit_plain_and_none() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../schemas/orchbee.terminal.link-resolve.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let id = "11111111111111111111111111111111";
    let base = json!({"schema":"orchbee.terminal.link-resolve","version":1,"source":{"kind":"workbench-snapshot","sessionId":id,"revision":"1"},"result":{"terminal":{"terminalId":id,"workerId":id,"boundTo":{"presentationId":id,"panelId":id,"tabId":id,"terminalContextId":id}},"contentRevision":u64::MAX.to_string(),"contentGeneration":"1","hostEpoch":id,"origin":{"row":0,"col":0},"offsetFromBottom":u64::MAX.to_string(),"viewportStartRow":"0","kind":"none","url":null,"regions":[]}});
    validator.validate(&base).unwrap();
    for kind in ["explicit", "plain-text"] {
        let mut doc = base.clone();
        doc["result"]["kind"] = json!(kind);
        doc["result"]["url"] = json!("https://example.com");
        if kind == "plain-text" {
            doc["result"]["regions"] = json!([{"row":0,"startCol":0,"endCol":5}]);
        }
        validator.validate(&doc).unwrap();
        doc["result"]["url"] = serde_json::Value::Null;
        assert!(!validator.is_valid(&doc));
    }
    for (field, value) in [
        ("contentRevision", json!(u64::MAX)),
        ("extra", json!(true)),
        ("origin", json!({"row":65536,"col":0})),
        ("url", json!("http://a")),
        ("regions", json!([{"row":0,"startCol":0,"endCol":1}])),
    ] {
        let mut doc = base.clone();
        doc["result"][field] = value;
        assert!(!validator.is_valid(&doc), "{field}");
    }
}
#[test]
fn link_resolve_text_localizes_none_and_escapes_link_controls() {
    let cursor = TerminalTextPoint { row: 1, col: 2 };
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let found = ResolvedLink {
            url: Some("https://a/\x1b[31m\n".into()),
            regions: vec![LinkRegion {
                row: 1,
                start_col: 0,
                end_col: 3,
            }],
        };
        let text =
            CliText::new(locale).cli_terminal_link("id", u64::MAX, cursor, 0, "plain-text", &found);
        assert!(!text.contains('\x1b'));
        assert!(text.contains("1:0-3"));
        let empty = CliText::new(locale).cli_terminal_link(
            "id",
            0,
            cursor,
            0,
            "none",
            &ResolvedLink::none(),
        );
        assert!(empty.contains(if locale == Locale::EnUs {
            "none"
        } else {
            "无"
        }));
    }
}
