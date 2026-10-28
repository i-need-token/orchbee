//! Highlight the live divider exposed by the current successful-frame candidate.
use crate::{
    client_view::{
        Binding, ClientView, KeyCode, KeyLabelGlyphs, LabelStyle, Modifiers,
        format_binding_with_glyphs,
    },
    localization::Locale,
    tui_layout::{DividerDrag, project_shell_areas, status_chip_area},
    tui_pointer::{HitMapBuilder, ProjectedPointer},
    tui_theme::{GlyphMode, Role, Theme},
};
use ratatui::{Frame, layout::Rect};

/// Layout has already moved the panes; preserve its divider and junction glyphs.
pub fn render_divider_preview(
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
        || !hits.exposes_divider(&drag.projected_target())
    {
        return false;
    }
    let line = drag.preview;
    let glyphs = match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    };
    let pointer = hits.projected_divider_pointer(&drag.projected_target());
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
    frame.buffer_mut().set_style(line, preview_style);
    let areas = project_shell_areas(frame.area(), view.sidebar_surface());
    render_drag_hint(frame, status_chip_area(&areas), locale, glyphs, theme);
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
    let _ = super::status_line::render_context_chip(
        frame,
        area,
        vec![copy.divider, copy.release_to_apply],
        vec![(key, copy.cancel)],
        theme,
    );
}
