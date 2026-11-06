//! The Mobile tier's top strip: the sidebar toggle, the current Tab centered on the bar, and the
//! Tabs button that opens the Tab list. A press anywhere between the two buttons opens the list
//! too. From the touch-spacing height the bar takes three rows: its content row and a blank line
//! on each side, which its fills show as halves and its targets share. A shorter bar closes up to
//! its content row and its buttons drop the blank cell inside each bracket.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
};

use super::{
    controls::{Availability, ControlSurface, PointerPresentation, icon_action_style},
    sidebar_chrome::{self, TouchRow},
    sidebar_text::state_glyph,
    sidebar_touch::Halos,
    tabs::pointer_presentation,
};
use crate::{
    client_view::ClientView,
    localization::Locale,
    machine_picker::TabAgent,
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_hit_map::{
        GlobalChromeAction, HitLayer, HitMapBuilder, InteractionSpec, InteractionTarget,
        PrimaryContract, TooltipText,
    },
    tui_theme::{Role, Theme},
};

/// Blank cells the current Tab's fill keeps inside the space between the buttons' targets.
const GAP: u16 = 2;
/// The fewest name cells the current Tab keeps beside the Tabs button's word; a narrower bar
/// shows the count alone.
const MINIMUM_NAME_CELLS: u16 = 8;
/// The current Tab's fill pads its content by one cell on each side; the word rule also reserves
/// a state glyph and its space.
const NAME_RESERVE: u16 = 4;

/// What the bar centers: the current Tab, or a pending Workspace's first Tab.
pub(super) struct Current<'a> {
    pub(super) label: std::borrow::Cow<'a, str>,
    pub(super) agent: Option<TabAgent>,
}

pub(super) struct Context<'a> {
    pub(super) view: &'a ClientView,
    pub(super) locale: Locale,
    pub(super) theme: &'a Theme,
    pub(super) current: Option<Current<'a>>,
    /// Tabs the list would show, group headers and the pinned rule aside.
    pub(super) count: usize,
    /// The sidebar can open, so the bar leads with its toggle.
    pub(super) toggle: bool,
}

const fn open_target() -> InteractionTarget {
    InteractionTarget::GlobalChrome(GlobalChromeAction::OpenTabList)
}

fn spec() -> InteractionSpec {
    InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
}

fn fill_style(theme: &Theme, fill: Option<Role>, role: Option<Role>, content: Style) -> Style {
    theme.content_on_surface(Role::SurfaceStrip, fill, role, content)
}

/// The Workbench Client's view of the bar: its current Tab, or the pending Workspace's first Tab
/// while that replaces the content, and the Tabs the strip would show.
pub(super) struct Native<'a, 'n> {
    pub(super) view: &'a ClientView,
    pub(super) native: &'a crate::workbench_client::WorkbenchClientView<'n>,
    pub(super) locale: Locale,
    pub(super) theme: &'a Theme,
    pub(super) toggle: bool,
}

pub(super) fn render_native(
    frame: &mut Frame<'_>,
    bar: Rect,
    source: Native<'_, '_>,
    hits: &mut HitMapBuilder,
) {
    let Native {
        view,
        native,
        locale,
        theme,
        toggle,
    } = source;
    let pending = native
        .workspace_creation_preview()
        .filter(|preview| preview.replaces_content());
    let shown = (0..native.tab_count())
        .filter_map(|index| native.tab(index))
        .filter(|tab| !native.tab_leaving(tab.tab_id()));
    let (count, current) = match pending {
        Some(preview) => (
            1,
            Some(Current {
                label: super::workspace_creation::pending_tab_label(preview, locale).into(),
                agent: None,
            }),
        ),
        None => {
            let mut count = 0;
            let mut current = None;
            for tab in shown {
                count += 1;
                if tab.active() {
                    current = Some(Current {
                        label: tab.display_label(locale),
                        agent: view.machines.tab_agent(tab.tab_id()),
                    });
                }
            }
            (count, current)
        }
    };
    render(
        frame,
        bar,
        &Context {
            view,
            locale,
            theme,
            current,
            count,
            toggle,
        },
        hits,
    );
}

/// Paints the bar over `bar`, the strip's whole area.
pub(super) fn render(
    frame: &mut Frame<'_>,
    bar: Rect,
    context: &Context<'_>,
    hits: &mut HitMapBuilder,
) {
    let bar = bar.intersection(frame.area());
    if bar.is_empty() {
        return;
    }
    let Context {
        view,
        locale,
        theme,
        ref current,
        count,
        toggle,
    } = *context;
    let touch = bar.height >= crate::tui_layout::MOBILE_BAR_TOUCH_ROWS;
    let row = Rect::new(bar.x, bar.y + u16::from(touch), bar.width, 1);
    let pad = u16::from(touch);
    let closed = u16::from(!touch);
    let place = TouchRow {
        row,
        top: bar.y,
        height: bar.height,
    };
    // Only the content row is the strip; the lines beside it show half of its fill, so the bar
    // reads as centered on its target.
    frame
        .buffer_mut()
        .set_style(bar, theme.style(Role::SurfaceCanvas));
    frame
        .buffer_mut()
        .set_style(row, theme.style(Role::SurfaceStrip));
    let mut halos = Halos::new(bar);
    halos.text(row);
    let layer = HitLayer::Foreground;

    let toggle_target = toggle
        .then(|| {
            sidebar_chrome::render_bar_toggle(
                frame,
                place,
                touch,
                &sidebar_chrome::Context {
                    view,
                    locale,
                    theme,
                    layer,
                },
                &mut halos,
                hits,
            )
        })
        .flatten();
    // The space between the buttons' targets starts a blank cell after a closed-up button.
    let from = toggle_target.map_or(bar.x, |target| target.right() + closed);

    let count_text = count.to_string();
    let label = locale.tab_list_label();
    let named = format!("{label} {count_text}");
    let end_width =
        |text: &str| 2 + 2 * pad + u16::try_from(display_width(text)).unwrap_or(u16::MAX);
    let room = |end: u16| {
        bar.right()
            .saturating_sub(from)
            .saturating_sub(pad + end + pad + closed)
            .saturating_sub(2 * GAP)
    };
    let word = room(end_width(&named)).saturating_sub(NAME_RESERVE) >= MINIMUM_NAME_CELLS;
    let end_text = if word { &named } else { &count_text };
    let Some(end_x) = bar
        .right()
        .checked_sub(pad + end_width(end_text))
        .filter(|x| x.saturating_sub(pad) >= from)
    else {
        finish(frame, row, theme, halos);
        return;
    };
    let button_target = Rect::new(
        end_x - pad,
        bar.y,
        bar.right().saturating_sub(end_x - pad),
        bar.height,
    );
    paint_button(
        frame,
        PaintedButton {
            x: end_x,
            row,
            target: button_target,
            label: word.then_some(label),
            count: &count_text,
            touch,
        },
        theme,
        &mut halos,
        hits,
    );

    // The title's target fills the space between the buttons' targets.
    let to = (end_x - pad).saturating_sub(1 + closed);
    if to >= from {
        let title = Rect::new(from, bar.y, to - from + 1, bar.height);
        paint_title(
            frame,
            title,
            row,
            current.as_ref(),
            view,
            theme,
            &mut halos,
            hits,
        );
    }
    finish(frame, row, theme, halos);
}

/// Shows the halves of every fill, the strip's own last so the fills painted over it win.
fn finish(frame: &mut Frame<'_>, row: Rect, theme: &Theme, mut halos: Halos) {
    halos.fill(theme, row, Some(Role::SurfaceStrip));
    halos.paint(frame, theme);
}

struct PaintedButton<'a> {
    x: u16,
    row: Rect,
    target: Rect,
    /// The word before the count; none shows the count alone, as an icon action.
    label: Option<&'a str>,
    count: &'a str,
    touch: bool,
}

/// `[ Tabs 8 ]`, or `[ 8 ]` alone: hover and press fill the target's band on the content row.
fn paint_button(
    frame: &mut Frame<'_>,
    button: PaintedButton<'_>,
    theme: &Theme,
    halos: &mut Halos,
    hits: &mut HitMapBuilder,
) {
    let PaintedButton {
        x,
        row,
        target,
        label,
        count,
        touch,
    } = button;
    let pointer = pointer_presentation(hits.projected_pointer(
        HitLayer::Foreground,
        target,
        target,
        &open_target(),
        spec(),
    ));
    let fill = match pointer {
        PointerPresentation::Rest => None,
        PointerPresentation::Hovered => label.is_some().then_some(Role::SurfaceHover),
        PointerPresentation::Pressed => Some(Role::SurfacePressed),
    };
    let band = Rect::new(target.x, row.y, target.width, 1);
    frame.buffer_mut().set_style(
        band,
        fill_style(theme, fill, None, theme.style(Role::SurfaceStrip)),
    );
    halos.fill(theme, band, fill);
    let inner = if touch { " " } else { "" };
    let (open, close) = (format!("[{inner}"), format!("{inner}]"));
    let pieces: Vec<(&str, Style)> = match label {
        Some(label) => {
            let primary = fill_style(
                theme,
                fill,
                Some(Role::TextPrimary),
                theme.style(Role::TextPrimary),
            );
            let word = if pointer == PointerPresentation::Pressed {
                primary.add_modifier(Modifier::BOLD)
            } else {
                primary
            };
            let quiet = fill_style(
                theme,
                fill,
                Some(Role::TextSecondary),
                theme.style(Role::TextSecondary),
            );
            vec![
                (&open, primary),
                (label, word),
                (" ", primary),
                (count, quiet),
                (&close, primary),
            ]
        }
        None => {
            let style =
                icon_action_style(theme, ControlSurface::Strip, Availability::Enabled, pointer)
                    .patch(fill_style(theme, fill, None, Style::new()));
            vec![(&open, style), (count, style), (&close, style)]
        }
    };
    let mut column = x;
    for (text, style) in pieces {
        let (next, _) = frame.buffer_mut().set_stringn(
            column,
            row.y,
            text,
            usize::from(row.right().saturating_sub(column)),
            style,
        );
        column = next;
    }
    hits.record_interaction(HitLayer::Foreground, target, target, open_target(), spec());
}

/// The current Tab centered on the bar and held [`GAP`] cells inside `title`, the space between the
/// buttons' targets, which opens the Tab list as a whole. At rest the Tab shows its current fill;
/// a press fills the whole space.
#[expect(
    clippy::too_many_arguments,
    reason = "one title paint takes its geometry, content and shared ports together"
)]
fn paint_title(
    frame: &mut Frame<'_>,
    title: Rect,
    row: Rect,
    current: Option<&Current<'_>>,
    view: &ClientView,
    theme: &Theme,
    halos: &mut Halos,
    hits: &mut HitMapBuilder,
) {
    let pointer = pointer_presentation(hits.projected_pointer(
        HitLayer::Foreground,
        title,
        title,
        &open_target(),
        spec(),
    ));
    let pressed = pointer == PointerPresentation::Pressed;
    if pressed {
        let band = Rect::new(title.x, row.y, title.width, 1);
        frame.buffer_mut().set_style(
            band,
            fill_style(
                theme,
                Some(Role::SurfacePressed),
                None,
                theme.style(Role::SurfaceStrip),
            ),
        );
        halos.fill(theme, band, Some(Role::SurfacePressed));
    }
    let mut cut = None;
    if let Some(current) = current {
        let working_frame = view.machines.working_frame();
        let lo = title.x + GAP;
        let hi = title.right().saturating_sub(1 + GAP);
        let glyph = current
            .agent
            .map(|agent| state_glyph(agent.state, theme, working_frame));
        let unread = current.agent.is_some_and(|agent| agent.unread);
        let fixed = 2
            + glyph.map_or(0, |glyph| display_width(glyph) + 1)
            + if unread {
                1 + display_width(theme.glyphs.status_bullet)
            } else {
                0
            };
        let full = sanitize_terminal_text(&current.label);
        let room = usize::from((hi + 1).saturating_sub(lo))
            .saturating_sub(fixed)
            .max(1);
        let name = truncate_display(&full, room, theme.glyphs.ellipsis);
        if name != full {
            cut = Some(full.into_owned());
        }
        let width = u16::try_from(fixed + display_width(&name)).unwrap_or(u16::MAX);
        let centered = row.x + row.width.saturating_sub(width) / 2;
        let x = centered.min((hi + 1).saturating_sub(width)).max(lo);
        let band = Rect::new(x, row.y, width, 1).intersection(title);
        let fill = if pressed {
            Role::SurfacePressed
        } else {
            Role::SurfaceCurrent
        };
        if !pressed {
            frame.buffer_mut().set_style(
                band,
                fill_style(theme, Some(fill), None, theme.style(Role::SurfaceStrip)),
            );
            halos.fill(theme, band, Some(fill));
        }
        let primary = fill_style(
            theme,
            Some(fill),
            Some(Role::TextPrimary),
            theme.style(Role::TextPrimary),
        )
        .add_modifier(Modifier::BOLD);
        let mut pieces: Vec<(&str, Style)> = Vec::new();
        if let (Some(glyph), Some(agent)) = (glyph, current.agent) {
            pieces.push((
                glyph,
                fill_style(
                    theme,
                    Some(fill),
                    Some(Theme::agent_role(agent.state)),
                    theme.agent_style(agent.state, working_frame),
                ),
            ));
            pieces.push((" ", primary));
        }
        pieces.push((&name, primary));
        if unread {
            pieces.push((" ", primary));
            pieces.push((
                theme.glyphs.status_bullet,
                fill_style(
                    theme,
                    Some(fill),
                    Some(Role::StatusUnread),
                    theme.style(Role::StatusUnread),
                ),
            ));
        }
        let mut column = band.x + 1;
        for (text, style) in pieces {
            let (next, _) = frame.buffer_mut().set_stringn(
                column,
                row.y,
                text,
                usize::from(band.right().saturating_sub(column)),
                style,
            );
            column = next;
        }
    }
    // A cut name shows whole in the title's tooltip.
    if let Some(text) = cut
        && let Some(content) = super::tooltip::describe(
            Some(TooltipText {
                text,
                middle: false,
            }),
            Vec::new(),
        )
    {
        hits.record_interaction_tooltip(HitLayer::Foreground, title, &open_target(), content);
    }
    hits.record_interaction(HitLayer::Foreground, title, title, open_target(), spec());
}
