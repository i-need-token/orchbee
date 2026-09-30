//! Closed copy receipts preserve integer precision and localize safe text.
use super::*;
use crate::terminal_copy::CopySearchResult;

fn common(schema: &str, result: Value) -> Value {
    let id = "11111111111111111111111111111111";
    let mut result = result;
    result["terminal"] = json!({"terminalId":id,"workerId":id,
        "boundTo":{"presentationId":id,"panelId":id,"tabId":id,"terminalContextId":id}});
    result["contentRevision"] = json!(u64::MAX.to_string());
    result["contentGeneration"] = json!("1");
    result["hostEpoch"] = json!("22222222222222222222222222222222");
    json!({"schema":schema,"version":1,"source":{"kind":"workbench-snapshot","siteId":id,"revision":"1"},"result":result})
}
#[test]
fn copy_receipts_are_closed_precise_and_keep_empty_search_indexes_consistent() {
    let motion: Value = serde_json::from_str(include_str!(
        "../../schemas/orchbee.terminal.copy-motion.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&motion).unwrap();
    let doc = common(
        "orchbee.terminal.copy-motion",
        json!({"origin":{"row":0,"col":0},"motion":"line-end","cursor":{"row":0,"col":1}}),
    );
    validator.validate(&doc).unwrap();
    assert_eq!(
        motion["properties"]["result"]["properties"]["motion"]["enum"],
        json!(
            CopyMotion::ALL
                .into_iter()
                .map(CopyMotion::token)
                .collect::<Vec<_>>()
        )
    );
    for (field, value) in [
        ("contentRevision", json!(u64::MAX)),
        ("motion", json!("up")),
        ("contentGeneration", json!("0")),
        ("extra", json!(true)),
    ] {
        let mut bad = doc.clone();
        bad["result"][field] = value;
        assert!(!validator.is_valid(&bad), "{field}");
    }
    let search: Value = serde_json::from_str(include_str!(
        "../../schemas/orchbee.terminal.copy-search.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&search).unwrap();
    let doc = common(
        "orchbee.terminal.copy-search",
        json!({"origin":{"row":0,"col":0},"query":"x","direction":"forward",
        "previous":null,"matches":[],"total":"0","current":null,"currentGlobal":null}),
    );
    validator.validate(&doc).unwrap();
    for (field, value) in [
        ("total", json!("1")),
        ("current", json!(0)),
        ("currentGlobal", json!("0")),
        ("extra", json!(0)),
    ] {
        let mut bad = doc.clone();
        bad["result"][field] = value;
        assert!(!validator.is_valid(&bad), "{field}");
    }
    let mut nonempty = doc.clone();
    nonempty["result"]["matches"] = json!([{"start":{"row":0,"col":0},"end":{"row":0,"col":1}}]);
    nonempty["result"]["total"] = json!("18446744073709551615");
    nonempty["result"]["current"] = json!(0);
    nonempty["result"]["currentGlobal"] = json!("18446744073709551614");
    validator.validate(&nonempty).unwrap();
    nonempty["result"]["current"] = json!(1024);
    assert!(!validator.is_valid(&nonempty));
}
#[test]
fn copy_text_is_localized_and_escapes_queries_before_adding_match_markers() {
    let origin = TerminalTextPoint { row: 2, col: 1 };
    let result = CopySearchResult {
        matches: vec![TerminalTextRange {
            start: origin,
            end: origin,
        }],
        total: 1,
        current: Some(0),
        current_global: Some(0),
    };
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = locale.cli_terminal_copy_search(
            "11111111111111111111111111111111",
            7,
            "x\n\u{1b}[31m",
            &result,
        );
        assert!(!text.contains('\u{1b}'));
        assert!(text.contains("\\n\\u{001b}[31m"));
        assert!(text.contains("* 2:1–2:1"));
        let motion = locale.cli_terminal_copy_motion(
            "11111111111111111111111111111111",
            7,
            "line-end",
            origin,
            origin,
        );
        assert!(motion.contains("2:1 → 2:1"));
    }
}
