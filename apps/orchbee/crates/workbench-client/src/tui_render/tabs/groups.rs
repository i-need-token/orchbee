//! The Windows Tab strip: group headers, Agent cues, hover-revealed close and overflow, painted
//! with the hit regions derived from the same pass.
use super::TabDisplay;
use crate::{
    machine_picker::TabAgent,
    terminal_text::{display_width, truncate_display},
    tui_pointer::ProjectedPointer,
    tui_render::sidebar_text::state_glyph,
    tui_theme::{Role, Theme},
};
use orchbee_workbench_protocol::{TabGroup, TabGroupColor};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
};
#[cfg(test)]
mod tests;

pub(in crate::tui_render) struct GroupTabDisplay<'a> {
    pub(in crate::tui_render) tab: TabDisplay<'a>,
    pub(in crate::tui_render) group: Option<&'a TabGroup>,
    pub(in crate::tui_render) collapsed: bool,
    pub(in crate::tui_render) selected: bool,
    pub(in crate::tui_render) pinned: bool,
    pub(in crate::tui_render) agent: Option<TabAgent>,
}

/// The painted part of a token that a pointer projection or hit region belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::tui_render) enum StripPart {
    /// A Tab's body, including its close track while the control is hidden.
    Tab,
    /// A group header.
    Header,
    /// A Tab's revealed close control.
    Close,
}

/// Longer Tab labels end in an ellipsis.
const LABEL_LIMIT: usize = 24;
/// Every Tab reserves its close control, so revealing it never resizes the strip.
const CLOSE_WIDTH: usize = 3;
/// A truncated Tab keeps its close track only from this width.
const MINIMUM_CLOSE_TOKEN: usize = 7;
/// A truncated label keeps this many leading cells before its ellipsis.
const MINIMUM_LABEL_CELLS: usize = 3;

enum Kind {
    Tab {
        active: bool,
        checked: bool,
        label: String,
    },
    Header {
        color: TabGroupColor,
        collapsed: bool,
        current: bool,
        checked: bool,
        name: String,
    },
    Divider,
}

struct Token {
    /// The Tab this token activates; a header's representative member.
    index: usize,
    /// The header of the group this token belongs to.
    header: Option<usize>,
    agent: Option<TabAgent>,
    kind: Kind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Piece {
    Pad,
    Glyph,
    Label,
    Unread,
    Arrow,
    Close,
}

impl Token {
    fn is_tab(&self) -> bool {
        matches!(self.kind, Kind::Tab { .. })
    }

    fn active(&self) -> bool {
        match self.kind {
            Kind::Tab { active, .. } => active,
            Kind::Header { current, .. } => current,
            Kind::Divider => false,
        }
    }

    fn text(&self) -> &str {
        match &self.kind {
            Kind::Tab { label, .. } => label,
            Kind::Header { name, .. } => name,
            Kind::Divider => "",
        }
    }

    fn natural_width(&self, theme: &Theme) -> usize {
        self.pieces(theme, 0, usize::MAX).map_or(0, |pieces| {
            pieces.iter().map(|(text, _)| display_width(text)).sum()
        })
    }

    /// The token's cells within `budget`, truncating its label before dropping the close track.
    /// A label keeps three cells and its ellipsis, or the whole of a shorter label; a token that
    /// cannot is not painted, and the strip's edge marker stands for it.
    fn pieces(
        &self,
        theme: &Theme,
        working_frame: usize,
        budget: usize,
    ) -> Option<Vec<(String, Piece)>> {
        if matches!(self.kind, Kind::Divider) {
            return (budget > 0).then(|| vec![(theme.glyphs.vertical_rule.to_owned(), Piece::Pad)]);
        }
        let glyph = self
            .agent
            .map(|agent| state_glyph(agent.state, theme, working_frame).to_owned());
        let unread = self.agent.is_some_and(|agent| agent.unread);
        let lead = 1 + glyph.as_ref().map_or(0, |glyph| display_width(glyph) + 1);
        let trail = if unread {
            1 + display_width(theme.glyphs.status_bullet)
        } else {
            0
        };
        let (tail, closes): (usize, &[bool]) = if self.is_tab() {
            if budget >= MINIMUM_CLOSE_TOKEN {
                (1, &[true, false])
            } else {
                (1, &[false])
            }
        } else {
            (3, &[false])
        };
        let text = self.text();
        let least = display_width(text)
            .min(MINIMUM_LABEL_CELLS + display_width(theme.glyphs.ellipsis))
            .max(1);
        for &close in closes {
            let fixed = lead + trail + tail + usize::from(close) * CLOSE_WIDTH;
            let Some(room) = budget.checked_sub(fixed).filter(|room| *room >= least) else {
                continue;
            };
            let mut pieces = vec![(" ".to_owned(), Piece::Pad)];
            if let Some(glyph) = &glyph {
                pieces.push((glyph.clone(), Piece::Glyph));
                pieces.push((" ".to_owned(), Piece::Pad));
            }
            pieces.push((
                truncate_display(text, room, theme.glyphs.ellipsis),
                Piece::Label,
            ));
            if unread {
                pieces.push((format!(" {}", theme.glyphs.status_bullet), Piece::Unread));
            }
            if self.is_tab() {
                pieces.push((" ".to_owned(), Piece::Pad));
                if close {
                    pieces.push((" ".repeat(CLOSE_WIDTH), Piece::Close));
                }
            } else {
                pieces.push((" ".to_owned(), Piece::Pad));
                pieces.push((" ".to_owned(), Piece::Arrow));
                pieces.push((" ".to_owned(), Piece::Pad));
            }
            return Some(pieces);
        }
        None
    }
}

fn tokens(tabs: &[GroupTabDisplay<'_>], unnamed_group: &str, theme: &Theme) -> Vec<Token> {
    let mut tokens = Vec::<Token>::new();
    let mut current_header = None;
    for (index, item) in tabs.iter().enumerate() {
        if let Some(group) = item.group {
            if index == 0 || tabs[index - 1].group.map(|g| &g.group_id) != Some(&group.group_id) {
                let members: Vec<_> = tabs[index..]
                    .iter()
                    .take_while(|item| item.group.map(|g| &g.group_id) == Some(&group.group_id))
                    .collect();
                let active = members.iter().position(|item| item.tab.active);
                current_header = Some(tokens.len());
                tokens.push(Token {
                    index: index + active.unwrap_or(0),
                    header: None,
                    // A collapsed group carries its hidden members' cue.
                    agent: item
                        .collapsed
                        .then(|| summarize(members.iter().filter_map(|item| item.agent)))
                        .flatten(),
                    kind: Kind::Header {
                        color: group.color,
                        collapsed: item.collapsed,
                        current: item.collapsed && active.is_some(),
                        checked: item.collapsed && members.iter().any(|item| item.selected),
                        name: crate::terminal_text::sanitize_terminal_text(
                            if group.label.is_empty() {
                                unnamed_group
                            } else {
                                &group.label
                            },
                        )
                        .into_owned(),
                    },
                });
            }
            if item.collapsed {
                continue;
            }
        } else {
            current_header = None;
        }
        tokens.push(Token {
            index,
            header: current_header,
            agent: item.agent,
            kind: Kind::Tab {
                active: item.tab.active,
                checked: item.selected,
                label: truncate_display(
                    &crate::terminal_text::sanitize_terminal_text(&item.tab.label),
                    LABEL_LIMIT,
                    theme.glyphs.ellipsis,
                ),
            },
        });
        // Pinned Tabs lead the strip; one rule separates them from the rest.
        if item.pinned && tabs.get(index + 1).is_some_and(|next| !next.pinned) {
            tokens.push(Token {
                index,
                header: None,
                agent: None,
                kind: Kind::Divider,
            });
        }
    }
    tokens
}

/// The most urgent member state, and whether any member is unread.
fn summarize(agents: impl Iterator<Item = TabAgent>) -> Option<TabAgent> {
    let agents: Vec<_> = agents.collect();
    Some(TabAgent {
        state: crate::machine_picker::most_urgent(agents.iter().map(|agent| agent.state))?,
        unread: agents.iter().any(|agent| agent.unread),
    })
}

/// Paints the strip into `area` and returns the column after which the create control belongs.
pub(in crate::tui_render) fn render<C, P, R>(
    frame: &mut Frame<'_>,
    area: Rect,
    tabs: &[GroupTabDisplay<'_>],
    theme: &Theme,
    unnamed_group: &str,
    working_frame: usize,
    mut interactions: GroupInteractions<'_, C, P, R>,
) -> u16
where
    P: FnMut(&C, usize, StripPart, Rect) -> ProjectedPointer,
    R: FnMut(&mut C, usize, StripPart, Rect),
{
    if area.is_empty() {
        return 0;
    }
    let tokens = tokens(tabs, unnamed_group, theme);
    let width = usize::from(area.width);
    let marker_width = display_width(theme.glyphs.ellipsis);
    // Keep a complete active tab target on tiny strips. Wider strips reserve
    // noninteractive edge markers before selecting the visible token window.
    let overflow_track = if width >= 2 * (marker_width + 1) + 4 {
        marker_width + 1
    } else {
        0
    };
    let widths: Vec<_> = tokens
        .iter()
        .map(|token| token.natural_width(theme))
        .collect();
    let total_width = widths
        .iter()
        .sum::<usize>()
        .saturating_add(tokens.len().saturating_sub(1));
    let window_width = if total_width <= width {
        width
    } else {
        width.saturating_sub(2 * overflow_track)
    };
    let active = tokens.iter().position(Token::active).unwrap_or(0);
    let mut start = active;
    let mut occupied = widths.get(active).copied().unwrap_or(0);
    while start > 0 {
        let previous = widths[start - 1] + 1;
        if occupied.saturating_add(previous) > window_width {
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
    let prefix = if missing_header.is_some() && window_width >= 8 {
        start = active;
        tokens.get(active).and_then(|token| token.header)
    } else {
        None
    };
    let mut used = 0;
    let indices: Vec<_> = prefix.into_iter().chain(start..tokens.len()).collect();
    if start > 0 && overflow_track > 0 {
        paint_text(
            frame,
            area,
            0,
            &format!("{} ", theme.glyphs.ellipsis),
            theme.content_on_surface(
                Role::SurfaceStrip,
                None,
                Some(Role::TextSecondary),
                theme.style(Role::TextSecondary),
            ),
        );
        used = overflow_track;
    }
    let needed = indices
        .iter()
        .map(|index| {
            if Some(*index) == prefix {
                widths[*index].min(window_width / 3)
            } else {
                widths[*index]
            }
        })
        .sum::<usize>()
        .saturating_add(indices.len().saturating_sub(1));
    let trailing =
        overflow_track > 0 && needed > width.saturating_sub(used) && active + 1 < tokens.len();
    let content_end = width.saturating_sub(usize::from(trailing) * overflow_track);
    let mut painted_end = 0;
    for (position, token_index) in indices.into_iter().enumerate() {
        let token = &tokens[token_index];
        if position > 0 && used < content_end {
            used += 1;
        }
        let budget = if Some(token_index) == prefix {
            (window_width / 3)
                .max(1)
                .min(content_end.saturating_sub(used))
        } else {
            content_end.saturating_sub(used)
        };
        let Some(pieces) = token.pieces(theme, working_frame, budget) else {
            // A shortened group identity yields to its member rather than hiding it.
            if Some(token_index) == prefix {
                continue;
            }
            break;
        };
        let cells: usize = pieces.iter().map(|(text, _)| display_width(text)).sum();
        let rect = Rect::new(
            area.x
                .saturating_add(u16::try_from(used).unwrap_or(u16::MAX)),
            area.y,
            u16::try_from(cells).unwrap_or(u16::MAX),
            1,
        );
        paint_token(
            frame,
            rect,
            token,
            &pieces,
            theme,
            working_frame,
            &mut interactions,
        );
        if !matches!(token.kind, Kind::Divider) {
            painted_end = used + cells;
        }
        used += cells;
        if used >= content_end {
            break;
        }
    }
    if trailing {
        paint_text(
            frame,
            area,
            content_end + 1,
            theme.glyphs.ellipsis,
            theme.content_on_surface(
                Role::SurfaceStrip,
                None,
                Some(Role::TextSecondary),
                theme.style(Role::TextSecondary),
            ),
        );
        // Hidden Tabs follow the marker; creation moves to the strip's end.
        return area.width.saturating_add(1);
    }
    u16::try_from(painted_end + usize::from(painted_end > 0)).unwrap_or(u16::MAX)
}

fn paint_text(frame: &mut Frame<'_>, area: Rect, offset: usize, text: &str, style: Style) {
    let Ok(offset) = u16::try_from(offset) else {
        return;
    };
    if offset < area.width {
        frame.buffer_mut().set_stringn(
            area.x + offset,
            area.y,
            text,
            usize::from(area.width - offset),
            style,
        );
    }
}

fn paint_token<C, P, R>(
    frame: &mut Frame<'_>,
    rect: Rect,
    token: &Token,
    pieces: &[(String, Piece)],
    theme: &Theme,
    working_frame: usize,
    interactions: &mut GroupInteractions<'_, C, P, R>,
) where
    P: FnMut(&C, usize, StripPart, Rect) -> ProjectedPointer,
    R: FnMut(&mut C, usize, StripPart, Rect),
{
    let on_screen = |rect: Rect| frame.area().intersection(rect) == rect;
    let close = pieces
        .iter()
        .any(|(_, piece)| *piece == Piece::Close)
        .then(|| {
            let width = u16::try_from(CLOSE_WIDTH).unwrap_or(u16::MAX);
            Rect::new(rect.right().saturating_sub(width), rect.y, width, 1)
        });
    let body = close.map_or(rect, |close| {
        Rect::new(rect.x, rect.y, close.x.saturating_sub(rect.x), 1)
    });
    let part = if token.is_tab() {
        StripPart::Tab
    } else {
        StripPart::Header
    };
    let visible = on_screen(rect) && !matches!(token.kind, Kind::Divider);
    let project = |interactions: &mut GroupInteractions<'_, C, P, R>, part, rect| {
        if visible {
            interactions.project(token.index, part, rect)
        } else {
            ProjectedPointer::Rest
        }
    };
    let body_pointer = project(interactions, part, body);
    let track_pointer = close.map_or(ProjectedPointer::Rest, |close| {
        project(interactions, StripPart::Tab, close)
    });
    let close_pointer = close.map_or(ProjectedPointer::Rest, |close| {
        project(interactions, StripPart::Close, close)
    });
    // A pointer on the close control still hovers its Tab.
    let pointer = match (body_pointer, track_pointer, close_pointer) {
        (ProjectedPointer::Pressed, _, _) | (_, ProjectedPointer::Pressed, _) => {
            ProjectedPointer::Pressed
        }
        (ProjectedPointer::Rest, ProjectedPointer::Rest, ProjectedPointer::Rest) => {
            ProjectedPointer::Rest
        }
        _ => ProjectedPointer::Hovered,
    };
    let revealed = token.active() || pointer != ProjectedPointer::Rest;
    let style = |piece: Piece| {
        piece_style(
            token,
            piece,
            pointer,
            close_pointer,
            revealed,
            theme,
            working_frame,
        )
    };
    let mut x = rect.x;
    for (text, piece) in pieces {
        let text = match piece {
            Piece::Close if revealed => "[x]",
            Piece::Arrow => match &token.kind {
                Kind::Header {
                    collapsed: true, ..
                } => theme.glyphs.disclosure_closed,
                Kind::Header { .. } if pointer != ProjectedPointer::Rest => {
                    theme.glyphs.disclosure_open
                }
                _ => text,
            },
            _ => text,
        };
        let (next, _) = frame.buffer_mut().set_stringn(
            x,
            rect.y,
            text,
            usize::from(rect.right().saturating_sub(x)),
            style(*piece),
        );
        x = next;
    }
    if !visible {
        return;
    }
    interactions.record(token.index, part, body);
    if let Some(close) = close {
        let close_part = if revealed {
            StripPart::Close
        } else {
            StripPart::Tab
        };
        interactions.record(token.index, close_part, close);
    }
}

fn piece_style(
    token: &Token,
    piece: Piece,
    pointer: ProjectedPointer,
    close_pointer: ProjectedPointer,
    revealed: bool,
    theme: &Theme,
    working_frame: usize,
) -> Style {
    let pointer_surface = match pointer {
        ProjectedPointer::Rest => None,
        ProjectedPointer::Hovered => Some(Role::SurfaceHover),
        ProjectedPointer::Pressed => Some(Role::SurfacePressed),
    };
    match &token.kind {
        Kind::Divider => theme.content_on_surface(
            Role::SurfaceStrip,
            None,
            None,
            theme.style(Role::FrameDivider),
        ),
        Kind::Tab {
            active, checked, ..
        } => {
            let surface = pointer_surface
                .or_else(|| checked.then_some(Role::SurfaceSelection))
                .or_else(|| active.then_some(Role::SurfaceCurrent));
            let (role, content) = match piece {
                Piece::Label => {
                    // Hover raises an inactive label to primary text; bold stays with current and press.
                    let role = if *active || pointer != ProjectedPointer::Rest {
                        Role::TextPrimary
                    } else {
                        Role::TextSecondary
                    };
                    let emphasis = *active || pointer == ProjectedPointer::Pressed;
                    let style = theme.style(role);
                    (
                        Some(role),
                        if emphasis {
                            style.add_modifier(Modifier::BOLD)
                        } else {
                            style
                        },
                    )
                }
                Piece::Glyph => token.agent.map_or((None, Style::new()), |agent| {
                    (
                        Some(Theme::agent_role(agent.state)),
                        theme.agent_style(agent.state, working_frame),
                    )
                }),
                Piece::Unread => (None, theme.style(Role::StatusUnread)),
                Piece::Close if revealed => {
                    return match close_pointer {
                        ProjectedPointer::Rest => theme.content_on_surface(
                            Role::SurfaceStrip,
                            surface,
                            Some(Role::TextSecondary),
                            theme.style(Role::TextSecondary),
                        ),
                        ProjectedPointer::Hovered | ProjectedPointer::Pressed => theme
                            .content_on_surface(
                                Role::SurfaceStrip,
                                Some(if close_pointer == ProjectedPointer::Pressed {
                                    Role::SurfacePressed
                                } else {
                                    Role::SurfaceHover
                                }),
                                Some(Role::AccentDestructive),
                                theme.style(Role::AccentDestructive),
                            ),
                    };
                }
                Piece::Pad | Piece::Arrow | Piece::Close => (None, Style::new()),
            };
            theme.content_on_surface(Role::SurfaceStrip, surface, role, content)
        }
        Kind::Header {
            color,
            current,
            checked,
            ..
        } => {
            let surface = pointer_surface
                .or_else(|| checked.then_some(Role::SurfaceSelection))
                .or_else(|| current.then_some(Role::SurfaceCurrent));
            let accent = Style::new().fg(theme.tab_groups.accent(*color));
            let (role, content) = match piece {
                Piece::Label => (None, accent.add_modifier(Modifier::BOLD)),
                Piece::Arrow if pointer_surface.is_some() => (None, theme.style(Role::TextPrimary)),
                Piece::Arrow => (None, accent),
                Piece::Glyph => token.agent.map_or((None, Style::new()), |agent| {
                    (
                        Some(Theme::agent_role(agent.state)),
                        theme.agent_style(agent.state, working_frame),
                    )
                }),
                Piece::Unread => (None, theme.style(Role::StatusUnread)),
                Piece::Pad | Piece::Close => (None, Style::new()),
            };
            theme.tab_group_header_style(*color, surface, role, content)
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
    P: for<'context> FnMut(&'context C, usize, StripPart, Rect) -> ProjectedPointer,
    R: for<'context> FnMut(&'context mut C, usize, StripPart, Rect),
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
    P: FnMut(&C, usize, StripPart, Rect) -> ProjectedPointer,
    R: FnMut(&mut C, usize, StripPart, Rect),
{
    fn project(&mut self, index: usize, part: StripPart, rect: Rect) -> ProjectedPointer {
        (self.project_pointer)(&*self.context, index, part, rect)
    }

    fn record(&mut self, index: usize, part: StripPart, rect: Rect) {
        (self.record_token)(self.context, index, part, rect);
    }
}
