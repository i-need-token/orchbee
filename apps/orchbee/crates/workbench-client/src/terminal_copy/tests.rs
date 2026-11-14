//! Reference-derived decisions are tested without a Client or transport.
use super::*;
use orchbee_terminal_protocol::{TerminalRowPromptClass, TerminalTextCellMapping};

fn p(row: u32, col: u16) -> TerminalTextPoint {
    TerminalTextPoint { row, col }
}
fn r(row: u32, text: &str) -> TerminalTextWindowRow {
    TerminalTextWindowRow {
        index: u64::from(row),
        prompt_class: TerminalRowPromptClass::None,
        wrap: false,
        wrap_continuation: false,
        identity_ascii: true,
        text: text.into(),
        mappings: Vec::new(),
    }
}
fn mapped(row: u32, cells: &[(&str, u8)]) -> TerminalTextWindowRow {
    let mut result = r(row, "");
    result.identity_ascii = false;
    let mut text = String::new();
    let mut column = 0;
    for (cell, width) in cells {
        result.mappings.push(TerminalTextCellMapping {
            byte_offset: text.len() as u32,
            utf16_offset: text.encode_utf16().count() as u32,
            column,
            width: *width,
        });
        text.push_str(cell);
        column += u16::from(*width);
    }
    result.text = text.into();
    result
}
fn range(a: TerminalTextPoint, b: TerminalTextPoint) -> TerminalTextRange {
    TerminalTextRange { start: a, end: b }
}

#[test]
fn copy_motion_all_ten_motions_follow_reference_punctuation_big_words_and_blank_rows() {
    use CopyMotion::*;
    let rows = vec![r(0, "foo.bar baz qux/quux"), r(1, ""), r(2, "next")];
    for (motion, origin, expected) in [
        (LineEnd, p(0, 0), p(0, 19)),
        (FirstNonBlank, p(0, 10), p(0, 0)),
        (NextWordStart, p(0, 0), p(0, 3)),
        (PreviousWordStart, p(0, 5), p(0, 4)),
        (NextWordEnd, p(0, 0), p(0, 2)),
        (NextBigWordStart, p(0, 0), p(0, 8)),
        (PreviousBigWordStart, p(0, 19), p(0, 12)),
        (NextBigWordEnd, p(0, 0), p(0, 6)),
        (PreviousParagraph, p(2, 3), p(1, 3)),
        (NextParagraph, p(0, 3), p(1, 3)),
        (NextBigWordEnd, p(0, 6), p(0, 10)),
        (NextBigWordStart, p(0, 8), p(0, 12)),
        (PreviousBigWordStart, p(0, 12), p(0, 8)),
        (NextWordStart, p(0, 3), p(0, 4)),
        (NextWordStart, p(0, 18), p(2, 0)),
        (PreviousWordStart, p(0, 0), p(0, 0)),
        (NextWordEnd, p(2, 3), p(2, 3)),
    ] {
        assert_eq!(
            motion_target(&rows, 20, origin, motion),
            expected,
            "{motion:?} at {origin:?}"
        );
    }
    let rows = vec![r(0, "a_b.c "), mapped(1, &[("—", 1), ("d", 1)])];
    assert_eq!(motion(&rows, 6, p(0, 0), NextWordStart), p(0, 3));
    assert_eq!(motion(&rows, 6, p(0, 4), NextWordStart), p(1, 0));
    assert_eq!(motion(&rows, 6, p(1, 1), PreviousWordStart), p(1, 0));
}
fn motion_target(
    rows: &[TerminalTextWindowRow],
    columns: u16,
    cursor: TerminalTextPoint,
    kind: CopyMotion,
) -> TerminalTextPoint {
    motion(rows, columns, cursor, kind)
}
#[test]
fn copy_motion_preserves_wide_heads_combining_clusters_and_padded_cursor_semantics() {
    use CopyMotion::*;
    let rows = vec![mapped(
        0,
        &[("中", 2), ("e\u{301}", 1), ("文", 2), (" ", 1), ("x", 1)],
    )];
    for (kind, origin, expected) in [
        (LineEnd, p(0, 0), p(0, 6)),
        (FirstNonBlank, p(0, 8), p(0, 0)),
        (NextWordEnd, p(0, 1), p(0, 3)),
        (NextWordStart, p(0, 4), p(0, 6)),
        (PreviousWordStart, p(0, 2), p(0, 0)),
        (PreviousWordStart, p(0, 1), p(0, 1)),
        (NextBigWordStart, p(0, 7), p(0, 7)),
        (PreviousBigWordStart, p(0, 8), p(0, 6)),
    ] {
        assert_eq!(motion(&rows, 9, origin, kind), expected, "{kind:?}");
    }
    let cluster = vec![mapped(0, &[(" ", 1), (" \u{301}", 1), ("x", 1)])];
    assert_eq!(motion(&cluster, 3, p(0, 0), FirstNonBlank), p(0, 1));
    let rows = vec![r(0, "  ab "), r(1, "")];
    assert_eq!(motion(&rows, 6, p(0, 1), FirstNonBlank), p(0, 2));
    assert_eq!(motion(&rows, 6, p(0, 1), LineEnd), p(0, 3));
    assert_eq!(motion(&rows, 6, p(1, 5), LineEnd), p(1, 0));
}

#[test]
fn copy_paragraph_motion_normalizes_a_wide_whitespace_continuation_to_its_head() {
    use CopyMotion::*;
    let rows = vec![
        r(0, "before"),
        mapped(1, &[("\u{3000}", 2), (" ", 1)]),
        r(2, "after"),
    ];
    assert_eq!(motion(&rows, 6, p(0, 1), NextParagraph), p(1, 0));
    assert_eq!(motion(&rows, 6, p(2, 1), PreviousParagraph), p(1, 0));
    assert_eq!(motion(&rows, 6, p(0, 2), NextParagraph), p(1, 2));
}

#[test]
fn copy_motion_crosses_soft_wrap_and_dropped_wide_spacer_without_a_false_word_boundary() {
    use CopyMotion::*;
    let mut rows = vec![
        r(0, "abcd"),
        mapped(1, &[("界", 2), ("x", 1), ("y", 1), ("z", 1)]),
    ];
    rows[0].wrap = true;
    rows[1].wrap_continuation = true;
    assert_eq!(motion(&rows, 5, p(0, 0), NextWordEnd), p(1, 4));
    assert_eq!(motion(&rows, 5, p(0, 0), NextWordStart), p(0, 0));
    assert_eq!(motion(&rows, 5, p(1, 0), PreviousWordStart), p(0, 0));
    rows[0].wrap = false;
    rows[1].wrap_continuation = false;
    assert_eq!(motion(&rows, 5, p(0, 0), NextWordStart), p(1, 0));
}
#[test]
fn copy_search_is_literal_unicode_smart_case_and_requires_complete_glyph_boundaries() {
    let rows = vec![mapped(
        0,
        &[
            ("C", 1),
            ("A", 1),
            ("F", 1),
            ("É", 1),
            (" ", 1),
            ("中", 2),
            ("e\u{301}", 1),
            (" ", 1),
            ("a", 1),
            (".", 1),
            ("b", 1),
        ],
    )];
    let read = |q| {
        search(
            &rows,
            14,
            q,
            CopySearchCase::Smart,
            CopySearchDirection::Forward,
            p(0, 13),
            None,
        )
        .unwrap()
    };
    assert_eq!(read("café").matches, vec![range(p(0, 0), p(0, 3))]);
    assert_eq!(read("Café").total, 0);
    assert_eq!(read("中").matches, vec![range(p(0, 5), p(0, 6))]);
    assert_eq!(read("e").total, 0);
    assert_eq!(read("e\u{301}").matches, vec![range(p(0, 7), p(0, 7))]);
    assert_eq!(read("a.b").total, 1);
    assert_eq!(read("a?b").total, 0);
    assert_eq!(read("").total, 0);
    let graphics = vec![mapped(0, &[("a", 1), ("\u{10eeee}\u{301}", 1), ("b", 1)])];
    let image_search = |query| {
        search(
            &graphics,
            3,
            query,
            CopySearchCase::Smart,
            CopySearchDirection::Forward,
            p(0, 0),
            None,
        )
        .unwrap()
    };
    assert_eq!(image_search("a b").matches, vec![range(p(0, 0), p(0, 2))]);
    assert_eq!(image_search("\u{10eeee}").total, 0);
    assert_eq!(
        motion(&graphics, 3, p(0, 0), CopyMotion::NextWordStart),
        p(0, 2)
    );
}
#[test]
fn an_explicit_case_choice_replaces_smart_case() {
    let rows = vec![r(0, "Error error")];
    let total = |query, case| {
        search(
            &rows,
            11,
            query,
            case,
            CopySearchDirection::Forward,
            p(0, 0),
            None,
        )
        .unwrap()
        .total
    };
    assert_eq!(total("Error", CopySearchCase::Smart), 1);
    assert_eq!(total("Error", CopySearchCase::Insensitive), 2);
    assert_eq!(total("error", CopySearchCase::Sensitive), 1);
}
#[test]
fn copy_search_joins_verified_wraps_keeps_inner_spaces_and_skips_wide_spacers() {
    let mut rows = vec![
        r(0, "abcd"),
        mapped(1, &[("界", 2), ("x", 1), ("y", 1), ("z", 1)]),
    ];
    rows[0].wrap = true;
    rows[1].wrap_continuation = true;
    let read = |rows: &[TerminalTextWindowRow], q| {
        search(
            rows,
            5,
            q,
            CopySearchCase::Smart,
            CopySearchDirection::Forward,
            p(0, 0),
            None,
        )
        .unwrap()
    };
    assert_eq!(read(&rows, "d界").matches, vec![range(p(0, 3), p(1, 1))]);
    rows[0] = r(0, "ab");
    rows[0].wrap = true;
    rows[1] = r(1, "cd");
    rows[1].wrap_continuation = true;
    assert_eq!(read(&rows, "b   c").matches, vec![range(p(0, 1), p(1, 0))]);
    rows[0].wrap = false;
    rows[1].wrap_continuation = false;
    assert_eq!(read(&rows, "bc").total, 0);
    assert_eq!(read(&rows, "cd ").total, 0);
}
#[test]
fn copy_search_direction_previous_and_wrap_keep_strict_inclusive_origins() {
    let rows = vec![r(0, "x x x")];
    let read = |direction, cursor, previous| {
        search(
            &rows,
            8,
            "x",
            CopySearchCase::Smart,
            direction,
            cursor,
            previous,
        )
        .unwrap()
    };
    let a = read(CopySearchDirection::Forward, p(0, 0), None);
    assert_eq!(
        (a.total, a.current, a.current_global),
        (3, Some(1), Some(1))
    );
    assert_eq!(
        read(CopySearchDirection::Forward, p(0, 4), None).current,
        Some(0)
    );
    assert_eq!(
        read(CopySearchDirection::Backward, p(0, 0), None).current,
        Some(2)
    );
    assert_eq!(
        read(CopySearchDirection::Backward, p(0, 3), None).current,
        Some(1)
    );
    assert_eq!(
        read(
            CopySearchDirection::Forward,
            p(0, 7),
            Some(range(p(0, 0), p(0, 2)))
        )
        .current,
        Some(2)
    );
    assert_eq!(
        read(
            CopySearchDirection::Backward,
            p(0, 7),
            Some(range(p(0, 2), p(0, 4)))
        )
        .current,
        Some(0)
    );
}
#[test]
fn copy_search_bounds_the_window_around_current_without_losing_exact_global_total() {
    let rows = (0..3000).map(|i| r(i, "x")).collect::<Vec<_>>();
    let result = search(
        &rows,
        1,
        "x",
        CopySearchCase::Smart,
        CopySearchDirection::Forward,
        p(1499, 0),
        None,
    )
    .unwrap();
    assert_eq!(
        (result.total, result.current, result.current_global),
        (3000, Some(512), Some(1500))
    );
    assert_eq!(result.matches.len(), 1024);
    assert_eq!(result.matches[0].start, p(988, 0));
    assert_eq!(result.matches[1023].start, p(2011, 0));
    let empty = search(
        &rows,
        1,
        "z",
        CopySearchCase::Smart,
        CopySearchDirection::Forward,
        p(0, 0),
        None,
    )
    .unwrap();
    assert!(empty.matches.is_empty());
    assert_eq!(empty.current, None);
    assert_eq!(empty.current_global, None);
}
#[test]
fn copy_search_bounds_implicit_blank_expansion_before_large_logical_allocation() {
    let mut rows = (0..2050).map(|i| r(i, "")).collect::<Vec<_>>();
    for row in &mut rows {
        row.wrap = true;
        row.wrap_continuation = true;
    }
    assert_eq!(
        search(
            &rows,
            4096,
            "x",
            CopySearchCase::Smart,
            CopySearchDirection::Forward,
            p(0, 0),
            None
        ),
        Err(CopyError::LimitExceeded)
    );
}

#[test]
fn copy_motion_reader_expands_forward_in_complete_verified_chunks() {
    let rows = (0..130)
        .map(|row| {
            r(
                row,
                if row == 0 {
                    "a"
                } else if row == 65 {
                    "b"
                } else {
                    ""
                },
            )
        })
        .collect::<Vec<_>>();
    let mut reads = Vec::new();
    let target = read_motion_with(
        1,
        rows.len() as u64,
        p(0, 0),
        CopyMotion::NextWordStart,
        |range| {
            reads.push(range.clone());
            Ok::<_, &'static str>(rows[range.start as usize..range.end as usize].to_vec())
        },
        || "limit",
        || "join",
    )
    .unwrap();
    assert_eq!(target, p(65, 0));
    assert_eq!(reads, vec![0..64, 64..128]);
}

#[test]
fn copy_motion_reader_rejects_an_unverified_expansion_join() {
    let mut rows = (0..130).map(|row| r(row, "")).collect::<Vec<_>>();
    rows[0].text = "a".into();
    rows[64].wrap_continuation = true;
    assert_eq!(
        read_motion_with(
            1,
            rows.len() as u64,
            p(0, 0),
            CopyMotion::NextWordStart,
            |range| Ok::<_, &'static str>(rows[range.start as usize..range.end as usize].to_vec()),
            || "limit",
            || "join",
        ),
        Err("join")
    );
}
