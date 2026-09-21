//! Retained-history pane surface: validated OBRW text rows plus the mode
//! badge. The window this paints is already fenced to the pane's current
//! generation and column geometry by the viewport core — this layer only
//! clips to the leaf. Copy, search, selection, styled cells, clipboard
//! integration, and persistence are deliberately out of this slice.

use orchbee_terminal_protocol::{TerminalTextFrame, TerminalTextRowWindow, TerminalTextWindowRow};
use ratatui::{Frame, layout::Rect, widgets::Clear};

use crate::{
    localization::Locale,
    terminal_session::NativeViewportSurface,
    terminal_text::{display_width, sanitize_terminal_text},
    tui_theme::{Role, Theme},
};

use super::terminal_cells;

/// One non-Live surface paint. `Entering` keeps the last verified live
/// frame beneath the badge; `Window` replaces the leaf with retained rows;
/// `AwaitingWindow`/`Recovering` clear the leaf so no stale content can
/// linger under their indicators.
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    surface: NativeViewportSurface<'_>,
    live_frame: Option<&TerminalTextFrame>,
    show_cursor: bool,
    locale: Locale,
    theme: &Theme,
) {
    match surface {
        NativeViewportSurface::Window(window) => {
            frame.render_widget(Clear, area);
            paint_rows(frame, area, window);
            badge(frame, area, locale.terminal_history_badge(), theme);
        }
        NativeViewportSurface::Entering => {
            if let Some(terminal) = live_frame {
                terminal_cells::render(frame, area, terminal, show_cursor);
            } else {
                frame.render_widget(Clear, area);
            }
            badge(frame, area, locale.terminal_history_loading(), theme);
        }
        NativeViewportSurface::AwaitingWindow => {
            frame.render_widget(Clear, area);
            badge(frame, area, locale.terminal_history_loading(), theme);
        }
        NativeViewportSurface::Recovering => {
            frame.render_widget(Clear, area);
            badge(frame, area, locale.terminal_history_recovering(), theme);
        }
    }
}

/// Plain-text rows, top-aligned inside the leaf: rows clip to the leaf
/// bounds, mapped cells land on their recorded columns, forbidden scalars
/// are sanitized, and anything the row cannot express paints blank —
/// never guessed.
fn paint_rows(frame: &mut Frame<'_>, area: Rect, window: &TerminalTextRowWindow) {
    for (offset, row) in window.rows.iter().enumerate() {
        let Ok(offset) = u16::try_from(offset) else {
            break;
        };
        if offset >= area.height {
            break;
        }
        paint_row(frame, area, offset, row);
    }
}

fn paint_row(frame: &mut Frame<'_>, area: Rect, offset: u16, row: &TerminalTextWindowRow) {
    let y = area.y + offset;
    if row.identity_ascii {
        // Identity rows are one ASCII cell per byte, columns in order.
        let text = sanitize_terminal_text(&row.text);
        frame
            .buffer_mut()
            .set_stringn(area.x, y, text.as_ref(), usize::from(area.width));
        return;
    }
    for (index, mapping) in row.mappings.iter().enumerate() {
        if mapping.column >= area.width {
            continue;
        }
        let end = row
            .mappings
            .get(index + 1)
            .map_or(row.text.len(), |next| next.byte_offset as usize);
        let start = mapping.byte_offset as usize;
        if start >= end || end > row.text.len() {
            continue;
        }
        let text = sanitize_terminal_text(&row.text[start..end]);
        if text.is_empty() {
            continue;
        }
        let column = area.x + mapping.column;
        if mapping.width == 2 && usize::from(mapping.column) + 1 >= usize::from(area.width) {
            // A double-width cell at the leaf's last column has no room:
            // the decoder proved it occupies two columns, so paint the
            // visible half blank rather than overflowing the leaf.
            frame.buffer_mut().set_stringn(column, y, " ", 1);
            continue;
        }
        frame
            .buffer_mut()
            .set_stringn(column, y, text.as_ref(), usize::from(mapping.width));
    }
}

/// The right-aligned mode badge on the leaf's first row — the only
/// chrome this surface owns.
fn badge(frame: &mut Frame<'_>, area: Rect, label: &str, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let label = format!(" {label} ");
    let width = u16::try_from(display_width(&label)).unwrap_or(u16::MAX);
    if width == 0 || width > area.width {
        return;
    }
    let x = area.x + area.width - width;
    frame
        .buffer_mut()
        .set_stringn(x, area.y, &label, usize::from(width));
    frame.buffer_mut().set_style(
        Rect::new(x, area.y, width, 1),
        theme.style(Role::StatusWarning),
    );
}
