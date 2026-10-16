//! Retained action-menu projection; only a complete successful frame arms input.

use crate::{
    action_menu::{MenuAction, MenuControl, MenuDraw, MenuView},
    action_projection::menu_binding_action,
    client_view::{
        Binding, Bindings, KeyCode, KeyLabelGlyphs, LabelStyle, Modifiers,
        format_binding_with_glyphs, join_key_labels,
    },
    localization::Locale,
    terminal_session::ModalPresentation,
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_pointer::{
        HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
        ProjectedPointer,
    },
    tui_theme::{GlyphMode, Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Clear, Padding, Paragraph},
};

use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    IconActionGlyph, IconActionSpec, InteractionState, KeyHint, NavigationState, OperationState,
    PointerPresentation, RowContentBand, RowFocusCue, RowTracks, measure_key_hints,
    paint_row_interaction, render_key_hints,
};

/// Keeps the right-aligned effective chord visually independent from the action label.
const ROW_BINDING_GAP: u16 = 2;

pub(super) fn render(
    frame: &mut Frame<'_>,
    content: Rect,
    menu: MenuView<'_>,
    bindings: &Bindings,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalPresentation<MenuDraw> {
    if content.width < 16 || content.height < if menu.context_menu { 3 } else { 5 } {
        return ModalPresentation::NotPresented;
    }

    let footer_rows = usize::from(!menu.context_menu);

    let key_glyphs = key_label_glyphs(theme);
    let binding_labels = menu
        .actions
        .iter()
        .map(|action| effective_binding(bindings, &menu, *action, key_glyphs))
        .collect::<Vec<_>>();
    let marker_width = display_width(theme.glyphs.navigation_marker);
    if marker_width == 0 {
        return ModalPresentation::NotPresented;
    }
    let desired = menu
        .actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let label = action_label(
                locale,
                *action,
                menu.labels[index].as_deref(),
                theme.glyphs.middle_dot,
                theme.glyphs.disclosure_closed,
            );
            marker_width
                .saturating_add(display_width(&label))
                .saturating_add(binding_labels[index].as_ref().map_or(0, |binding| {
                    display_width(binding).saturating_add(usize::from(ROW_BINDING_GAP))
                }))
                // The row owns one padding cell beside each frame edge.
                .saturating_add(2)
        })
        .max()
        .unwrap_or(16);
    let width = u16::try_from(desired.max(26))
        .unwrap_or(u16::MAX)
        .min(content.width);
    let reason_rows = usize::from(menu.unavailable.iter().any(Option::is_some));
    let height =
        u16::try_from(menu.row_height(0, menu.actions.len()) + 2 + footer_rows + reason_rows)
            .unwrap_or(u16::MAX)
            .min(content.height);
    let (x, y) = menu.anchor.unwrap_or((
        content.x + (content.width - width) / 2,
        content.y + (content.height - height) / 2,
    ));
    let area = Rect::new(
        x.clamp(content.x, content.right() - width),
        y.clamp(content.y, content.bottom() - height),
        width,
        height,
    );
    let legacy = matches!(
        menu.source,
        crate::action_menu::MenuTarget::Workspace(
            crate::terminal_session::NativeContextLocation::Standalone(_)
        )
    );
    let title = if menu.title.is_empty() && legacy {
        format!(
            "{} {} {}",
            locale.menu_title(),
            theme.glyphs.middle_dot,
            locale.shell().standalone
        )
    } else if menu.title.is_empty() {
        locale.menu_title().to_owned()
    } else {
        format!(
            "{} {} {}",
            locale.menu_title(),
            theme.glyphs.middle_dot,
            sanitize_terminal_text(menu.title)
        )
    };
    let block = super::overlay_surface(theme, true, Padding::horizontal(1));
    let block = if menu.context_menu {
        block
    } else {
        block.title(truncate_display(
            &title,
            usize::from(area.width.saturating_sub(6)),
            theme.glyphs.ellipsis,
        ))
    };
    let inner = block.inner(area);
    let rows = usize::from(inner.height).saturating_sub(footer_rows + reason_rows);
    if rows == 0 {
        return ModalPresentation::NotPresented;
    }
    let first = menu.first_visible_row(rows);
    let visible_rows = visible_action_rows(&menu, first, rows);
    if visible_rows == 0 {
        return ModalPresentation::NotPresented;
    }
    let selected_visible = (first..first.saturating_add(visible_rows)).contains(&menu.selected);

    let position = format!("{}/{}", menu.selected + 1, menu.actions.len());
    let position_width = u16::try_from(display_width(&position)).unwrap_or(u16::MAX);
    let Some(hints_width) = inner.width.checked_sub(position_width.saturating_add(1)) else {
        return ModalPresentation::NotPresented;
    };
    let footer_area = Rect::new(inner.x, inner.bottom() - 1, hints_width, 1);
    let exit_label = if menu.transferring {
        locale.menu_cancel_hint()
    } else if menu.has_parent {
        locale.menu_back_button()
    } else {
        locale.menu_close_button()
    };
    let key_style = LabelStyle::for_host();
    let exit_key = format_binding_with_glyphs(
        &Binding::new(KeyCode::Esc, Modifiers::NONE),
        key_style,
        key_glyphs,
    );
    let enter_key = format_binding_with_glyphs(
        &Binding::new(KeyCode::Enter, Modifiers::NONE),
        key_style,
        key_glyphs,
    );
    let navigation_key = join_key_labels(
        &[
            format_binding_with_glyphs(
                &Binding::new(KeyCode::Up, Modifiers::NONE),
                key_style,
                key_glyphs,
            ),
            format_binding_with_glyphs(
                &Binding::new(KeyCode::Down, Modifiers::NONE),
                key_style,
                key_glyphs,
            ),
        ],
        key_style,
        "/",
    );
    let mut hints = vec![KeyHint::new(exit_key.as_str(), exit_label)];
    if !menu.transferring && selected_visible && menu.unavailable[menu.selected].is_none() {
        hints.push(KeyHint::new(enter_key.as_str(), locale.menu_run_hint()));
    }
    if !menu.transferring {
        hints.push(KeyHint::new(
            navigation_key.as_str(),
            locale.menu_select_hint(),
        ));
    }
    if !menu.context_menu && measure_key_hints(&hints, theme).is_none() {
        return ModalPresentation::NotPresented;
    }
    let required_hint_width = if menu.context_menu {
        0
    } else {
        let Some(width) = measure_key_hints(&hints[..1], theme) else {
            return ModalPresentation::NotPresented;
        };
        width
    };
    if !menu.context_menu && required_hint_width > usize::from(footer_area.width) {
        return ModalPresentation::NotPresented;
    }
    let close_rect = if menu.context_menu {
        None
    } else {
        match super::controls::close_rect(frame.area(), area) {
            Some(rect) => Some(rect),
            None => return ModalPresentation::NotPresented,
        }
    };
    let footer_action = if menu.context_menu {
        None
    } else {
        footer_action_layout(
            frame.area(),
            footer_area,
            exit_label,
            required_hint_width,
            theme,
        )
    };
    let hint_area = footer_action.map_or(footer_area, |(_, hint_area)| hint_area);

    // Admission is complete. Only a menu that will paint may replace lower overlay hits and claim
    // the unresolved pointer backdrop for this frame.
    hits.start_action_menu();

    // A surviving wide lead outside the menu would make the terminal diff skip its border. Remove
    // only crossing glyph remnants, retaining their style.
    let buffer = frame.buffer_mut();
    for y in area.y..area.bottom() {
        let mut x = buffer.area.x;
        while x < area.right() {
            let cell = &buffer[(x, y)];
            let width = u16::try_from(display_width(cell.symbol()).max(1)).unwrap_or(u16::MAX);
            let next = x.saturating_add(width).min(buffer.area.right());
            if (x < area.x && next > area.x) || next > area.right() {
                let style = cell.style();
                for column in x..next {
                    if column < area.x || column >= area.right() {
                        let cell = &mut buffer[(column, y)];
                        cell.reset();
                        cell.set_style(style);
                    }
                }
            }
            x = next;
        }
    }
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    if let Some(close_rect) = close_rect {
        render_close(frame, close_rect, &menu, theme, hits);
    }

    let mut painted_rows = 0;
    let mut used_rows = 0;
    for (offset, action) in menu.actions.iter().enumerate().skip(first) {
        if offset > first && menu.separator_before(offset) {
            if used_rows + 1 >= rows {
                break;
            }
            let separator = match theme.glyphs.mode() {
                GlyphMode::Unicode => "─",
                #[cfg(test)]
                GlyphMode::Simplified => "-",
            };
            frame.render_widget(
                Paragraph::new(separator.repeat(usize::from(inner.width)))
                    .style(theme.style(Role::TextSecondary)),
                Rect::new(
                    inner.x,
                    inner.y + u16::try_from(used_rows).unwrap_or(0),
                    inner.width,
                    1,
                ),
            );
            used_rows += 1;
        }
        if used_rows >= rows {
            break;
        }
        let row_area = Rect::new(
            inner.x - 1,
            inner.y + u16::try_from(used_rows).unwrap_or(0),
            inner.width + 2,
            1,
        );
        render_row(
            frame,
            row_area,
            &menu,
            offset,
            *action,
            binding_labels[offset].as_deref(),
            locale,
            theme,
            hits,
        );
        used_rows += 1;
        painted_rows += 1;
    }
    debug_assert_eq!(painted_rows, visible_rows);

    let unavailable = menu.unavailable[menu.selected];
    if let Some(reason) = unavailable {
        let full = locale.menu_unavailable(reason, false);
        let reason = if Line::raw(full).width() <= usize::from(inner.width) {
            full
        } else {
            locale.menu_unavailable(reason, true)
        };
        frame.render_widget(
            Paragraph::new(reason).style(theme.style(Role::TextSecondary)),
            Rect::new(
                inner.x,
                inner.bottom() - 1 - u16::try_from(footer_rows).unwrap_or(0),
                inner.width,
                1,
            ),
        );
    }

    if let Some((action_rect, _)) = footer_action {
        render_footer_action(
            frame,
            action_rect,
            footer_area,
            exit_label,
            if menu.has_parent && !menu.transferring {
                MenuControl::Back
            } else {
                MenuControl::Close
            },
            &menu,
            theme,
            hits,
        );
    }
    if !menu.context_menu {
        let painted_hints = render_key_hints(frame, hint_area, &hints, theme)
            .expect("the required exit hint was measured in the same area");
        debug_assert!(hint_area.intersection(painted_hints) == painted_hints);
        frame.render_widget(
            Paragraph::new(position).style(theme.style(Role::TextSecondary)),
            Rect::new(
                inner.right() - position_width,
                inner.bottom() - 1,
                position_width,
                1,
            ),
        );
    }

    let rows_height = u16::try_from(rows).unwrap_or(u16::MAX);
    hits.record(
        HitLayer::OverlaySurface,
        Rect::new(inner.x - 1, inner.y, inner.width + 2, rows_height),
        HitTarget::ListScroll(crate::tui_pointer::ListScrollTarget::Menu(menu.token)),
    );
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    ModalPresentation::Presented(MenuDraw {
        token: menu.token,
        first,
        visible_rows: painted_rows,
        selected: menu.selected,
    })
}

fn visible_action_rows(menu: &MenuView<'_>, first: usize, rows: usize) -> usize {
    let mut painted_rows = 0;
    let mut used_rows = 0;
    for index in first..menu.actions.len() {
        if index > first && menu.separator_before(index) {
            if used_rows + 1 >= rows {
                break;
            }
            used_rows += 1;
        }
        if used_rows >= rows {
            break;
        }
        used_rows += 1;
        painted_rows += 1;
    }
    painted_rows
}

fn effective_binding(
    bindings: &Bindings,
    menu: &MenuView<'_>,
    action: MenuAction,
    glyphs: KeyLabelGlyphs,
) -> Option<String> {
    let action = menu_binding_action(menu.source, action)?;
    bindings
        .action_labels_with_glyphs(action, glyphs)
        .into_iter()
        .next()
        .map(|label| sanitize_terminal_text(&label).into_owned())
        .filter(|label| display_width(label) > 0)
}

const fn key_label_glyphs(theme: &Theme) -> KeyLabelGlyphs {
    match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(test)]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    }
}

fn pointer_presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
}

fn interaction_spec(enabled: bool) -> InteractionSpec {
    if enabled {
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
    } else {
        InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
    }
}

fn interaction_state(
    focused: bool,
    enabled: bool,
    pointer: PointerPresentation,
) -> InteractionState {
    InteractionState::new(
        if focused {
            FocusState::Focused
        } else {
            FocusState::Unfocused
        },
        pointer,
        if focused {
            NavigationState::Provisional
        } else {
            NavigationState::None
        },
        CurrentState::NotCurrent,
        CheckedState::Unchecked,
        if enabled {
            Availability::Enabled
        } else {
            Availability::Disabled
        },
        OperationState::Idle,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_row(
    frame: &mut Frame<'_>,
    row: Rect,
    menu: &MenuView<'_>,
    index: usize,
    action: MenuAction,
    binding: Option<&str>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let marker_width = u16::try_from(display_width(theme.glyphs.navigation_marker).max(1))
        .unwrap_or(u16::MAX)
        .min(row.width.saturating_sub(1));
    let navigation = Rect::new(row.x, row.y, marker_width, 1);
    let binding_width = binding
        .map(display_width)
        .and_then(|width| u16::try_from(width).ok())
        .filter(|width| {
            row.width
                .saturating_sub(marker_width)
                .saturating_sub(*width)
                > ROW_BINDING_GAP
        });
    let binding_rect = binding_width.map(|width| Rect::new(row.right() - width, row.y, width, 1));
    let primary_right =
        binding_rect.map_or(row.right(), |rect| rect.x.saturating_sub(ROW_BINDING_GAP));
    let primary = Rect::new(
        navigation.right(),
        row.y,
        primary_right.saturating_sub(navigation.right()),
        1,
    );
    let label = truncate_display(
        &action_label(
            locale,
            action,
            menu.labels[index].as_deref(),
            theme.glyphs.middle_dot,
            theme.glyphs.disclosure_closed,
        ),
        usize::from(primary.width),
        theme.glyphs.ellipsis,
    );
    frame.render_widget(
        Paragraph::new(label).style(theme.style(Role::TextPrimary)),
        primary,
    );
    let mut bands = Vec::new();
    if let (Some(binding), Some(rect)) = (binding, binding_rect) {
        frame.render_widget(
            Paragraph::new(binding).style(theme.style(Role::HintKey)),
            rect,
        );
        bands.push(RowContentBand::semantic(rect, Role::HintKey));
    }

    let enabled = menu.unavailable[index].is_none() && !menu.transferring;
    let focused = index == menu.selected;
    let target = InteractionTarget::ActionMenuRow {
        source: menu.source.clone(),
        token: menu.token,
        row: index,
    };
    let spec = interaction_spec(enabled);
    let pointer = if menu.transferring {
        PointerPresentation::Rest
    } else {
        pointer_presentation(hits.projected_pointer(HitLayer::Overlay, row, row, &target, spec))
    };
    let tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary)
        .with_provisional(navigation)
        .with_primary_role(Role::TextPrimary);
    let painted = paint_row_interaction(
        frame,
        row,
        tracks,
        &bands,
        ControlSurface::Overlay,
        interaction_state(focused, enabled, pointer),
        theme,
    )
    .expect("action-menu row geometry was resolved before painting");
    debug_assert_eq!(painted, row);
    if !menu.transferring {
        hits.record_interaction(HitLayer::Overlay, row, painted, target, spec);
    }
}

fn action_state(pointer: PointerPresentation) -> InteractionState {
    InteractionState::new(
        FocusState::Unfocused,
        pointer,
        NavigationState::None,
        CurrentState::NotCurrent,
        CheckedState::Unchecked,
        Availability::Enabled,
        OperationState::Idle,
    )
}

fn action_spec(label: &str, pointer: PointerPresentation) -> ActionSpec<'_> {
    ActionSpec::new(
        label,
        ActionKind::Secondary,
        ControlSurface::Overlay,
        action_state(pointer),
    )
}

fn footer_action_layout(
    bounds: Rect,
    footer: Rect,
    label: &str,
    required_hint_width: usize,
    theme: &Theme,
) -> Option<(Rect, Rect)> {
    let action = super::controls::action_rect(
        bounds,
        footer,
        action_spec(label, PointerPresentation::Rest),
        theme,
    )?;
    let hints_x = action.right().checked_add(2)?;
    let hint_width = footer.right().checked_sub(hints_x)?;
    if usize::from(hint_width) < required_hint_width {
        return None;
    }
    Some((
        action,
        Rect::new(hints_x, footer.y, hint_width, footer.height),
    ))
}

#[allow(clippy::too_many_arguments)]
fn render_footer_action(
    frame: &mut Frame<'_>,
    rect: Rect,
    area: Rect,
    label: &str,
    control: MenuControl,
    menu: &MenuView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let target = InteractionTarget::ActionMenuControl {
        source: menu.source.clone(),
        token: menu.token,
        control,
    };
    let spec = interaction_spec(true);
    let pointer =
        pointer_presentation(hits.projected_pointer(HitLayer::Overlay, rect, rect, &target, spec));
    let painted = super::controls::render_action(frame, area, action_spec(label, pointer), theme)
        .expect("preflighted menu action remains paintable");
    debug_assert_eq!(painted, rect);
    hits.record_interaction(HitLayer::Overlay, rect, painted, target, spec);
}

fn render_close(
    frame: &mut Frame<'_>,
    rect: Rect,
    menu: &MenuView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let target = InteractionTarget::ActionMenuControl {
        source: menu.source.clone(),
        token: menu.token,
        control: MenuControl::Close,
    };
    let spec = interaction_spec(true);
    let pointer =
        pointer_presentation(hits.projected_pointer(HitLayer::Overlay, rect, rect, &target, spec));
    let painted = super::controls::render_icon_action(
        frame,
        rect,
        IconActionSpec::new(
            IconActionGlyph::Close,
            ControlSurface::Overlay,
            Availability::Enabled,
            pointer,
        ),
        theme,
    )
    .expect("the preflighted action-menu close target remains paintable");
    debug_assert_eq!(painted, rect);
    hits.record_interaction(HitLayer::Overlay, rect, painted, target, spec);
}

fn action_label(
    locale: Locale,
    action: MenuAction,
    label: Option<&str>,
    separator: &str,
    disclosure: &str,
) -> String {
    let text = label.map_or_else(
        || locale.menu_action(action).to_owned(),
        |label| {
            format!(
                "{} {separator} {}",
                locale.menu_action(action),
                sanitize_terminal_text(if label.is_empty() {
                    locale.shell().unnamed_tab_group
                } else {
                    label
                })
            )
        },
    );
    if action.submenu() {
        format!("{text} {disclosure}")
    } else {
        text
    }
}

#[cfg(test)]
mod tests;
