use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::terminal_text::sanitize_terminal_text;

use super::{EMPHASIS, MUTED, TEXT};

const ELLIPSIS: char = '…';

#[derive(Clone, Copy)]
pub(super) struct TabDisplay<'a> {
    pub(super) position: usize,
    pub(super) active: bool,
    pub(super) label: &'a str,
}

pub(super) fn render_compact<'a>(
    frame: &mut Frame<'_>,
    area: Rect,
    tab_count: usize,
    active_position: Option<usize>,
    tab_at: impl Fn(usize) -> Option<TabDisplay<'a>>,
) {
    let line = compact_line(usize::from(area.width), tab_count, active_position, &tab_at);
    frame.render_widget(Paragraph::new(line), area);
}

pub(super) fn render_navigator<'a>(
    frame: &mut Frame<'_>,
    area: Rect,
    tab_count: usize,
    active_position: Option<usize>,
    tab_at: impl Fn(usize) -> Option<TabDisplay<'a>>,
) {
    let lines = navigator_lines(
        usize::from(area.width),
        usize::from(area.height),
        tab_count,
        active_position,
        &tab_at,
    );
    frame.render_widget(Paragraph::new(lines), area);
}

fn compact_line<'a>(
    width: usize,
    tab_count: usize,
    active_position: Option<usize>,
    tab_at: &impl Fn(usize) -> Option<TabDisplay<'a>>,
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
        let Some((token, token_width)) = compact_token(tab, remaining) else {
            break;
        };
        spans.push(Span::styled(token, compact_style(tab.active)));
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
    let active_width = compact_token_width(active);
    if active_width > width {
        return active_index;
    }

    let mut start = active_index;
    let mut occupied = active_width;
    while start > 0 {
        let Some(previous) = tab_at(start - 1) else {
            break;
        };
        let additional = compact_token_width(previous).saturating_add(1);
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
) -> Vec<Line<'static>> {
    if height == 0 || width == 0 || tab_count == 0 {
        return Vec::new();
    }
    let active_index = active_position
        .and_then(|position| position.checked_sub(1))
        .filter(|index| *index < tab_count);
    let start = active_index
        .map(|index| index.saturating_sub(height - 1))
        .unwrap_or(0)
        .min(tab_count.saturating_sub(height));
    let end = start.saturating_add(height).min(tab_count);

    (start..end)
        .map_while(tab_at)
        .map(|tab| {
            Line::from(Span::styled(
                navigator_row(tab, width),
                navigator_style(tab.active),
            ))
        })
        .collect()
}

fn compact_token(tab: TabDisplay<'_>, max_width: usize) -> Option<(String, usize)> {
    let safe_label = sanitize_terminal_text(tab.label);
    let marker = if tab.active { '*' } else { ' ' };
    let prefix = format!("[{marker}{} ", tab.position);
    let prefix_width = Line::raw(prefix.as_str()).width();
    let token = format!("{prefix}{safe_label}]");
    let width = Line::raw(token.as_str()).width();
    if width <= max_width {
        return Some((token, width));
    }
    let label_budget = max_width
        .checked_sub(prefix_width)
        .and_then(|rest| rest.checked_sub(2));
    let Some(label_budget) = label_budget.filter(|budget| *budget >= 1) else {
        let clipped = truncate_display(&token, max_width);
        let clipped_width = Line::raw(clipped.as_str()).width();
        return Some((clipped, clipped_width));
    };
    let truncated = format!(
        "{prefix}{}{ELLIPSIS}]",
        truncate_display(&safe_label, label_budget)
    );
    Some((truncated.clone(), Line::raw(truncated.as_str()).width()))
}

fn compact_token_width(tab: TabDisplay<'_>) -> usize {
    let safe_label = sanitize_terminal_text(tab.label);
    let prefix_width = tab.position.to_string().len().saturating_add(3);
    prefix_width
        .saturating_add(Line::raw(safe_label.as_ref()).width())
        .saturating_add(1)
}

fn navigator_row(tab: TabDisplay<'_>, width: usize) -> String {
    let safe_label = sanitize_terminal_text(tab.label);
    let marker = if tab.active { '*' } else { ' ' };
    let prefix = format!("[{marker}{}] ", tab.position);
    let prefix_width = Line::raw(prefix.as_str()).width();
    let row = format!("{prefix}{safe_label}");
    if Line::raw(row.as_str()).width() <= width {
        return row;
    }
    let Some(label_budget) = width
        .checked_sub(prefix_width)
        .and_then(|rest| rest.checked_sub(1))
        .filter(|budget| *budget >= 1)
    else {
        return truncate_display(&row, width);
    };
    format!(
        "{prefix}{}{ELLIPSIS}",
        truncate_display(&safe_label, label_budget)
    )
}

pub(super) fn truncate_display(label: &str, budget: usize) -> String {
    let mut text = String::new();
    let mut used = 0_usize;
    for grapheme in Line::raw(label).styled_graphemes(Style::default()) {
        let width = Line::raw(grapheme.symbol).width();
        if used.saturating_add(width) > budget {
            break;
        }
        text.push_str(grapheme.symbol);
        used = used.saturating_add(width);
    }
    text
}

fn compact_style(active: bool) -> Style {
    if active {
        Style::new().fg(EMPHASIS).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(MUTED)
    }
}

fn navigator_style(active: bool) -> Style {
    if active {
        Style::new().fg(EMPHASIS).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(TEXT)
    }
}

#[cfg(test)]
mod tests;
