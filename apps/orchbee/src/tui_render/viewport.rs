//! Styled retained rows reuse the live cell painter, with fenced history chrome.
use orchbee_terminal_protocol::TerminalTextFrame;

use ratatui::{Frame, buffer::CellWidth, layout::Rect, style::Style, widgets::Clear};

use crate::{
    localization::Locale,
    terminal_live_view::LiveView,
    terminal_text::{display_width, truncate_display},
    tui_theme::{Role, Theme},
    workbench_client::NativeViewportSurface,
};

use super::{
    controls::{KeyHint, measure_key_hints, render_key_hints},
    terminal_cells,
};

/// A local live crop reserves its own first row, so the indicator never hides the prompt.
#[expect(clippy::too_many_arguments)]
pub(super) fn render_live(
    frame: &mut Frame<'_>,
    area: Rect,
    terminal: &TerminalTextFrame,
    view: LiveView,
    show_cursor: bool,
    history_key: Option<&str>,
    occupied_header: Option<Rect>,
    locale: Locale,
    theme: &Theme,
) {
    terminal_cells::render_live(frame, area, terminal, view, show_cursor, theme);
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
    let label = locale.terminal_live_crop();
    if let Some(key) = history_key.filter(|_| !terminal.alternate_screen) {
        let hints = [KeyHint::new(key, locale.terminal_view_history())];
        if let Some(width) = measure_key_hints(&hints, theme)
            && display_width(label).saturating_add(width).saturating_add(2)
                <= usize::from(header.width)
        {
            frame.buffer_mut().set_string(
                header.x,
                header.y,
                label,
                theme.style(Role::TextSecondary),
            );
            // The complete hint was measured against this u16 rectangle.
            let width = u16::try_from(width).expect("hint fits the pane");
            let _ = render_key_hints(
                frame,
                Rect::new(header.right() - width, header.y, width, 1),
                &hints,
                theme,
            );
            return;
        }
    }
    let compact = locale.terminal_live_crop_compact();
    let text = if !terminal.alternate_screen
        && terminal.interaction.mouse_tracking() == orchbee_terminal_protocol::MouseTracking::None
        && display_width(compact) <= usize::from(header.width)
    {
        compact
    } else {
        label
    };
    if display_width(text) <= usize::from(header.width) {
        frame
            .buffer_mut()
            .set_string(header.x, header.y, text, theme.style(Role::TextSecondary));
    }
}

/// One non-Live surface paint. `Entering` keeps the last verified live
/// frame beneath the badge; `Window` replaces the leaf with retained rows;
/// `AwaitingWindow`/`Recovering` clear the leaf so no stale content can
/// linger under their indicators.
#[cfg(test)]
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    surface: NativeViewportSurface<'_>,
    live_frame: Option<&TerminalTextFrame>,
    show_cursor: bool,
    locale: Locale,
    theme: &Theme,
) {
    render_selected(
        frame,
        area,
        surface,
        live_frame,
        show_cursor,
        locale,
        theme,
        None,
        false,
    );
}

#[expect(clippy::too_many_arguments)]
pub(super) fn render_selected(
    frame: &mut Frame<'_>,
    area: Rect,
    surface: NativeViewportSurface<'_>,
    live_frame: Option<&TerminalTextFrame>,
    show_cursor: bool,
    locale: Locale,
    theme: &Theme,
    selected: Option<&orchbee_terminal_protocol::SelectionChangedState>,
    copy_mode: bool,
) {
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
    );
}

#[expect(clippy::too_many_arguments)]
fn render_content(
    frame: &mut Frame<'_>,
    area: Rect,
    surface: NativeViewportSurface<'_>,
    live_frame: Option<&TerminalTextFrame>,
    show_cursor: bool,
    locale: Locale,
    theme: &Theme,
    selected: Option<&orchbee_terminal_protocol::SelectionChangedState>,
    chrome: bool,
) {
    match surface {
        NativeViewportSurface::Window { window, position } => {
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
            if chrome {
                badge(frame, area, locale.terminal_history_badge(), theme);
                bottom_line(frame, area, position, locale, theme);
            }
        }
        NativeViewportSurface::Entering => {
            if let Some(terminal) = live_frame {
                terminal_cells::render(frame, area, terminal, show_cursor, theme);
            } else {
                frame.render_widget(Clear, area);
            }
            if chrome {
                badge(frame, area, locale.terminal_history_loading(), theme);
                bottom_line(frame, area, None, locale, theme);
            }
        }
        NativeViewportSurface::AwaitingWindow => {
            frame.render_widget(Clear, area);
            if chrome {
                badge(frame, area, locale.terminal_history_loading(), theme);
                bottom_line(frame, area, None, locale, theme);
            }
        }
        NativeViewportSurface::Recovering => {
            frame.render_widget(Clear, area);
            if chrome {
                badge(frame, area, locale.terminal_history_recovering(), theme);
                bottom_line(frame, area, None, locale, theme);
            }
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
