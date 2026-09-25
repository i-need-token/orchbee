//! Source selection and bounded text projection over one validated terminal sample.

use orchbee_terminal_protocol::{TerminalTextFrame, TerminalTextRowWindow};
use serde_json::{Value, json};

use super::{Failure, FailureKind, output, results::TerminalRow};
use crate::invocation::{OutputSelection, OutputSource};

#[cfg(windows)]
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
        let mut result = json!({
            "terminal": super::results::terminal_row_json(&self.terminal),
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
        });
        if self.ansi {
            result["encoding"] = json!("ansi");
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rows(items: &[&str]) -> Vec<OutputRow> {
        items
            .iter()
            .enumerate()
            .map(|(index, text)| OutputRow {
                index: 10 + index as u64,
                text: (*text).to_owned(),
                from_previous: false,
                into_next: false,
            })
            .collect()
    }
    fn selection(source: OutputSource, lines: Option<u16>) -> OutputSelection {
        OutputSelection { source, lines }
    }

    #[test]
    fn recent_endpoint_respects_content_cursor_empty_and_alternate_screens() {
        let visible = rows(&["a", "b", "", ""]);
        let choose = |source, lines, cursor, alternate, rows: &[OutputRow], capacity| {
            select_range(
                selection(source, lines),
                10,
                14,
                cursor,
                alternate,
                rows.iter()
                    .rev()
                    .find(|row| !row.text.trim().is_empty())
                    .map(|row| row.index),
                capacity,
            )
        };
        let range = choose(OutputSource::Recent, Some(5), 0, false, &visible, 1000);
        assert_eq!((range.start, range.count), (7, 5));
        assert_eq!(
            choose(OutputSource::Recent, Some(5), 2, false, &visible, 1000).start,
            8
        );
        assert_eq!(
            choose(OutputSource::Recent, Some(5), 0, true, &visible, 1000).start,
            9
        );
        assert_eq!(
            choose(
                OutputSource::Recent,
                Some(5),
                0,
                false,
                &rows(&["", "", "", ""]),
                1000
            )
            .start,
            9
        );
        let capped = choose(OutputSource::Recent, None, 0, false, &visible, 3);
        assert_eq!(
            (capped.start, capped.count, capped.capacity_omitted),
            (9, 3, 9)
        );
        assert_eq!(
            choose(OutputSource::Detection, Some(1), 0, false, &visible, 1000).start,
            8
        );
        assert_eq!(
            choose(OutputSource::Visible, None, 0, false, &visible, 1000).start,
            10
        );
        assert_eq!(
            choose(OutputSource::Recent, Some(0), 0, false, &visible, 1000).count,
            0
        );
    }

    #[test]
    fn physical_tails_blank_rows_and_soft_wraps_have_distinct_meanings() {
        let mut sample = rows(&["ab中", "文 cd", "", "  "]);
        sample[0].into_next = true;
        sample[1].from_previous = true;
        prepare_rows(
            &mut sample,
            selection(OutputSource::RecentUnwrapped, Some(4)),
        );
        assert_eq!(join_text(&sample, true).expect("text"), "ab中文 cd\n");
        assert_eq!(join_text(&sample, false).expect("text"), "ab中\n文 cd\n");
        prepare_rows(&mut sample, selection(OutputSource::Detection, Some(1)));
        assert_eq!(sample.len(), 1);
        assert!(sample[0].from_previous);
        assert_eq!(join_text(&sample, true).expect("fragment"), "文 cd\n");
        prepare_rows(&mut sample, selection(OutputSource::Visible, Some(0)));
        assert!(join_text(&sample, false).expect("empty").is_empty());
        let huge = rows(&[&"x".repeat(output::LIMIT)]);
        assert_eq!(
            join_text(&huge, false).expect_err("bound").kind,
            FailureKind::OutputTooLarge
        );
    }
}
