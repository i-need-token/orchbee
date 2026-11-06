//! The Mobile tier's Tab list: today's Tab strip turned vertical over the whole window, with the
//! strip's own tokens as its rows in the strip's order. Pinned Tabs lead with the strip's rule
//! under them, a Tab Group shows as its name in its color with `▸` after a collapsed name and its
//! members indented beneath, and every row keeps the strip's targets, so a press selects, a drag
//! reorders and the secondary button opens the Tab menu as on the strip.
//!
//! From the touch-spacing height it is spaced like the sidebar list: blank, `[ New Tab ]` and
//! `[ « ]`, blank, blank, the Workspace's name, blank, then a row and its blank line per Tab. A
//! shorter window closes it up to one line a row, the rule taking a line of its own.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Clear},
};

use super::groups::{self, Kind, PartCue, Token};
use crate::{
    client_view::{
        ClientView,
        tab_list::{Paint, Row},
    },
    localization::{Locale, MenuLabel},
    terminal_text::truncate_display,
    tui_hit_map::{
        HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, ListScrollTarget,
        PrimaryContract, ProjectedPointer, TooltipText, chrome::TabTarget,
    },
    tui_render::{
        controls::{ActionKind, TOUCH_ICON_ACTION_WIDTH},
        sidebar_chrome::{self, TouchRow},
        sidebar_text::state_glyph,
        sidebar_touch::{Halos, TouchAction, paint_action, resolve},
    },
    tui_theme::{Role, Theme},
    workbench_client::WorkbenchClientView,
};

const LAYER: HitLayer = HitLayer::Overlay;
/// The plain icon action's width, which the closed-up list's controls take.
const ICON_WIDTH: u16 = 3;

pub(in crate::tui_render) struct Context<'a, 'n> {
    pub(in crate::tui_render) native: &'a WorkbenchClientView<'n>,
    pub(in crate::tui_render) view: &'a ClientView,
    pub(in crate::tui_render) locale: Locale,
    pub(in crate::tui_render) theme: &'a Theme,
}

/// Where the list's lines fall: touch-spaced from the touch-spacing height, else closed up.
#[derive(Clone, Copy)]
struct Lines {
    touch: bool,
    action: u16,
    workspace: u16,
    rows: u16,
    /// Lines per row: its text line and, when spaced, the blank line it owns after it.
    spacing: u16,
}

impl Lines {
    const fn of(height: u16) -> Self {
        if height >= crate::tui_layout::TOUCH_SPACING_MINIMUM_ROWS {
            Self {
                touch: true,
                action: 1,
                workspace: 4,
                rows: 6,
                spacing: 2,
            }
        } else {
            Self {
                touch: false,
                action: 0,
                workspace: 1,
                rows: 3,
                spacing: 1,
            }
        }
    }

    /// How many rows the list shows from its first row line. Its last line, where the status chip
    /// floats, carries no row text: a spaced list's last row may leave only its blank line there.
    fn capacity(self, area: Rect) -> usize {
        let end = area.height.saturating_sub(1);
        usize::from(end.saturating_sub(self.rows).div_ceil(self.spacing))
    }
}

/// One row of the list: a Tab or a group header, by its strip token.
struct Entry<'t> {
    token: &'t Token,
    /// The strip's rule between pinned Tabs and the rest falls before this row.
    rule_before: bool,
}

pub(in crate::tui_render) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    context: &Context<'_, '_>,
    hits: &mut HitMapBuilder,
) {
    let area = area.intersection(frame.area());
    if area.width < 8 || area.height == 0 {
        return;
    }
    let Context {
        native,
        view,
        locale,
        theme,
    } = *context;
    frame.render_widget(Clear, area);
    frame.render_widget(Block::new().style(theme.style(Role::SurfaceChrome)), area);
    let lines = Lines::of(area.height);
    // One cell of trailing padding, as the sidebar list keeps; targets run through it.
    let inner = Rect::new(area.x, area.y, area.width - 1, area.height);
    let mut halos = Halos::new(area);
    let source = native.tab_create_target();

    // [ New Tab ] and the way back share the first action line.
    let row = Rect::new(inner.x, area.y + lines.action, inner.width, 1);
    let place = if lines.touch {
        TouchRow {
            row,
            top: area.y,
            height: 3,
        }
    } else {
        TouchRow {
            row,
            top: row.y,
            height: 1,
        }
    };
    let action = source.clone().map(|source| TouchAction {
        label: locale.menu_action(MenuLabel::NewTab),
        kind: ActionKind::Primary,
        enabled: true,
        target: InteractionTarget::TabCreate(source),
        place,
    });
    let resolved = action
        .as_ref()
        .and_then(|action| resolve(frame, action, theme));
    let back_x = trailing_x(row, resolved, lines.touch);
    let end = back_x.map_or(area.right(), |x| x - 1);
    if let Some(action) = action {
        let _ = paint_action(frame, action, end, theme, LAYER, &mut halos, Some(hits));
    }
    halos.text(row);
    if back_x.is_some() {
        let _ = sidebar_chrome::render_close_tabs(
            frame,
            place,
            resolved,
            lines.touch,
            &sidebar_chrome::Context {
                view,
                locale,
                theme,
                layer: LAYER,
            },
            &mut halos,
            Some(hits),
        );
    }

    // The current Workspace's name, which the Tabs belong to.
    let workspace = (0..native.context_count())
        .filter_map(|index| native.context_at(index))
        .find(|context| context.active())
        .and_then(|context| context.workspace_label())
        .map_or(locale.shell().no_workspace, |label| {
            locale.workspace_display_label(label)
        });
    let name_y = area.y + lines.workspace;
    if name_y < area.bottom() {
        let name = truncate_display(
            &crate::terminal_text::sanitize_terminal_text(workspace),
            usize::from(inner.width.saturating_sub(1)),
            theme.glyphs.ellipsis,
        );
        frame.buffer_mut().set_stringn(
            inner.x + 1,
            name_y,
            &name,
            usize::from(inner.width.saturating_sub(1)),
            theme.content_on_surface(
                Role::SurfaceChrome,
                None,
                Some(Role::TextSecondary),
                theme.style(Role::TextSecondary),
            ),
        );
        halos.text(Rect::new(area.x, name_y, area.width, 1));
    }

    let (shown, displayed) = super::shown_tabs(native, view, locale);
    let tokens = groups::tokens(&displayed, locale.shell().unnamed_tab_group, theme);
    let mut entries = Vec::new();
    let mut entry_of = vec![None; tokens.len()];
    let mut rule = false;
    for (index, token) in tokens.iter().enumerate() {
        if matches!(token.kind, Kind::Divider) {
            rule = true;
            continue;
        }
        entry_of[index] = Some(entries.len());
        entries.push(Entry {
            token,
            rule_before: std::mem::take(&mut rule),
        });
    }
    let tab_id = |index: usize| {
        native
            .tab(*shown.get(index)?)
            .map(|tab| tab.tab_id().clone())
    };
    let target_of = |entry: &Entry<'_>| -> Option<TabTarget> {
        let tab = tab_id(entry.token.index)?;
        Some(match entry.token.kind {
            Kind::Header { .. } => TabTarget::Group {
                group: displayed.get(entry.token.index)?.group?.group_id.clone(),
                tab,
            },
            _ => TabTarget::Tab(tab),
        })
    };
    let rows: Vec<(Row, &Entry<'_>)> = entries
        .iter()
        .filter_map(|entry| {
            Some((
                Row {
                    target: target_of(entry)?,
                    collapsed: matches!(
                        entry.token.kind,
                        Kind::Header {
                            collapsed: true,
                            ..
                        }
                    ),
                    header: entry
                        .token
                        .header
                        .and_then(|header| entry_of.get(header).copied().flatten()),
                },
                entry,
            ))
        })
        .collect();
    let current = rows.iter().position(|(_, entry)| match entry.token.kind {
        Kind::Tab { active, .. } => active,
        Kind::Header { current, .. } => current,
        Kind::Divider => false,
    });
    let state = view.tab_list();
    let focus = state
        .focus()
        .and_then(|focus| rows.iter().position(|(row, _)| row.target == *focus));
    let capacity = lines.capacity(area).max(1);
    // A closed-up list's rule takes a line of its own in any window it falls inside.
    let rule_row = (!lines.touch)
        .then(|| rows.iter().position(|(_, entry)| entry.rule_before))
        .flatten();
    // The first row of the shortest window that still shows `last`.
    let ending_at = |last: usize| {
        let first = (last + 1).saturating_sub(capacity);
        first + usize::from(rule_row.is_some_and(|rule| rule > first && rule <= last))
    };
    let last_first = ending_at(rows.len().saturating_sub(1));
    let first = state
        .first()
        .unwrap_or_else(|| focus.or(current).map_or(0, ending_at))
        .min(last_first);

    let list_top = area.y + lines.rows;
    // The last line belongs to the status chip, or to a spaced last row's blank line.
    let list_bottom = area.bottom().saturating_sub(1);
    let mut y = list_top;
    let mut visible = 0;
    for (index, (row, entry)) in rows.iter().enumerate().skip(first) {
        if entry.rule_before && !lines.touch && index > first {
            if y >= list_bottom {
                break;
            }
            paint_rule(frame, inner, y, theme);
            halos.text(Rect::new(area.x, y, area.width, 1));
            y += 1;
        }
        if y >= list_bottom {
            break;
        }
        // A spaced rule takes the blank line before its row, the last pinned Tab's.
        if entry.rule_before && lines.touch {
            paint_rule(frame, inner, y - 1, theme);
            halos.text(Rect::new(area.x, y - 1, area.width, 1));
        }
        let height = lines.spacing.min(area.bottom() - y);
        paint_row(
            frame,
            RowPlace {
                area,
                inner,
                y,
                height,
                touch: lines.touch,
                focused: focus == Some(index),
            },
            row,
            entry.token,
            source.as_ref(),
            view,
            locale,
            theme,
            &mut halos,
            hits,
        );
        visible += 1;
        y += lines.spacing;
    }
    if list_bottom > list_top {
        hits.record(
            HitLayer::OverlaySurface,
            Rect::new(area.x, list_top, area.width, list_bottom - list_top),
            HitTarget::ListScroll(ListScrollTarget::TabList { first }),
        );
    }
    hits.record_tab_list(Paint {
        rows: rows.into_iter().map(|(row, _)| row).collect(),
        current,
        first,
        visible,
    });
    halos.paint(frame, theme);
}

/// Where the way back, or the current Tab's close, takes its brackets at the end of `row`.
fn trailing_x(row: Rect, leading: Option<Rect>, touch: bool) -> Option<u16> {
    if touch {
        return sidebar_chrome::touch_trailing_x(row, leading);
    }
    let x = row
        .right()
        .checked_sub(ICON_WIDTH)
        .filter(|&x| x >= row.x)?;
    (!leading.is_some_and(|leading| x < leading.right().saturating_add(2))).then_some(x)
}

fn paint_rule(frame: &mut Frame<'_>, inner: Rect, y: u16, theme: &Theme) {
    let rule = match theme.glyphs.mode() {
        crate::tui_theme::GlyphMode::Unicode => "─",
        #[cfg(any(test, feature = "test-support"))]
        crate::tui_theme::GlyphMode::Simplified => "-",
    };
    let width = usize::from(inner.width.saturating_sub(1));
    frame.buffer_mut().set_stringn(
        inner.x + 1,
        y,
        rule.repeat(width),
        width,
        theme.content_on_surface(
            Role::SurfaceChrome,
            None,
            Some(Role::FrameDivider),
            theme.style(Role::FrameDivider),
        ),
    );
}

#[derive(Clone, Copy)]
struct RowPlace {
    area: Rect,
    inner: Rect,
    y: u16,
    /// The row's text line and the blank line it owns after it.
    height: u16,
    touch: bool,
    focused: bool,
}

/// One Tab or header row: its fill across the whole width on its text line, its content, the
/// current Tab's close, and its strip target over its lines.
#[expect(
    clippy::too_many_arguments,
    reason = "one row paint takes its place, model and shared ports together"
)]
fn paint_row(
    frame: &mut Frame<'_>,
    place: RowPlace,
    row: &Row,
    token: &Token,
    source: Option<&crate::action_menu::DrawnTabCreate>,
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
    halos: &mut Halos,
    hits: &mut HitMapBuilder,
) {
    let RowPlace {
        area,
        inner,
        y,
        height,
        touch,
        focused,
    } = place;
    let line = Rect::new(inner.x, y, inner.width, 1);
    let (active, checked) = match token.kind {
        Kind::Tab {
            active, checked, ..
        } => (active, checked),
        Kind::Header {
            current, checked, ..
        } => (current, checked),
        Kind::Divider => (false, false),
    };
    let close_tab = match &row.target {
        TabTarget::Tab(tab) if active => source.map(|source| (source, tab)),
        _ => None,
    };
    let close_x = close_tab.and_then(|_| trailing_x(line, None, touch));
    let close_width = if touch {
        TOUCH_ICON_ACTION_WIDTH + 2
    } else {
        ICON_WIDTH
    };
    let close_target = close_x.map(|x| {
        let start = if touch { x - 1 } else { x };
        Rect::new(
            start,
            y,
            area.right().saturating_sub(start).min(close_width),
            height,
        )
    });
    let target_rect = Rect::new(
        area.x,
        y,
        close_target.map_or(area.width, |close| close.x - area.x),
        height,
    );
    let hit = row.target.hit_target();
    let pointer = source.map_or(ProjectedPointer::Rest, |source| {
        hits.projected_tab_pointer_on(LAYER, source, &hit, target_rect)
    });
    let fill = match pointer {
        ProjectedPointer::Pressed => Some(Role::SurfacePressed),
        _ if focused => Some(Role::SurfaceSelection),
        ProjectedPointer::Hovered => Some(Role::SurfaceHover),
        ProjectedPointer::Rest if checked => Some(Role::SurfaceSelection),
        ProjectedPointer::Rest if active => Some(Role::SurfaceCurrent),
        ProjectedPointer::Rest => None,
    };
    let band = Rect::new(area.x, y, area.width, 1);
    let on = |role: Option<Role>, content: Style| {
        theme.content_on_surface(Role::SurfaceChrome, fill, role, content)
    };
    frame
        .buffer_mut()
        .set_style(band, on(None, theme.style(Role::SurfaceChrome)));
    halos.fill(theme, band, fill);

    let working_frame = view.machines.working_frame();
    let unread = token.agent.is_some_and(|agent| agent.unread);
    let glyph = token
        .agent
        .map(|agent| (state_glyph(agent.state, theme, working_frame), agent));
    let mut cue = PartCue {
        states: &token.states,
        unread,
        ..PartCue::default()
    };
    let mut cut = None;
    let write = |frame: &mut Frame<'_>, x: u16, text: &str, style: Style, end: u16| -> Rect {
        let (next, _) =
            frame
                .buffer_mut()
                .set_stringn(x, y, text, usize::from(end.saturating_sub(x)), style);
        Rect::new(x, y, next.saturating_sub(x), 1)
    };
    let list_width = inner.width;
    match &token.kind {
        Kind::Header {
            color,
            collapsed,
            name,
            ..
        } => {
            let accent = Style::new().fg(theme.tab_groups.accent(*color));
            let label = truncate_display(
                name,
                usize::from(list_width.saturating_sub(3)),
                theme.glyphs.ellipsis,
            );
            if label != *name {
                cut = Some(name.clone());
            }
            let painted = write(
                frame,
                inner.x + 1,
                &label,
                on(None, accent.add_modifier(Modifier::BOLD)),
                inner.right(),
            );
            let mut x = painted.right();
            if *collapsed {
                x = write(
                    frame,
                    x,
                    &format!(" {}", theme.glyphs.disclosure_closed),
                    on(None, accent),
                    inner.right(),
                )
                .right();
                // A collapsed group carries its hidden members' cue, as on the strip.
                if let Some((glyph, agent)) = glyph {
                    let glyph_x = x + 1;
                    let rect = write(
                        frame,
                        glyph_x,
                        glyph,
                        on(
                            Some(Theme::agent_role(agent.state)),
                            theme.agent_style(agent.state, working_frame),
                        ),
                        inner.right(),
                    );
                    cue.glyph = (!rect.is_empty()).then_some(rect);
                    x = rect.right();
                }
                if unread {
                    let rect = write(
                        frame,
                        x + 1,
                        theme.glyphs.status_bullet,
                        on(Some(Role::StatusUnread), theme.style(Role::StatusUnread)),
                        inner.right(),
                    );
                    cue.dot = (!rect.is_empty()).then_some(rect);
                }
            }
        }
        Kind::Tab { full, .. } => {
            let member = token.header.is_some();
            let gx = inner.x + 1 + if member { 2 } else { 0 };
            if let Some((glyph, agent)) = glyph {
                let rect = write(
                    frame,
                    gx,
                    glyph,
                    on(
                        Some(Theme::agent_role(agent.state)),
                        theme.agent_style(agent.state, working_frame),
                    ),
                    inner.right(),
                );
                cue.glyph = (!rect.is_empty()).then_some(rect);
            }
            let dot = close_x.is_none() && unread;
            let end = close_x.map_or_else(
                || inner.right().saturating_sub(1 + if dot { 2 } else { 0 }),
                |x| x.saturating_sub(2),
            );
            let room = usize::from(end.saturating_sub(gx).saturating_sub(1));
            let label = truncate_display(full, room.max(1), theme.glyphs.ellipsis);
            if label != *full {
                cut = Some(full.clone());
            }
            write(
                frame,
                gx + 2,
                &label,
                on(Some(Role::TextPrimary), theme.style(Role::TextPrimary)),
                end.saturating_add(1),
            );
            if dot {
                let rect = write(
                    frame,
                    inner.right().saturating_sub(1),
                    theme.glyphs.status_bullet,
                    on(Some(Role::StatusUnread), theme.style(Role::StatusUnread)),
                    inner.right(),
                );
                cue.dot = (!rect.is_empty()).then_some(rect);
            }
        }
        Kind::Divider => {}
    }
    if touch {
        halos.text(line);
    }

    if let (Some((source, tab)), Some(x), Some(target)) = (close_tab, close_x, close_target) {
        paint_close(
            frame,
            CloseButton {
                x,
                y,
                target,
                touch,
                fill,
                interaction: InteractionTarget::TabCloseRequest {
                    source: source.clone(),
                    tab: tab.clone(),
                },
            },
            view,
            locale,
            theme,
            halos,
            hits,
        );
    }

    let Some(source) = source else {
        return;
    };
    cue.name = cut.as_deref();
    for (glyph, content) in super::strip_glyph_tooltips(view, cue, locale, theme) {
        hits.record_target_part_tooltip(LAYER, glyph, target_rect, hit.clone(), content);
    }
    if let Some(text) = cut
        && let Some(content) = crate::tui_render::tooltip::describe(
            Some(TooltipText {
                text,
                middle: false,
            }),
            Vec::new(),
        )
    {
        hits.record_tooltip(LAYER, target_rect, hit.clone(), content);
    }
    hits.record_tab_target_on(LAYER, source, target_rect, hit);
}

struct CloseButton {
    x: u16,
    y: u16,
    target: Rect,
    touch: bool,
    /// The row's fill, which the close sits on at rest.
    fill: Option<Role>,
    interaction: InteractionTarget,
}

/// The current Tab's close at the end of its row: `[ x ]` with a target one cell wider on each
/// side across the row's lines when spaced, else `[x]`. A press fills its target on the text line.
fn paint_close(
    frame: &mut Frame<'_>,
    button: CloseButton,
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
    halos: &mut Halos,
    hits: &mut HitMapBuilder,
) {
    let CloseButton {
        x,
        y,
        target,
        touch,
        fill,
        interaction,
    } = button;
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let pointer = hits.projected_pointer(LAYER, target, target, &interaction, spec);
    let pressed = pointer == ProjectedPointer::Pressed;
    let fill = if pressed {
        Some(Role::SurfacePressed)
    } else {
        fill
    };
    if pressed {
        let band = Rect::new(target.x, y, target.width, 1);
        frame.buffer_mut().set_style(
            band,
            theme.content_on_surface(Role::SurfaceChrome, fill, None, Style::new()),
        );
        halos.fill(theme, band, fill);
    }
    let role = if pointer == ProjectedPointer::Rest {
        Role::TextSecondary
    } else {
        Role::TextPrimary
    };
    let mut content = theme.style(role);
    if pressed {
        content = content.add_modifier(Modifier::BOLD);
    }
    let text = if touch { "[ x ]" } else { "[x]" };
    frame.buffer_mut().set_stringn(
        x,
        y,
        text,
        usize::from(target.right().saturating_sub(x)),
        theme.content_on_surface(Role::SurfaceChrome, fill, Some(role), content),
    );
    hits.record_interaction_tooltip(
        LAYER,
        target,
        &interaction,
        crate::tui_hit_map::TooltipContent::Action {
            label: locale.menu_action(MenuLabel::CloseSelectedTabs).to_owned(),
            key: crate::tui_render::tooltip::action_key(
                view,
                crate::client_view::ClientAction::RequestCloseTab,
                theme,
            ),
        },
    );
    hits.record_interaction(LAYER, target, target, interaction, spec);
}
