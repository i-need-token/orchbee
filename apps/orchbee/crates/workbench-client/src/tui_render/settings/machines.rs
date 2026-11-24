//! Settings › Machines' rows: the list's two tool lines above its card, this computer, each saved
//! machine, and a machine's page. Their words come from the machine facts the page mirrors; the
//! parent draws frames, headings and notes.
use super::{
    BASE, BUTTON_NAME_MINIMUM, END_INSET, NAME_INSET, fill, focused, interaction_spec, on,
    paint_sides, pointer_surface, put, render_field, update_action_spec,
};
use crate::{
    action_projection::ActionTone,
    localization::{Locale, MachineText},
    machine_capacity,
    machine_color::MachineColor,
    machine_settings::{Machine, State},
    native_settings::{Choice, PageView, Row},
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_hit_map::{HitLayer, HitMapBuilder, InteractionTarget, ProjectedPointer},
    tui_render::controls::{QueryField, action_rect, render_action, render_query_field},
    tui_theme::{Role, Theme},
};
use ratatui::{Frame, layout::Rect};

/// Blank cells between a row's name and the state word after it.
const TAG_GAP: usize = 2;

/// Whether `row` belongs to Settings › Machines, which this module draws.
pub(super) const fn draws(row: Row) -> bool {
    matches!(
        row,
        Row::AddMachine
            | Row::FindMachine
            | Row::ThisComputer
            | Row::Machine(_)
            | Row::MachineName
            | Row::MachineColor
            | Row::MachineAddress
            | Row::MachineStatus
            | Row::MachineFree
            | Row::MachinePassword
            | Row::MachineStop
            | Row::MachineRemove
    )
}

/// One row of Settings › Machines in `rect`: a tool line above the card, or a framed row whose
/// first line is drawn when `first_drawn`.
#[expect(clippy::too_many_arguments, reason = "one row's drawing inputs")]
pub(super) fn render_row(
    frame: &mut Frame<'_>,
    rect: Rect,
    first_drawn: bool,
    row: Row,
    page: PageView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    match row {
        Row::AddMachine | Row::FindMachine => {
            render_tool(frame, rect, row, page, locale, theme, hits)
        }
        _ => {
            if let Some(parts) = parts(row, page, locale) {
                render_framed(frame, rect, first_drawn, row, page, parts, theme, hits);
            }
        }
    }
}

/// What one framed row shows.
struct Parts {
    name: String,
    /// The name stays readable while the row's action cannot run, as Status's does.
    name_always: bool,
    /// A word after the name in its own color: a machine's state on its Status row.
    tag: Option<(&'static str, Role)>,
    /// The text at the row's end, where no button is offered.
    value: Option<(String, Role)>,
    /// Activating the row opens a page, which the marker after its value says.
    opens: bool,
    /// Activating the row opens a list under it, which the marker after its value says.
    lists: bool,
    /// A saved machine's color: its name on a list row, or the value of its Color row.
    name_color: Option<MachineColor>,
    value_color: Option<MachineColor>,
    /// The row's one action, a labeled button at its end.
    button: Option<(&'static str, ActionTone)>,
    /// The line under the name.
    description: (String, Role),
}

impl Parts {
    fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            name_always: false,
            tag: None,
            value: None,
            opens: false,
            lists: false,
            name_color: None,
            value_color: None,
            button: None,
            description: (description.into(), Role::TextMuted),
        }
    }
}

/// The color of a machine's state word, as the machine list's tone gives it.
pub(in crate::tui_render) const fn state_role(machine: &Machine) -> Role {
    if machine.stopping {
        return Role::StatusPending;
    }
    match machine.state {
        State::Online => Role::StatusSuccess,
        State::Connecting | State::Reconnecting => Role::StatusPending,
        State::NeedsUpdate | State::LocalNeedsUpdate | State::Other(_) => Role::StatusWarning,
        State::Offline | State::Disabled | State::NeedsLogin => Role::TextSecondary,
    }
}

fn parts(row: Row, page: PageView<'_>, locale: Locale) -> Option<Parts> {
    let facts = page.machine_facts();
    let label = |machine: &Machine| sanitize_terminal_text(&machine.label).into_owned();
    let address = |machine: &Machine| {
        machine.address.as_deref().map_or_else(
            || machine_capacity::UNKNOWN.to_owned(),
            |address| sanitize_terminal_text(address).into_owned(),
        )
    };
    if let Row::Machine(index) = row {
        let machine = page.listed_machine(index)?;
        let end = if machine.state == State::Online && !machine.stopping {
            (
                machine_capacity::summary(machine.capacity.as_ref()),
                Role::TextSecondary,
            )
        } else {
            (
                locale.machine_state_word(machine).to_owned(),
                state_role(machine),
            )
        };
        let mut parts = Parts::new(label(machine), address(machine));
        parts.name_always = true;
        parts.value = Some(end);
        parts.opens = true;
        parts.name_color = Some(machine.color);
        return Some(parts);
    }
    if row == Row::ThisComputer {
        let note = if facts.machines.is_empty() {
            MachineText::NoMachine
        } else {
            MachineText::ColumnNote
        };
        let mut parts = Parts::new(row.name(locale), locale.machine_text(note));
        parts.value = Some((
            machine_capacity::summary(facts.local.as_ref()),
            Role::TextSecondary,
        ));
        return Some(parts);
    }
    let machine = page.shown_machine()?;
    let mut parts = Parts::new(row.name(locale), row.description(locale));
    match row {
        Row::MachineName => parts.value = Some((label(machine), Role::TextSecondary)),
        Row::MachineColor => {
            parts.value = Some((
                locale
                    .settings_choice(Choice::MachineColor(machine.color))
                    .to_owned(),
                Role::TextPrimary,
            ));
            parts.lists = true;
            parts.value_color = Some(machine.color);
        }
        Row::MachineAddress => parts.value = Some((address(machine), Role::TextSecondary)),
        Row::MachineStatus => {
            parts.name_always = true;
            parts.tag = Some((locale.machine_state_word(machine), state_role(machine)));
            parts.button = machine
                .action()
                .map(|action| (locale.machine_action(action), ActionTone::Ordinary));
            parts.description.0 = locale.machine_status(machine);
        }
        Row::MachineFree => {
            parts.description.0 = locale.machine_capacity_long(machine.capacity.as_ref());
        }
        Row::MachinePassword => {
            parts.button = Some((
                locale.machine_text(MachineText::Forget),
                ActionTone::Ordinary,
            ));
        }
        Row::MachineStop => {
            parts.button = Some((
                locale.machine_text(MachineText::Stop),
                ActionTone::Destructive,
            ));
            if page.confirm_stop() {
                parts.description = (
                    locale.machine_stop_confirmation(machine),
                    Role::StatusWarning,
                );
            } else if machine.stopping {
                parts.description = (
                    locale.machine_text(MachineText::Stopping).to_owned(),
                    Role::StatusPending,
                );
            } else if machine.state != State::Online {
                parts.description.0 = locale.machine_not_connected(&machine.label);
            }
        }
        Row::MachineRemove => {
            parts.button = Some((
                locale.machine_text(MachineText::Remove),
                ActionTone::Destructive,
            ));
            parts.description.0 = locale.machine_remove_description(machine.workspaces);
        }
        _ => return None,
    }
    Some(parts)
}

/// A framed two-line row: name, state word and the value or button on the first line, the
/// description, a confirmation, or the open name field on the second.
#[expect(clippy::too_many_arguments, reason = "one row's drawing inputs")]
fn render_framed(
    frame: &mut Frame<'_>,
    rect: Rect,
    first_drawn: bool,
    row: Row,
    page: PageView<'_>,
    parts: Parts,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let enabled = page.enabled(row);
    let is_focused = focused(page, row);
    let target = InteractionTarget::SettingsRow {
        token: page.token(),
        row,
    };
    let spec = interaction_spec(enabled);
    let inner = Rect::new(
        rect.x + 1,
        rect.y,
        rect.width.saturating_sub(2),
        rect.height,
    );
    let pointer = hits.projected_pointer(HitLayer::Overlay, inner, inner, &target, spec);
    let name_x = rect.x + NAME_INSET;
    let end = rect.right().saturating_sub(END_INSET);
    let available = usize::from(end.saturating_sub(name_x));
    // A button is painted whole at the row's end and the name gives up cells to it; where it
    // cannot fit beside the least a name needs, the row shows no button.
    let button = parts
        .button
        .filter(|_| first_drawn)
        .and_then(|(label, tone)| {
            let rest = update_action_spec(label, enabled, ProjectedPointer::Rest).with_tone(tone);
            let slot = Rect::new(name_x, rect.y, end.saturating_sub(name_x), 1);
            let width = action_rect(frame.area(), slot, rest, theme)?.width;
            (usize::from(width) + 1 + BUTTON_NAME_MINIMUM <= available)
                .then_some((label, tone, width))
        });
    // The whole row is the button's target, and the button alone shows the pointer.
    let interaction = pointer_surface(pointer)
        .filter(|_| button.is_none())
        .or(is_focused.then_some(Role::SurfaceSelection));
    for y in rect.y..rect.bottom() {
        paint_sides(frame, rect, y, theme);
    }
    fill(frame, inner, BASE, interaction, theme);
    let style = |role| on(theme, BASE, interaction, role);
    // The colored part of the value: all of it before its marker.
    let value_width = parts
        .value
        .as_ref()
        .map_or(0, |(text, _)| display_width(text));
    let value = parts.value.map(|(text, role)| {
        if parts.opens {
            (format!("{text} {}", theme.glyphs.navigation_marker), role)
        } else if parts.lists {
            (format!("{text} {}", theme.glyphs.disclosure_open), role)
        } else {
            (text, role)
        }
    });
    if first_drawn {
        let y = rect.y;
        if is_focused {
            put(
                frame,
                rect.x + 1,
                y,
                1,
                theme.glyphs.navigation_marker,
                style(Role::AccentFocus),
            );
        }
        let end_width = match (button, &value) {
            (Some((_, _, width)), _) => usize::from(width),
            (None, Some((text, _))) => display_width(text).min(available / 2),
            (None, None) => 0,
        };
        let name_budget = available.saturating_sub(end_width + 1);
        let name = truncate_display(&parts.name, name_budget, theme.glyphs.ellipsis);
        let name_role = if enabled || parts.name_always {
            Role::TextPrimary
        } else {
            Role::TextDisabled
        };
        let name_style = match parts.name_color {
            Some(color) if name_role == Role::TextPrimary => {
                theme.machine_name_style(color, interaction, style(name_role))
            }
            _ => style(name_role),
        };
        put(
            frame,
            name_x,
            y,
            u16::try_from(name_budget).unwrap_or(0),
            &name,
            name_style,
        );
        if let Some((tag, role)) = parts.tag {
            let lead = display_width(&name) + TAG_GAP;
            let room = name_budget.saturating_sub(lead);
            if room > 0 {
                put(
                    frame,
                    name_x + u16::try_from(lead).unwrap_or(u16::MAX),
                    y,
                    u16::try_from(room).unwrap_or(0),
                    tag,
                    style(role),
                );
            }
        }
        if let Some((label, tone, width)) = button {
            let painted = render_action(
                frame,
                Rect::new(end - width, y, width, 1),
                update_action_spec(label, enabled, pointer).with_tone(tone),
                theme,
            );
            debug_assert_eq!(painted, Some(Rect::new(end - width, y, width, 1)));
        } else if let Some((text, role)) = &value {
            let text = truncate_display(text, end_width, theme.glyphs.ellipsis);
            let x = end.saturating_sub(u16::try_from(display_width(&text)).unwrap_or(0));
            put(
                frame,
                x,
                y,
                u16::try_from(end_width).unwrap_or(0),
                &text,
                style(*role),
            );
            if let Some(color) = parts.value_color.filter(|_| enabled) {
                let width = value_width.min(display_width(&text));
                frame.buffer_mut().set_style(
                    Rect::new(x, y, u16::try_from(width).unwrap_or(0), 1),
                    theme.machine_name_style(color, interaction, style(*role)),
                );
            }
        }
    }
    let second = if first_drawn { rect.y + 1 } else { rect.y };
    if second < rect.bottom() {
        let width = end.saturating_sub(name_x);
        if is_focused && let Some((text, cursor)) = page.text() {
            render_field(
                frame,
                Rect::new(name_x, second, width, 1),
                text,
                cursor,
                theme,
            );
        } else {
            let (description, role) = &parts.description;
            put(frame, name_x, second, width, description, style(*role));
        }
    }
    hits.record_interaction(HitLayer::Overlay, inner, inner, target, spec);
}

/// A one-line tool above the list's card: `Add machine…` as a button at the line's end, or the
/// filter's field.
fn render_tool(
    frame: &mut Frame<'_>,
    rect: Rect,
    row: Row,
    page: PageView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let enabled = page.enabled(row);
    let is_focused = focused(page, row);
    let target = InteractionTarget::SettingsRow {
        token: page.token(),
        row,
    };
    let spec = interaction_spec(enabled);
    let inner = Rect::new(rect.x + 1, rect.y, rect.width.saturating_sub(2), 1);
    let pointer = hits.projected_pointer(HitLayer::Overlay, inner, inner, &target, spec);
    let name_x = rect.x + NAME_INSET;
    let end = rect.right().saturating_sub(END_INSET);
    let slot = Rect::new(name_x, rect.y, end.saturating_sub(name_x), 1);
    let label = row.name(locale);
    let button = (row == Row::AddMachine)
        .then(|| {
            let rest = update_action_spec(label, enabled, ProjectedPointer::Rest);
            action_rect(frame.area(), slot, rest, theme).map(|button| button.width)
        })
        .flatten();
    let interaction = pointer_surface(pointer)
        .filter(|_| button.is_none())
        .or(is_focused.then_some(Role::SurfaceSelection));
    fill(frame, inner, BASE, interaction, theme);
    let style = |role| on(theme, BASE, interaction, role);
    if is_focused {
        put(
            frame,
            rect.x + 1,
            rect.y,
            1,
            theme.glyphs.navigation_marker,
            style(Role::AccentFocus),
        );
    }
    match (row, button) {
        (Row::AddMachine, Some(width)) => {
            let painted = render_action(
                frame,
                Rect::new(end - width, rect.y, width, 1),
                update_action_spec(label, enabled, pointer),
                theme,
            );
            debug_assert_eq!(painted, Some(Rect::new(end - width, rect.y, width, 1)));
        }
        (Row::FindMachine, _) => {
            let query = page.machine_query();
            let field = match page.text().filter(|_| is_focused) {
                Some((draft, cursor)) => Some(QueryField::Editing {
                    label,
                    query: draft,
                    cursor,
                }),
                None if !query.is_empty() => Some(QueryField::Retained { label, query }),
                None => None,
            };
            if field.is_none_or(|field| render_query_field(frame, slot, field, theme).is_none()) {
                put(
                    frame,
                    slot.x,
                    slot.y,
                    slot.width,
                    label,
                    style(Role::TextSecondary),
                );
            }
        }
        _ => put(
            frame,
            slot.x,
            slot.y,
            slot.width,
            label,
            style(Role::TextPrimary),
        ),
    }
    hits.record_interaction(HitLayer::Overlay, inner, inner, target, spec);
}
