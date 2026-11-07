//! Styled retained rows reuse the live cell painter; pending history states show an indicator.
use orchbee_terminal_protocol::TerminalTextFrame;

use ratatui::{
    Frame,
    buffer::CellWidth,
    layout::{Position, Rect},
    style::Style,
    widgets::Clear,
};

use crate::{
    localization::Locale,
    terminal_live_view::LiveView,
    terminal_text::display_width,
    tui_theme::{Role, Theme},
    workbench_client::ViewportSurface,
};

use super::terminal_cells;

/// A local live crop reserves its own first row, so the indicator never hides the prompt.
#[expect(clippy::too_many_arguments)]
pub(super) fn render_live(
    frame: &mut Frame<'_>,
    area: Rect,
    terminal: &TerminalTextFrame,
    view: LiveView,
    show_cursor: bool,
    occupied_header: Option<Rect>,
    locale: Locale,
    theme: &Theme,
) -> Option<Position> {
    let cursor = terminal_cells::render_live(frame, area, terminal, view, show_cursor, theme);
    render_live_header(frame, area, terminal, view, occupied_header, locale, theme);
    cursor
}

fn render_live_header(
    frame: &mut Frame<'_>,
    area: Rect,
    terminal: &TerminalTextFrame,
    view: LiveView,
    occupied_header: Option<Rect>,
    locale: Locale,
    theme: &Theme,
) {
    if !view.clipped || area.height < 2 || area.width == 0 {
        return;
    }
    let mut header = Rect::new(area.x, area.y, area.width, 1);
    if let Some(occupied) =
        occupied_header.filter(|occupied| !header.intersection(*occupied).is_empty())
    {
        // A caller can share the chrome row with this indicator. Yield only
        // those occupied columns; live content and cursor keep their geometry.
        header.width = occupied.x.saturating_sub(header.x).min(header.width);
    }
    if header.is_empty() {
        return;
    }
    frame.render_widget(Clear, header);
    frame
        .buffer_mut()
        .set_style(header, theme.style(Role::SurfaceChrome));
    let compact = locale.terminal_live_crop_compact();
    let text = if !terminal.alternate_screen
        && terminal.interaction.mouse_tracking() == orchbee_terminal_protocol::MouseTracking::None
        && display_width(compact) <= usize::from(header.width)
    {
        compact
    } else {
        locale.terminal_live_crop()
    };
    if display_width(text) <= usize::from(header.width) {
        frame
            .buffer_mut()
            .set_string(header.x, header.y, text, theme.style(Role::TextSecondary));
    }
}

/// One non-Live surface paint. `Entering` keeps the last verified live
/// frame beneath its loading indicator; `Window` replaces the leaf with
/// retained rows; `AwaitingWindow`/`Recovering` clear the leaf so no stale
/// content can linger under their indicators.
#[cfg(test)]
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    surface: ViewportSurface<'_>,
    live_frame: Option<&TerminalTextFrame>,
    show_cursor: bool,
    locale: Locale,
    theme: &Theme,
) {
    if let Some(cursor) = render_selected(
        frame,
        area,
        surface,
        live_frame,
        show_cursor,
        locale,
        theme,
        None,
        false,
    ) {
        frame.set_cursor_position(cursor);
    }
}

#[expect(clippy::too_many_arguments)]
pub(super) fn render_selected(
    frame: &mut Frame<'_>,
    area: Rect,
    surface: ViewportSurface<'_>,
    live_frame: Option<&TerminalTextFrame>,
    show_cursor: bool,
    locale: Locale,
    theme: &Theme,
    selected: Option<&orchbee_terminal_protocol::SelectionChangedState>,
    copy_mode: bool,
) -> Option<Position> {
    let chrome = true;
    let chrome = chrome && !copy_mode;
    render_content(
        frame,
        area,
        surface,
        live_frame,
        show_cursor && chrome,
        locale,
        theme,
        selected,
        chrome,
    )
}

#[expect(clippy::too_many_arguments)]
fn render_content(
    frame: &mut Frame<'_>,
    area: Rect,
    surface: ViewportSurface<'_>,
    live_frame: Option<&TerminalTextFrame>,
    show_cursor: bool,
    locale: Locale,
    theme: &Theme,
    selected: Option<&orchbee_terminal_protocol::SelectionChangedState>,
    chrome: bool,
) -> Option<Position> {
    let mut cursor = None;
    match surface {
        ViewportSurface::Window { window, .. } => {
            frame.render_widget(Clear, area);
            let terminal_theme =
                theme.with_host_terminal_colors(window.foreground, window.background);
            terminal_cells::render_rows(frame, area, &window.rows, &terminal_theme);
            super::selection::paint(
                frame,
                area,
                window.route.start_row,
                selected,
                theme.style(Role::SurfaceSelection),
            );
        }
        ViewportSurface::Entering => {
            if let Some(terminal) = live_frame {
                cursor =
                    terminal_cells::render_with_cursor(frame, area, terminal, show_cursor, theme);
            } else {
                frame.render_widget(Clear, area);
            }
            if chrome {
                badge(frame, area, locale.terminal_history_loading(), theme);
            }
        }
        ViewportSurface::AwaitingWindow => {
            frame.render_widget(Clear, area);
            if chrome {
                badge(frame, area, locale.terminal_history_loading(), theme);
            }
        }
        ViewportSurface::Recovering => {
            frame.render_widget(Clear, area);
            if chrome {
                badge(frame, area, locale.terminal_history_recovering(), theme);
            }
        }
    }
    cursor
}

/// The right-aligned loading or recovery indicator on the leaf's first row.
/// Retained rows paint without chrome.
fn badge_rect(area: Rect, label: &str) -> Option<Rect> {
    let width = u16::try_from(display_width(label).saturating_add(2)).ok()?;
    (!area.is_empty() && width <= area.width)
        .then(|| Rect::new(area.right() - width, area.y, width, 1))
}

fn badge(frame: &mut Frame<'_>, area: Rect, label: &str, theme: &Theme) {
    let Some(rect) = badge_rect(area, label) else {
        return;
    };
    let label = format!(" {label} ");
    let width = rect.width;
    let x = rect.x;
    if x > area.x
        && let Some(cell) = frame.buffer_mut().cell_mut((x - 1, area.y))
        && cell.cell_width() > 1
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
