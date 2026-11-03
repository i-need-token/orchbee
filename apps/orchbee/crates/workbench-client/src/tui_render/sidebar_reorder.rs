//! Transient feedback for a sidebar drag. The sidebar already paints the move in place; this marks
//! the held entry, the bottom drop area and the localized result.
use crate::{
    localization::Locale,
    terminal_text::{sanitize_terminal_text, truncate_display},
    tui_hit_map::{SidebarReorderDraw, sidebar_reorder::Placement},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Clear, Paragraph},
};

pub fn render_sidebar_reorder_preview(
    frame: &mut Frame<'_>,
    preview: &SidebarReorderDraw,
    locale: Locale,
    theme: &Theme,
) {
    let viewport = frame.area();
    let emphasis = theme
        .style(Role::SurfaceSelection)
        .patch(theme.style(Role::TextPrimary));
    // The held entry stays legible in its identity colors; dimming it reads as the placeholder of
    // a lifted row and survives collapsed colors.
    if let Some(area) = preview.grabbed {
        frame.buffer_mut().set_style(
            area.intersection(viewport),
            Style::new().add_modifier(Modifier::DIM),
        );
    }
    let footer = preview.footer.intersection(frame.area());
    let footer_active = preview
        .destination
        .as_ref()
        .is_some_and(|(placement, _)| matches!(placement, Placement::Ungrouped | Placement::End));
    frame.render_widget(Clear, footer);
    frame.render_widget(
        Paragraph::new(truncate_display(
            &format!("[ {} ]", locale.sidebar_drop_zone(preview.workspace)),
            usize::from(footer.width),
            theme.glyphs.ellipsis,
        ))
        .style(if footer_active {
            emphasis.add_modifier(Modifier::BOLD)
        } else {
            theme
                .style(Role::SurfaceChrome)
                .patch(theme.style(Role::TextSecondary))
        }),
        footer,
    );
    let area = frame.area();
    if area.height > 0 {
        let status = Rect::new(area.x, area.bottom() - 1, area.width, 1);
        let hint = locale.sidebar_reorder_hint(
            preview
                .destination
                .as_ref()
                .map(|(placement, label)| ((*placement).into(), label.as_str())),
        );
        // Paragraph paints text and styles, but does not erase an older notice's tail.
        frame.render_widget(Clear, status);
        frame.render_widget(
            Paragraph::new(truncate_display(
                &sanitize_terminal_text(&hint),
                usize::from(area.width),
                theme.glyphs.ellipsis,
            ))
            .style(emphasis),
            status,
        );
    }
}

#[cfg(test)]
mod tests;
