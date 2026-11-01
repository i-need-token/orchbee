//! App-private controls own fixed cell geometry and semantic interaction styling.
//!
//! Feature owners retain semantic identity, availability reasons, and typed
//! [`crate::tui_hit_map::HitTarget`] values. This module paints admitted state and returns the
//! complete painted rectangle; it never derives an intent from product copy or glyphs.
use crate::{
    terminal_text::display_width,
    tui_theme::{Role, Theme},
};
use ratatui::{Frame, layout::Rect, style::Style};

mod action;
mod control_state;
mod hint;
mod icon;
mod query;
mod row;

pub(crate) use action::{ActionKind, ActionSpec, action_rect, close_rect, render_action};
pub(crate) use control_state::{
    Availability, CheckedState, ControlSurface, CurrentState, FocusState, InteractionState,
    NavigationState, OperationState, PointerPresentation,
};
pub(crate) use hint::{
    KeyHint, measure_key_hints, measure_overlay_hints, render_key_hints, render_overlay_hints,
};
pub(crate) use icon::{
    IconActionGlyph, IconActionSpec, frame_close_rect, icon_action_rect, icon_action_style,
    render_frame_icon_action, render_icon_action,
};
pub(crate) use query::{QueryField, render_divider, render_query_field};
pub(crate) use row::{RowContentBand, RowFocusCue, RowTracks, paint_row_interaction};

/// Applies pointer feedback to one already-painted semantic child target.
///
/// The owner first paints the target's rest or disabled content and any enclosing row interaction.
/// Rest and disabled calls validate geometry but leave that composition intact. Hover and press
/// keep the symbols, restore the declared semantic role, and apply the child interaction over its
/// exact extent. Success returns that same extent for typed hit publication.
pub(crate) fn paint_pointer_target(
    frame: &mut Frame<'_>,
    rect: Rect,
    surface: ControlSurface,
    pointer: PointerPresentation,
    role: Role,
    availability: Availability,
    theme: &Theme,
) -> Option<Rect> {
    if rect.is_empty() || frame.area().intersection(rect) != rect || !is_content_role(role) {
        return None;
    }
    if availability == Availability::Disabled || pointer == PointerPresentation::Rest {
        return Some(rect);
    }
    let interaction = match pointer {
        PointerPresentation::Hovered => Role::SurfaceHover,
        PointerPresentation::Pressed => Role::SurfacePressed,
        PointerPresentation::Rest => unreachable!("rest returned before composition"),
    };
    let style = content_style(
        theme,
        surface,
        Some(interaction),
        Some(role),
        theme.style(role),
    );
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            let cell = &mut frame.buffer_mut()[(x, y)];
            cell.set_style(Style::reset()).set_style(style);
        }
    }
    Some(rect)
}

fn content_style(
    theme: &Theme,
    surface: ControlSurface,
    interaction: Option<Role>,
    role: Option<Role>,
    content: Style,
) -> Style {
    theme.content_on_surface(surface.role(), interaction, role, content)
}

fn is_content_role(role: Role) -> bool {
    match role {
        Role::TextPrimary
        | Role::TextSecondary
        | Role::TextDisabled
        | Role::TextInverse
        | Role::AccentAction
        | Role::AccentDestructive
        | Role::AccentFocus
        | Role::AccentSelection
        | Role::StatusSuccess
        | Role::StatusWarning
        | Role::StatusError
        | Role::StatusInfo
        | Role::StatusPending
        | Role::StatusWorking
        | Role::StatusUnread
        | Role::HintKey
        | Role::HintLabel => true,
        #[cfg(any(test, feature = "test-support"))]
        Role::AccentLink => true,
        _ => false,
    }
}

fn pad_to_width(value: &str, width: usize) -> String {
    let mut value = value.to_owned();
    value.push_str(&" ".repeat(width.saturating_sub(display_width(&value))));
    value
}

pub(super) fn mode_bar(
    viewport: ratatui::layout::Rect,
    sidebar: crate::client_view::SidebarSurface,
) -> ratatui::layout::Rect {
    let areas = crate::tui_layout::project_shell_areas(viewport, sidebar);
    if areas.tabs.is_empty() {
        ratatui::layout::Rect::new(
            areas.workbench.x,
            areas.workbench.y,
            areas.workbench.width,
            areas.workbench.height.min(1),
        )
    } else {
        areas.tabs
    }
}

#[cfg(test)]
mod tests;
