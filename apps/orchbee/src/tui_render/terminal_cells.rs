use orchbee_terminal_protocol::{TerminalTextFrame, TerminalTextRow, TerminalTextStyle};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
};

use crate::terminal_text::is_forbidden_terminal_scalar;

pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    terminal: &TerminalTextFrame,
    show_cursor: bool,
) {
    render_rows(
        frame,
        area,
        &terminal.rows,
        terminal.foreground,
        terminal.background,
    );
    if show_cursor
        && terminal.cursor.visible
        && terminal.cursor.column < area.width
        && terminal.cursor.row < area.height
    {
        frame.set_cursor_position((
            area.x + terminal.cursor.column,
            area.y + terminal.cursor.row,
        ));
    }
}
pub(super) fn render_rows(
    frame: &mut Frame<'_>,
    area: Rect,
    rows: &[TerminalTextRow],
    foreground: u32,
    background: u32,
) {
    for row in rows {
        if row.index >= area.height {
            continue;
        }
        for (column, cell) in row.cells.iter().enumerate().take(usize::from(area.width)) {
            let Ok(column) = u16::try_from(column) else {
                continue;
            };
            let Some(output) = frame
                .buffer_mut()
                .cell_mut((area.x + column, area.y + row.index))
            else {
                continue;
            };
            output.set_style(style(cell.style, foreground, background));
            if cell.width == 2 {
                output.set_diff_option(ratatui::buffer::CellDiffOption::Skip);
                continue;
            }
            if cell.width == 3 || cell.text.is_empty() {
                output.set_symbol(" ");
                continue;
            }
            let width = if cell.width == 1 { 2 } else { 1 };
            let printable = !cell.text.chars().any(is_forbidden_terminal_scalar)
                && Span::raw(cell.text.as_ref()).width() == width;
            if width == 2 && column.saturating_add(1) >= area.width {
                output.set_symbol(" ");
            } else if printable {
                output.set_symbol(&cell.text);
            } else {
                output.set_symbol(if width == 2 { "� " } else { "�" });
            }
        }
    }
}
fn style(value: TerminalTextStyle, foreground: u32, background: u32) -> Style {
    let mut style = Style::new()
        .fg(color(value.foreground, foreground))
        .bg(color(value.background, background));
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
fn color(value: u32, default: u32) -> Color {
    let value = if value <= 0xffffff { value } else { default };
    Color::Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

#[cfg(test)]
mod tests;
