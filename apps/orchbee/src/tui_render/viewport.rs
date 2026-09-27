//! Styled retained rows reuse the live cell painter, with fenced history chrome.
use orchbee_terminal_protocol::TerminalTextFrame;

use ratatui::{Frame, layout::Rect, style::Style, widgets::Clear};

use crate::{
    localization::Locale,
    terminal_session::NativeViewportSurface,
    terminal_text::{display_width, truncate_display},
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
            terminal_cells::render_rows(
                frame,
                area,
                &window.rows,
                window.foreground,
                window.background,
            );
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
    if x > area.x
        && let Some(cell) = frame.buffer_mut().cell_mut((x - 1, area.y))
        && display_width(cell.symbol()) > 1
    {
        cell.reset();
    }
    frame.render_widget(Clear, Rect::new(x, area.y, width, 1));
    frame
        .buffer_mut()
        .set_stringn(x, area.y, &label, usize::from(width), Style::new());
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
    frame.render_widget(Clear, Rect::new(area.x, y, area.width, 1));
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
                .set_stringn(area.x, y, &label, label_width, Style::new());
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
            .set_stringn(x, y, &hint, usize::from(hint_width), Style::new());
        frame
            .buffer_mut()
            .set_style(Rect::new(x, y, hint_width, 1), theme.style(Role::HintLabel));
    }
}
