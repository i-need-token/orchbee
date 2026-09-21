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
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
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
        NativeViewportSurface::Window { window, position } => {
            frame.render_widget(Clear, area);
            paint_rows(frame, area, window);
            badge(frame, area, locale.terminal_history_badge(), theme);
            bottom_line(frame, area, position, locale, theme);
        }
        NativeViewportSurface::Entering => {
            if let Some(terminal) = live_frame {
                terminal_cells::render(frame, area, terminal, show_cursor);
            } else {
                frame.render_widget(Clear, area);
            }
            badge(frame, area, locale.terminal_history_loading(), theme);
            bottom_line(frame, area, None, locale, theme);
        }
        NativeViewportSurface::AwaitingWindow => {
            frame.render_widget(Clear, area);
            badge(frame, area, locale.terminal_history_loading(), theme);
            bottom_line(frame, area, None, locale, theme);
        }
        NativeViewportSurface::Recovering => {
            frame.render_widget(Clear, area);
            badge(frame, area, locale.terminal_history_recovering(), theme);
            bottom_line(frame, area, None, locale, theme);
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

/// The leaf's last row while non-Live: position on the left (only while a
/// validated window supplies it — never for an expired or stale window),
/// the return-to-live hint pinned right. The hint keeps priority as the
/// pane narrows: the position truncates first, then the hint, and both
/// cuts stay on grapheme boundaries so wide characters never split.
fn bottom_line(
    frame: &mut Frame<'_>,
    area: Rect,
    position: Option<(u64, u64)>,
    locale: Locale,
    theme: &Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let y = area.y + area.height - 1;
    let width = usize::from(area.width);
    let hint = truncate_display(locale.terminal_history_hint(), width, "…");
    let hint_width = u16::try_from(display_width(&hint)).unwrap_or(u16::MAX);
    if let Some((start_row, total_rows)) = position {
        let position_budget = width.saturating_sub(if hint_width == 0 {
            0
        } else {
            usize::from(hint_width) + 1
        });
        let label = truncate_display(
            &locale.terminal_history_position(start_row, total_rows),
            position_budget,
            "…",
        );
        let label_width = usize::from(u16::try_from(display_width(&label)).unwrap_or(u16::MAX));
        if label_width > 0 {
            frame
                .buffer_mut()
                .set_stringn(area.x, y, &label, label_width);
            frame.buffer_mut().set_style(
                Rect::new(area.x, y, label_width as u16, 1),
                theme.style(Role::TextSecondary),
            );
        }
    }
    if hint_width > 0 {
        let x = area.x + area.width - hint_width;
        frame
            .buffer_mut()
            .set_stringn(x, y, &hint, usize::from(hint_width));
        frame
            .buffer_mut()
            .set_style(Rect::new(x, y, hint_width, 1), theme.style(Role::HintLabel));
    }
}
