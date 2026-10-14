//! Paint only reversible divider feedback exposed by the current successful-frame candidate.
use crate::{
    client_view::{
        Binding, ClientView, KeyCode, KeyLabelGlyphs, LabelStyle, Modifiers,
        format_binding_with_glyphs,
    },
    localization::Locale,
    terminal_text::display_width,
    tui_layout::{DividerDrag, project_shell_areas},
    tui_pointer::{HitMapBuilder, ProjectedPointer},
    tui_theme::{GlyphMode, Role, Theme},
};
use orchbee_workbench_protocol::PanelOrientation;
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Clear, Paragraph},
};

use super::controls::{KeyHint, measure_key_hints, render_key_hints};

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
    let glyphs = match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(test)]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    };
    let glyph = match (drag.target.orientation, theme.glyphs.mode()) {
        (PanelOrientation::Horizontal, GlyphMode::Unicode) => "│",
        (PanelOrientation::Vertical, GlyphMode::Unicode) => "─",
        #[cfg(test)]
        (PanelOrientation::Horizontal, GlyphMode::Simplified) => "|",
        #[cfg(test)]
        (PanelOrientation::Vertical, GlyphMode::Simplified) => "-",
    };
    let pointer = hits.projected_divider_pointer(&drag.target);
    let interaction = match pointer {
        ProjectedPointer::Rest => None,
        ProjectedPointer::Hovered => Some(Role::SurfaceHover),
        ProjectedPointer::Pressed => Some(Role::SurfacePressed),
    };
    let preview_style = interaction.map_or_else(
        || theme.style(Role::FrameDivider),
        |interaction| {
            theme.content_on_surface(
                Role::SurfaceCanvas,
                Some(interaction),
                Some(Role::FrameDivider),
                theme.style(Role::FrameDivider),
            )
        },
    );
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
    render_drag_hint(frame, areas.status, locale, glyphs, theme);
    true
}

fn render_drag_hint(
    frame: &mut Frame<'_>,
    area: Rect,
    locale: Locale,
    glyphs: KeyLabelGlyphs,
    theme: &Theme,
) {
    if area.is_empty() {
        return;
    }
    let copy = locale.status();
    let key = format_binding_with_glyphs(
        &Binding::new(KeyCode::Esc, Modifiers::NONE),
        LabelStyle::for_host(),
        glyphs,
    );
    let hints = [KeyHint::new(&key, copy.cancel)];
    let Some(hint_width) = measure_key_hints(&hints, theme) else {
        return;
    };
    let context = format!(
        "{} {} {}",
        copy.divider, theme.glyphs.middle_dot, copy.release_to_apply
    );
    let context_width = display_width(&context);
    let separator_width = display_width(theme.glyphs.middle_dot).saturating_add(2);
    let full_width = context_width
        .saturating_add(separator_width)
        .saturating_add(hint_width);
    let (context, width) = if full_width <= usize::from(area.width) {
        (Some(context), full_width)
    } else if hint_width <= usize::from(area.width) {
        (None, hint_width)
    } else {
        return;
    };
    let width = u16::try_from(width).unwrap_or(u16::MAX).min(area.width);
    let right = Rect::new(area.right() - width, area.y, width, 1);
    frame.render_widget(Clear, right);
    frame
        .buffer_mut()
        .set_style(right, theme.style(Role::SurfaceChrome));
    let hint_x = if let Some(context) = context {
        let context_width = u16::try_from(context_width).unwrap_or(u16::MAX);
        frame.render_widget(
            Paragraph::new(context).style(theme.style(Role::TextPrimary)),
            Rect::new(right.x, right.y, context_width, 1),
        );
        let separator = format!(" {} ", theme.glyphs.middle_dot);
        let separator_width = u16::try_from(separator_width).unwrap_or(u16::MAX);
        frame.render_widget(
            Paragraph::new(separator).style(theme.style(Role::HintLabel)),
            Rect::new(right.x + context_width, right.y, separator_width, 1),
        );
        right.x + context_width + separator_width
    } else {
        right.x
    };
    let hint_width = u16::try_from(hint_width).unwrap_or(u16::MAX);
    let _ = render_key_hints(
        frame,
        Rect::new(hint_x, right.y, hint_width, 1),
        &hints,
        theme,
    );
}
