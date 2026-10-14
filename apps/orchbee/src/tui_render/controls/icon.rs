//! Fixed three-cell icon actions for space-constrained, discoverable chrome.

use crate::tui_theme::{Role, Theme};
use ratatui::{Frame, layout::Rect, style::Style, widgets::Paragraph};

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
    const fn label(self) -> &'static str {
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

/// Paints one fixed icon action and returns its complete visible and clickable extent.
pub(crate) fn render_icon_action(
    frame: &mut Frame<'_>,
    area: Rect,
    spec: IconActionSpec,
    theme: &Theme,
) -> Option<Rect> {
    let rect = icon_action_rect(frame.area(), area)?;
    let enabled = spec.availability == Availability::Enabled;
    let role = if enabled {
        Role::TextSecondary
    } else {
        Role::TextDisabled
    };
    let interaction = if enabled {
        match spec.pointer {
            PointerPresentation::Rest => None,
            PointerPresentation::Hovered => Some(Role::SurfaceHover),
            PointerPresentation::Pressed => Some(Role::SurfacePressed),
        }
    } else {
        None
    };
    let style = content_style(
        theme,
        spec.surface,
        interaction,
        Some(role),
        theme.style(role),
    );
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
