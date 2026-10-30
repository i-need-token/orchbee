//! Retained action-menu projection; only a complete successful frame arms input.

use crate::{
    action_menu::menu_binding_action,
    action_menu::{
        MenuAction, MenuAnchor, MenuControl, MenuDraw, MenuRowNote, MenuTarget, MenuView,
        TabMenuScope,
    },
    client_view::{
        Binding, Bindings, KeyCode, KeyLabelGlyphs, LabelStyle, Modifiers,
        format_binding_with_glyphs, join_key_labels,
    },
    localization::Locale,
    terminal_text::{
        display_width, sanitize_terminal_text, truncate_display, truncate_display_middle,
    },
    tui_pointer::hit_map::{
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

/// The key labels a frame's footer and widths are measured with.
struct Keys {
    glyphs: KeyLabelGlyphs,
    style: LabelStyle,
    exit: String,
    enter: String,
}

/// One menu frame's rows, candidate and natural size, before it is placed.
struct Measured<'v> {
    menu: MenuView<'v>,
    binding_labels: Vec<Option<String>>,
    result_actions: Vec<usize>,
    selected: usize,
    width: u16,
    height: u16,
    footer_rows: usize,
    search_rows: usize,
    reason_rows: usize,
    exit_label: &'static str,
}

/// A placed frame's interior and the result window it shows.
struct Placed {
    area: Rect,
    inner: Rect,
    rows: usize,
    first: usize,
    visible_rows: usize,
    rows_y: u16,
    project_suffixes: Vec<String>,
}

/// How a frame's rows take input: the open menu, or a cascade parent kept painted beside it.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Level {
    Active,
    Parent,
}

/// Paints the open menu, and for a cascade every parent beside it. Every frame must fit;
/// a cascade that cannot place one of its frames is not presented at all.
#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    frame: &mut Frame<'_>,
    content: Rect,
    menu: MenuView<'_>,
    parents: &[MenuView<'_>],
    bindings: &Bindings,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalPresentation<MenuDraw> {
    if content.width < 16 || content.height < if menu.context_menu { 3 } else { 5 } {
        return ModalPresentation::NotPresented;
    }
    let glyphs = key_label_glyphs(theme);
    let style = LabelStyle::for_host();
    let keys = Keys {
        glyphs,
        style,
        exit: format_binding_with_glyphs(
            &Binding::new(KeyCode::Esc, Modifiers::NONE),
            style,
            glyphs,
        ),
        enter: format_binding_with_glyphs(
            &Binding::new(KeyCode::Enter, Modifiers::NONE),
            style,
            glyphs,
        ),
    };
    let mut levels = Vec::with_capacity(parents.len() + 1);
    for view in parents.iter().copied().chain(std::iter::once(menu)) {
        let Some(measured) = measure(view, content, bindings, locale, theme, &keys) else {
            return ModalPresentation::NotPresented;
        };
        levels.push(measured);
    }
    let mut placed: Vec<Placed> = Vec::with_capacity(levels.len());
    for (index, measured) in levels.iter().enumerate() {
        let area = match placed.last() {
            None => place_root(measured, content, hits),
            Some(parent) => {
                let open = &levels[index - 1];
                row_y(open, parent, open.selected)
                    .and_then(|row| place_child(parent.area, row, measured, content))
            }
        };
        let Some(layout) = area.and_then(|area| layout(measured, area, theme)) else {
            return ModalPresentation::NotPresented;
        };
        placed.push(layout);
    }
    let (Some(active), Some(active_layout)) = (levels.last(), placed.last()) else {
        return ModalPresentation::NotPresented;
    };
    let Some(chrome) = Chrome::admit(frame.area(), active, active_layout, locale, theme, &keys)
    else {
        return ModalPresentation::NotPresented;
    };

    // Admission is complete. Only a menu that will paint may replace lower overlay hits and claim
    // the unresolved pointer backdrop for this frame.
    hits.start_action_menu();
    for (index, (measured, layout)) in levels.iter().zip(&placed).enumerate() {
        let level = if index + 1 == levels.len() {
            Level::Active
        } else {
            Level::Parent
        };
        paint_frame(frame, measured, layout, level, locale, theme, hits);
    }
    chrome.paint(frame, active, active_layout, locale, theme, hits);

    let rows_height = u16::try_from(active_layout.rows).unwrap_or(u16::MAX);
    let inner = active_layout.inner;
    hits.record(
        HitLayer::OverlaySurface,
        Rect::new(
            inner.x - 1,
            active_layout.rows_y,
            inner.width + 2,
            rows_height,
        ),
        HitTarget::ListScroll(crate::tui_pointer::hit_map::ListScrollTarget::Menu(
            active.menu.token,
        )),
    );
    hits.record(
        HitLayer::OverlaySurface,
        active_layout.area,
        HitTarget::OverlaySurface,
    );
    // A click on the parent of a cascade returns to it; the submenu's frame keeps its own cells.
    if let Some(parent) = placed.len().checked_sub(2).map(|index| &placed[index]) {
        let target = InteractionTarget::ActionMenuControl {
            source: active.menu.source.clone(),
            token: active.menu.token,
            control: MenuControl::Back,
        };
        let spec = interaction_spec(true);
        for rect in subtract(parent.area, active_layout.area) {
            hits.record_interaction(HitLayer::Overlay, rect, rect, target.clone(), spec);
        }
    }
    for layout in &placed[..placed.len() - 1] {
        hits.record(
            HitLayer::OverlaySurface,
            layout.area,
            HitTarget::OverlaySurface,
        );
    }
    ModalPresentation::Presented(MenuDraw {
        token: active.menu.token,
        first: active_layout.first,
        visible_rows: active_layout.visible_rows,
        selected: active.selected,
        candidate_visible: !active.menu.candidate_hidden,
        result_actions: active.result_actions.clone(),
    })
}

fn measure<'v>(
    menu: MenuView<'v>,
    content: Rect,
    bindings: &Bindings,
    locale: Locale,
    theme: &Theme,
    keys: &Keys,
) -> Option<Measured<'v>> {
    let footer_rows = usize::from(!menu.context_menu);
    let binding_labels = menu
        .actions
        .iter()
        .map(|action| effective_binding(bindings, &menu, action, keys.glyphs))
        .collect::<Vec<_>>();
    let result_actions =
        search_result_actions(&menu, bindings, locale, keys.glyphs, &binding_labels);
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
        return None;
    }
    let full_project_suffixes = project_suffixes(&menu, usize::MAX, theme.glyphs.ellipsis);
    let desired =
        menu.actions
            .iter()
            .enumerate()
            .map(|(index, action)| {
                let label = action_label(
                    locale,
                    menu.source,
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
                    .saturating_add(
                        trailing(binding_labels[index].as_deref(), menu.notes[index], locale)
                            .map_or(0, |(text, _)| {
                                display_width(text).saturating_add(usize::from(ROW_BINDING_GAP))
                            }),
                    )
                    // The frame's two edges, then the row's right padding cell and one blank
                    // cell after its content.
                    .saturating_add(4)
            })
            .max()
            .unwrap_or(16);
    // A group heading starts in the label column of the rows it introduces.
    let desired = (0..result_actions.len())
        .filter_map(|position| menu.result_heading_before(&result_actions, position))
        .map(|heading| {
            marker_width
                .saturating_add(display_width(locale.menu_heading(heading)))
                .saturating_add(4)
        })
        .fold(desired, usize::max);
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
    // A footer-owning menu widens until its exit button, exit and run hints, and the widest
    // position counter share one row inside the frame and its padding.
    let desired = if menu.context_menu {
        desired
    } else {
        let count = result_actions.len().max(1);
        measure_overlay_hints(
            &[
                KeyHint::new(keys.exit.as_str(), exit_label),
                KeyHint::new(keys.enter.as_str(), locale.menu_run_hint()),
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
    // The reason row exists only while one of the rows is unavailable.
    let reason_rows = usize::from(
        result_actions
            .iter()
            .any(|index| menu.unavailable[*index].is_some()),
    );
    let search_rows = usize::from(menu.query.is_some());
    let result_row_count = if result_actions.is_empty() {
        1
    } else {
        menu.result_row_height(&result_actions, 0, result_actions.len())
    };
    let height = u16::try_from(result_row_count + search_rows + 2 + footer_rows + reason_rows)
        .unwrap_or(u16::MAX);
    Some(Measured {
        menu,
        binding_labels,
        result_actions,
        selected,
        width,
        height,
        footer_rows,
        search_rows,
        reason_rows,
        exit_label,
    })
}

/// The complete trigger that owns an anchor cell in the frame being drawn, else the cell itself.
fn trigger(hits: &HitMapBuilder, content: Rect, column: u16, row: u16) -> Rect {
    hits.context_menu_anchor_rect(column, row)
        .unwrap_or(Rect::new(column, row, 1, 1))
        .intersection(content)
}

fn place_root(measured: &Measured<'_>, content: Rect, hits: &HitMapBuilder) -> Option<Rect> {
    let width = measured.width.min(content.width);
    let height = measured.height.min(content.height);
    match measured.menu.anchor {
        None => Some(Rect::new(
            content.x + (content.width - width) / 2,
            content.y + (content.height - height) / 2,
            width,
            height,
        )),
        // Beside a keyboard-focused row: level with it, moving up only to fit.
        Some(MenuAnchor::Row { column, row }) => {
            let trigger = trigger(hits, content, column, row);
            Some(Rect::new(
                trigger.right().min(content.right() - width),
                trigger.y.min(content.bottom() - height).max(content.y),
                width,
                height,
            ))
        }
        // At the pressed column on the row below the trigger, else ending on the row above it;
        // neither side fitting shortens the frame on the larger side, never covering the trigger.
        Some(MenuAnchor::Pointer { column, row }) => {
            let trigger = trigger(hits, content, column, row);
            let below = content.bottom().saturating_sub(trigger.bottom());
            let above = trigger.y.saturating_sub(content.y);
            let use_below = below >= height || (above < height && below >= above);
            let height = height.min(if use_below { below } else { above });
            if height < 3 {
                return None;
            }
            Some(Rect::new(
                column.clamp(content.x, content.right() - width),
                if use_below {
                    trigger.bottom()
                } else {
                    trigger.y - height
                },
                width,
                height,
            ))
        }
    }
}

/// A submenu opens beside its parent, first row level with the row that opened it; it moves up
/// only to fit, and opens to the left, else over the parent's end, when the right has no room.
/// Like any frame, a submenu taller than the viewport scrolls its rows.
fn place_child(
    parent: Rect,
    open_row: u16,
    measured: &Measured<'_>,
    content: Rect,
) -> Option<Rect> {
    let (width, height) = (measured.width, measured.height.min(content.height));
    if width > content.width {
        return None;
    }
    let x = if parent.right().saturating_add(width) <= content.right() {
        parent.right()
    } else if parent.x >= content.x.saturating_add(width) {
        parent.x - width
    } else {
        content.right() - width
    };
    let y = open_row
        .saturating_sub(1)
        .clamp(content.y, content.bottom() - height);
    Some(Rect::new(x, y, width, height))
}

fn layout(measured: &Measured<'_>, area: Rect, theme: &Theme) -> Option<Placed> {
    let menu = &measured.menu;
    let inner = super::overlay_surface(theme, Padding::horizontal(1)).inner(area);
    let marker_width = display_width(theme.glyphs.navigation_marker);
    // A row's content spans the inner width and its left padding cell, which holds the marker.
    let project_budget = usize::from(inner.width)
        .saturating_add(1)
        .saturating_sub(marker_width);
    let project_suffixes = project_suffixes(menu, project_budget, theme.glyphs.ellipsis);
    if project_suffixes
        .iter()
        .any(|suffix| display_width(suffix) > project_budget)
    {
        return None;
    }
    let rows = usize::from(inner.height)
        .saturating_sub(measured.footer_rows + measured.reason_rows + measured.search_rows);
    if rows == 0 {
        return None;
    }
    let first = menu.first_visible_result_row(rows, &measured.result_actions, measured.selected);
    let visible_rows = visible_action_rows(menu, &measured.result_actions, first, rows);
    if visible_rows == 0 && !measured.result_actions.is_empty() {
        return None;
    }
    Some(Placed {
        area,
        inner,
        rows,
        first,
        visible_rows,
        rows_y: inner.y + u16::try_from(measured.search_rows).unwrap_or(0),
        project_suffixes,
    })
}

/// The terminal row of one visible result, following the same walk as painting.
fn row_y(measured: &Measured<'_>, layout: &Placed, position: usize) -> Option<u16> {
    let mut used = 0;
    for current in layout.first..layout.first + layout.visible_rows {
        if current > layout.first
            && measured
                .menu
                .result_separator_before(&measured.result_actions, current)
        {
            used += 1;
        }
        if measured
            .menu
            .result_heading_before(&measured.result_actions, current)
            .is_some()
        {
            used += 1;
        }
        if current == position {
            return Some(layout.rows_y + u16::try_from(used).ok()?);
        }
        used += 1;
    }
    None
}

/// The parts of `outer` outside `inner`, as at most four bands.
fn subtract(outer: Rect, inner: Rect) -> Vec<Rect> {
    let overlap = outer.intersection(inner);
    if overlap.is_empty() {
        return vec![outer];
    }
    [
        Rect::new(outer.x, outer.y, outer.width, overlap.y - outer.y),
        Rect::new(
            outer.x,
            overlap.bottom(),
            outer.width,
            outer.bottom() - overlap.bottom(),
        ),
        Rect::new(outer.x, overlap.y, overlap.x - outer.x, overlap.height),
        Rect::new(
            overlap.right(),
            overlap.y,
            outer.right() - overlap.right(),
            overlap.height,
        ),
    ]
    .into_iter()
    .filter(|rect| !rect.is_empty())
    .collect()
}

fn title(measured: &Measured<'_>, area: Rect, locale: Locale, theme: &Theme) -> Option<String> {
    let menu = &measured.menu;
    // A cascaded submenu continues its parent's subject, so it carries no heading of its own.
    if menu.context_menu && menu.has_parent {
        return None;
    }
    let legacy = matches!(
        menu.source,
        MenuTarget::Workspace(crate::workbench_client::ClientContextLocation::NoWorkspace)
    );
    Some(
        if matches!(
            menu.source,
            MenuTarget::Tab(_) | MenuTarget::TabGroups { .. }
        ) {
            tab_title(
                menu,
                locale,
                theme,
                usize::from(area.width.saturating_sub(6)),
            )
        } else if matches!(menu.source, MenuTarget::TabProfiles(_)) {
            locale
                .menu_action(crate::localization::MenuLabel::NewTab)
                .to_owned()
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
        },
    )
}

/// The global menu's own controls: its close button, footer action, hints and position.
struct Chrome {
    close: Option<Rect>,
    footer: Option<(Rect, Rect, Rect)>,
    hints: Vec<(String, &'static str)>,
    position: String,
}

impl Chrome {
    fn admit(
        bounds: Rect,
        measured: &Measured<'_>,
        layout: &Placed,
        locale: Locale,
        theme: &Theme,
        keys: &Keys,
    ) -> Option<Self> {
        let menu = &measured.menu;
        let position = if measured.result_actions.is_empty() {
            "0/0".to_owned()
        } else {
            format!(
                "{}/{}",
                measured.selected + 1,
                measured.result_actions.len()
            )
        };
        if menu.context_menu {
            return Some(Self {
                close: None,
                footer: None,
                hints: Vec::new(),
                position,
            });
        }
        let inner = layout.inner;
        let position_width = u16::try_from(display_width(&position)).unwrap_or(u16::MAX);
        let hints_width = inner.width.checked_sub(position_width.saturating_add(1))?;
        let footer_area = Rect::new(inner.x, inner.bottom() - 1, hints_width, 1);
        let navigation_key = join_key_labels(
            &[
                format_binding_with_glyphs(
                    &Binding::new(KeyCode::Up, Modifiers::NONE),
                    keys.style,
                    keys.glyphs,
                ),
                format_binding_with_glyphs(
                    &Binding::new(KeyCode::Down, Modifiers::NONE),
                    keys.style,
                    keys.glyphs,
                ),
            ],
            keys.style,
            "/",
        );
        let selected_visible = !menu.candidate_hidden
            && (layout.first..layout.first.saturating_add(layout.visible_rows))
                .contains(&measured.selected);
        let selected_action = measured.result_actions.get(measured.selected).copied();
        let mut hints = vec![(keys.exit.clone(), measured.exit_label)];
        if !menu.transferring
            && selected_visible
            && selected_action.is_some_and(|index| menu.unavailable[index].is_none())
        {
            hints.push((keys.enter.clone(), locale.menu_run_hint()));
        }
        if !menu.transferring {
            hints.push((navigation_key, locale.menu_select_hint()));
        }
        let key_hints = hints
            .iter()
            .map(|(key, label)| KeyHint::new(key.as_str(), label))
            .collect::<Vec<_>>();
        measure_overlay_hints(&key_hints, theme)?;
        let required_hint_width = measure_overlay_hints(&key_hints[..1], theme)?;
        if required_hint_width > usize::from(footer_area.width) {
            return None;
        }
        let close = super::controls::frame_close_rect(bounds, layout.area)?;
        let footer = footer_action_layout(
            bounds,
            footer_area,
            measured.exit_label,
            required_hint_width,
            theme,
        )
        .map(|(action, hints)| (action, footer_area, hints));
        Some(Self {
            close: Some(close),
            footer,
            hints,
            position,
        })
    }

    fn paint(
        &self,
        frame: &mut Frame<'_>,
        measured: &Measured<'_>,
        layout: &Placed,
        _locale: Locale,
        theme: &Theme,
        hits: &mut HitMapBuilder,
    ) {
        let menu = &measured.menu;
        if menu.context_menu {
            return;
        }
        let inner = layout.inner;
        if let Some(close) = self.close {
            render_close(frame, close, menu, theme, hits);
        }
        let position_width = u16::try_from(display_width(&self.position)).unwrap_or(u16::MAX);
        let footer_area = Rect::new(
            inner.x,
            inner.bottom() - 1,
            inner.width.saturating_sub(position_width.saturating_add(1)),
            1,
        );
        if let Some((action, area, _)) = self.footer {
            render_footer_action(
                frame,
                action,
                area,
                measured.exit_label,
                if menu.has_parent && !menu.transferring {
                    MenuControl::Back
                } else {
                    MenuControl::Close
                },
                menu,
                theme,
                hits,
            );
        }
        let hint_area = self.footer.map_or(footer_area, |(_, _, hints)| hints);
        let hints = self
            .hints
            .iter()
            .map(|(key, label)| KeyHint::new(key.as_str(), label))
            .collect::<Vec<_>>();
        let painted_hints = render_overlay_hints(frame, hint_area, &hints, theme)
            .expect("the required exit hint was measured in the same area");
        debug_assert!(hint_area.intersection(painted_hints) == painted_hints);
        frame.render_widget(
            Paragraph::new(self.position.clone()).style(theme.style(Role::TextSecondary)),
            Rect::new(
                inner.right() - position_width,
                inner.bottom() - 1,
                position_width,
                1,
            ),
        );
    }
}

fn paint_frame(
    frame: &mut Frame<'_>,
    measured: &Measured<'_>,
    layout: &Placed,
    level: Level,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let menu = &measured.menu;
    let area = layout.area;
    let inner = layout.inner;
    let block = super::OverlayFrame::new(super::overlay_surface(theme, Padding::horizontal(1)));
    let block = match title(measured, area, locale, theme) {
        Some(title) if menu.context_menu => block.heading(super::overlay_heading(
            &title,
            usize::from(area.width).saturating_sub(6),
            theme,
        )),
        Some(title) => block.heading(super::overlay_title(&title, area.width, theme)),
        None => block,
    };

    super::clear_crossing_wide_cells(frame, area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    if let Some(query) = menu.query {
        render_search_row(
            frame,
            Rect::new(inner.x, inner.y, inner.width, 1),
            query,
            measured.result_actions.len(),
            locale,
            theme,
        );
    }

    let rule = match theme.glyphs.mode() {
        GlyphMode::Unicode => "─",
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => "-",
    };
    let marker_width =
        u16::try_from(display_width(theme.glyphs.navigation_marker).max(1)).unwrap_or(u16::MAX);
    let mut painted_rows = 0;
    let mut used_rows = 0;
    let line = |used: usize| layout.rows_y + u16::try_from(used).unwrap_or(0);
    for position in layout.first..measured.result_actions.len() {
        let action_index = measured.result_actions[position];
        let action = &menu.actions[action_index];
        let heading = menu.result_heading_before(&measured.result_actions, position);
        let separator = position > layout.first
            && menu.result_separator_before(&measured.result_actions, position);
        let lead = usize::from(separator) + usize::from(heading.is_some());
        if used_rows + lead >= layout.rows {
            break;
        }
        if separator {
            frame.render_widget(
                Paragraph::new(rule.repeat(usize::from(inner.width)))
                    .style(theme.style(Role::TextSecondary)),
                Rect::new(inner.x, line(used_rows), inner.width, 1),
            );
            used_rows += 1;
        }
        if let Some(heading) = heading {
            // Headings name a group in the label column; they are never a candidate.
            let x = (inner.x - 1).saturating_add(marker_width);
            frame.render_widget(
                Paragraph::new(locale.menu_heading(heading))
                    .style(theme.style(Role::TextSecondary)),
                Rect::new(x, line(used_rows), inner.right().saturating_sub(x), 1),
            );
            used_rows += 1;
        }
        let row_area = Rect::new(inner.x - 1, line(used_rows), inner.width + 2, 1);
        render_row(
            frame,
            row_area,
            menu,
            Row {
                position,
                action_index,
                state: match level {
                    Level::Parent if position == measured.selected => RowState::Open,
                    Level::Parent => RowState::Inert,
                    Level::Active if position == measured.selected && !menu.candidate_hidden => {
                        RowState::Candidate
                    }
                    Level::Active => RowState::Live,
                },
            },
            action,
            trailing(
                measured.binding_labels[action_index].as_deref(),
                menu.notes[action_index],
                locale,
            ),
            &layout.project_suffixes[action_index],
            locale,
            theme,
            hits,
        );
        used_rows += 1;
        painted_rows += 1;
    }
    debug_assert_eq!(painted_rows, layout.visible_rows);

    if measured.result_actions.is_empty() {
        frame.render_widget(
            Paragraph::new(locale.menu_search_no_results()).style(theme.style(Role::TextSecondary)),
            Rect::new(inner.x, layout.rows_y, inner.width, 1),
        );
    }

    // Only the candidate explains why it is unavailable; a hidden candidate explains nothing.
    let unavailable = (level == Level::Active && !menu.candidate_hidden)
        .then(|| measured.result_actions.get(measured.selected))
        .flatten()
        .and_then(|index| menu.unavailable[*index]);
    if let Some(reason) = unavailable {
        let full = locale.menu_unavailable(reason, false);
        let reason = if display_width(full) <= usize::from(inner.width) {
            full
        } else {
            locale.menu_unavailable(reason, true)
        };
        frame.render_widget(
            Paragraph::new(reason).style(theme.style(Role::TextSecondary)),
            Rect::new(
                inner.x,
                inner.bottom() - 1 - u16::try_from(measured.footer_rows).unwrap_or(0),
                inner.width,
                1,
            ),
        );
    }
}

fn visible_action_rows(menu: &MenuView<'_>, results: &[usize], first: usize, rows: usize) -> usize {
    let mut painted_rows = 0;
    let mut used_rows = 0;
    for position in first..results.len() {
        let lead = usize::from(position > first && menu.result_separator_before(results, position))
            + usize::from(menu.result_heading_before(results, position).is_some());
        if used_rows + lead >= rows {
            break;
        }
        used_rows += lead + 1;
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
                menu.source,
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
                                    &child.source,
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
        #[cfg(any(test, feature = "test-support"))]
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
    navigation: NavigationState,
    enabled: bool,
    pointer: PointerPresentation,
    checked: CheckedState,
) -> InteractionState {
    InteractionState::new(
        if navigation == NavigationState::Provisional {
            FocusState::Focused
        } else {
            FocusState::Unfocused
        },
        pointer,
        navigation,
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

/// How one painted row takes part in input.
#[derive(Clone, Copy, Eq, PartialEq)]
enum RowState {
    /// A row of the open menu that is not its candidate.
    Live,
    /// The open menu's visible candidate.
    Candidate,
    /// The cascade parent's row that opened the submenu: filled, without the marker.
    Open,
    /// Any other cascade parent row: painted without input.
    Inert,
}

#[derive(Clone, Copy)]
struct Row {
    position: usize,
    action_index: usize,
    state: RowState,
}

#[allow(clippy::too_many_arguments)]
fn render_row(
    frame: &mut Frame<'_>,
    row: Rect,
    menu: &MenuView<'_>,
    Row {
        position: result_position,
        action_index,
        state: row_state,
    }: Row,
    action: &MenuAction,
    trailing: Option<(&str, Role)>,
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
            u16::try_from(display_width(theme.glyphs.checked_marker)).unwrap_or(1),
            1,
        )
    });
    let swatch = checked.map(|rect| Rect::new(rect.right() + 1, row.y, 2, 1));
    let label_left = swatch.map_or(navigation.right(), |rect| rect.right() + 1);
    // Text stops before the row's right padding cell; a destructive band still reaches the edge.
    let content_right = row.right().saturating_sub(1);
    let binding_width = trailing
        .map(|(text, _)| display_width(text))
        .and_then(|width| u16::try_from(width).ok())
        .filter(|width| {
            content_right
                .saturating_sub(label_left)
                .saturating_sub(*width)
                > ROW_BINDING_GAP
        });
    let binding_rect = binding_width.map(|width| Rect::new(content_right - width, row.y, width, 1));
    let primary_right =
        binding_rect.map_or(row.right(), |rect| rect.x.saturating_sub(ROW_BINDING_GAP));
    let primary = Rect::new(
        label_left,
        row.y,
        primary_right.saturating_sub(label_left),
        1,
    );
    // A folder row names a path, which truncates in the middle (docs/tui/layout.md); every other
    // label truncates at its end.
    let truncate = if matches!(action, MenuAction::WorkspaceInFolder(_)) {
        truncate_display_middle
    } else {
        truncate_display
    };
    let label = truncate(
        &action_label(
            locale,
            menu.source,
            action,
            menu.labels[action_index].as_deref(),
            menu.tabs.map(|tabs| tabs.rows[action_index]),
            theme.glyphs.middle_dot,
            theme.glyphs.disclosure_closed,
        ),
        usize::from(primary_right.min(content_right).saturating_sub(label_left))
            .saturating_sub(display_width(project_suffix)),
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
    if let (Some((text, role)), Some(rect)) = (trailing, binding_rect) {
        frame.render_widget(Paragraph::new(text).style(theme.style(role)), rect);
        bands.push(RowContentBand::semantic(rect, role));
    }

    let enabled = menu.unavailable[action_index].is_none() && !menu.transferring;
    let live = matches!(row_state, RowState::Live | RowState::Candidate) && !menu.transferring;
    let target = InteractionTarget::ActionMenuRow {
        source: menu.source.clone(),
        token: menu.token,
        row: result_position,
    };
    let spec = interaction_spec(enabled);
    let pointer = if live {
        pointer_presentation(hits.projected_pointer(HitLayer::Overlay, row, row, &target, spec))
    } else {
        PointerPresentation::Rest
    };
    let mut tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary)
        .with_provisional(navigation)
        .with_primary_role(primary_role);
    if let Some(rect) = checked {
        tracks = tracks.with_checked(rect, theme.glyphs.checked_marker);
    }
    let applied = if menu.tabs.is_some_and(|tabs| tabs.checked(action)) {
        CheckedState::Checked
    } else {
        CheckedState::Unchecked
    };
    let navigation = match row_state {
        RowState::Candidate => NavigationState::Provisional,
        RowState::Open => NavigationState::Open,
        RowState::Live | RowState::Inert => NavigationState::None,
    };
    let state = interaction_state(navigation, enabled, pointer, applied);
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
    if live {
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

/// A row's trailing text: its shortcut, else its note.
fn trailing(
    binding: Option<&str>,
    note: Option<MenuRowNote>,
    locale: Locale,
) -> Option<(&str, Role)> {
    binding
        .map(|binding| (binding, Role::HintKey))
        .or_else(|| note.map(|note| (locale.menu_row_note(note), Role::TextSecondary)))
}

fn action_label(
    locale: Locale,
    source: &MenuTarget,
    action: &MenuAction,
    label: Option<&str>,
    scope: Option<TabMenuScope>,
    separator: &str,
    disclosure: &str,
) -> String {
    // A choice row under its group shows only its own name.
    let choice = matches!(
        action,
        MenuAction::JoinWorkspaceProject(_)
            | MenuAction::WorkspaceInFolder(_)
            | MenuAction::OpenWorktreeAt(_)
            | MenuAction::JoinGroup(_)
    );
    let text = scope.map_or_else(
        || locale.menu_action(action).to_owned(),
        |scope| locale.menu_tab_action(action, scope),
    );
    if let MenuAction::NewTabWithProfile(profile) = action {
        let name = locale.launch_profile_label(profile.name.as_deref());
        // The new-Tab button's own menu lists only the profile each row opens.
        return if matches!(source, MenuTarget::TabProfiles(_)) {
            name
        } else {
            format!("{text} {separator} {name}")
        };
    }
    let text = if let Some(label) = label {
        let label = if matches!(action, MenuAction::JoinWorkspaceProject(_)) {
            crate::machine_discovery::display_label(label)
        } else if label.is_empty()
            && matches!(
                action,
                MenuAction::JoinGroup(_) | MenuAction::NewTabInRecentGroup
            )
        {
            locale.shell().unnamed_tab_group.to_owned()
        } else {
            sanitize_terminal_text(label).into_owned()
        };
        if choice {
            label
        } else {
            format!("{text} {separator} {label}")
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
        display_width(theme.glyphs.checked_marker) + 4
    } else {
        0
    }
}

/// A Tab menu names its Tab; a group menu names its group and the group's Tab count. The name
/// comes first: a title too wide for both drops the count whole before the name truncates.
fn tab_title(menu: &MenuView<'_>, locale: Locale, theme: &Theme, width: usize) -> String {
    let Some(tabs) = menu.tabs else {
        return String::new();
    };
    let name = sanitize_terminal_text(if menu.title.is_empty() {
        locale.shell().unnamed_tab_group
    } else {
        menu.title
    });
    if matches!(menu.source, MenuTarget::Tab(_)) {
        return truncate_display(&name, width, theme.glyphs.ellipsis);
    }
    let titled = format!(
        "{name} {} {}",
        theme.glyphs.middle_dot,
        locale.menu_group_path(tabs.scope)
    );
    if display_width(&titled) <= width {
        titled
    } else {
        truncate_display(&name, width, theme.glyphs.ellipsis)
    }
}

#[cfg(test)]
mod tests;
