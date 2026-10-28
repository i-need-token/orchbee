//! Fixed three-cell icon actions for space-constrained, discoverable chrome.

use crate::tui_theme::{Role, Theme};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
};

use super::{Availability, ControlSurface, PointerPresentation, content_style};

const ICON_ACTION_WIDTH: u16 = 3;

/// Closed visual grammar for icon actions; semantic intent remains with the owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IconActionGlyph {
    Close,
    Previous,
    Next,
    Up,
    Down,
    Help,
    Add,
}

impl IconActionGlyph {
    fn label(self) -> &'static str {
        match self {
            Self::Close => "[x]",
            Self::Previous => "[<]",
            Self::Next => "[>]",
            Self::Up => "[^]",
            Self::Down => "[v]",
            Self::Help => "[?]",
            Self::Add => "[+]",
        }
    }
}

/// Owner-supplied presentation for one fixed icon action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IconActionSpec {
    glyph: IconActionGlyph,
    surface: ControlSurface,
    availability: Availability,
    pointer: PointerPresentation,
}

impl IconActionSpec {
    pub(crate) const fn new(
        glyph: IconActionGlyph,
        surface: ControlSurface,
        availability: Availability,
        pointer: PointerPresentation,
    ) -> Self {
        Self {
            glyph,
            surface,
            availability,
            pointer,
        }
    }
}

/// Resolves the complete three-cell extent at the offered area's leading edge.
///
/// Renderers use this pure result for pointer projection before paint. There is no compact
/// fallback: a clipped bracketed action is not an action.
pub(crate) fn icon_action_rect(bounds: Rect, area: Rect) -> Option<Rect> {
    if area.height == 0 || area.width < ICON_ACTION_WIDTH {
        return None;
    }
    let _right = area.x.checked_add(ICON_ACTION_WIDTH)?;
    let _bottom = area.y.checked_add(1)?;
    let rect = Rect::new(area.x, area.y, ICON_ACTION_WIDTH, 1);
    (area.intersection(rect) == rect && bounds.intersection(rect) == rect).then_some(rect)
}

/// Resolves the close control a framed surface carries on its top edge as `─ [x] ─┐`.
///
/// One blank cell separates the glyph from the rule on each side, and one rule cell remains before
/// the corner. A frame too narrow for that complete arrangement has no close geometry.
pub(crate) fn frame_close_rect(bounds: Rect, area: Rect) -> Option<Rect> {
    // Corner, rule and blank on each side of the three-cell glyph.
    if area.height == 0 || area.width < ICON_ACTION_WIDTH + 6 {
        return None;
    }
    let rect = Rect::new(
        area.right().checked_sub(ICON_ACTION_WIDTH + 3)?,
        area.y,
        ICON_ACTION_WIDTH,
        1,
    );
    icon_action_rect(bounds, rect).filter(|rect| area.intersection(*rect) == *rect)
}

/// Paints a frame-borne icon action with one blank surface cell on each side of its glyph.
///
/// The blanks separate the action from the frame rule; they are not part of its hit extent.
pub(crate) fn render_frame_icon_action(
    frame: &mut Frame<'_>,
    area: Rect,
    spec: IconActionSpec,
    theme: &Theme,
) -> Option<Rect> {
    let rect = icon_action_rect(frame.area(), area)?;
    let gaps = [rect.x.checked_sub(1)?, rect.right()];
    let bounds = frame.area();
    if gaps.iter().any(|x| *x < bounds.x || *x >= bounds.right()) {
        return None;
    }
    let style = theme.style(spec.surface.role());
    for x in gaps {
        frame.buffer_mut()[(x, rect.y)]
            .set_symbol(" ")
            .set_style(Style::reset())
            .set_style(style);
    }
    render_icon_action(frame, rect, spec, theme)
}

/// Paints one fixed icon action and returns its complete visible and clickable extent.
pub(crate) fn render_icon_action(
    frame: &mut Frame<'_>,
    area: Rect,
    spec: IconActionSpec,
    theme: &Theme,
) -> Option<Rect> {
    let rect = icon_action_rect(frame.area(), area)?;
    let pointer = if spec.availability == Availability::Enabled {
        spec.pointer
    } else {
        PointerPresentation::Rest
    };
    // A glyph this small reads its hover through a stronger text role rather than a fill block;
    // press adds weight and the pressed surface so release feedback stays distinct from hover.
    let (role, interaction) = match (spec.availability, pointer) {
        (Availability::Disabled, _) => (Role::TextDisabled, None),
        (Availability::Enabled, PointerPresentation::Rest) => (Role::TextSecondary, None),
        (Availability::Enabled, PointerPresentation::Hovered) => (Role::TextPrimary, None),
        (Availability::Enabled, PointerPresentation::Pressed) => {
            (Role::TextPrimary, Some(Role::SurfacePressed))
        }
    };
    let mut content = theme.style(role);
    if pointer == PointerPresentation::Pressed {
        content = content.add_modifier(Modifier::BOLD);
    }
    let style = content_style(theme, spec.surface, interaction, Some(role), content);
    frame.render_widget(Paragraph::new(spec.glyph.label()), rect);
    for x in rect.x..rect.right() {
        frame.buffer_mut()[(x, rect.y)]
            .set_style(Style::reset())
            .set_style(style);
    }
    Some(rect)
}

#[cfg(test)]
mod tests;
