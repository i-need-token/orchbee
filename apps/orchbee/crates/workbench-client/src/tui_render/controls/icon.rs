//! Fixed three-cell icon actions for space-constrained, discoverable chrome, and their five-cell
//! touch size with a blank cell inside each bracket.

use crate::tui_theme::{GlyphSet, Role, Theme};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::Paragraph,
};

use super::{Availability, ControlSurface, PointerPresentation, content_style};

const ICON_ACTION_WIDTH: u16 = 3;
pub(crate) const TOUCH_ICON_ACTION_WIDTH: u16 = 5;

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
    /// The sidebar toggle while the sidebar is visible.
    HideSidebar,
    /// The sidebar toggle while the sidebar is hidden.
    ShowSidebar,
}

impl IconActionGlyph {
    /// The one cell between the brackets; only the sidebar toggle takes a non-ASCII glyph, so it
    /// follows the host's glyph set.
    fn glyph(self, glyphs: GlyphSet) -> &'static str {
        match self {
            Self::Close => "x",
            Self::Previous => "<",
            Self::Next => ">",
            Self::Up => "^",
            Self::Down => "v",
            Self::Help => "?",
            Self::Add => "+",
            Self::HideSidebar => glyphs.sidebar_hide,
            Self::ShowSidebar => glyphs.sidebar_show,
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
    sized_rect(bounds, area, ICON_ACTION_WIDTH)
}

/// Resolves the complete five-cell touch extent at the offered area's leading edge.
pub(crate) fn touch_icon_action_rect(bounds: Rect, area: Rect) -> Option<Rect> {
    sized_rect(bounds, area, TOUCH_ICON_ACTION_WIDTH)
}

fn sized_rect(bounds: Rect, area: Rect, width: u16) -> Option<Rect> {
    if area.height == 0 || area.width < width {
        return None;
    }
    let _right = area.x.checked_add(width)?;
    let _bottom = area.y.checked_add(1)?;
    let rect = Rect::new(area.x, area.y, width, 1);
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

/// The style of an icon action's glyph, shared with unbracketed glyph controls such as the Tab
/// strip's edge markers.
pub(crate) fn icon_action_style(
    theme: &Theme,
    surface: ControlSurface,
    availability: Availability,
    pointer: PointerPresentation,
) -> Style {
    let pointer = if availability == Availability::Enabled {
        pointer
    } else {
        PointerPresentation::Rest
    };
    // A glyph this small reads its hover through a stronger text role rather than a fill block;
    // press adds weight and the pressed surface so release feedback stays distinct from hover.
    let (role, interaction) = match (availability, pointer) {
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
    content_style(theme, surface, interaction, Some(role), content)
}

/// Paints one fixed icon action and returns its complete visible and clickable extent.
pub(crate) fn render_icon_action(
    frame: &mut Frame<'_>,
    area: Rect,
    spec: IconActionSpec,
    theme: &Theme,
) -> Option<Rect> {
    let rect = icon_action_rect(frame.area(), area)?;
    paint_icon(
        frame,
        rect,
        ["[", spec.glyph.glyph(theme.glyphs), "]"],
        spec,
        theme,
    );
    Some(rect)
}

/// Paints one touch-sized icon action, `[ g ]`, and returns its five painted cells. Its owner
/// decides the larger target around them.
pub(crate) fn render_touch_icon_action(
    frame: &mut Frame<'_>,
    area: Rect,
    spec: IconActionSpec,
    theme: &Theme,
) -> Option<Rect> {
    let rect = touch_icon_action_rect(frame.area(), area)?;
    paint_icon(
        frame,
        rect,
        ["[ ", spec.glyph.glyph(theme.glyphs), " ]"],
        spec,
        theme,
    );
    Some(rect)
}

fn paint_icon(
    frame: &mut Frame<'_>,
    rect: Rect,
    parts: [&str; 3],
    spec: IconActionSpec,
    theme: &Theme,
) {
    let style = icon_action_style(theme, spec.surface, spec.availability, spec.pointer);
    frame.render_widget(Paragraph::new(Line::from_iter(parts)), rect);
    for x in rect.x..rect.right() {
        frame.buffer_mut()[(x, rect.y)]
            .set_style(Style::reset())
            .set_style(style);
    }
}

#[cfg(test)]
mod tests;
