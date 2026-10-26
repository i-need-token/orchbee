use super::*;
use orchbee_terminal_protocol::{
    AttachmentId, HostEpoch, TerminalRowPromptClass, TerminalTextCellMapping, TerminalTextWindowRow,
};
use std::num::NonZeroU64;

fn p(row: u32, col: u16) -> TerminalTextPoint {
    TerminalTextPoint { row, col }
}
fn ascii(index: u64, text: &str) -> TerminalTextWindowRow {
    TerminalTextWindowRow {
        index,
        prompt_class: TerminalRowPromptClass::None,
        wrap: false,
        wrap_continuation: false,
        identity_ascii: true,
        text: text.into(),
        mappings: Vec::new(),
    }
}
fn window(rows: Vec<TerminalTextWindowRow>) -> TerminalTextRowWindow {
    TerminalTextRowWindow {
        attachment_id: AttachmentId::parse("11111111111111111111111111111111").unwrap(),
        host_epoch: HostEpoch::parse("22222222222222222222222222222222").unwrap(),
        sequence: NonZeroU64::MIN,
        revision: 14,
        generation: NonZeroU64::MIN,
        total_rows: rows.len() as u64,
        start_row: 0,
        columns: 8,
        ignored_optional_flags: 0,
        rows,
    }
}
#[test]
fn selection_read_orders_inclusive_endpoints_and_preserves_blank_hard_lines() {
    let rows = window(vec![
        ascii(0, "alpha   "),
        ascii(1, "        "),
        ascii(2, "beta "),
    ]);
    assert_eq!(extract(&rows, p(2, 2), p(0, 1)).unwrap(), "lpha\n\nbet");
    let row = window(vec![ascii(0, "alpha")]);
    assert_eq!(extract(&row, p(0, 2), p(0, 2)).unwrap(), "p");
    assert_eq!(extract(&row, p(0, 7), p(0, 7)).unwrap(), "");
    assert_eq!(extract(&row, p(0, 4), p(0, 1)).unwrap(), "lpha");
}
#[test]
fn selection_read_joins_only_correlated_soft_wraps_without_trimming_inner_spaces() {
    let mut rows = window(vec![
        ascii(0, "alpha  "),
        ascii(1, "beta   "),
        ascii(2, "end"),
    ]);
    rows.rows[0].wrap = true;
    rows.rows[1].wrap_continuation = true;
    assert_eq!(
        extract(&rows, p(0, 0), p(2, 2)).unwrap(),
        "alpha  beta\nend"
    );
    rows.rows[1].wrap_continuation = false;
    assert_eq!(
        extract(&rows, p(0, 0), p(2, 2)).unwrap(),
        "alpha\nbeta\nend"
    );
}
#[test]
fn selection_read_uses_cell_mapping_for_wide_tails_and_combining_clusters() {
    let mut row = ascii(0, "中e\u{301}文x");
    row.identity_ascii = false;
    row.mappings = vec![
        TerminalTextCellMapping {
            byte_offset: 0,
            utf16_offset: 0,
            column: 0,
            width: 2,
        },
        TerminalTextCellMapping {
            byte_offset: 3,
            utf16_offset: 1,
            column: 2,
            width: 1,
        },
        TerminalTextCellMapping {
            byte_offset: 6,
            utf16_offset: 3,
            column: 3,
            width: 2,
        },
        TerminalTextCellMapping {
            byte_offset: 9,
            utf16_offset: 4,
            column: 5,
            width: 1,
        },
    ];
    let rows = window(vec![row]);
    for (a, b, expected) in [
        (1, 1, "中"),
        (2, 2, "e\u{301}"),
        (4, 4, "文"),
        (2, 4, "e\u{301}文"),
        (5, 7, "x"),
    ] {
        assert_eq!(extract(&rows, p(0, a), p(0, b)).unwrap(), expected);
    }
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text =
            locale.cli_terminal_selection("11111111111111111111111111111111", 14, "中\n\u{1b}[31m");
        assert!(!text.contains('\u{1b}'));
        assert!(text.contains("\\n\\u{001b}[31m"));
    }
}
#[test]
fn selection_read_refuses_incomplete_windows_outside_points_and_broken_mappings() {
    let mut rows = window(vec![ascii(0, "alpha"), ascii(1, "beta")]);
    assert!(extract(&rows, p(0, 8), p(1, 0)).is_err());
    assert!(extract(&rows, p(0, 0), p(2, 0)).is_err());
    rows.rows[1].index = 2;
    assert!(extract(&rows, p(0, 0), p(1, 0)).is_err());
    let mut row = ascii(0, "中");
    row.identity_ascii = false;
    row.mappings = vec![TerminalTextCellMapping {
        byte_offset: 1,
        utf16_offset: 0,
        column: 0,
        width: 2,
    }];
    assert!(extract(&window(vec![row]), p(0, 0), p(0, 1)).is_err());
}
#[test]
fn selection_read_schema_is_closed_and_requires_physical_content_identity() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../schemas/orchbee.terminal.selection.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let doc = json!({"schema":"orchbee.terminal.selection","version":1,
        "source":{"kind":"workbench-snapshot","sessionId":"11111111111111111111111111111111","revision":"1"},
        "result":{"terminal":{"terminalId":"11111111111111111111111111111111","workerId":"11111111111111111111111111111111","boundTo":{"presentationId":"11111111111111111111111111111111","panelId":"11111111111111111111111111111111","tabId":"11111111111111111111111111111111","terminalContextId":"11111111111111111111111111111111"}},
            "anchor":{"row":0,"col":0},"cursor":{"row":0,"col":1},"contentRevision":"18446744073709551615",
            "contentGeneration":"1","hostEpoch":"22222222222222222222222222222222","text":"中文"}});
    validator.validate(&doc).unwrap();
    for field in [
        "contentRevision",
        "contentGeneration",
        "hostEpoch",
        "anchor",
        "cursor",
    ] {
        let mut bad = doc.clone();
        bad["result"].as_object_mut().unwrap().remove(field);
        assert!(!validator.is_valid(&bad), "{field}");
    }
    for (field, value) in [
        ("contentRevision", json!(1)),
        ("contentRevision", json!("01")),
        ("contentGeneration", json!("0")),
        ("extra", json!(true)),
    ] {
        let mut bad = doc.clone();
        bad["result"][field] = value;
        assert!(!validator.is_valid(&bad), "{field}");
    }
}
