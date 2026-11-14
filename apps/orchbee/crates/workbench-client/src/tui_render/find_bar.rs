//! The Find bar over the focused Panel's first row and the highlights of its matches. Painting
//! only: the query, the matches and every effect stay with the Find owner.
use crate::{
    localization::Locale,
    terminal_find::{FindBar, FindCounter},
    terminal_text::{display_width, truncate_display},
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
/// The draft keeps at least this many columns before the controls give way.
const MINIMUM_DRAFT: usize = 8;
/// `Aa`, then `[^]`, `[v]` and `[x]`, one cell apart.
const CONTROLS_WIDTH: u16 = 2 + 1 + 3 + 1 + 3 + 1 + 3;

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

/// Paints the bar right-aligned over the first row of `area` and returns where the terminal
/// cursor belongs in its draft. The full bar shows when it fits; otherwise the label, the draft
/// and the counter.
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    bar: &FindBar,
    locale: Locale,
    theme: &Theme,
) -> Option<Position> {
    let width = MAXIMUM_WIDTH.min(area.width.saturating_sub(2));
    if width < 4 || area.height == 0 {
        return None;
    }
    let rect = Rect::new(area.right() - width - 1, area.y, width, 1);
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
    let fixed = label_width + 1 + 1 + counter_width;
    let full = usize::from(inner.width) >= usize::from(fixed + 1 + CONTROLS_WIDTH) + MINIMUM_DRAFT;
    let mut right = inner.right();
    if full {
        right -= CONTROLS_WIDTH;
        paint_controls(
            frame,
            Rect::new(right, inner.y, CONTROLS_WIDTH, 1),
            view.match_case,
            view.can_step,
            theme,
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
    if counter_width > 0 && right >= inner.x + fixed {
        right -= counter_width;
        buffer.set_string(right, inner.y, &counter, theme.style(Role::TextSecondary));
        right -= 1;
    }
    let draft_x = inner.x + label_width + 1;
    let columns = usize::from(right.saturating_sub(draft_x));
    if columns == 0 {
        return None;
    }
    let (draft, caret) = draft_window(view.draft, view.cursor, columns.saturating_sub(1));
    let draft_role = if view.selected {
        Role::SurfaceSelection
    } else {
        Role::TextPrimary
    };
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
            theme.style(draft_role),
        );
    }
    Some(Position::new(draft_x + caret, inner.y))
}

/// `Aa`, previous, next and close. They are drawn without pointer routing; their keys act.
fn paint_controls(
    frame: &mut Frame<'_>,
    area: Rect,
    match_case: bool,
    can_step: bool,
    theme: &Theme,
) {
    frame.buffer_mut().set_string(
        area.x,
        area.y,
        "Aa",
        theme.style(if match_case {
            Role::SurfaceSelection
        } else {
            Role::TextMuted
        }),
    );
    let step = if can_step {
        Availability::Enabled
    } else {
        Availability::Disabled
    };
    for (offset, glyph, availability) in [
        (3, IconActionGlyph::Up, step),
        (7, IconActionGlyph::Down, step),
        (11, IconActionGlyph::Close, Availability::Enabled),
    ] {
        super::controls::render_icon_action(
            frame,
            Rect::new(area.x + offset, area.y, 3, 1),
            IconActionSpec::new(
                glyph,
                ControlSurface::Overlay,
                availability,
                PointerPresentation::Rest,
            ),
            theme,
        );
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
