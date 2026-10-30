//! Lays out and paints the open pointer tooltip: the menu's frame without a heading, wholly below
//! or above its target from the pointer's column, never over its target, the pointer or the
//! floating status. The pointer layer decides when one is open; feature painters supply content.

use orchbee_workbench_protocol::MetadataState;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Clear, Padding},
};

use crate::{
    client_view::{ClientAction, ClientView, PrefixAction},
    localization::Locale,
    terminal_text::{display_width, truncate_display, truncate_display_middle},
    tui_pointer::hit_map::{LegendItem, OpenTooltip, TooltipContent, TooltipText},
    tui_theme::{Role, Theme},
};

/// Text never grows wider than this many cells.
const TEXT_WIDTH: usize = 40;
/// Text never takes more than this many lines.
const TEXT_LINES: usize = 2;
/// Narrower room says too little to be worth covering the screen.
const MINIMUM_TEXT_WIDTH: usize = 8;
/// Border and padding cells beside the text, both sides together.
const FRAME_COLUMNS: usize = 4;
/// Border rows above and below the text.
const FRAME_ROWS: usize = 2;
/// Legend items stand this far apart, with no separator glyph.
const LEGEND_GAP: &str = "  ";

/// The first key the binding table shows now for a Client action, in the theme's key glyphs.
pub(super) fn action_key(view: &ClientView, action: ClientAction, theme: &Theme) -> Option<String> {
    view.bindings()
        .action_labels_with_glyphs(
            PrefixAction::Client(action),
            super::agent_sidebar::key_label_glyphs(theme),
        )
        .into_iter()
        .next()
}

/// Each distinct Agent state in the order its target shows them, then unread when the target
/// shows the unread dot.
pub(super) fn legend(
    states: impl IntoIterator<Item = MetadataState>,
    unread: bool,
    working_frame: usize,
    locale: Locale,
    theme: &Theme,
) -> Vec<LegendItem> {
    let mut seen = Vec::new();
    let mut items = Vec::new();
    for state in states {
        if !seen.contains(&state) {
            seen.push(state);
            items.push(agent_state(state, working_frame, locale, theme));
        }
    }
    if unread {
        items.push(self::unread(locale, theme));
    }
    items
}

/// A legend tooltip, or none when its target shows no glyph that needs a word.
pub(super) fn describe(
    text: Option<TooltipText>,
    legend: Vec<LegendItem>,
) -> Option<TooltipContent> {
    (text.is_some() || !legend.is_empty()).then_some(TooltipContent::Describe { text, legend })
}

/// An Agent state glyph and its word, as the sidebar, the Tab strip and pane titles draw it.
fn agent_state(
    state: MetadataState,
    working_frame: usize,
    locale: Locale,
    theme: &Theme,
) -> LegendItem {
    LegendItem {
        glyph: super::sidebar_text::state_glyph(state, theme, working_frame).to_owned(),
        role: Theme::agent_role(state),
        style: theme.agent_style(state, working_frame),
        label: locale
            .machine_picker_text(super::sidebar_text::state_role(state).0)
            .to_owned(),
    }
}

/// The unread dot and its word.
fn unread(locale: Locale, theme: &Theme) -> LegendItem {
    LegendItem {
        glyph: theme.glyphs.status_bullet.to_owned(),
        role: Role::StatusUnread,
        style: theme.style(Role::StatusUnread),
        label: locale.sidebar_marker_label("unread").to_owned(),
    }
}

/// Paints `tooltip` on the whole screen and returns the box it covers, or nothing when it fits
/// neither below nor above its target clear of `avoid`.
pub(super) fn paint(
    frame: &mut Frame<'_>,
    tooltip: OpenTooltip<'_>,
    avoid: &[Rect],
    theme: &Theme,
) -> Option<Rect> {
    let area = frame.area();
    let room = TEXT_WIDTH.min(usize::from(area.width).saturating_sub(FRAME_COLUMNS));
    if room < MINIMUM_TEXT_WIDTH {
        return None;
    }
    let lines = lines(tooltip.content, room, theme);
    let width = lines.iter().map(Line::width).max()? + FRAME_COLUMNS;
    let size = (
        u16::try_from(width).ok()?,
        u16::try_from(lines.len() + FRAME_ROWS).ok()?,
    );
    let area_box = place(area, tooltip.target, tooltip.pointer, size, avoid)?;
    super::clear_crossing_wide_cells(frame, area_box);
    frame.render_widget(Clear, area_box);
    frame.render_widget(
        super::overlay_surface(theme, Padding::horizontal(1)),
        area_box,
    );
    let text_width = area_box.width.saturating_sub(4);
    for (row, line) in (area_box.y + 1..).zip(&lines) {
        frame
            .buffer_mut()
            .set_line(area_box.x + 2, row, line, text_width);
    }
    Some(area_box)
}

/// Wholly below the target's last row, else wholly above its first; from the pointer's column,
/// shifted left to stay on screen; clear of the target, the pointer and `avoid`.
fn place(
    area: Rect,
    target: Rect,
    pointer: (u16, u16),
    (width, height): (u16, u16),
    avoid: &[Rect],
) -> Option<Rect> {
    if width > area.width || height > area.height {
        return None;
    }
    let x = pointer.0.min(area.right() - width).max(area.x);
    let keep = [target, Rect::new(pointer.0, pointer.1, 1, 1)];
    let fits = |candidate: &Rect| {
        candidate.y >= area.y
            && candidate.bottom() <= area.bottom()
            && !keep
                .iter()
                .chain(avoid)
                .any(|kept| kept.intersects(*candidate))
    };
    [Some(target.bottom()), target.y.checked_sub(height)]
        .into_iter()
        .flatten()
        .map(|y| Rect::new(x, y, width, height))
        .find(fits)
}

fn lines(content: &TooltipContent, room: usize, theme: &Theme) -> Vec<Line<'static>> {
    let on_box = |role: Role, style: Style| {
        theme.content_on_surface(Role::SurfaceOverlay, None, Some(role), style)
    };
    match content {
        TooltipContent::Action { label, key } => {
            // The label truncates; the key stays whole.
            let key_width = key
                .as_deref()
                .map_or(0, |key| display_width(LEGEND_GAP) + display_width(key));
            let mut spans = vec![Span::styled(
                truncate_display(label, room.saturating_sub(key_width), theme.glyphs.ellipsis),
                on_box(Role::TextPrimary, theme.style(Role::TextPrimary)),
            )];
            if let Some(key) = key {
                spans.push(Span::raw(LEGEND_GAP));
                spans.push(Span::styled(
                    key.clone(),
                    on_box(Role::HintKey, theme.style(Role::HintKey)),
                ));
            }
            vec![Line::from(spans)]
        }
        TooltipContent::Describe { text, legend } => {
            let text_wanted = text.as_ref().map_or(0, |text| wrap(&text.text, room).len());
            let legend_lines = legend_lines(legend, room, usize::MAX, theme);
            // With both, the name keeps at least one line and the legend takes the rest.
            let text_count = if text_wanted == 0 {
                0
            } else {
                text_wanted.min(TEXT_LINES.saturating_sub(legend_lines.len()).max(1))
            };
            let legend_count = legend_lines.len().min(TEXT_LINES - text_count);
            let mut lines: Vec<_> = text
                .as_ref()
                .map(|text| text_lines(text, room, text_count, theme))
                .unwrap_or_default()
                .into_iter()
                .map(|line| {
                    Line::from(Span::styled(
                        line,
                        on_box(Role::TextPrimary, theme.style(Role::TextPrimary)),
                    ))
                })
                .collect();
            lines.extend(self::legend_lines(legend, room, legend_count, theme));
            lines
        }
    }
}

/// A name's lines within `count`; whatever is left out ends the last line with the ellipsis, or
/// the middle of a path.
fn text_lines(text: &TooltipText, room: usize, count: usize, theme: &Theme) -> Vec<String> {
    let strings = |lines: Vec<(usize, String)>| lines.into_iter().map(|(_, line)| line).collect();
    let lines = wrap(&text.text, room);
    if lines.len() <= count {
        return strings(lines);
    }
    let ellipsis = theme.glyphs.ellipsis;
    if text.middle {
        return (1..=room * count)
            .rev()
            .map(|budget| wrap(&truncate_display_middle(&text.text, budget, ellipsis), room))
            .find(|lines| lines.len() <= count)
            .map(strings)
            .unwrap_or_default();
    }
    let Some(last) = count.checked_sub(1) else {
        return Vec::new();
    };
    // The last line holds the rest of the source text, not a re-join of its wrapped lines.
    let rest = &text.text[lines[last].0..];
    let mut cut: Vec<String> = strings(lines).into_iter().take(last).collect();
    cut.push(truncate_display(rest, room, ellipsis));
    cut
}

/// Breaks `text` into lines of at most `room` cells by the shared prose rule. Each line keeps the
/// byte offset where it starts in `text`.
fn wrap(text: &str, room: usize) -> Vec<(usize, String)> {
    crate::terminal_text::wrap_str(text, room)
        .into_iter()
        .filter(|range| !range.is_empty())
        .map(|range| (range.start, text[range].to_owned()))
        .collect()
}

/// Legend items packed into at most `count` lines, two cells apart. When items are left out, the
/// last line drops whole items until a trailing ellipsis fits.
fn legend_lines(
    items: &[LegendItem],
    room: usize,
    count: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let on_box = |role: Role, style: Style| {
        theme.content_on_surface(Role::SurfaceOverlay, None, Some(role), style)
    };
    let group = |item: &LegendItem| {
        vec![
            Span::styled(item.glyph.clone(), on_box(item.role, item.style)),
            Span::styled(
                format!(" {}", item.label),
                on_box(Role::TextPrimary, theme.style(Role::TextPrimary)),
            ),
        ]
    };
    let join = |groups: &[Vec<Span<'static>>]| {
        let mut spans = Vec::new();
        for (index, group) in groups.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw(LEGEND_GAP));
            }
            spans.extend(group.iter().cloned());
        }
        spans
    };
    let width = |spans: &[Span<'_>]| spans.iter().map(Span::width).sum::<usize>();
    let mut packed: Vec<Vec<Vec<Span<'static>>>> = Vec::new();
    for item in items {
        let next = group(item);
        match packed.last_mut() {
            Some(last) if width(&join(last)) + display_width(LEGEND_GAP) + width(&next) <= room => {
                last.push(next);
            }
            _ => packed.push(vec![next]),
        }
    }
    let cut = packed.len() > count && count > 0;
    packed.truncate(count);
    let more = Span::styled(
        format!("{LEGEND_GAP}{}", theme.glyphs.ellipsis),
        on_box(Role::TextSecondary, theme.style(Role::TextSecondary)),
    );
    if cut && let Some(last) = packed.last_mut() {
        while last.len() > 1 && width(&join(last)) + more.width() > room {
            last.pop();
        }
    }
    let last = packed.len().saturating_sub(1);
    packed
        .iter()
        .enumerate()
        .map(|(index, groups)| {
            let mut spans = join(groups);
            if cut && index == last {
                spans.push(more.clone());
            }
            Line::from(truncate_spans(spans, room, theme))
        })
        .collect()
}

/// Cuts styled spans to `room` cells with a trailing ellipsis, never splitting a glyph span.
fn truncate_spans(spans: Vec<Span<'static>>, room: usize, theme: &Theme) -> Vec<Span<'static>> {
    if spans.iter().map(Span::width).sum::<usize>() <= room {
        return spans;
    }
    let ellipsis = theme.glyphs.ellipsis;
    let budget = room.saturating_sub(display_width(ellipsis));
    let mut kept = Vec::new();
    let mut used = 0;
    let mut style = Style::default();
    for span in spans {
        let width = span.width();
        style = span.style;
        if used + width <= budget {
            used += width;
            kept.push(span);
            continue;
        }
        let cut = truncate_display(&span.content, budget - used, "");
        if !cut.is_empty() {
            kept.push(Span::styled(cut, span.style));
        }
        break;
    }
    kept.push(Span::styled(ellipsis, style));
    kept
}

#[cfg(test)]
mod tests;
