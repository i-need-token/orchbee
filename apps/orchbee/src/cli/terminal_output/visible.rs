//! Select borrowed visible rows before expanding any shared cell strings.
use orchbee_terminal_protocol::{TerminalTextFrame, TerminalTextRow};
use std::collections::HashMap;

use super::{Failure, FailureKind, OutputRow, OutputSelection, output};

#[derive(Default)]
struct TextFacts {
    nonblank: HashMap<(usize, usize), bool>,
    trimmed: HashMap<(usize, usize), usize>,
}

impl TextFacts {
    fn nonblank(&mut self, text: &str) -> bool {
        *self
            .nonblank
            .entry((text.as_ptr() as usize, text.len()))
            .or_insert_with(|| !text.trim().is_empty())
    }

    fn trimmed(&mut self, text: &str) -> usize {
        *self
            .trimmed
            .entry((text.as_ptr() as usize, text.len()))
            .or_insert_with(|| text.trim_end_matches(' ').len())
    }

    fn last_content(&mut self, rows: &[TerminalTextRow]) -> Option<usize> {
        rows.iter().rposition(|row| {
            row.cells
                .iter()
                .any(|cell| cell.width < 2 && self.nonblank(&cell.text))
        })
    }
}

pub(super) fn last_content_row(frame: &TerminalTextFrame) -> Option<u64> {
    TextFacts::default()
        .last_content(&frame.rows)
        .map(|index| frame.scrollback_rows + u64::from(frame.rows[index].index))
}

pub(super) fn rows(
    frame: &TerminalTextFrame,
    selection: OutputSelection,
) -> Result<Vec<OutputRow>, Failure> {
    if selection.lines == Some(0) {
        return Ok(Vec::new());
    }
    let mut facts = TextFacts::default();
    let end = facts.last_content(&frame.rows).map_or(0, |index| index + 1);
    let start = selection
        .lines
        .map_or(0, |limit| end.saturating_sub(usize::from(limit)));
    let mut size = 0;
    frame.rows[start..end]
        .iter()
        .map(|row| {
            let kept = row
                .cells
                .iter()
                .rposition(|cell| cell.width < 2 && facts.trimmed(&cell.text) != 0)
                .map_or(0, |index| index + 1);
            let mut text = String::new();
            // Reserve this row's separator in the aggregate bound, even for an interior empty row.
            reserve(&mut size, 1)?;
            for (index, cell) in row.cells[..kept].iter().enumerate() {
                if cell.width == 2 {
                    continue;
                }
                let part = if cell.width == 3 || cell.text.is_empty() {
                    " "
                } else if index + 1 == kept {
                    &cell.text[..facts.trimmed(&cell.text)]
                } else {
                    &cell.text
                };
                reserve(&mut size, part.len())?;
                text.push_str(part);
            }
            Ok(OutputRow {
                index: frame.scrollback_rows + u64::from(row.index),
                text,
                from_previous: row.flags & 2 != 0,
                into_next: row.flags & 1 != 0,
            })
        })
        .collect()
}

fn reserve(size: &mut usize, count: usize) -> Result<(), Failure> {
    if count > output::LIMIT - *size {
        return Err(
            Failure::new(FailureKind::OutputTooLarge).details(serde_json::Map::from_iter([(
                "limitBytes".into(),
                serde_json::json!(output::LIMIT),
            )])),
        );
    }
    *size += count;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchbee_terminal_protocol::{
        TerminalTextCell, TerminalTextStyle, decode_terminal_text_frame,
    };
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
            "/../../packages/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin"
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
}
