//! Transient feedback for a sidebar drag. The sidebar already paints the move in place, with a held
//! Workspace filled as current; this adds the bottom drop target and the localized result.
use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    InteractionState, NavigationState, OperationState, PointerPresentation, render_action,
};
use crate::{
    localization::Locale,
    terminal_text::{sanitize_terminal_text, truncate_display},
    tui_hit_map::{SidebarReorderDraw, sidebar_reorder::Placement},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Clear, Paragraph},
};

pub fn render_sidebar_reorder_preview(
    frame: &mut Frame<'_>,
    preview: &SidebarReorderDraw,
    locale: Locale,
    theme: &Theme,
) {
    let emphasis = theme
        .style(Role::SurfaceSelection)
        .patch(theme.style(Role::TextPrimary));
    // The whole footer accepts the drop, yet only one row shows it: a sidebar action, hovered
    // while the pointer is on the footer. A blank row keeps it apart from the last group when the
    // footer has room for one.
    let footer = preview.footer.intersection(frame.area());
    if !footer.is_empty() {
        let row = Rect::new(
            footer.x,
            footer.y + u16::from(footer.height > 1),
            footer.width,
            1,
        );
        let active = preview.destination.as_ref().is_some_and(|(placement, _)| {
            matches!(placement, Placement::Ungrouped | Placement::End)
        });
        let state = InteractionState::new(
            FocusState::Unfocused,
            if active {
                PointerPresentation::Hovered
            } else {
                PointerPresentation::Rest
            },
            NavigationState::None,
            CurrentState::NotCurrent,
            CheckedState::Unchecked,
            Availability::Enabled,
            OperationState::Idle,
        );
        // Paragraph paints text and styles, but does not erase the range row it may replace.
        frame.render_widget(Clear, row);
        frame
            .buffer_mut()
            .set_style(row, theme.style(Role::SurfaceChrome));
        let _ = render_action(
            frame,
            row,
            ActionSpec::new(
                locale.sidebar_drop_zone(preview.workspace),
                ActionKind::Secondary,
                ControlSurface::Chrome,
                state,
            ),
            theme,
        );
    }
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
