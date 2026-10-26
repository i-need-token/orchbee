//! Source selection and bounded text projection over one validated terminal sample.

use orchbee_terminal_protocol::{TerminalTextFrame, TerminalTextRowWindow};
use serde_json::{Value, json};

use super::{Failure, FailureKind, output, results::TerminalRow};
use crate::invocation::{OutputSelection, OutputSource};

#[cfg(any(target_os = "linux", windows))]
pub(crate) mod ansi;

#[derive(Clone, Debug, Default)]
pub(crate) struct ContentSpan {
    pub(crate) physical_rows: usize,
    pub(crate) first_row: Option<u64>,
    pub(crate) last_row: Option<u64>,
    pub(crate) starts_mid_line: bool,
    pub(crate) ends_mid_line: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedContent {
    pub(crate) span: ContentSpan,
    pub(crate) text: String,
    pub(crate) ansi: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct OutputRow {
    pub(crate) index: u64,
    pub(crate) text: String,
    pub(crate) from_previous: bool,
    pub(crate) into_next: bool,
}

mod visible;

pub(crate) fn frame_rows(
    frame: &TerminalTextFrame,
    selection: OutputSelection,
) -> Result<Vec<OutputRow>, Failure> {
    visible::rows(frame, selection)
}

pub(crate) fn window_rows(window: &TerminalTextRowWindow) -> Vec<OutputRow> {
    window
        .rows
        .iter()
        .map(|row| OutputRow {
            index: row.index,
            text: row.text.to_string(),
            from_previous: row.wrap_continuation,
            into_next: row.wrap,
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SampleRange {
    pub(crate) start: u64,
    pub(crate) count: u16,
    pub(crate) total: u64,
    pub(crate) capacity_omitted: u64,
}

pub(crate) fn range(
    frame: &TerminalTextFrame,
    selection: OutputSelection,
) -> Result<SampleRange, Failure> {
    let total = frame
        .scrollback_rows
        .checked_add(u64::from(frame.geometry.rows))
        .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?;
    let capacity = u64::try_from(orchbee_terminal_protocol::MAXIMUM_FRAME_CELLS)
        .unwrap_or(u64::MAX)
        / u64::from(frame.geometry.columns);
    Ok(select_range(
        selection,
        frame.scrollback_rows,
        total,
        frame.cursor.row,
        frame.alternate_screen,
        visible::last_content_row(frame),
        capacity,
    ))
}

fn select_range(
    selection: OutputSelection,
    scrollback: u64,
    total: u64,
    cursor: u16,
    alternate: bool,
    last_content_row: Option<u64>,
    capacity: u64,
) -> SampleRange {
    let end = if selection.source == OutputSource::Visible || alternate {
        total
    } else {
        last_content_row.map_or(total, |index| index.max(scrollback + u64::from(cursor)) + 1)
    };
    let wanted = if selection.lines == Some(0) {
        0
    } else {
        match selection.source {
            OutputSource::Visible | OutputSource::Detection => total - scrollback,
            OutputSource::Recent | OutputSource::RecentUnwrapped => {
                u64::from(selection.lines.unwrap_or(80))
            }
        }
    };
    let entitled = wanted.min(end);
    let count = entitled.min(capacity);
    SampleRange {
        start: end - count,
        count: u16::try_from(count).expect("bounded screen or 1000 rows"),
        total,
        capacity_omitted: entitled - count,
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TerminalOutputDoc {
    pub(crate) terminal: TerminalRow,
    pub(crate) selection: OutputSelection,
    pub(crate) revision: u64,
    pub(crate) generation: u64,
    pub(crate) alternate: bool,
    pub(crate) columns: u16,
    pub(crate) screen_rows: u16,
    pub(crate) range: SampleRange,
    pub(crate) span: ContentSpan,
    pub(crate) text: String,
    pub(crate) ansi: bool,
    pub(crate) cursor: Option<String>,
}

pub(crate) fn document(
    terminal: TerminalRow,
    frame: &TerminalTextFrame,
    selection: OutputSelection,
    range: SampleRange,
    mut rows: Vec<OutputRow>,
) -> Result<TerminalOutputDoc, Failure> {
    prepare_rows(&mut rows, selection);
    let text = join_text(&rows, selection.source == OutputSource::RecentUnwrapped)?;
    Ok(prepared_document(
        terminal,
        frame,
        selection,
        range,
        PreparedContent {
            text,
            ansi: false,
            span: ContentSpan {
                physical_rows: rows.len(),
                first_row: rows.first().map(|row| row.index),
                last_row: rows.last().map(|row| row.index),
                starts_mid_line: rows.first().is_some_and(|row| row.from_previous),
                ends_mid_line: rows.last().is_some_and(|row| row.into_next),
            },
        },
    ))
}

pub(crate) fn prepared_document(
    terminal: TerminalRow,
    frame: &TerminalTextFrame,
    selection: OutputSelection,
    range: SampleRange,
    content: PreparedContent,
) -> TerminalOutputDoc {
    let cursor = capture_cursor(&terminal.terminal_id, frame).map(|value| value.encode());
    TerminalOutputDoc {
        terminal,
        selection,
        revision: frame.route.revision,
        generation: frame.route.generation.get(),
        alternate: frame.alternate_screen,
        columns: frame.geometry.columns,
        screen_rows: frame.geometry.rows,
        range,
        span: content.span,
        text: content.text,
        ansi: content.ansi,
        cursor,
    }
}

pub(super) fn prepare_rows(rows: &mut Vec<OutputRow>, selection: OutputSelection) {
    while rows.last().is_some_and(|row| row.text.trim().is_empty()) {
        rows.pop();
    }
    if matches!(
        selection.source,
        OutputSource::Visible | OutputSource::Detection
    ) && let Some(limit) = selection.lines
    {
        let omitted = rows.len().saturating_sub(usize::from(limit));
        rows.drain(..omitted);
    }
}

fn join_text(rows: &[OutputRow], unwrap: bool) -> Result<String, Failure> {
    let mut text = String::new();
    for (index, row) in rows.iter().enumerate() {
        let separator = usize::from(index > 0 && !(unwrap && row.from_previous));
        if row
            .text
            .len()
            .saturating_add(text.len())
            .saturating_add(separator + 1)
            > output::LIMIT
        {
            return Err(Failure::new(FailureKind::OutputTooLarge).details(
                serde_json::Map::from_iter([("limitBytes".into(), json!(output::LIMIT))]),
            ));
        }
        if separator != 0 {
            text.push('\n');
        }
        text.push_str(&row.text);
    }
    if !text.is_empty() {
        text.push('\n');
    }
    Ok(text)
}

impl TerminalOutputDoc {
    pub(crate) fn result(&self) -> Value {
        json!({
            "terminal": super::results::terminal_row_json(&self.terminal),
            "cursor": self.cursor,
            "encoding": if self.ansi { "ansi" } else { "text" },
            "selection": { "source": self.selection.source.token(), "lines": self.selection.lines },
            "observed": { "revision": self.revision.to_string(), "generation": self.generation.to_string(),
                "alternateScreen": self.alternate, "columns": self.columns, "screenRows": self.screen_rows,
                "totalRows": self.range.total.to_string(), "windowStart": self.range.start.to_string(),
                "windowRows": self.range.count, "capacityOmittedRows": self.range.capacity_omitted.to_string() },
            "content": { "text": self.text, "physicalRows": self.span.physical_rows,
                "firstRow": self.span.first_row.map(|index| index.to_string()),
                "lastRow": self.span.last_row.map(|index| index.to_string()),
                "startsMidLine": self.span.starts_mid_line,
                "endsMidLine": self.span.ends_mid_line },
        })
    }
}

/// A cursor excludes the currently occupied physical row. Resize, screen replacement
/// and retained-row eviction invalidate its coordinate system rather than moving it.
pub(crate) fn capture_cursor(
    terminal_id: &str,
    frame: &TerminalTextFrame,
) -> Option<crate::invocation::OutputCursor> {
    if frame.alternate_screen {
        return None;
    }
    Some(crate::invocation::OutputCursor {
        terminal_id: terminal_id.to_owned(),
        host_epoch: frame.route.host_epoch.to_text(),
        generation: frame.route.generation.get(),
        output_epoch: frame.command_observation?.output_epoch.get(),
        revision: frame.route.revision,
        row: frame
            .scrollback_rows
            .checked_add(u64::from(frame.cursor.row))?
            .max(visible::last_content_row(frame).unwrap_or(0)),
    })
}

#[cfg(test)]
mod tests;
