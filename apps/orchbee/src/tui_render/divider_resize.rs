//! Paint only reversible divider feedback exposed by the current successful-frame candidate.
use crate::{
    client_view::ClientView,
    localization::Locale,
    terminal_text::display_width,
    tui_layout::{DividerDrag, project_shell_areas},
    tui_pointer::HitMapBuilder,
    tui_theme::{Role, Theme},
};
use orchbee_workbench_protocol::PanelOrientation;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::Modifier,
    widgets::{Clear, Paragraph},
};

/// Paints the preview without changing hit regions or canonical pane geometry.
pub(crate) fn render_divider_preview(
    frame: &mut Frame<'_>,
    view: &ClientView,
    locale: Locale,
    drag: &DividerDrag,
    theme: &Theme,
    hits: &HitMapBuilder,
) -> bool {
    if view.overlay().is_some()
        || view.prefix_active()
        || view.resize_mode()
        || view.machines.open
        || frame.area() != drag.target.viewport
        || !hits.exposes_divider(&drag.target)
    {
        return false;
    }
    let line = drag.preview;
    let ascii = theme.glyphs.ellipsis == "..";
    let glyph = match (drag.target.orientation, ascii) {
        (PanelOrientation::Horizontal, false) => "│",
        (PanelOrientation::Vertical, false) => "─",
        (PanelOrientation::Horizontal, true) => "|",
        (PanelOrientation::Vertical, true) => "-",
    };
    let preview_style = theme
        .style(Role::AccentSelection)
        .add_modifier(Modifier::BOLD);
    let buffer = frame.buffer_mut();
    for y in line.y..line.bottom() {
        let mut x = buffer.area.x;
        while x < line.right() {
            let cell = &buffer[(x, y)];
            let width = u16::try_from(display_width(cell.symbol()).max(1)).unwrap_or(u16::MAX);
            let next = x.saturating_add(width).min(buffer.area.right());
            if next > line.x {
                // A surviving wide lead would make the terminal diff skip the preview.
                // Preserve its visible style on every intersected glyph's cell, including
                // outside remnants; continuations may instead carry Reset or Skip.
                let style = cell.style();
                for column in x..next {
                    let cell = &mut buffer[(column, y)];
                    cell.reset();
                    cell.set_style(style);
                    if column >= line.x && column < line.right() {
                        cell.set_symbol(glyph).set_style(preview_style);
                    }
                }
            }
            x = next;
        }
    }
    let areas = project_shell_areas(frame.area(), view.sidebar_surface());
    let hint = locale.divider_drag_hint(areas.status.width >= 100);
    let width = u16::try_from(ratatui::text::Line::raw(hint).width())
        .unwrap_or(u16::MAX)
        .min(areas.status.width);
    let [_, right] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(width)]).areas(areas.status);
    frame.render_widget(Clear, right);
    frame.render_widget(
        Paragraph::new(hint).style(theme.style(Role::HintLabel)),
        right,
    );
    true
}
