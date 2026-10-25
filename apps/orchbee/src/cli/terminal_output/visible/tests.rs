use super::*;
use orchbee_terminal_protocol::{TerminalTextCell, TerminalTextStyle, decode_terminal_text_frame};
use std::sync::Arc;

const DEFAULT: TerminalTextStyle = TerminalTextStyle {
    foreground: 0x0100_0000,
    background: 0x0100_0000,
    underline_color: 0x0100_0000,
    flags: 0,
    underline: 0,
};
fn cell(text: &str, width: u8) -> TerminalTextCell {
    TerminalTextCell {
        text: Arc::from(text),
        width,
        style: DEFAULT,
    }
}
fn frame(rows: Vec<Vec<TerminalTextCell>>) -> TerminalTextFrame {
    let mut frame = decode_terminal_text_frame(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/terminal/v1/fixtures/ghostty-surface.bin"
    )))
    .expect("Host fixture");
    let columns = rows.iter().map(Vec::len).max().unwrap_or(1);
    frame.geometry.columns = columns as u16;
    frame.geometry.rows = rows.len() as u16;
    frame.cursor.row = 0;
    frame.cursor.column = 0;
    frame.rows = rows
        .into_iter()
        .enumerate()
        .map(|(index, mut cells)| {
            cells.resize(columns, cell(" ", 0));
            TerminalTextRow {
                prompt_class: 0,
                index: index as u16,
                flags: 0,
                cells,
            }
        })
        .collect();
    frame
}
fn selection(lines: Option<u16>) -> OutputSelection {
    OutputSelection {
        source: super::super::OutputSource::Visible,
        lines,
    }
}

#[test]
fn selected_visible_text_preserves_spaces_controls_wide_cells_and_empty_rows() {
    let mut sample = frame(vec![
        vec![
            cell("a ", 0),
            cell("", 0),
            cell("界", 1),
            cell("ignored", 2),
            cell(" ", 0),
        ],
        vec![cell(" ", 0)],
        vec![cell("\t\x1b ", 0), cell("", 3)],
        vec![cell("\t ", 0)],
    ]);
    sample.rows[2].flags = 3;
    let all = rows(&sample, selection(None)).expect("bounded");
    assert_eq!(
        all.iter().map(|row| row.text.as_str()).collect::<Vec<_>>(),
        vec!["a  界", "", "\t\x1b"]
    );
    let tail = rows(&sample, selection(Some(1))).expect("tail");
    assert_eq!(tail.len(), 1);
    assert!(tail[0].from_previous && tail[0].into_next);
    assert_eq!(last_content_row(&sample), Some(sample.scrollback_rows + 2));
}

#[test]
fn omitted_or_blank_aliases_never_expand_and_selected_aliases_fail_within_budget() {
    let huge = vec![cell(&"x".repeat(16 * 1024), 0); 1000];
    let blank = vec![cell(&" ".repeat(16 * 1024), 0); 1000];
    let sample = frame(vec![huge, vec![cell("tail", 0)], blank]);
    assert_eq!(last_content_row(&sample), Some(sample.scrollback_rows + 1));
    assert_eq!(
        rows(&sample, selection(Some(1))).expect("omit huge row")[0].text,
        "tail"
    );
    assert!(rows(&sample, selection(Some(0))).expect("zero").is_empty());
    assert_eq!(
        rows(&sample, selection(None))
            .expect_err("bounded aliases")
            .kind,
        FailureKind::OutputTooLarge
    );
}

#[test]
fn selected_visible_text_aggregate_accepts_exact_limit_and_refuses_one_more_byte() {
    let unit = cell(&"x".repeat(16 * 1024), 0);
    let mut sample = frame(vec![
        vec![unit; 511],
        vec![cell(&"x".repeat(16 * 1024 - 2), 0)],
    ]);
    let at_limit = rows(&sample, selection(None)).expect("exact bound");
    assert_eq!(
        at_limit.iter().map(|row| row.text.len() + 1).sum::<usize>(),
        output::LIMIT
    );
    sample.rows[1].cells[0].text = Arc::from("x".repeat(16 * 1024 - 1));
    assert_eq!(
        rows(&sample, selection(None))
            .expect_err("one extra byte")
            .kind,
        FailureKind::OutputTooLarge
    );
}
