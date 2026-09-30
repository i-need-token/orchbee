use super::*;
use orchbee_terminal_protocol::{TerminalRowPromptClass, TerminalTextCellMapping};

fn ascii(index: u64, text: &str, wrap: bool, continuation: bool) -> TerminalTextWindowRow {
    TerminalTextWindowRow {
        index,
        prompt_class: TerminalRowPromptClass::None,
        wrap,
        wrap_continuation: continuation,
        identity_ascii: true,
        text: text.into(),
        mappings: Vec::new(),
    }
}
fn mapped(
    index: u64,
    cells: &[(u16, u8, &str)],
    wrap: bool,
    continuation: bool,
) -> TerminalTextWindowRow {
    let mut row = ascii(index, "", wrap, continuation);
    row.identity_ascii = false;
    let mut text = String::new();
    for &(column, width, value) in cells {
        row.mappings.push(TerminalTextCellMapping {
            column,
            width,
            byte_offset: text.len() as u32,
            utf16_offset: text.encode_utf16().count() as u32,
        });
        text.push_str(value);
    }
    row.text = text.into_boxed_str();
    row
}
fn result(
    rows: &[TerminalTextWindowRow],
    columns: u16,
    row: u32,
    col: u16,
    viewport: Range<u64>,
) -> ResolvedLink {
    let LinkRead::Complete(result) = resolve(
        rows,
        columns,
        TerminalTextPoint { row, col },
        rows.last().unwrap().index + 1,
        viewport,
    ) else {
        panic!("complete token")
    };
    result
}

#[test]
fn plain_link_punctuation_and_click_regions_match_pinned_reference_examples() {
    for (text, url) in [
        (
            "see https://example.com/a(b)c.",
            "https://example.com/a(b)c",
        ),
        (
            "[docs](https://example.com/docs),",
            "https://example.com/docs",
        ),
        ("https://a/[b]{c}(d)!?", "https://a/[b]{c}(d)"),
        ("https://a/x)))]]}},", "https://a/x"),
    ] {
        let start = text.find("https").unwrap();
        let rows = [ascii(0, text, false, false)];
        for col in start..start + url.len() {
            let found = result(&rows, 80, 0, col as u16, 0..1);
            assert_eq!(found.url.as_deref(), Some(url), "{text}:{col}");
            assert_eq!(
                found.regions,
                vec![LinkRegion {
                    row: 0,
                    start_col: start as u16,
                    end_col: (start + url.len() - 1) as u16
                }]
            );
        }
        if start > 0 {
            assert_eq!(result(&rows, 80, 0, 0, 0..1), ResolvedLink::none());
        }
        if start + url.len() < text.len() {
            assert_eq!(
                result(&rows, 80, 0, (start + url.len()) as u16, 0..1),
                ResolvedLink::none()
            );
        }
    }
    for text in [
        "file:///tmp/a",
        "javascript:alert(1)",
        "HTTPS://example.com",
        "plain words",
    ] {
        assert_eq!(
            result(&[ascii(0, text, false, false)], 80, 0, 0, 0..1),
            ResolvedLink::none()
        );
    }
}
#[test]
fn plain_link_whole_unicode_glyphs_and_wide_spacers_clip_to_viewport() {
    let mut rows = vec![ascii(0, "https://example.com/", true, false)];
    // Column20 is the verified dropped spacer head before a wide glyph.
    rows.push(mapped(
        1,
        &[(0, 2, "路"), (2, 1, "e\u{301}"), (3, 1, ")"), (4, 1, ",")],
        false,
        true,
    ));
    for col in 0..=2 {
        let found = result(&rows, 21, 1, col, 1..2);
        assert_eq!(found.url.as_deref(), Some("https://example.com/路e\u{301}"));
        assert_eq!(
            found.regions,
            vec![LinkRegion {
                row: 0,
                start_col: 0,
                end_col: 2
            }]
        );
    }
    assert_eq!(result(&rows, 21, 0, 20, 0..2), ResolvedLink::none());
    assert_eq!(result(&rows, 21, 1, 3, 0..2), ResolvedLink::none());
    let prefixed = [mapped(
        0,
        &[
            (0, 1, "e\u{301}"),
            (1, 1, "("),
            (2, 1, "h"),
            (3, 1, "t"),
            (4, 1, "t"),
            (5, 1, "p"),
            (6, 1, ":"),
            (7, 1, "/"),
            (8, 1, "/"),
            (9, 2, "路"),
            (11, 1, ")"),
        ],
        false,
        false,
    )];
    assert_eq!(
        result(&prefixed, 20, 0, 10, 0..1).regions,
        vec![LinkRegion {
            row: 0,
            start_col: 2,
            end_col: 10
        }]
    );
}
#[test]
fn plain_link_requires_both_offscreen_token_ends_and_possible_spacer_context() {
    let row = ascii(5, "https://a/x", true, true);
    assert_eq!(
        resolve(&[row], 10, TerminalTextPoint { row: 5, col: 3 }, 7, 5..6),
        LinkRead::NeedContext {
            before: true,
            after: true
        }
    );
    let row = ascii(0, "https://a", true, false);
    assert_eq!(
        resolve(&[row], 10, TerminalTextPoint { row: 0, col: 3 }, 2, 0..1),
        LinkRead::NeedContext {
            before: false,
            after: true
        }
    );
    let rows = [
        ascii(0, "https://a", true, false),
        mapped(1, &[(0, 2, "路")], false, true),
    ];
    assert_eq!(
        result(&rows, 10, 0, 3, 0..1).url.as_deref(),
        Some("https://a路")
    );
    let rows = [
        ascii(0, "https://a", true, false),
        ascii(1, "x", false, true),
    ];
    assert_eq!(
        result(&rows, 10, 0, 3, 0..1).url.as_deref(),
        Some("https://a")
    );
}
#[test]
fn plain_link_hard_rows_and_unicode_whitespace_prove_a_short_token() {
    let rows = [
        ascii(0, "https://a", false, false),
        ascii(1, "tail", false, false),
    ];
    assert_eq!(
        result(&rows, 10, 0, 2, 0..2).url.as_deref(),
        Some("https://a")
    );
    let rows = [mapped(
        0,
        &[
            (0, 1, "h"),
            (1, 1, "t"),
            (2, 1, "t"),
            (3, 1, "p"),
            (4, 1, ":"),
            (5, 1, "/"),
            (6, 1, "/"),
            (7, 1, "a"),
            (8, 1, "\u{a0}"),
            (9, 1, "x"),
        ],
        false,
        false,
    )];
    assert_eq!(
        result(&rows, 10, 0, 2, 0..1).url.as_deref(),
        Some("http://a")
    );
    assert_eq!(result(&rows, 10, 0, 8, 0..1), ResolvedLink::none());
}
#[test]
fn plain_link_token_cell_bound_does_not_publish_truncated_urls() {
    for size in [8192, 8193] {
        let text = format!("https://a/{}", "x".repeat(size - 10));
        let rows: Vec<_> = text
            .as_bytes()
            .chunks(4096)
            .enumerate()
            .map(|(i, chunk)| {
                ascii(
                    i as u64,
                    std::str::from_utf8(chunk).unwrap(),
                    (i + 1) * 4096 < text.len(),
                    i > 0,
                )
            })
            .collect();
        let found = result(&rows, 4096, 0, 0, 0..rows.len() as u64);
        assert_eq!(found.url.is_some(), size == 8192);
        if size == 8192 {
            assert_eq!(found.url.as_deref(), Some(text.as_str()));
        } else {
            assert!(found.regions.is_empty());
        }
    }
    // The closing policy stays linear and refuses a click on a trimmed closer.
    let text = format!("https://a/{}", ")".repeat(8000));
    assert_eq!(url_range(&text, 0), Some(0..10));
    assert_eq!(url_range(&text, text.len() - 1), None);
}

#[test]
fn plain_link_budget_includes_boundary_probes_and_wide_spacer_cells() {
    let text = format!("https://a/{}", "x".repeat(8182));
    let rows = [
        ascii(1, &text[..4096], true, false),
        ascii(2, &text[4096..], false, true),
    ];
    // At a hard full-row end, the previous physical row's boundary still costs
    // one inspection, so8192 selected cells cannot be admitted from row1.
    assert_eq!(result(&rows, 4096, 1, 0, 1..3), ResolvedLink::none());
    let rows: Vec<_> = text
        .as_bytes()
        .chunks(4095)
        .enumerate()
        .map(|(i, chunk)| {
            ascii(
                i as u64,
                std::str::from_utf8(chunk).unwrap(),
                (i + 1) * 4095 < text.len(),
                i > 0,
            )
        })
        .collect();
    assert_eq!(result(&rows, 4095, 0, 0, 0..3), ResolvedLink::none());
    let mut rows = vec![ascii(0, "https://a", true, false)];
    for index in 1..911 {
        rows.push(mapped(
            index,
            &[
                (0, 2, "路"),
                (2, 1, "x"),
                (3, 1, "x"),
                (4, 1, "x"),
                (5, 1, "x"),
                (6, 1, "x"),
                (7, 1, "x"),
                (8, 1, "x"),
            ],
            true,
            true,
        ));
    }
    rows.push(mapped(911, &[(0, 2, "路")], false, true));
    // Spacer heads do not appear in URL/regions but count toward work budget.
    assert_eq!(result(&rows, 10, 0, 0, 0..912), ResolvedLink::none());
}
