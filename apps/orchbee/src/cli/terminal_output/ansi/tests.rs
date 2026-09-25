use super::*;

const DEFAULT: TerminalTextStyle = TerminalTextStyle {
    foreground: 0x0100_0000,
    background: 0x0100_0000,
    underline_color: 0x0200_0000,
    flags: 0,
    underline: 0,
};

fn cell(text: &str) -> Cell<'_> {
    Cell {
        text,
        style: DEFAULT,
        width: 0,
    }
}

fn export(
    items: &[(u64, u8, Vec<Cell<'_>>)],
    source: OutputSource,
    lines: Option<u16>,
) -> PreparedContent {
    render(
        items.iter().map(|(index, flags, cells)| Row {
            index: *index,
            flags: *flags,
            cells: cells.iter().copied(),
            kept_cells: 0,
        }),
        OutputSelection { source, lines },
    )
    .expect("small export")
}

fn plain(ansi: &str) -> String {
    regex::Regex::new("\x1b\\[[0-9;:]*m")
        .expect("SGR grammar")
        .replace_all(ansi, "")
        .into_owned()
}

#[test]
fn every_style_is_reconstructed_and_reset_before_transitions_and_row_boundaries() {
    let decorated = Cell {
        text: "彩",
        width: 1,
        style: TerminalTextStyle {
            foreground: 0x123456,
            background: 0xabcdef,
            underline_color: 0x654321,
            flags: 255,
            underline: 3,
        },
    };
    let rows = vec![(
        10,
        0,
        vec![
            decorated,
            Cell {
                width: 2,
                ..decorated
            },
            cell("e\u{301}"),
        ],
    )];
    let out = export(&rows, OutputSource::Visible, None);
    assert_eq!(
        out.text,
        concat!(
            "\x1b[0m\x1b[1m\x1b[3m\x1b[2m\x1b[5m\x1b[7m\x1b[8m\x1b[9m\x1b[53m",
            "\x1b[4:3m\x1b[38;2;18;52;86m\x1b[48;2;171;205;239m\x1b[58;2;101;67;33m彩\x1b[0me\u{301}\x1b[0m\n"
        )
    );
    for underline in 1..=5 {
        let row = [(
            2,
            0,
            vec![Cell {
                style: TerminalTextStyle {
                    underline,
                    ..DEFAULT
                },
                ..cell("x")
            }],
        )];
        assert!(
            export(&row, OutputSource::Visible, None)
                .text
                .contains(&format!("\x1b[4:{underline}m"))
        );
    }
}

#[test]
fn styled_blanks_survive_trimming_and_visible_tail_uses_prepared_rows() {
    let blank = Cell {
        style: TerminalTextStyle {
            background: 0x112233,
            ..DEFAULT
        },
        ..cell(" ")
    };
    let rows = vec![
        (3, 0, vec![cell("a"), cell(" ")]),
        (4, 0, vec![blank, blank, cell("")]),
        (5, 0, vec![cell(" ")]),
    ];
    let out = export(&rows, OutputSource::Visible, Some(1));
    assert_eq!(plain(&out.text), "  \n");
    assert_eq!(
        (
            out.span.first_row,
            out.span.last_row,
            out.span.physical_rows
        ),
        (Some(4), Some(4), 1)
    );
    assert!(out.text.contains("\x1b[48;2;17;34;51m"));
    assert!(
        export(&rows, OutputSource::Visible, Some(0))
            .text
            .is_empty()
    );
    assert!(
        export(&[(0, 0, vec![cell(" ")])], OutputSource::Recent, None)
            .text
            .is_empty()
    );
}

#[test]
fn verified_wraps_join_without_wide_spacers_and_preserve_edges() {
    let rows = vec![
        (
            7,
            1,
            vec![
                cell("a"),
                Cell {
                    width: 3,
                    ..cell("")
                },
            ],
        ),
        (
            8,
            3,
            vec![
                Cell {
                    width: 1,
                    ..cell("界")
                },
                Cell {
                    width: 2,
                    ..cell("")
                },
            ],
        ),
        (9, 2, vec![cell("👩🏽‍💻")]),
        (10, 0, vec![cell("")]),
        (11, 0, vec![cell("z")]),
    ];
    let logical = export(&rows, OutputSource::RecentUnwrapped, None);
    assert_eq!(plain(&logical.text), "a界👩🏽‍💻\n\nz\n");
    assert_eq!(
        plain(&export(&rows, OutputSource::Recent, None).text),
        "a\n界\n👩🏽‍💻\n\nz\n"
    );
    let edge = export(&rows[1..2], OutputSource::RecentUnwrapped, None);
    assert!(edge.span.starts_mid_line && edge.span.ends_mid_line);
    assert!(edge.text.starts_with("\x1b[0m"));
    assert!(edge.text.ends_with("\x1b[0m\n"));
}

#[test]
fn cell_control_characters_are_data_and_only_generated_sgr_remains() {
    let rows = [(0, 0, vec![cell("中文\x1b]52;c;x\x07\n\r\t\u{85}\x7f")])];
    let out = export(&rows, OutputSource::Visible, None);
    assert_eq!(
        plain(&out.text),
        "中文\\u{1b}]52;c;x\\u{7}\\u{a}\\u{d}\\u{9}\\u{85}\\u{7f}\n"
    );
    assert!(!plain(&out.text).contains('\x1b'));
}

#[test]
fn detection_escapes_controls_after_physical_tail_selection() {
    let rows = ["omitted", "中文\x1b]52;c;x\x07\n\r\t\u{85}\x7f", "  "]
        .into_iter()
        .enumerate()
        .map(|(index, text)| OutputRow {
            index: index as u64 + 10,
            text: text.to_owned(),
            from_previous: true,
            into_next: true,
        })
        .collect::<Vec<_>>();
    let selection = OutputSelection {
        source: OutputSource::Detection,
        lines: Some(1),
    };
    let out = detection(rows.clone(), selection).expect("bounded text");
    assert_eq!(
        out.text,
        "中文\\u{1b}]52;c;x\\u{7}\\u{a}\\u{d}\\u{9}\\u{85}\\u{7f}\n"
    );
    assert!(out.ansi && out.span.starts_mid_line && out.span.ends_mid_line);
    assert_eq!(out.span.physical_rows, 1);
    assert_eq!(out.span.first_row, Some(11));
    assert_eq!(out.span.last_row, Some(11));
    let empty = detection(
        rows,
        OutputSelection {
            lines: Some(0),
            ..selection
        },
    )
    .expect("empty tail");
    assert!(empty.text.is_empty());
    assert_eq!(empty.span.physical_rows, 0);
}

#[test]
fn detection_control_expansion_is_bounded_across_rows() {
    let rows = (0..2)
        .map(|index| OutputRow {
            index,
            text: "\x1b".repeat(output::LIMIT / 10),
            from_previous: false,
            into_next: false,
        })
        .collect();
    let error = detection(
        rows,
        OutputSelection {
            source: OutputSource::Detection,
            lines: None,
        },
    )
    .expect_err("escaped aggregate exceeds output limit");
    assert_eq!(error.kind, FailureKind::OutputTooLarge);
}

#[test]
fn repeated_borrowed_graphemes_cannot_expand_past_preparation_limit() {
    let text = "x".repeat(16 * 1024);
    let cells = vec![cell(&text); 600];
    let error = render(
        std::iter::once(Row {
            index: 0,
            flags: 0,
            cells: cells.iter().copied(),
            kept_cells: 0,
        }),
        OutputSelection {
            source: OutputSource::Visible,
            lines: None,
        },
    )
    .expect_err("bounded expansion");
    assert_eq!(error.kind, FailureKind::OutputTooLarge);
}

#[test]
fn repeated_blank_aliases_need_no_expanded_text_to_trim() {
    let text = " ".repeat(16 * 1024);
    let result = render(
        (0..50).map(|index| Row {
            index,
            flags: 0,
            cells: std::iter::repeat_n(cell(&text), 4000),
            kept_cells: 0,
        }),
        OutputSelection {
            source: OutputSource::Visible,
            lines: None,
        },
    )
    .expect("bounded blank export");
    assert!(result.text.is_empty());
    assert_eq!(result.span.physical_rows, 0);
}
