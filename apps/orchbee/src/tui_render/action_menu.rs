//! Retained action-menu projection; only a complete successful frame arms input.

use crate::{
    action_menu::{MenuAction, MenuControl, MenuDraw, MenuView, TabMenuScope},
    action_projection::menu_binding_action,
    client_view::{
        Binding, Bindings, KeyCode, KeyLabelGlyphs, LabelStyle, Modifiers,
        format_binding_with_glyphs, join_key_labels,
    },
    localization::Locale,
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_pointer::{
        HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
        ProjectedPointer,
    },
    tui_theme::{GlyphMode, Role, Theme},
    workbench_client::ModalPresentation,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Clear, Padding, Paragraph},
};

use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    IconActionGlyph, IconActionSpec, InteractionState, KeyHint, NavigationState, OperationState,
    PointerPresentation, QueryField, RowContentBand, RowFocusCue, RowTracks, measure_overlay_hints,
    paint_row_interaction, render_overlay_hints, render_query_field,
};

/// Keeps the right-aligned effective chord visually independent from the action label.
const ROW_BINDING_GAP: u16 = 2;

mod tab_header;

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
    let header_rows = usize::from(menu.context_menu && menu.tabs.is_some());

    let key_glyphs = key_label_glyphs(theme);
    let key_style = LabelStyle::for_host();
    let exit_key = format_binding_with_glyphs(
        &Binding::new(KeyCode::Esc, Modifiers::NONE),
        key_style,
        key_glyphs,
    );
    let binding_labels = menu
        .actions
        .iter()
        .map(|action| effective_binding(bindings, &menu, action, key_glyphs))
        .collect::<Vec<_>>();
    let result_actions =
        search_result_actions(&menu, bindings, locale, key_glyphs, &binding_labels);
    let selected = menu
        .pending_selected_action
        .and_then(|action| {
            result_actions
                .iter()
                .position(|index| menu.actions.get(*index) == Some(action))
        })
        .unwrap_or_else(|| {
            if result_actions.is_empty() {
                0
            } else {
                menu.selected.min(result_actions.len() - 1)
            }
        });
    let marker_width = display_width(theme.glyphs.navigation_marker);
    if marker_width == 0 {
        return ModalPresentation::NotPresented;
    }
    let full_project_suffixes = project_suffixes(&menu, usize::MAX, theme.glyphs.ellipsis);
    let desired = menu
        .actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let label = action_label(
                locale,
                action,
                menu.labels[index].as_deref(),
                menu.tabs.map(|tabs| tabs.rows[index]),
                theme.glyphs.middle_dot,
                theme.glyphs.disclosure_closed,
            );
            marker_width
                .saturating_add(color_track_width(action, theme))
                .saturating_add(display_width(&label))
                .saturating_add(display_width(&full_project_suffixes[index]))
                .saturating_add(binding_labels[index].as_ref().map_or(0, |binding| {
                    display_width(binding).saturating_add(usize::from(ROW_BINDING_GAP))
                }))
                // The row owns one padding cell beside each frame edge.
                .saturating_add(2)
        })
        .max()
        .unwrap_or(16);
    let desired = menu.tabs.map_or(desired, |tabs| {
        let path = locale.menu_tab_path(menu.source, tabs.scope, theme.glyphs.disclosure_closed);
        desired.max(
            (display_width(&path) + display_width(menu.title).min(16) + 9).saturating_add(
                tab_header::back_width(content, &menu, locale, &exit_key, theme),
            ),
        )
    });
    let desired = if result_actions.is_empty() {
        // Preserve the full message inside the one-cell frame and horizontal padding on both sides.
        desired.max(display_width(locale.menu_search_no_results()).saturating_add(4))
    } else {
        desired
    };
    let exit_label = if menu.transferring {
        locale.menu_cancel_hint()
    } else if menu.has_parent {
        locale.menu_back_button()
    } else {
        locale.menu_close_button()
    };
    let enter_key = format_binding_with_glyphs(
        &Binding::new(KeyCode::Enter, Modifiers::NONE),
        key_style,
        key_glyphs,
    );
    // A footer-owning menu widens until its exit button, exit and run hints, and the widest
    // position counter share one row inside the frame and its padding.
    let desired = if menu.context_menu {
        desired
    } else {
        let count = result_actions.len().max(1);
        measure_overlay_hints(
            &[
                KeyHint::new(exit_key.as_str(), exit_label),
                KeyHint::new(enter_key.as_str(), locale.menu_run_hint()),
            ],
            theme,
        )
        .map_or(desired, |hints| {
            desired.max(
                // The exit button paints ` [ label ]`, then a two-cell gap precedes the hints.
                display_width(exit_label)
                    .saturating_add(7)
                    .saturating_add(hints)
                    .saturating_add(1)
                    .saturating_add(display_width(&format!("{count}/{count}")))
                    .saturating_add(4),
            )
        })
    };
    let width = u16::try_from(desired.max(26))
        .unwrap_or(u16::MAX)
        .min(content.width);
    // Discovery may change availability, never the frame's edge-clamped position or row slots.
    let selected_action = result_actions.get(selected).copied();
    let reason_rows = usize::from(
        menu.actions
            .contains(&crate::action_menu::MenuAction::WorkspaceWorktrees)
            || result_actions
                .iter()
                .any(|index| menu.unavailable[*index].is_some()),
    );
    let search_rows = usize::from(menu.query.is_some());
    let result_row_count = if result_actions.is_empty() {
        1
    } else {
        menu.result_row_height(&result_actions, 0, result_actions.len())
    };
    let height =
        u16::try_from(result_row_count + search_rows + 2 + footer_rows + reason_rows + header_rows)
            .unwrap_or(u16::MAX)
            .min(content.height);
    let area = if let Some((x, y)) = menu.anchor {
        let avoid = hits
            .context_menu_anchor_rect(x, y)
            .unwrap_or(Rect::new(x, y, 1, 1))
            .intersection(content);
        let sidebar = matches!(
            menu.source,
            crate::action_menu::MenuTarget::Workspace(_)
                | crate::action_menu::MenuTarget::Project(_)
        );
        if sidebar && avoid.right().saturating_add(width) <= content.right() {
            Rect::new(
                avoid.right(),
                avoid.y.clamp(content.y, content.bottom() - height),
                width,
                height,
            )
        } else {
            let below = content.bottom().saturating_sub(avoid.bottom());
            let above = avoid.y.saturating_sub(content.y);
            let use_below = below >= height || below >= above;
            let available = if use_below { below } else { above };
            let height = height.min(available);
            if height < 3 {
                return ModalPresentation::NotPresented;
            }
            Rect::new(
                x.clamp(content.x, content.right() - width),
                if use_below {
                    avoid.bottom()
                } else {
                    avoid.y - height
                },
                width,
                height,
            )
        }
    } else {
        Rect::new(
            content.x + (content.width - width) / 2,
            content.y + (content.height - height) / 2,
            width,
            height,
        )
    };
    let legacy = matches!(
        menu.source,
        crate::action_menu::MenuTarget::Workspace(
            crate::workbench_client::ClientContextLocation::NoWorkspace
        )
    );
    let title = if menu.tabs.is_some() {
        tab_title(
            &menu,
            locale,
            theme,
            usize::from(area.width.saturating_sub(6)),
        )
    } else if menu.context_menu && !menu.title.is_empty() {
        sanitize_terminal_text(menu.title).into_owned()
    } else if menu.title.is_empty() && legacy {
        format!(
            "{} {} {}",
            locale.menu_title(),
            theme.glyphs.middle_dot,
            locale.shell().no_workspace
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
    let block = super::overlay_surface(theme, Padding::horizontal(1));
    let block = if menu.context_menu && menu.tabs.is_some() {
        block
    } else {
        // A compact context menu has no close control to reserve beside its heading.
        block.title(if menu.context_menu {
            super::overlay_heading(&title, usize::from(area.width).saturating_sub(6), theme)
        } else {
            super::overlay_title(&title, area.width, theme)
        })
    };
    let inner = block.inner(area);
    let project_budget = usize::from(inner.width)
        .saturating_add(2)
        .saturating_sub(marker_width);
    let project_suffixes = project_suffixes(&menu, project_budget, theme.glyphs.ellipsis);
    if project_suffixes
        .iter()
        .any(|suffix| display_width(suffix) > project_budget)
    {
        return ModalPresentation::NotPresented;
    }
    let rows = usize::from(inner.height)
        .saturating_sub(footer_rows + reason_rows + header_rows + search_rows);
    if rows == 0 {
        return ModalPresentation::NotPresented;
    }
    let first = menu.first_visible_result_row(rows, &result_actions, selected);
    let visible_rows = visible_action_rows(&menu, &result_actions, first, rows);
    if visible_rows == 0 && !result_actions.is_empty() {
        return ModalPresentation::NotPresented;
    }
    let tab_header = if header_rows == 0 {
        None
    } else {
        let Some(header) =
            tab_header::TabHeader::layout(frame.area(), inner, &menu, locale, &exit_key, theme)
        else {
            return ModalPresentation::NotPresented;
        };
        Some(header)
    };
    let rows_y = inner.y + u16::try_from(header_rows + search_rows).unwrap_or(0);
    let selected_visible = (first..first.saturating_add(visible_rows)).contains(&selected);

    let position = if result_actions.is_empty() {
        "0/0".to_owned()
    } else {
        format!("{}/{}", selected + 1, result_actions.len())
    };
    let position_width = u16::try_from(display_width(&position)).unwrap_or(u16::MAX);
    let Some(hints_width) = inner.width.checked_sub(position_width.saturating_add(1)) else {
        return ModalPresentation::NotPresented;
    };
    let footer_area = Rect::new(inner.x, inner.bottom() - 1, hints_width, 1);
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
    if !menu.transferring
        && selected_visible
        && selected_action.is_some_and(|index| menu.unavailable[index].is_none())
    {
        hints.push(KeyHint::new(enter_key.as_str(), locale.menu_run_hint()));
    }
    if !menu.transferring {
        hints.push(KeyHint::new(
            navigation_key.as_str(),
            locale.menu_select_hint(),
        ));
    }
    if !menu.context_menu && measure_overlay_hints(&hints, theme).is_none() {
        return ModalPresentation::NotPresented;
    }
    let required_hint_width = if menu.context_menu {
        0
    } else {
        let Some(width) = measure_overlay_hints(&hints[..1], theme) else {
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
        match super::controls::frame_close_rect(frame.area(), area) {
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
    if let Some(header) = tab_header {
        header.paint(frame, &menu, locale, &exit_key, theme, hits);
    }

    if let Some(query) = menu.query {
        render_search_row(
            frame,
            Rect::new(
                inner.x,
                inner.y + u16::try_from(header_rows).unwrap_or(0),
                inner.width,
                1,
            ),
            query,
            result_actions.len(),
            locale,
            theme,
        );
    }

    let mut painted_rows = 0;
    let mut used_rows = 0;
    for position in first..result_actions.len() {
        let action_index = result_actions[position];
        let action = &menu.actions[action_index];
        if position > first && menu.result_separator_before(&result_actions, position) {
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
                    rows_y + u16::try_from(used_rows).unwrap_or(0),
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
            rows_y + u16::try_from(used_rows).unwrap_or(0),
            inner.width + 2,
            1,
        );
        render_row(
            frame,
            row_area,
            &menu,
            position,
            action_index,
            selected,
            action,
            binding_labels[action_index].as_deref(),
            &project_suffixes[action_index],
            locale,
            theme,
            hits,
        );
        used_rows += 1;
        painted_rows += 1;
    }
    debug_assert_eq!(painted_rows, visible_rows);

    if result_actions.is_empty() {
        frame.render_widget(
            Paragraph::new(locale.menu_search_no_results()).style(theme.style(Role::TextSecondary)),
            Rect::new(inner.x, rows_y, inner.width, 1),
        );
    }

    let unavailable = selected_action.and_then(|index| menu.unavailable[index]);
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
        let painted_hints = render_overlay_hints(frame, hint_area, &hints, theme)
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
        Rect::new(inner.x - 1, rows_y, inner.width + 2, rows_height),
        HitTarget::ListScroll(crate::tui_pointer::ListScrollTarget::Menu(menu.token)),
    );
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    ModalPresentation::Presented(MenuDraw {
        token: menu.token,
        first,
        visible_rows: painted_rows,
        selected,
        result_actions,
    })
}

fn visible_action_rows(menu: &MenuView<'_>, results: &[usize], first: usize, rows: usize) -> usize {
    let mut painted_rows = 0;
    let mut used_rows = 0;
    for position in first..results.len() {
        if position > first && menu.result_separator_before(results, position) {
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

fn search_result_actions(
    menu: &MenuView<'_>,
    bindings: &Bindings,
    locale: Locale,
    glyphs: KeyLabelGlyphs,
    binding_labels: &[Option<String>],
) -> Vec<usize> {
    let Some(query) = menu.query.filter(|query| !query.is_empty()) else {
        return (0..menu.actions.len()).collect();
    };
    menu.actions
        .iter()
        .enumerate()
        .filter_map(|(index, action)| {
            let label = action_label(
                locale,
                action,
                menu.labels[index].as_deref(),
                menu.tabs.map(|tabs| tabs.rows[index]),
                "·",
                "›",
            );
            let own_match = action_matches_query(
                query,
                locale,
                action,
                &label,
                binding_labels[index].as_deref(),
            );
            let child_match = !own_match
                && menu.search_children.iter().any(|child| {
                    child.parent == *action
                        && child
                            .actions
                            .iter()
                            .enumerate()
                            .any(|(child_index, action)| {
                                let label = action_label(
                                    locale,
                                    action,
                                    child.labels.get(child_index).and_then(Option::as_deref),
                                    None,
                                    "·",
                                    "›",
                                );
                                let binding = effective_binding_for_source(
                                    bindings,
                                    &child.source,
                                    action,
                                    glyphs,
                                );
                                action_matches_query(
                                    query,
                                    locale,
                                    action,
                                    &label,
                                    binding.as_deref(),
                                )
                            })
                });
            (own_match || child_match).then_some(index)
        })
        .collect()
}

fn action_matches_query(
    query: &str,
    locale: Locale,
    action: &MenuAction,
    label: &str,
    binding: Option<&str>,
) -> bool {
    let aliases = crate::action_projection::menu_metadata(action.clone())
        .action
        .search_aliases
        .iter()
        .map(|alias| locale.menu_search_alias(*alias))
        .collect::<Vec<_>>()
        .join(" ");
    let candidate = format!("{label} {aliases} {}", binding.unwrap_or_default());
    fuzzy_contains(query, &candidate)
}

fn fuzzy_contains(query: &str, candidate: &str) -> bool {
    let query = query.to_lowercase();
    let candidate = candidate.to_lowercase();
    let mut candidate = candidate.chars();
    query
        .chars()
        .all(|character| candidate.by_ref().any(|candidate| candidate == character))
}

fn render_search_row(
    frame: &mut Frame<'_>,
    area: Rect,
    query: &str,
    count: usize,
    locale: Locale,
    theme: &Theme,
) {
    let count = truncate_display(
        &locale.menu_search_count(count),
        usize::from(area.width / 2),
        theme.glyphs.ellipsis,
    );
    let count_width = u16::try_from(display_width(&count)).unwrap_or(u16::MAX);
    let left_width = area.width.saturating_sub(count_width.saturating_add(1));
    let left = Rect::new(area.x, area.y, left_width, area.height);
    let right = Rect::new(
        area.right().saturating_sub(count_width),
        area.y,
        count_width,
        area.height,
    );
    // Typing always reaches the menu query, so the slot keeps its editing presentation.
    let _ = render_query_field(
        frame,
        left,
        QueryField::Editing {
            label: locale.menu_search_label(),
            query,
            cursor: query.len(),
        },
        theme,
    );
    if count_width > 0 {
        frame.render_widget(
            Paragraph::new(count).style(theme.style(Role::TextSecondary)),
            right,
        );
    }
}

fn effective_binding(
    bindings: &Bindings,
    menu: &MenuView<'_>,
    action: &MenuAction,
    glyphs: KeyLabelGlyphs,
) -> Option<String> {
    effective_binding_for_source(bindings, menu.source, action, glyphs)
}

fn effective_binding_for_source(
    bindings: &Bindings,
    source: &crate::action_menu::MenuTarget,
    action: &MenuAction,
    glyphs: KeyLabelGlyphs,
) -> Option<String> {
    let action = menu_binding_action(source, action.clone())?;
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
    checked: CheckedState,
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
        checked,
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
    result_position: usize,
    action_index: usize,
    selected: usize,
    action: &MenuAction,
    binding: Option<&str>,
    project_suffix: &str,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let primary_role = crate::action_projection::menu_metadata(action.clone())
        .tone
        .content_role(Role::TextPrimary);
    let marker_width = u16::try_from(display_width(theme.glyphs.navigation_marker).max(1))
        .unwrap_or(u16::MAX)
        .min(row.width.saturating_sub(1));
    let navigation = Rect::new(row.x, row.y, marker_width, 1);
    let color = matches!(action, MenuAction::GroupColor(_));
    let checked = color.then(|| {
        Rect::new(
            navigation.right(),
            row.y,
            u16::try_from(display_width(theme.glyphs.active_marker)).unwrap_or(1),
            1,
        )
    });
    let swatch = checked.map(|rect| Rect::new(rect.right() + 1, row.y, 2, 1));
    let label_left = swatch.map_or(navigation.right(), |rect| rect.right() + 1);
    let binding_width = binding
        .map(display_width)
        .and_then(|width| u16::try_from(width).ok())
        .filter(|width| {
            row.width
                .saturating_sub(label_left - row.x)
                .saturating_sub(*width)
                > ROW_BINDING_GAP
        });
    let binding_rect = binding_width.map(|width| Rect::new(row.right() - width, row.y, width, 1));
    let primary_right =
        binding_rect.map_or(row.right(), |rect| rect.x.saturating_sub(ROW_BINDING_GAP));
    let primary = Rect::new(
        label_left,
        row.y,
        primary_right.saturating_sub(label_left),
        1,
    );
    let label = truncate_display(
        &action_label(
            locale,
            action,
            menu.labels[action_index].as_deref(),
            menu.tabs.map(|tabs| tabs.rows[action_index]),
            theme.glyphs.middle_dot,
            theme.glyphs.disclosure_closed,
        ),
        usize::from(primary.width).saturating_sub(display_width(project_suffix)),
        theme.glyphs.ellipsis,
    );
    frame.render_widget(
        Paragraph::new(format!("{label}{project_suffix}")).style(theme.style(primary_role)),
        primary,
    );
    let mut bands = Vec::new();
    if let Some(rect) = swatch {
        bands.push(RowContentBand::explicit(rect));
    }
    if let (Some(binding), Some(rect)) = (binding, binding_rect) {
        frame.render_widget(
            Paragraph::new(binding).style(theme.style(Role::HintKey)),
            rect,
        );
        bands.push(RowContentBand::semantic(rect, Role::HintKey));
    }

    let enabled = menu.unavailable[action_index].is_none() && !menu.transferring;
    let focused = result_position == selected;
    let target = InteractionTarget::ActionMenuRow {
        source: menu.source.clone(),
        token: menu.token,
        row: result_position,
    };
    let spec = interaction_spec(enabled);
    let pointer = if menu.transferring {
        PointerPresentation::Rest
    } else {
        pointer_presentation(hits.projected_pointer(HitLayer::Overlay, row, row, &target, spec))
    };
    let mut tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary)
        .with_provisional(navigation)
        .with_primary_role(primary_role);
    if let Some(rect) = checked {
        tracks = tracks.with_checked(rect, theme.glyphs.active_marker);
    }
    let applied = if menu.tabs.is_some_and(|tabs| tabs.checked(action)) {
        CheckedState::Checked
    } else {
        CheckedState::Unchecked
    };
    let state = interaction_state(focused, enabled, pointer, applied);
    let painted = paint_row_interaction(
        frame,
        row,
        tracks,
        &bands,
        ControlSurface::Overlay,
        state,
        theme,
    )
    .expect("action-menu row geometry was resolved before painting");
    debug_assert_eq!(painted, row);
    if let (MenuAction::GroupColor(color), Some(rect)) = (action, swatch) {
        // The swatch shows the accent its group header uses, independently of the row's fill.
        let style = if enabled {
            Style::new()
                .bg(theme.tab_groups.accent(*color))
                .remove_modifier(Modifier::all())
        } else {
            theme.style(Role::TextDisabled)
        };
        frame.render_widget(Paragraph::new("  ").style(style), rect);
    }
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
    let painted = super::controls::render_frame_icon_action(
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
    action: &MenuAction,
    label: Option<&str>,
    scope: Option<TabMenuScope>,
    separator: &str,
    disclosure: &str,
) -> String {
    let choice = matches!(
        action,
        MenuAction::JoinWorkspaceProject(_) | MenuAction::OpenWorktreeAt(_)
    );
    let text = scope.map_or_else(
        || locale.menu_action(action).to_owned(),
        |scope| locale.menu_tab_action(action, scope),
    );
    if let MenuAction::NewTabWithProfile(profile) = action {
        return format!(
            "{text} {separator} {}",
            locale.launch_profile_label(profile.name.as_deref())
        );
    }
    let text = if let Some(label) = label {
        if choice {
            if matches!(action, MenuAction::JoinWorkspaceProject(_)) {
                crate::machine_discovery::display_label(label)
            } else {
                sanitize_terminal_text(label).into_owned()
            }
        } else {
            format!(
                "{text} {separator} {}",
                sanitize_terminal_text(if label.is_empty() {
                    locale.shell().unnamed_tab_group
                } else {
                    label
                })
            )
        }
    } else {
        text
    };
    if action.submenu() {
        format!("{text} {disclosure}")
    } else {
        text
    }
}

fn project_suffixes(menu: &MenuView<'_>, width: usize, ellipsis: &str) -> Vec<String> {
    let mut groups = std::collections::BTreeMap::<String, Vec<(usize, &str)>>::new();
    for (index, action) in menu.actions.iter().enumerate() {
        if let MenuAction::JoinWorkspaceProject(id) = action {
            let label = crate::machine_discovery::display_label(
                menu.labels[index].as_deref().unwrap_or(""),
            );
            groups
                .entry(truncate_display(&label, width, ellipsis))
                .or_default()
                .push((index, id.as_str()));
        }
    }
    let mut suffixes = vec![String::new(); menu.actions.len()];
    for peers in groups.values().filter(|peers| peers.len() > 1) {
        let ids = peers.iter().map(|(_, id)| *id).collect::<Vec<_>>();
        for (index, id) in peers {
            suffixes[*index] = crate::terminal_text::unique_identity_suffix(id, &ids);
        }
    }
    suffixes
}

fn color_track_width(action: &MenuAction, theme: &Theme) -> usize {
    if matches!(action, MenuAction::GroupColor(_)) {
        display_width(theme.glyphs.active_marker) + 4
    } else {
        0
    }
}

fn tab_title(menu: &MenuView<'_>, locale: Locale, theme: &Theme, width: usize) -> String {
    let Some(tabs) = menu.tabs else {
        return String::new();
    };
    let path = locale.menu_tab_path(menu.source, tabs.scope, theme.glyphs.disclosure_closed);
    let name = sanitize_terminal_text(if menu.title.is_empty() {
        locale.shell().unnamed_tab_group
    } else {
        menu.title
    });
    if matches!(menu.source, crate::action_menu::MenuTarget::Tab(_)) {
        return truncate_display(&name, width, theme.glyphs.ellipsis);
    }
    let separator = format!(" {} ", theme.glyphs.middle_dot);
    let budget = width.saturating_sub(display_width(&path) + display_width(&separator));
    let name = truncate_display(&name, budget, theme.glyphs.ellipsis);
    if name.is_empty() {
        truncate_display(&path, width, theme.glyphs.ellipsis)
    } else {
        format!("{name}{separator}{path}")
    }
}

#[cfg(test)]
mod tests;
