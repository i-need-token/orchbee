//! Group headers and collapsed-member projection for the compact strip, with paint-derived hits.
use super::{TabDisplay, compact_style, compact_token, compact_token_width};
use crate::{
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_theme::Theme,
};
use orchbee_workbench_protocol::{TabGroup, TabGroupColor};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};
#[cfg(test)]
mod tests;

pub(in crate::tui_render) struct GroupTabDisplay<'a> {
    pub(in crate::tui_render) tab: TabDisplay<'a>,
    pub(in crate::tui_render) group: Option<&'a TabGroup>,
    pub(in crate::tui_render) collapsed: bool,
}

struct Token {
    index: usize,
    active: bool,
    label: String,
    color: Option<TabGroupColor>,
    header: Option<usize>,
}

pub(in crate::tui_render) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    tabs: &[GroupTabDisplay<'_>],
    theme: &Theme,
    on_token: &mut impl FnMut(usize, bool, usize, usize),
) {
    if area.is_empty() {
        return;
    }
    let mut tokens = Vec::<Token>::new();
    let mut current_header = None;
    for (index, item) in tabs.iter().enumerate() {
        if let Some(group) = item.group {
            if index == 0 || tabs[index - 1].group.map(|g| &g.group_id) != Some(&group.group_id) {
                let members = tabs[index..]
                    .iter()
                    .take_while(|item| item.group.map(|g| &g.group_id) == Some(&group.group_id));
                let count = members.clone().count();
                let active = members.clone().any(|item| item.tab.active);
                let representative = members
                    .enumerate()
                    .find(|(_, item)| item.tab.active)
                    .map_or(index, |(offset, _)| index + offset);
                current_header = Some(tokens.len());
                tokens.push(Token {
                    index: representative,
                    active: item.collapsed && active,
                    label: format!(
                        " {}{} {} ({count}) ",
                        if item.collapsed && active {
                            theme.glyphs.active_marker
                        } else {
                            ""
                        },
                        if item.collapsed {
                            theme.glyphs.disclosure_closed
                        } else {
                            theme.glyphs.disclosure_open
                        },
                        sanitize_terminal_text(&group.label)
                    ),
                    color: Some(group.color),
                    header: None,
                });
            }
            if item.collapsed {
                continue;
            }
        } else {
            current_header = None;
        }
        if let Some((label, _)) = compact_token(&item.tab, compact_token_width(&item.tab), theme) {
            tokens.push(Token {
                index,
                active: item.tab.active,
                label,
                color: None,
                header: current_header,
            });
        }
    }
    let width = usize::from(area.width);
    let active = tokens.iter().position(|token| token.active).unwrap_or(0);
    let mut start = active;
    let mut occupied = tokens
        .get(active)
        .map_or(0, |token| display_width(&token.label));
    while start > 0 {
        let previous = display_width(&tokens[start - 1].label) + 1;
        if occupied.saturating_add(previous) > width {
            break;
        }
        occupied += previous;
        start -= 1;
    }
    let missing_header = tokens
        .get(start)
        .and_then(|token| token.header)
        .filter(|header| *header < start);
    // Keep the active member and its group identity together when the window starts inside a group.
    let prefix = if missing_header.is_some() && width >= 8 {
        start = active;
        tokens.get(active).and_then(|token| token.header)
    } else {
        None
    };
    let mut spans = Vec::new();
    let mut used = 0;
    let indices = prefix.into_iter().chain(start..tokens.len());
    for token_index in indices {
        let token = &tokens[token_index];
        if used > 0 && used < width {
            spans.push(Span::raw(" "));
            used += 1;
        }
        let budget = if Some(token_index) == prefix {
            (width / 3).max(1)
        } else {
            width.saturating_sub(used)
        };
        if budget == 0 {
            break;
        }
        let label = truncate_display(&token.label, budget, theme.glyphs.ellipsis);
        let cells = display_width(&label);
        if cells == 0 {
            break;
        }
        let style = token.color.map_or_else(
            || compact_style(token.active, theme),
            |color| theme.tab_group_style(color).add_modifier(Modifier::BOLD),
        );
        spans.push(Span::styled(label, style));
        on_token(token.index, token.color.is_some(), used, cells);
        used += cells;
        if used >= width {
            break;
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
