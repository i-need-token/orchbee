use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_theme::{Role, Theme},
};

#[derive(Clone)]
pub(super) struct TabDisplay<'a> {
    pub(super) position: usize,
    pub(super) active: bool,
    pub(super) label: std::borrow::Cow<'a, str>,
}

/// Paints the compact tab strip and reports each painted token's
/// `(tab index, x offset, width)` to `on_token` in the same pass, so a
/// recorded hit region can never disagree with what was drawn.
pub(super) fn render_compact<'a>(
    frame: &mut Frame<'_>,
    area: Rect,
    tab_count: usize,
    active_position: Option<usize>,
    tab_at: impl Fn(usize) -> Option<TabDisplay<'a>>,
    theme: &Theme,
    on_token: &mut impl FnMut(usize, usize, usize),
) {
    let line = compact_line(
        usize::from(area.width),
        tab_count,
        active_position,
        &tab_at,
        theme,
        on_token,
    );
    frame.render_widget(Paragraph::new(line), area);
}

/// Paints the navigator's tab column and reports each painted row's
/// `(tab index, row offset)` to `on_row` in the same pass.
pub(super) fn render_navigator<'a>(
    frame: &mut Frame<'_>,
    area: Rect,
    tab_count: usize,
    active_position: Option<usize>,
    tab_at: impl Fn(usize) -> Option<TabDisplay<'a>>,
    theme: &Theme,
    on_row: &mut impl FnMut(usize, u16),
) {
    let lines = navigator_lines(
        usize::from(area.width),
        usize::from(area.height),
        tab_count,
        active_position,
        &tab_at,
        theme,
        on_row,
    );
    frame.render_widget(Paragraph::new(lines), area);
}

fn compact_line<'a>(
    width: usize,
    tab_count: usize,
    active_position: Option<usize>,
    tab_at: &impl Fn(usize) -> Option<TabDisplay<'a>>,
    theme: &Theme,
    on_token: &mut impl FnMut(usize, usize, usize),
) -> Line<'static> {
    if width == 0 || tab_count == 0 {
        return Line::default();
    }

    let start = compact_window_start(width, tab_count, active_position, tab_at);
    let mut spans = Vec::new();
    let mut used = 0_usize;
    for index in start..tab_count {
        let Some(tab) = tab_at(index) else {
            break;
        };
        if !spans.is_empty() {
            if used >= width {
                break;
            }
            spans.push(Span::raw(" "));
            used = used.saturating_add(1);
        }
        let remaining = width.saturating_sub(used);
        let Some((token, token_width)) = compact_token(&tab, remaining, theme) else {
            break;
        };
        let token_x = used;
        spans.push(Span::styled(token, compact_style(tab.active, theme)));
        on_token(index, token_x, token_width);
        used = used.saturating_add(token_width);
        if used >= width {
            break;
        }
    }
    Line::from(spans)
}

fn compact_window_start<'a>(
    width: usize,
    tab_count: usize,
    active_position: Option<usize>,
    tab_at: &impl Fn(usize) -> Option<TabDisplay<'a>>,
) -> usize {
    let Some(active_index) = active_position
        .and_then(|position| position.checked_sub(1))
        .filter(|index| *index < tab_count)
    else {
        return 0;
    };
    let Some(active) = tab_at(active_index) else {
        return 0;
    };
    let active_width = compact_token_width(&active);
    if active_width > width {
        return active_index;
    }

    let mut start = active_index;
    let mut occupied = active_width;
    while start > 0 {
        let Some(previous) = tab_at(start - 1) else {
            break;
        };
        let additional = compact_token_width(&previous).saturating_add(1);
        if additional > width.saturating_sub(occupied) {
            break;
        }
        start -= 1;
        occupied = occupied.saturating_add(additional);
    }
    start
}

fn navigator_lines<'a>(
    width: usize,
    height: usize,
    tab_count: usize,
    active_position: Option<usize>,
    tab_at: &impl Fn(usize) -> Option<TabDisplay<'a>>,
    theme: &Theme,
    on_row: &mut impl FnMut(usize, u16),
) -> Vec<Line<'static>> {
    if height == 0 || width == 0 || tab_count == 0 {
        return Vec::new();
    }
    // A clipped window always keeps its last row for the current/total
    // position so hidden entries are presented as clipped, not as absent.
    let item_rows = if tab_count > height {
        height.saturating_sub(1)
    } else {
        height
    };
    let active_index = active_position
        .and_then(|position| position.checked_sub(1))
        .filter(|index| *index < tab_count);
    let start = active_index
        .map(|index| index.saturating_sub(item_rows.saturating_sub(1)))
        .unwrap_or(0)
        .min(tab_count.saturating_sub(item_rows));
    let end = start.saturating_add(item_rows).min(tab_count);

    let mut lines = Vec::new();
    for (offset, index) in (start..end).enumerate() {
        let Some(tab) = tab_at(index) else {
            break;
        };
        lines.push(Line::from(Span::styled(
            navigator_row(&tab, width, theme),
            navigator_style(tab.active, theme),
        )));
        on_row(index, u16::try_from(offset).unwrap_or(u16::MAX));
    }
    if tab_count > height {
        let current = active_index.map_or(end, |index| index.saturating_add(1));
        lines.push(Line::styled(
            format!("{current}/{tab_count}"),
            theme.style(Role::TextSecondary),
        ));
    }
    lines
}

fn compact_token(tab: &TabDisplay<'_>, max_width: usize, theme: &Theme) -> Option<(String, usize)> {
    let safe_label = sanitize_terminal_text(&tab.label);
    let marker = if tab.active {
        theme.glyphs.active_marker
    } else {
        " "
    };
    let prefix = format!("[{marker}{} ", tab.position);
    let prefix_width = display_width(&prefix);
    let token = format!("{prefix}{safe_label}]");
    let width = display_width(&token);
    if width <= max_width {
        return Some((token, width));
    }
    if let Some(label_budget) = max_width
        .checked_sub(prefix_width)
        .and_then(|rest| rest.checked_sub(1))
    {
        let truncated = truncate_display(&safe_label, label_budget, theme.glyphs.ellipsis);
        let token = format!("{prefix}{truncated}]");
        let width = display_width(&token);
        if width <= max_width {
            return Some((token, width));
        }
    }
    let clipped = truncate_display(&token, max_width, "");
    let clipped_width = display_width(&clipped);
    (clipped_width > 0).then_some((clipped, clipped_width))
}

fn compact_token_width(tab: &TabDisplay<'_>) -> usize {
    let safe_label = sanitize_terminal_text(&tab.label);
    let prefix_width = tab.position.to_string().len().saturating_add(3);
    prefix_width
        .saturating_add(display_width(safe_label.as_ref()))
        .saturating_add(1)
}

fn navigator_row(tab: &TabDisplay<'_>, width: usize, theme: &Theme) -> String {
    let safe_label = sanitize_terminal_text(&tab.label);
    let marker = if tab.active {
        theme.glyphs.active_marker
    } else {
        " "
    };
    let prefix = format!("[{marker}{}] ", tab.position);
    let prefix_width = display_width(&prefix);
    let row = format!("{prefix}{safe_label}");
    if display_width(&row) <= width {
        return row;
    }
    let Some(label_budget) = width.checked_sub(prefix_width) else {
        return truncate_display(&row, width, "");
    };
    format!(
        "{prefix}{}",
        truncate_display(&safe_label, label_budget, theme.glyphs.ellipsis)
    )
}

fn compact_style(active: bool, theme: &Theme) -> Style {
    if active {
        theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD)
    } else {
        theme.style(Role::TextSecondary)
    }
}

fn navigator_style(active: bool, theme: &Theme) -> Style {
    if active {
        theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD)
    } else {
        theme.style(Role::TextPrimary)
    }
}

#[cfg(test)]
mod tests;
