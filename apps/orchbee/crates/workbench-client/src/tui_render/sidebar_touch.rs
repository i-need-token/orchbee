//! Touch spacing for the Mobile tier's lists and bar: the fills that sit centered on their rows,
//! and the fixed actions that own their rows' blank lines.
//!
//! A spaced list leaves a blank line beside every row and fixed action. A fill covers only its
//! text lines; each blank line beside it shows half of it as an upper or lower half block, so the
//! fill reads as centered on the row and its blank line. A blank line between two fills of one
//! color takes that color whole. Hosts whose fills are not concrete colors draw no halves, so
//! their fills keep to the text lines.

use ratatui::{Frame, layout::Rect, style::Color};

use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    InteractionState, NavigationState, OperationState, PointerPresentation, action_rect,
    render_action,
};
use super::sidebar_chrome::TouchRow;
use crate::tui_hit_map::{
    HitLayer, HitMapBuilder, InteractionSpec, InteractionTarget, PrimaryContract,
};
use crate::tui_theme::{Role, Theme};

/// One fixed action at touch size: `[ Label ]` on `place.row`, owning every line of `place` from
/// the row's start to the column its owner passes when painting it. Hover and press fill the
/// whole target on the content row, key included.
pub(super) struct TouchAction<'a> {
    pub(super) label: &'a str,
    pub(super) kind: ActionKind,
    pub(super) enabled: bool,
    pub(super) target: InteractionTarget,
    pub(super) place: TouchRow,
}

pub(super) struct Painted {
    pub(super) action: Rect,
    pub(super) fill: Option<Role>,
}

fn rest_state(enabled: bool) -> InteractionState {
    InteractionState::new(
        FocusState::Unfocused,
        PointerPresentation::Rest,
        NavigationState::None,
        CurrentState::NotCurrent,
        CheckedState::Unchecked,
        if enabled {
            Availability::Enabled
        } else {
            Availability::Disabled
        },
        OperationState::Idle,
    )
}

/// Resolves the action's painted extent before anything paints, so its trailing icon action can
/// decide whether it fits beside it.
pub(super) fn resolve(frame: &Frame<'_>, action: &TouchAction<'_>, theme: &Theme) -> Option<Rect> {
    let rest = ActionSpec::new(
        action.label,
        action.kind,
        ControlSurface::Chrome,
        rest_state(action.enabled),
    );
    action_rect(frame.area(), action.place.row, rest, theme)
}

pub(super) fn paint_action(
    frame: &mut Frame<'_>,
    action: TouchAction<'_>,
    end: u16,
    theme: &Theme,
    layer: HitLayer,
    halos: &mut Halos,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Painted> {
    let TouchAction {
        label,
        kind,
        enabled,
        target,
        place,
    } = action;
    let row = place.row;
    let hit = Rect::new(row.x, place.top, end.saturating_sub(row.x), place.height);
    if hit.is_empty() || frame.area().intersection(hit) != hit {
        return None;
    }
    let interaction = if enabled {
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
    } else {
        InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
    };
    let pointer = hits.as_deref().map_or(PointerPresentation::Rest, |hits| {
        super::tabs::pointer_presentation(hits.projected_pointer(
            layer,
            hit,
            hit,
            &target,
            interaction,
        ))
    });
    let pointer = if enabled {
        pointer
    } else {
        PointerPresentation::Rest
    };
    let fill = match pointer {
        PointerPresentation::Rest => None,
        PointerPresentation::Hovered => Some(Role::SurfaceHover),
        PointerPresentation::Pressed => Some(Role::SurfacePressed),
    };
    let band = Rect::new(hit.x, row.y, hit.width, 1);
    if let Some(role) = fill {
        frame.buffer_mut().set_style(
            band,
            theme.content_on_surface(
                Role::SurfaceChrome,
                Some(role),
                None,
                theme.style(Role::SurfaceChrome),
            ),
        );
    }
    halos.fill(theme, band, fill);
    let painted = render_action(
        frame,
        row,
        ActionSpec::new(
            label,
            kind,
            ControlSurface::Chrome,
            rest_state(enabled).with_pointer(pointer),
        ),
        theme,
    )?;
    if let Some(hits) = hits {
        hits.record_interaction(layer, hit, hit, target, interaction);
    }
    Some(Painted {
        action: painted,
        fill,
    })
}

/// The fills and text lines one spaced list painted, for the halves its blank lines show.
pub(super) struct Halos {
    area: Rect,
    bands: Vec<(Rect, Color)>,
    text: Vec<bool>,
}

impl Halos {
    pub(super) fn new(area: Rect) -> Self {
        Self {
            area,
            bands: Vec::new(),
            text: vec![false; usize::from(area.height)],
        }
    }

    /// Marks every line of `rect` as text, which never shows a half.
    pub(super) fn text(&mut self, rect: Rect) {
        for y in rect.y..rect.bottom() {
            if let Some(line) = y
                .checked_sub(self.area.y)
                .and_then(|line| self.text.get_mut(usize::from(line)))
            {
                *line = true;
            }
        }
    }

    /// Records the fill `role` painted across `rect`, whose lines are text.
    pub(super) fn fill(&mut self, theme: &Theme, rect: Rect, role: Option<Role>) {
        self.text(rect);
        if let Some(color) = role.and_then(|role| theme.half_block_fill(role)) {
            self.bands.push((rect, color));
        }
    }

    fn color_at(&self, x: u16, y: Option<u16>) -> Option<Color> {
        let y = y?;
        self.bands
            .iter()
            .find(|(rect, _)| rect.contains((x, y).into()))
            .map(|(_, color)| *color)
    }

    /// Paints the halves into blank cells of the blank lines next to each recorded fill.
    pub(super) fn paint(&self, frame: &mut Frame<'_>, theme: &Theme) {
        let Some((upper, lower)) = theme.glyphs.half_blocks else {
            return;
        };
        if self.bands.is_empty() {
            return;
        }
        let area = self.area.intersection(frame.area());
        for y in area.y..area.bottom() {
            if self.text[usize::from(y - self.area.y)] {
                continue;
            }
            let above = y.checked_sub(1).filter(|above| *above >= area.y);
            let below = y.checked_add(1).filter(|below| *below < area.bottom());
            for x in area.x..area.right() {
                let up = self.color_at(x, above);
                let down = self.color_at(x, below);
                if up.is_none() && down.is_none() {
                    continue;
                }
                let cell = &mut frame.buffer_mut()[(x, y)];
                if !cell.symbol().trim().is_empty() {
                    continue;
                }
                match (up, down) {
                    (Some(up), Some(down)) if up == down => {
                        cell.set_symbol(" ").set_bg(up);
                    }
                    (Some(up), down) => {
                        cell.set_symbol(upper).set_fg(up);
                        if let Some(down) = down {
                            cell.set_bg(down);
                        }
                    }
                    (None, Some(down)) => {
                        cell.set_symbol(lower).set_fg(down);
                    }
                    (None, None) => {}
                }
            }
        }
    }
}
