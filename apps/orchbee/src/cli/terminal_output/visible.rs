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

/// The last nonblank live-screen row strictly above screen row `row`, as an absolute row.
pub(super) fn last_content_row_above(frame: &TerminalTextFrame, row: u16) -> Option<u64> {
    let above = frame
        .rows
        .partition_point(|candidate| candidate.index < row);
    TextFacts::default()
        .last_content(&frame.rows[..above])
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
mod tests;
