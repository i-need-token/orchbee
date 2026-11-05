//! Synthesize only SGR and text from validated cells, under the stdout bound.
use orchbee_terminal_protocol::{TerminalRenderRowWindow, TerminalTextFrame, TerminalTextStyle};
use std::collections::HashMap;

use super::{
    ContentSpan, Failure, FailureKind, OutputRow, OutputSelection, OutputSource, PreparedContent,
    output,
};

#[derive(Clone, Copy)]
struct Cell<'a> {
    text: &'a str,
    style: TerminalTextStyle,
    width: u8,
}

struct Row<C> {
    index: u64,
    flags: u8,
    cells: C,
    kept_cells: usize,
}

pub(crate) fn detection(
    mut rows: Vec<OutputRow>,
    selection: OutputSelection,
) -> Result<PreparedContent, Failure> {
    super::prepare_rows(&mut rows, selection);
    let span = ContentSpan {
        physical_rows: rows.len(),
        first_row: rows.first().map(|row| row.index),
        last_row: rows.last().map(|row| row.index),
        starts_mid_line: rows.first().is_some_and(|row| row.from_previous),
        ends_mid_line: rows.last().is_some_and(|row| row.into_next),
    };
    let bytes = output::render_text(|out| {
        for row in &rows {
            write_cell_text(out, &row.text)?;
            out.push("\n")?;
        }
        Ok(())
    })
    .map_err(output_too_large)?;
    Ok(PreparedContent {
        span,
        text: String::from_utf8(bytes).expect("only UTF-8 was written"),
        ansi: true,
    })
}

pub(crate) fn frame(
    frame: &TerminalTextFrame,
    selection: OutputSelection,
) -> Result<PreparedContent, Failure> {
    render(
        frame.rows.iter().map(|row| Row {
            index: frame.scrollback_rows + u64::from(row.index),
            flags: row.flags,
            cells: row.cells.iter().map(|cell| Cell {
                text: &cell.text,
                style: cell.style,
                width: cell.width,
            }),
            kept_cells: 0,
        }),
        selection,
    )
}

pub(crate) fn window(
    window: &TerminalRenderRowWindow<'_>,
    selection: OutputSelection,
) -> Result<PreparedContent, Failure> {
    render(
        window.projection().rows().map(|row| Row {
            index: window.route().start_row + u64::from(row.index()),
            flags: row.flags(),
            cells: row.cells().map(|cell| Cell {
                text: cell.text,
                style: cell.style,
                width: cell.width,
            }),
            kept_cells: 0,
        }),
        selection,
    )
}

fn unstyled(style: TerminalTextStyle) -> bool {
    // Default and none both serialize to the destination's default color.
    [style.foreground, style.background, style.underline_color]
        .into_iter()
        .all(|color| {
            matches!(
                color,
                orchbee_terminal_protocol::COLOR_DEFAULT | orchbee_terminal_protocol::COLOR_NONE
            )
        })
        && style.flags == 0
        && style.underline == 0
}

fn is_padding(text: &str, cache: &mut HashMap<(usize, usize), bool>) -> bool {
    if text.is_empty() {
        return true;
    }
    // A validated borrowed snapshot stays immutable throughout serialization.
    // Address/length keys avoid hashing the same large aliased string repeatedly.
    *cache
        .entry((text.as_ptr() as usize, text.len()))
        .or_insert_with(|| text.bytes().all(|byte| byte == b' '))
}

fn render<'a, C: Iterator<Item = Cell<'a>> + Clone>(
    rows: impl Iterator<Item = Row<C>>,
    selection: OutputSelection,
) -> Result<PreparedContent, Failure> {
    let unwrap = selection.source == OutputSource::RecentUnwrapped;
    let mut padding = HashMap::new();
    let mut rows: Vec<_> = rows
        .map(|mut row| {
            row.kept_cells = row
                .cells
                .clone()
                .enumerate()
                .filter(|(_, cell)| {
                    cell.width != 2
                        && !(unwrap && cell.width == 3)
                        && (!unstyled(cell.style)
                            || (cell.width != 3 && !is_padding(cell.text, &mut padding)))
                })
                .map(|(index, _)| index + 1)
                .last()
                .unwrap_or(0);
            row
        })
        .collect();
    while rows.last().is_some_and(|row| row.kept_cells == 0) {
        rows.pop();
    }
    if selection.source == OutputSource::Visible
        && let Some(limit) = selection.lines
    {
        let omitted = rows.len().saturating_sub(usize::from(limit));
        rows.drain(..omitted);
    }
    if selection.lines == Some(0) {
        rows.clear();
    }
    let span = ContentSpan {
        physical_rows: rows.len(),
        first_row: rows.first().map(|row| row.index),
        last_row: rows.last().map(|row| row.index),
        starts_mid_line: rows.first().is_some_and(|row| row.flags & 2 != 0),
        ends_mid_line: rows.last().is_some_and(|row| row.flags & 1 != 0),
    };
    let bytes = output::render_text(|out| {
        for (index, row) in rows.iter().enumerate() {
            let joined = unwrap
                && index > 0
                && row.flags & 2 != 0
                && rows[index - 1].flags & 1 != 0
                && rows[index - 1].index.checked_add(1) == Some(row.index);
            if index != 0 && !joined {
                out.push("\n")?;
            }
            let mut style = None;
            for cell in row.cells.clone().take(row.kept_cells) {
                if cell.width == 2 || (unwrap && cell.width == 3) {
                    continue;
                }
                if style != Some(cell.style) {
                    write_style(out, cell.style)?;
                    style = Some(cell.style);
                }
                if cell.width == 3 || cell.text.is_empty() {
                    out.push(" ")?;
                } else {
                    write_cell_text(out, cell.text)?;
                }
            }
            // Independent physical rows and selected tails never inherit an SGR state.
            if style.is_some() {
                out.push("\x1b[0m")?;
            }
        }
        if !rows.is_empty() {
            out.push("\n")?;
        }
        Ok(())
    })
    .map_err(output_too_large)?;
    Ok(PreparedContent {
        span,
        text: String::from_utf8(bytes).expect("only UTF-8 was written"),
        ansi: true,
    })
}

fn output_too_large(_: output::TooLarge) -> Failure {
    Failure::new(FailureKind::OutputTooLarge).details(serde_json::Map::from_iter([(
        "limitBytes".into(),
        serde_json::json!(output::LIMIT),
    )]))
}

pub(in crate::cli) fn write_cell_text(
    out: &mut output::Bounded,
    text: &str,
) -> Result<(), output::TooLarge> {
    // Borrow whole safe runs; no repeated allocation for aliased graphemes.
    let mut start = 0;
    for (offset, ch) in text.char_indices() {
        if ch.is_control() {
            out.push(&text[start..offset])?;
            out.write(format_args!("\\u{{{:x}}}", u32::from(ch)))?;
            start = offset + ch.len_utf8();
        }
    }
    out.push(&text[start..])
}

pub(in crate::cli) fn write_style(
    out: &mut output::Bounded,
    style: TerminalTextStyle,
) -> Result<(), output::TooLarge> {
    out.write(format_args!(
        "{}",
        orchbee_terminal_protocol::terminal_sgr_style(style)
    ))
}

#[cfg(test)]
mod tests;
