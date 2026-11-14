//! The Find bar over the focused Panel's first row and the highlights of its matches. Painting
//! and hit recording only: the query, the matches and every effect stay with the Find owner.
use crate::{
    localization::Locale,
    terminal_find::{FindBar, FindControl, FindCounter, FindToken},
    terminal_text::{display_width, truncate_display},
    tui_hit_map::{HitLayer, HitMapBuilder, HitTarget, InteractionAvailability, ProjectedPointer},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    widgets::Clear,
};

use super::controls::{
    Availability, ControlSurface, IconActionGlyph, IconActionSpec, PointerPresentation,
};

const MAXIMUM_WIDTH: u16 = 48;
/// A Panel at least this wide shows `Aa`; a narrower bar spans the whole Panel row.
const MATCH_CASE_COLUMNS: u16 = 40;
/// A Panel at least this wide shows previous and next.
const STEP_COLUMNS: u16 = 28;

/// Highlights the matches of the open bar inside a painted Panel view whose top row is the
/// absolute row `origin` and whose first column is `first_column`: the active match takes the
/// selection role, other visible matches a weaker one.
pub(super) fn paint_highlights(
    frame: &mut Frame<'_>,
    area: Rect,
    origin: u64,
    first_column: u16,
    bar: &FindBar,
    theme: &Theme,
) {
    let Some(matches) = bar.matches() else {
        return;
    };
    let bottom = origin.saturating_add(u64::from(area.height));
    for (index, range) in matches.ranges.iter().enumerate() {
        let first = u64::from(range.start.row);
        let last = u64::from(range.end.row);
        if last < origin || first >= bottom {
            continue;
        }
        let role = if index == matches.active {
            Role::SurfaceSelection
        } else {
            Role::SurfaceRelated
        };
        super::selection::paint_range(
            frame,
            area,
            origin,
            first_column,
            (first, range.start.col),
            (last, range.end.col),
            theme.style(role),
        );
    }
}

/// Paints the bar right-aligned over the first row of `area`, records its controls and the
/// searched `content` for the pointer, and returns where the terminal cursor belongs in its draft.
/// The label, the draft and the counter always show; a Panel under 40 columns drops `Aa`, and
/// one under 28 columns also drops previous and next.
#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    content: Rect,
    bar: &FindBar,
    token: FindToken,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Position> {
    hits.record(
        HitLayer::ChromeSurface,
        content,
        HitTarget::FindContent(token),
    );
    let (width, margin) = if area.width >= MATCH_CASE_COLUMNS {
        (MAXIMUM_WIDTH.min(area.width - 2), 1)
    } else {
        (area.width, 0)
    };
    if width < 4 || area.height == 0 {
        return None;
    }
    let rect = Rect::new(area.right() - width - margin, area.y, width, 1);
    frame.render_widget(Clear, rect);
    frame
        .buffer_mut()
        .set_style(rect, theme.style(Role::SurfaceOverlay));
    let view = bar.view();
    let inner = Rect::new(rect.x + 1, rect.y, rect.width - 2, 1);
    let label = locale.find_bar_label();
    let label_width = u16::try_from(display_width(label)).ok()?;
    let counter = match view.counter {
        FindCounter::Blank => String::new(),
        FindCounter::Searching => theme.glyphs.ellipsis.to_owned(),
        FindCounter::Position { current, total } => format!("{current}/{total}"),
    };
    let counter_width = u16::try_from(display_width(&counter)).ok()?;
    let controls = Controls::for_columns(area.width);
    let mut right = inner.right();
    if inner.width >= label_width + 2 + controls.width() {
        right -= controls.width();
        paint_controls(
            frame,
            Rect::new(right, inner.y, controls.width(), 1),
            controls,
            (view.match_case, view.can_step),
            token,
            theme,
            hits,
        );
        right -= 1;
    }
    let buffer = frame.buffer_mut();
    buffer.set_stringn(
        inner.x,
        inner.y,
        label,
        usize::from(inner.width),
        theme.style(Role::TextSecondary),
    );
    let fixed = label_width + 1 + 1 + counter_width;
    if counter_width > 0 && right >= inner.x + fixed {
        right -= counter_width;
        buffer.set_string(right, inner.y, &counter, theme.style(Role::TextSecondary));
        right -= 1;
    }
    let draft_x = inner.x + label_width + 1;
    let columns = usize::from(right.saturating_sub(draft_x));
    let cursor = (columns > 0).then(|| {
        let (draft, caret) = draft_window(view.draft, view.cursor, columns.saturating_sub(1));
        let painted = buffer.set_stringn(
            draft_x,
            inner.y,
            &draft,
            columns,
            theme.style(Role::TextPrimary),
        );
        if view.selected {
            let selected = painted.0.saturating_sub(draft_x);
            buffer.set_style(
                Rect::new(draft_x, inner.y, selected, 1),
                theme.style(Role::SurfaceSelection),
            );
        }
        Position::new(draft_x + caret, inner.y)
    });
    hits.record(HitLayer::Chrome, rect, HitTarget::FindSurface(token));
    cursor
}

/// The controls a Panel of a given width shows; close always shows.
#[derive(Clone, Copy)]
struct Controls {
    match_case: bool,
    step: bool,
}

impl Controls {
    const fn for_columns(columns: u16) -> Self {
        Self {
            match_case: columns >= MATCH_CASE_COLUMNS,
            step: columns >= STEP_COLUMNS,
        }
    }

    /// `Aa`, `[^]`, `[v]` and `[x]`, one cell apart.
    const fn width(self) -> u16 {
        let mut width = 3;
        if self.step {
            width += 8;
        }
        if self.match_case {
            width += 3;
        }
        width
    }
}

/// `Aa`, previous, next and close, each acting on its plain left release. `state` is the Match
/// case choice and whether there are matches to step through.
fn paint_controls(
    frame: &mut Frame<'_>,
    area: Rect,
    controls: Controls,
    state: (bool, bool),
    token: FindToken,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let (match_case, can_step) = state;
    let mut x = area.x;
    if controls.match_case {
        let rect = Rect::new(x, area.y, 2, 1);
        let target = HitTarget::FindControl {
            token,
            control: FindControl::MatchCase,
        };
        let pointer = presentation(hits.projected_modal_pointer(
            HitLayer::Chrome,
            rect,
            &target,
            InteractionAvailability::Enabled,
        ));
        frame.buffer_mut().set_string(
            rect.x,
            rect.y,
            "Aa",
            theme.style(if match_case {
                Role::SurfaceSelection
            } else {
                Role::TextMuted
            }),
        );
        let role = if match_case {
            Role::AccentSelection
        } else {
            Role::TextMuted
        };
        if super::controls::paint_pointer_target(
            frame,
            rect,
            ControlSurface::Overlay,
            pointer,
            role,
            Availability::Enabled,
            theme,
        ) == Some(rect)
        {
            hits.record_modal_control(
                HitLayer::Chrome,
                rect,
                target,
                InteractionAvailability::Enabled,
            );
        }
        x += 3;
    }
    let mut icons = Vec::with_capacity(3);
    if controls.step {
        icons.push((IconActionGlyph::Up, FindControl::Previous, can_step));
        icons.push((IconActionGlyph::Down, FindControl::Next, can_step));
    }
    icons.push((IconActionGlyph::Close, FindControl::Close, true));
    for (glyph, control, enabled) in icons {
        let slot = Rect::new(x, area.y, 3, 1);
        x += 4;
        let Some(rect) = super::controls::icon_action_rect(frame.area(), slot) else {
            continue;
        };
        let target = HitTarget::FindControl { token, control };
        let (availability, value) = if enabled {
            (InteractionAvailability::Enabled, Availability::Enabled)
        } else {
            (InteractionAvailability::Disabled, Availability::Disabled)
        };
        let pointer = presentation(hits.projected_modal_pointer(
            HitLayer::Chrome,
            rect,
            &target,
            availability,
        ));
        let painted = super::controls::render_icon_action(
            frame,
            slot,
            IconActionSpec::new(glyph, ControlSurface::Overlay, value, pointer),
            theme,
        );
        if painted == Some(rect) {
            hits.record_modal_control(HitLayer::Chrome, rect, target, availability);
        }
    }
}

const fn presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
}

/// The part of the draft that fits `columns`, scrolled to keep the caret visible, and the caret's
/// column inside it.
fn draft_window(draft: &str, cursor: usize, columns: usize) -> (String, u16) {
    use unicode_segmentation::UnicodeSegmentation;
    let before = draft.get(..cursor).unwrap_or(draft);
    let mut start = 0;
    let mut width = display_width(before);
    for (offset, grapheme) in before.grapheme_indices(true) {
        if width <= columns {
            break;
        }
        width = width.saturating_sub(display_width(grapheme));
        start = offset + grapheme.len();
    }
    (
        truncate_display(&draft[start..], columns.max(1), ""),
        u16::try_from(width).unwrap_or(u16::MAX),
    )
}

#[cfg(test)]
mod tests;
