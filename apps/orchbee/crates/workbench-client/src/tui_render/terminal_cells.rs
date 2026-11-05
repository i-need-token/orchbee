//! Paint captured cells with their owned grid widths; program text never emits terminal commands.
use std::num::NonZeroU16;

use orchbee_terminal_protocol::{TerminalTextFrame, TerminalTextRow, TerminalTextStyle};
use ratatui::{
    Frame,
    buffer::{CellDiffOption, CellWidth},
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    terminal_live_view::LiveView, terminal_text::is_forbidden_terminal_scalar, tui_theme::Theme,
};

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    terminal: &TerminalTextFrame,
    show_cursor: bool,
    theme: &Theme,
) {
    if let Some(cursor) = render_with_cursor(frame, area, terminal, show_cursor, theme) {
        frame.set_cursor_position(cursor);
    }
}

/// Paint one frame and return its cursor cell; the shell places the cursor after the floating
/// status, so a chip that covers the cell also hides the cursor.
pub(super) fn render_with_cursor(
    frame: &mut Frame<'_>,
    area: Rect,
    terminal: &TerminalTextFrame,
    show_cursor: bool,
    theme: &Theme,
) -> Option<Position> {
    let theme = theme.with_host_terminal_colors(terminal.foreground, terminal.background);
    render_rows(frame, area, &terminal.rows, &theme);
    (show_cursor
        && terminal.cursor.visible
        && terminal.cursor.column < area.width
        && terminal.cursor.row < area.height)
        .then(|| {
            Position::new(
                area.x + terminal.cursor.column,
                area.y + terminal.cursor.row,
            )
        })
}
pub(super) fn render_rows(
    frame: &mut Frame<'_>,
    area: Rect,
    rows: &[TerminalTextRow],
    theme: &Theme,
) {
    render_rows_at(frame, area, rows, 0, 0, theme);
}

/// Paint one client-local live crop with cursor and source coordinates kept together.
pub(super) fn render_live(
    frame: &mut Frame<'_>,
    area: Rect,
    terminal: &TerminalTextFrame,
    view: LiveView,
    show_cursor: bool,
    theme: &Theme,
) -> Option<Position> {
    let theme = theme.with_host_terminal_colors(terminal.foreground, terminal.background);
    let (foreground, background) = theme.terminal_defaults();
    frame
        .buffer_mut()
        .set_style(area, Style::new().fg(foreground).bg(background));
    let content = view.content(area);
    render_rows_at(
        frame,
        content,
        &terminal.rows,
        view.column,
        view.row,
        &theme,
    );
    if show_cursor
        && terminal.cursor.visible
        && let (Some(column), Some(row)) = (
            terminal.cursor.column.checked_sub(view.column),
            terminal.cursor.row.checked_sub(view.row),
        )
        && column < content.width
        && row < content.height
    {
        return Some(Position::new(content.x + column, content.y + row));
    }
    None
}

pub(super) fn render_rows_at(
    frame: &mut Frame<'_>,
    area: Rect,
    rows: &[TerminalTextRow],
    first_column: u16,
    first_row: u16,
    theme: &Theme,
) {
    let (foreground, background) = theme.terminal_defaults();
    frame
        .buffer_mut()
        .set_style(area, Style::new().fg(foreground).bg(background));
    for row in rows {
        let Some(local_row) = row
            .index
            .checked_sub(first_row)
            .filter(|row| *row < area.height)
        else {
            continue;
        };
        for (column, cell) in row
            .cells
            .iter()
            .skip(usize::from(first_column))
            .take(usize::from(area.width))
            .enumerate()
        {
            let Ok(column) = u16::try_from(column) else {
                continue;
            };
            let Some(output) = frame
                .buffer_mut()
                .cell_mut((area.x + column, area.y + local_row))
            else {
                continue;
            };
            output.set_style(style(cell.style, foreground, background, theme));
            if cell.width == 2 {
                // A crop beginning on a wide tail must paint a blank, rather than
                // leaving a skipped cell whose previous glyph could survive.
                if column == 0 {
                    output.set_symbol(" ");
                } else {
                    output.set_diff_option(CellDiffOption::Skip);
                }
                continue;
            }
            if cell.width == 3 || cell.text.is_empty() {
                output.set_symbol(" ");
                continue;
            }
            let width = if cell.width == 1 { 2 } else { 1 };
            let measured_width = cell.text.cell_width();
            let printable = !cell.text.chars().any(is_forbidden_terminal_scalar)
                && (measured_width == width || cell.text.graphemes(true).count() == 1);
            if width == 2 && column.saturating_add(1) >= area.width {
                output.set_symbol(" ");
            } else if printable {
                output.set_symbol(&cell.text);
                // The captured grid owns cell width. Re-measuring a regional
                // indicator with unicode-width loses its two-cell Host footprint.
                if measured_width != width {
                    output.set_diff_option(CellDiffOption::ForcedWidth(
                        NonZeroU16::new(width).expect("terminal widths are one or two"),
                    ));
                }
            } else {
                output.set_symbol(if width == 2 { "� " } else { "�" });
            }
        }
    }
}
pub(super) fn render_tail(
    frame: &mut Frame<'_>,
    area: Rect,
    terminal: &TerminalTextFrame,
    window: &crate::terminal_viewport::ViewportWindow,
    rows_before: u32,
    show_cursor: bool,
    theme: &Theme,
) -> Option<Position> {
    let theme = theme.with_host_terminal_colors(window.foreground, window.background);
    render_rows(frame, area, &window.rows, &theme);
    let cursor_row = u32::from(terminal.cursor.row).saturating_add(rows_before);
    (show_cursor
        && terminal.cursor.visible
        && terminal.cursor.column < area.width
        && cursor_row < u32::from(area.height))
    .then(|| Position::new(area.x + terminal.cursor.column, area.y + cursor_row as u16))
}
fn style(value: TerminalTextStyle, foreground: Color, background: Color, theme: &Theme) -> Style {
    let mut style = Style::new()
        .fg(theme.terminal_cell_color(value.foreground, foreground))
        .bg(theme.terminal_cell_color(value.background, background));
    for (bit, modifier) in [
        (1, Modifier::BOLD),
        (2, Modifier::ITALIC),
        (4, Modifier::DIM),
        (8, Modifier::SLOW_BLINK),
        (16, Modifier::REVERSED),
        (32, Modifier::HIDDEN),
        (64, Modifier::CROSSED_OUT),
    ] {
        if value.flags & bit != 0 {
            style = style.add_modifier(modifier);
        }
    }
    if value.underline != 0 {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}
#[cfg(test)]
mod tests;
