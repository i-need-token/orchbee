use super::*;
use crate::terminal_links::LinkRegion;
#[test]
fn link_resolve_receipt_is_closed_precise_and_distinguishes_explicit_plain_and_none() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../schemas/orchbee.terminal.link-resolve.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let id = "11111111111111111111111111111111";
    let base = json!({"schema":"orchbee.terminal.link-resolve","version":1,"source":{"kind":"workbench-snapshot","siteId":id,"revision":"1"},"result":{"terminal":{"terminalId":id,"workerId":id,"boundTo":{"presentationId":id,"panelId":id,"tabId":id,"terminalContextId":id}},"contentRevision":u64::MAX.to_string(),"contentGeneration":"1","hostEpoch":id,"origin":{"row":0,"col":0},"offsetFromBottom":u64::MAX.to_string(),"viewportStartRow":"0","kind":"none","url":null,"regions":[]}});
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
        let text = locale.cli_terminal_link("id", u64::MAX, cursor, 0, "plain-text", &found);
        assert!(!text.contains('\x1b'));
        assert!(text.contains("1:0-3"));
        let empty = locale.cli_terminal_link("id", 0, cursor, 0, "none", &ResolvedLink::none());
        assert!(empty.contains(if locale == Locale::EnUs {
            "none"
        } else {
            "无"
        }));
    }
}
