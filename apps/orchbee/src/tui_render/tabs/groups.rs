//! Group headers and collapsed-member projection for the compact strip, with paint-derived hits.
use super::{TabDisplay, compact_style, compact_token, compact_token_width, paint_token_pointer};
use crate::{
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_pointer::ProjectedPointer,
    tui_theme::{Role, Theme},
};
use orchbee_workbench_protocol::{TabGroup, TabGroupColor};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};
#[cfg(test)]
mod tests;

pub(in crate::tui_render) struct GroupTabDisplay<'a> {
    pub(in crate::tui_render) tab: TabDisplay<'a>,
    pub(in crate::tui_render) group: Option<&'a TabGroup>,
    pub(in crate::tui_render) collapsed: bool,
    pub(in crate::tui_render) selected: bool,
}

struct Token {
    index: usize,
    active: bool,
    checked: bool,
    label: String,
    color: Option<TabGroupColor>,
    header: Option<usize>,
}

pub(in crate::tui_render) fn render<C, P, R>(
    frame: &mut Frame<'_>,
    area: Rect,
    tabs: &[GroupTabDisplay<'_>],
    theme: &Theme,
    unnamed_group: &str,
    mut interactions: GroupInteractions<'_, C, P, R>,
) where
    P: FnMut(&C, usize, bool, Rect) -> ProjectedPointer,
    R: FnMut(&mut C, usize, bool, Rect),
{
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
                let marked = members.clone().filter(|item| item.selected).count();
                let count = if marked > 0 {
                    format!("{marked}/{count}")
                } else {
                    count.to_string()
                };
                let active = members.clone().any(|item| item.tab.active);
                let representative = members
                    .enumerate()
                    .find(|(_, item)| item.tab.active)
                    .map_or(index, |(offset, _)| index + offset);
                current_header = Some(tokens.len());
                tokens.push(Token {
                    index: representative,
                    active: item.collapsed && active,
                    checked: false,
                    label: format!(
                        " {}{} {} ({count}){} ",
                        if item.collapsed && active {
                            theme.glyphs.active_marker
                        } else {
                            " "
                        },
                        if item.collapsed {
                            theme.glyphs.disclosure_closed
                        } else {
                            theme.glyphs.disclosure_open
                        },
                        sanitize_terminal_text(if group.label.is_empty() {
                            unnamed_group
                        } else {
                            &group.label
                        }),
                        if item.collapsed { "" } else { " [" },
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
            // Group-member selection owns a fixed marker track. Changing the
            // checked value never moves its current marker, ordinal, or label.
            let mut label = format!(
                "{}{label}",
                if item.selected {
                    theme.glyphs.active_marker
                } else {
                    " "
                }
            );
            if let Some(group) = item.group
                && tabs
                    .get(index + 1)
                    .and_then(|tab| tab.group)
                    .map(|group| &group.group_id)
                    != Some(&group.group_id)
            {
                label.push_str(" ]");
            }
            // Stable three-cell close track; pointer movement never resizes tabs.
            label.push_str("   ");
            tokens.push(Token {
                index,
                active: item.tab.active,
                checked: item.selected,
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
    let mut painted = Vec::new();
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
        // An ASCII ellipsis can exceed a narrow header/member budget. Keep its painted target
        // by clipping without a marker instead of letting the empty label terminate the row.
        let marker = if display_width(theme.glyphs.ellipsis) <= budget {
            theme.glyphs.ellipsis
        } else {
            ""
        };
        let close_width = usize::from(token.color.is_none() && budget >= 7) * 3;
        let label = if close_width > 0 {
            let body = token.label.strip_suffix("   ").unwrap_or(&token.label);
            let body = truncate_display(body, budget - close_width, marker);
            format!("{body}   ")
        } else {
            truncate_display(&token.label, budget, marker)
        };
        let cells = display_width(&label);
        if cells == 0 {
            break;
        }
        let rect = Rect::new(
            area.x
                .saturating_add(u16::try_from(used).unwrap_or(u16::MAX)),
            area.y,
            u16::try_from(cells).unwrap_or(u16::MAX),
            1,
        );
        let pointer = if frame.area().intersection(rect) == rect {
            interactions.project(token.index, token.color.is_some(), rect)
        } else {
            ProjectedPointer::Rest
        };
        let style = token.color.map_or_else(
            || compact_style(token.active, theme),
            |color| {
                let surface = match pointer {
                    ProjectedPointer::Rest => token.active.then_some(Role::SurfaceCurrent),
                    ProjectedPointer::Hovered => Some(Role::SurfaceHover),
                    ProjectedPointer::Pressed => Some(Role::SurfacePressed),
                };
                theme.tab_group_header_style(color, surface)
            },
        );
        if token.checked
            && let Some(rest) = label.strip_prefix(theme.glyphs.active_marker)
        {
            let checked = theme.content_on_surface(
                Role::SurfaceChrome,
                token.active.then_some(Role::SurfaceCurrent),
                Some(Role::AccentSelection),
                theme.style(Role::AccentSelection),
            );
            spans.push(Span::styled(theme.glyphs.active_marker, checked));
            spans.push(Span::styled(rest.to_owned(), style));
        } else {
            spans.push(Span::styled(label, style));
        }
        if frame.area().intersection(rect) == rect {
            painted.push(PaintedToken {
                index: token.index,
                group_header: token.color.is_some(),
                rect,
                pointer,
            });
        }
        used += cells;
        if used >= width {
            break;
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
    for token in painted {
        if token.group_header
            || paint_token_pointer(frame, token.rect, token.pointer, theme).is_some()
        {
            interactions.record(token.index, token.group_header, token.rect);
        }
    }
}

pub(in crate::tui_render) struct GroupInteractions<'a, C, P, R> {
    context: &'a mut C,
    project_pointer: &'a mut P,
    record_token: &'a mut R,
}

impl<'a, C, P, R> GroupInteractions<'a, C, P, R>
where
    P: for<'context> FnMut(&'context C, usize, bool, Rect) -> ProjectedPointer,
    R: for<'context> FnMut(&'context mut C, usize, bool, Rect),
{
    pub(in crate::tui_render) const fn new(
        context: &'a mut C,
        project_pointer: &'a mut P,
        record_token: &'a mut R,
    ) -> Self {
        Self {
            context,
            project_pointer,
            record_token,
        }
    }
}

impl<C, P, R> GroupInteractions<'_, C, P, R>
where
    P: FnMut(&C, usize, bool, Rect) -> ProjectedPointer,
    R: FnMut(&mut C, usize, bool, Rect),
{
    fn project(&mut self, index: usize, group_header: bool, rect: Rect) -> ProjectedPointer {
        (self.project_pointer)(&*self.context, index, group_header, rect)
    }

    fn record(&mut self, index: usize, group_header: bool, rect: Rect) {
        (self.record_token)(self.context, index, group_header, rect);
    }
}

#[derive(Clone, Copy, Debug)]
struct PaintedToken {
    index: usize,
    group_header: bool,
    rect: Rect,
    pointer: ProjectedPointer,
}
