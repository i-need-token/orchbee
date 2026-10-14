//! Scrollable common settings; only controls from the last successful frame authorize input.
use crate::{
    client_view::{KeyLabelGlyphs, LabelStyle, format_binding_with_glyphs},
    localization::Locale,
    native_settings::{
        Adjustment, Control, DraftView, Page, Row, SettingControl, SettingsDraw, View,
        footer_bindings, setting_for_row,
    },
    terminal_session::ModalPresentation,
    terminal_text::{
        display_width, project_terminal_text, sanitize_terminal_text, truncate_display,
    },
    tui_pointer::{
        HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
        ProjectedPointer,
    },
    tui_theme::{GlyphMode, Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Clear, Padding, Paragraph},
};

use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    IconActionGlyph, IconActionSpec, InteractionState, KeyHint, NavigationState, OperationState,
    PointerPresentation, RowContentBand, RowFocusCue, RowTracks, measure_key_hints,
};

#[cfg(test)]
mod tests;

pub(super) fn render(
    frame: &mut Frame<'_>,
    content: Rect,
    view: View<'_>,
    pending: bool,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalPresentation<SettingsDraw> {
    let Some(layout) = SettingsLayout::resolve(frame.area(), content, view, locale, theme) else {
        return ModalPresentation::NotPresented;
    };
    hits.start_settings();
    let block =
        super::overlay_surface(theme, true, Padding::horizontal(1)).title(locale.settings_title());
    frame.render_widget(Clear, layout.area);
    frame.render_widget(block, layout.area);
    let painted_close = render_close(frame, layout.close, view.token(), theme, hits);
    debug_assert_eq!(painted_close, Some(layout.close));
    let draw = if let View::Draft(draft) = view {
        render_draft(
            frame,
            layout.inner,
            draft,
            layout
                .draft
                .expect("draft layout carries its admitted draw"),
            locale,
            theme,
            hits,
        )
    } else {
        if matches!(view, View::Saving { .. }) {
            frame.render_widget(
                Paragraph::new(locale.settings_saving_fact()).style(theme.style(Role::StatusInfo)),
                Rect::new(layout.inner.x, layout.inner.y, layout.inner.width, 1),
            );
        }
        if pending {
            let y = layout.inner.y
                + if matches!(view, View::Saving { .. }) {
                    1
                } else {
                    0
                };
            frame.render_widget(
                Paragraph::new(locale.settings_pending(matches!(view, View::Saving { .. })))
                    .style(theme.style(Role::StatusPending)),
                Rect::new(layout.inner.x, y, layout.inner.width, 1),
            );
        }
        SettingsDraw::surface(view.token())
    };
    let painted_hints = render_footer_hints(frame, layout.hints, view, locale, theme);
    debug_assert!(painted_hints.is_some());
    render_footer_actions(
        frame,
        layout.actions,
        layout.footer_actions,
        view,
        locale,
        theme,
        hits,
    );
    hits.record(
        HitLayer::OverlaySurface,
        layout.area,
        HitTarget::OverlaySurface,
    );
    ModalPresentation::Presented(draw)
}

#[derive(Clone, Copy)]
struct SettingsLayout {
    area: Rect,
    inner: Rect,
    close: Rect,
    hints: Rect,
    actions: Rect,
    footer_actions: FooterActionGeometry,
    draft: Option<SettingsDraw>,
}

impl SettingsLayout {
    fn resolve(
        bounds: Rect,
        content: Rect,
        view: View<'_>,
        locale: Locale,
        theme: &Theme,
    ) -> Option<Self> {
        if content.width < 36 || content.height < 10 || bounds.intersection(content) != content {
            return None;
        }
        let width = content.width.min(92);
        let height = content.height.min(26);
        let area = Rect::new(
            content.x + (content.width - width) / 2,
            content.y + (content.height - height) / 2,
            width,
            height,
        );
        if bounds.intersection(area) != area {
            return None;
        }
        let block = super::overlay_surface(theme, true, Padding::horizontal(1))
            .title(locale.settings_title());
        let inner = block.inner(area);
        if inner.width == 0 || inner.height < 6 || bounds.intersection(inner) != inner {
            return None;
        }
        let close = super::controls::close_rect(bounds, area)?;
        let hints = Rect::new(inner.x, inner.bottom().checked_sub(2)?, inner.width, 1);
        footer_hints_admitted(bounds, hints, view, locale, theme)?;
        let actions = Rect::new(inner.x, inner.bottom().checked_sub(1)?, inner.width, 1);
        let footer_actions = FooterActionGeometry::resolve(bounds, actions, view, locale, theme)?;
        let draft = match view {
            View::Draft(draft) => {
                let visible = usize::from(inner.height.checked_sub(5)?);
                let draw = draft.presentation(visible);
                draft_geometry_admitted(bounds, inner, draft, draw, locale, theme)
                    .then_some(draw)?;
                Some(draw)
            }
            View::Loading { .. } | View::Saving { .. } => None,
        };
        Some(Self {
            area,
            inner,
            close,
            hints,
            actions,
            footer_actions,
            draft,
        })
    }
}

struct ResolvedFooterHint {
    key: String,
    label: &'static str,
}

fn resolved_footer_hints(view: View<'_>, locale: Locale, theme: &Theme) -> Vec<ResolvedFooterHint> {
    let key_glyphs = match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(test)]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    };
    footer_bindings(view)
        .iter()
        .map(|hint| ResolvedFooterHint {
            key: hint
                .bindings()
                .map(|binding| {
                    format_binding_with_glyphs(&binding, LabelStyle::for_host(), key_glyphs)
                })
                .collect::<Vec<_>>()
                .join("/"),
            label: locale.settings_hint_label(hint.label()),
        })
        .collect()
}

fn key_hints(resolved: &[ResolvedFooterHint]) -> Vec<KeyHint<'_>> {
    resolved
        .iter()
        .map(|hint| KeyHint::new(&hint.key, hint.label))
        .collect()
}

fn footer_hints_admitted(
    bounds: Rect,
    area: Rect,
    view: View<'_>,
    locale: Locale,
    theme: &Theme,
) -> Option<()> {
    if area.is_empty() || bounds.intersection(area) != area {
        return None;
    }
    let resolved = resolved_footer_hints(view, locale, theme);
    let hints = key_hints(&resolved);
    let _ = measure_key_hints(&hints, theme)?;
    let first = measure_key_hints(hints.get(..1)?, theme)?;
    (first <= usize::from(area.width)).then_some(())
}

fn render_footer_hints(
    frame: &mut Frame<'_>,
    area: Rect,
    view: View<'_>,
    locale: Locale,
    theme: &Theme,
) -> Option<Rect> {
    let resolved = resolved_footer_hints(view, locale, theme);
    let hints = key_hints(&resolved);
    super::controls::render_key_hints(frame, area, &hints, theme)
}

fn rest_state(enabled: bool) -> InteractionState {
    InteractionState::new(
        FocusState::Unfocused,
        PointerPresentation::Rest,
        NavigationState::None,
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

fn interaction_spec(enabled: bool) -> InteractionSpec {
    if enabled {
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
    } else {
        InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
    }
}

fn pointer_presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
}

fn action_spec(
    label: &str,
    kind: ActionKind,
    enabled: bool,
    pointer: PointerPresentation,
) -> ActionSpec<'_> {
    ActionSpec::new(
        label,
        kind,
        ControlSurface::Overlay,
        rest_state(enabled).with_pointer(pointer),
    )
}

fn render_close(
    frame: &mut Frame<'_>,
    rect: Rect,
    token: crate::native_settings::Token,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    let target = InteractionTarget::SettingsControl {
        token,
        control: Control::Close,
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
    )?;
    if painted != rect {
        return None;
    }
    hits.record_interaction(HitLayer::Overlay, painted, painted, target, spec);
    Some(painted)
}

struct ActionPaint<'a> {
    area: Rect,
    rect: Rect,
    label: &'a str,
    kind: ActionKind,
    enabled: bool,
    target: InteractionTarget,
}

#[derive(Clone, Copy)]
enum FooterActionGeometry {
    Pair {
        first: Rect,
        second_area: Rect,
        second: Rect,
    },
    Single {
        rect: Rect,
    },
}

impl FooterActionGeometry {
    fn resolve(
        bounds: Rect,
        row: Rect,
        view: View<'_>,
        locale: Locale,
        theme: &Theme,
    ) -> Option<Self> {
        if row.is_empty() || bounds.intersection(row) != row {
            return None;
        }
        let child = matches!(
            view,
            View::Draft(draft) if draft.text.is_some() || draft.confirmation.is_some()
        );
        if child {
            let accept_enabled =
                !matches!(view, View::Draft(draft) if draft.text.is_some() && draft.invalid);
            let first_spec = action_spec(
                locale.settings_cancel(),
                ActionKind::Secondary,
                true,
                PointerPresentation::Rest,
            );
            let second_spec = action_spec(
                locale.settings_accept_button(),
                ActionKind::Primary,
                accept_enabled,
                PointerPresentation::Rest,
            );
            let first = super::controls::action_rect(bounds, row, first_spec, theme)?;
            let second_x = first.right().checked_add(2)?;
            let second_area = Rect::new(
                second_x,
                row.y,
                row.right().checked_sub(second_x)?,
                row.height,
            );
            let second = super::controls::action_rect(bounds, second_area, second_spec, theme)?;
            Some(Self::Pair {
                first,
                second_area,
                second,
            })
        } else {
            let label = if matches!(view, View::Draft(draft) if draft.page == Page::Profiles) {
                locale.settings_back_button()
            } else {
                locale.settings_close_button()
            };
            let spec = action_spec(
                label,
                ActionKind::Secondary,
                true,
                PointerPresentation::Rest,
            );
            let rect = super::controls::action_rect(bounds, row, spec, theme)?;
            Some(Self::Single { rect })
        }
    }
}

fn render_interactive_action(
    frame: &mut Frame<'_>,
    action: ActionPaint<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    let spec = interaction_spec(action.enabled);
    let pointer = pointer_presentation(hits.projected_pointer(
        HitLayer::Overlay,
        action.rect,
        action.rect,
        &action.target,
        spec,
    ));
    let painted = super::controls::render_action(
        frame,
        action.area,
        action_spec(action.label, action.kind, action.enabled, pointer),
        theme,
    )?;
    if painted != action.rect {
        return None;
    }
    hits.record_interaction(HitLayer::Overlay, painted, painted, action.target, spec);
    Some(painted)
}

fn render_footer_actions(
    frame: &mut Frame<'_>,
    row: Rect,
    geometry: FooterActionGeometry,
    view: View<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let child =
        matches!(view, View::Draft(draft) if draft.text.is_some() || draft.confirmation.is_some());
    if child {
        let FooterActionGeometry::Pair {
            first,
            second_area,
            second,
        } = geometry
        else {
            unreachable!("preflighted child actions use pair geometry");
        };
        let labels = [locale.settings_cancel(), locale.settings_accept_button()];
        let accept_enabled =
            !matches!(view, View::Draft(draft) if draft.text.is_some() && draft.invalid);
        let token = view.token();
        let Some(first) = render_interactive_action(
            frame,
            ActionPaint {
                area: row,
                rect: first,
                label: labels[0],
                kind: ActionKind::Secondary,
                enabled: true,
                target: InteractionTarget::SettingsControl {
                    token,
                    control: Control::Back,
                },
            },
            theme,
            hits,
        ) else {
            return;
        };
        let Some(painted_second) = render_interactive_action(
            frame,
            ActionPaint {
                area: second_area,
                rect: second,
                label: labels[1],
                kind: ActionKind::Primary,
                enabled: accept_enabled,
                target: InteractionTarget::SettingsControl {
                    token,
                    control: Control::Accept,
                },
            },
            theme,
            hits,
        ) else {
            return;
        };
        debug_assert_ne!(first, painted_second);
        return;
    }

    let back = matches!(view, View::Draft(draft) if draft.page == Page::Profiles);
    let FooterActionGeometry::Single { rect } = geometry else {
        unreachable!("preflighted root actions use single geometry");
    };
    let (label, control) = if back {
        (locale.settings_back_button(), Control::Back)
    } else {
        (locale.settings_close_button(), Control::Close)
    };
    let painted = render_interactive_action(
        frame,
        ActionPaint {
            area: row,
            rect,
            label,
            kind: ActionKind::Secondary,
            enabled: true,
            target: InteractionTarget::SettingsControl {
                token: view.token(),
                control,
            },
        },
        theme,
        hits,
    );
    debug_assert_eq!(painted, Some(rect));
}

fn render_draft(
    frame: &mut Frame<'_>,
    inner: Rect,
    draft: DraftView<'_>,
    draw: SettingsDraw,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> SettingsDraw {
    let rows = draft.page.rows();
    let selected = rows
        .iter()
        .position(|row| *row == draft.selected)
        .unwrap_or(0);
    let start = draw.first_visible().unwrap_or(0);
    let visible = draw.visible_rows().unwrap_or(0);
    let list_active = draft.text.is_none() && draft.confirmation.is_none();
    let context = RowContext {
        draft,
        list_active,
        locale,
        theme,
    };
    if list_active {
        hits.record(
            HitLayer::OverlaySurface,
            Rect::new(inner.x, inner.y + 1, inner.width, visible as u16),
            HitTarget::ListScroll(crate::tui_pointer::ListScrollTarget::Settings(draft.token)),
        );
    }
    let heading = format!(
        "{}  {}/{}",
        locale.settings_category(draft.selected),
        selected + 1,
        rows.len()
    );
    frame.render_widget(
        Paragraph::new(heading).style(theme.style(Role::TextSecondary)),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    for (index, row) in rows.iter().copied().enumerate().skip(start).take(visible) {
        render_setting_row(
            frame,
            Rect::new(
                inner.x,
                inner.y + 1 + (index - start) as u16,
                inner.width,
                1,
            ),
            context,
            row,
            hits,
        );
    }

    let detail_area = Rect::new(inner.x, inner.bottom() - 4, inner.width, 1);
    if let Some((text, cursor)) = draft.text {
        render_editor(
            frame,
            detail_area,
            draft.selected,
            text,
            cursor,
            locale,
            theme,
        );
    } else {
        frame.render_widget(
            Paragraph::new(locale.settings_detail(draft)).style(theme.style(if draft.invalid {
                Role::StatusError
            } else {
                Role::TextSecondary
            })),
            detail_area,
        );
    }
    render_state_facts(
        frame,
        Rect::new(inner.x, inner.bottom() - 3, inner.width, 1),
        draft,
        locale,
        theme,
    );
    draw
}

#[derive(Clone, Copy)]
struct RowContext<'a> {
    draft: DraftView<'a>,
    list_active: bool,
    locale: Locale,
    theme: &'a Theme,
}

#[derive(Clone, Copy)]
struct RowGeometry {
    navigation: Rect,
    dirty: Rect,
    checked_open: Rect,
    checked: Rect,
    checked_close: Rect,
    primary: Rect,
    trailing: Rect,
    status: Rect,
}

impl RowGeometry {
    fn new(row: Rect) -> Option<Self> {
        if row.height == 0 || row.width < 16 {
            return None;
        }
        let navigation = Rect::new(row.x, row.y, 1, 1);
        let dirty = Rect::new(row.x + 1, row.y, 1, 1);
        let checked_open = Rect::new(row.x + 2, row.y, 1, 1);
        let checked = Rect::new(row.x + 3, row.y, 1, 1);
        let checked_close = Rect::new(row.x + 4, row.y, 1, 1);
        let label_x = row.x + 6;
        let status = Rect::new(row.right() - 1, row.y, 1, 1);
        let available = status.x.saturating_sub(label_x);
        let trailing_width = (available / 2)
            .clamp(5, 28)
            .min(available.saturating_sub(2));
        let primary_width = available.saturating_sub(trailing_width).saturating_sub(1);
        if primary_width == 0 || trailing_width < 5 {
            return None;
        }
        let primary = Rect::new(label_x, row.y, primary_width, 1);
        let trailing = Rect::new(primary.right() + 1, row.y, trailing_width, 1);
        Some(Self {
            navigation,
            dirty,
            checked_open,
            checked,
            checked_close,
            primary,
            trailing,
            status,
        })
    }
}

#[derive(Clone, Copy)]
struct RowActionPaint<'a> {
    area: Rect,
    rect: Rect,
    label: &'a str,
    kind: ActionKind,
    role: Role,
}

fn row_action_kind(row: Row) -> Option<ActionKind> {
    match row {
        Row::Save => Some(ActionKind::Primary),
        Row::ResetAll | Row::Reload => Some(ActionKind::Secondary),
        _ => None,
    }
}

fn draft_geometry_admitted(
    bounds: Rect,
    inner: Rect,
    draft: DraftView<'_>,
    draw: SettingsDraw,
    locale: Locale,
    theme: &Theme,
) -> bool {
    let Some(start) = draw.first_visible() else {
        return false;
    };
    let Some(visible) = draw.visible_rows() else {
        return false;
    };
    let rows = draft.page.rows();
    let Some(end) = start.checked_add(visible) else {
        return false;
    };
    if visible == 0 || end > rows.len() {
        return false;
    }
    let Some(visible_height) = u16::try_from(visible).ok() else {
        return false;
    };
    let list = Rect::new(inner.x, inner.y + 1, inner.width, visible_height);
    if bounds.intersection(list) != list || inner.intersection(list) != list {
        return false;
    }
    for (offset, row) in rows[start..end].iter().copied().enumerate() {
        let Some(offset) = u16::try_from(offset).ok() else {
            return false;
        };
        let row_area = Rect::new(inner.x, inner.y + 1 + offset, inner.width, 1);
        let Some(geometry) = RowGeometry::new(row_area) else {
            return false;
        };
        if bounds.intersection(row_area) != row_area || inner.intersection(row_area) != row_area {
            return false;
        }
        let Some(kind) = row_action_kind(row) else {
            continue;
        };
        let Some(label) = locale.settings_row_action(row) else {
            return false;
        };
        let area = Rect::new(
            geometry.primary.x,
            row_area.y,
            geometry.status.x.saturating_sub(geometry.primary.x),
            1,
        );
        let spec = action_spec(label, kind, draft.enabled(row), PointerPresentation::Rest);
        if super::controls::action_rect(bounds, area, spec, theme).is_none() {
            return false;
        }
    }
    let detail = Rect::new(inner.x, inner.bottom() - 4, inner.width, 1);
    let facts = Rect::new(inner.x, inner.bottom() - 3, inner.width, 1);
    if bounds.intersection(detail) != detail
        || inner.intersection(detail) != detail
        || bounds.intersection(facts) != facts
        || inner.intersection(facts) != facts
    {
        return false;
    }
    draft.text.is_none() || editor_geometry_admitted(detail, draft.selected, locale, theme)
}

fn editor_geometry_admitted(area: Rect, row: Row, locale: Locale, theme: &Theme) -> bool {
    let label_width = usize::from(area.width / 3).max(1);
    let label = truncate_display(
        &sanitize_terminal_text(locale.settings_row(row)),
        label_width,
        theme.glyphs.ellipsis,
    );
    let prefix_width = display_width(&format!("{label}: ")).saturating_add(1);
    let Some(value_width) = usize::from(area.width).checked_sub(prefix_width.saturating_add(1))
    else {
        return false;
    };
    let Ok(prefix_width) = u16::try_from(prefix_width) else {
        return false;
    };
    let Ok(value_width) = u16::try_from(value_width) else {
        return false;
    };
    if value_width == 0 {
        return false;
    }
    let value_area = Rect::new(area.x + prefix_width, area.y, value_width, 1);
    area.intersection(value_area) == value_area
}

fn render_row_action(
    frame: &mut Frame<'_>,
    area: Rect,
    row: Row,
    enabled: bool,
    locale: Locale,
    theme: &Theme,
) -> Option<RowActionPaint<'static>> {
    let kind = row_action_kind(row)?;
    let label = locale.settings_row_action(row)?;
    let role = match kind {
        ActionKind::Primary => Role::AccentAction,
        ActionKind::Secondary => Role::TextPrimary,
    };
    let spec = action_spec(label, kind, enabled, PointerPresentation::Rest);
    let rect = super::controls::action_rect(frame.area(), area, spec, theme)?;
    let painted = super::controls::render_action(frame, area, spec, theme)?;
    (painted == rect).then_some(RowActionPaint {
        area,
        rect,
        label,
        kind,
        role,
    })
}

fn repaint_pressed_row_action(
    frame: &mut Frame<'_>,
    action: RowActionPaint<'_>,
    enabled: bool,
    pointer: PointerPresentation,
    theme: &Theme,
) -> Option<Rect> {
    if pointer != PointerPresentation::Pressed || action.kind != ActionKind::Primary {
        return Some(action.rect);
    }
    let painted = super::controls::render_action(
        frame,
        action.area,
        action_spec(action.label, action.kind, enabled, pointer),
        theme,
    )?;
    (painted == action.rect).then_some(painted)
}

fn render_setting_row(
    frame: &mut Frame<'_>,
    row_area: Rect,
    context: RowContext<'_>,
    row: Row,
    hits: &mut HitMapBuilder,
) {
    let RowContext {
        draft,
        list_active,
        locale,
        theme,
    } = context;
    let Some(geometry) = RowGeometry::new(row_area) else {
        return;
    };
    let enabled = draft.enabled(row);
    let selected = list_active && row == draft.selected;
    let checked = draft.checked(row);
    let base_state = InteractionState::new(
        if selected {
            FocusState::Focused
        } else {
            FocusState::Unfocused
        },
        PointerPresentation::Rest,
        if selected {
            NavigationState::Provisional
        } else {
            NavigationState::None
        },
        CurrentState::NotCurrent,
        if checked == Some(true) {
            CheckedState::Checked
        } else {
            CheckedState::Unchecked
        },
        if enabled {
            Availability::Enabled
        } else {
            Availability::Disabled
        },
        OperationState::Idle,
    );

    frame.render_widget(
        Paragraph::new(" ".repeat(usize::from(row_area.width)))
            .style(theme.style(Role::SurfaceOverlay)),
        row_area,
    );
    let action_kind = row_action_kind(row);
    let action = if action_kind.is_some() {
        let area = Rect::new(
            geometry.primary.x,
            row_area.y,
            geometry.status.x.saturating_sub(geometry.primary.x),
            1,
        );
        let Some(action) = render_row_action(frame, area, row, enabled, locale, theme) else {
            return;
        };
        Some(action)
    } else {
        let label = truncate_display(
            &sanitize_terminal_text(locale.settings_row(row)),
            usize::from(geometry.primary.width),
            theme.glyphs.ellipsis,
        );
        frame.render_widget(
            Paragraph::new(label).style(theme.style(Role::TextPrimary)),
            geometry.primary,
        );
        None
    };
    let mut bands = Vec::new();
    if checked.is_some() {
        frame.render_widget(
            Paragraph::new("[").style(theme.style(Role::TextSecondary)),
            geometry.checked_open,
        );
        frame.render_widget(
            Paragraph::new("]").style(theme.style(Role::TextSecondary)),
            geometry.checked_close,
        );
        bands.extend([
            RowContentBand::semantic(geometry.checked_open, Role::TextSecondary),
            RowContentBand::semantic(geometry.checked_close, Role::TextSecondary),
        ]);
    }
    if draft.changed(row) {
        frame.render_widget(
            Paragraph::new("~").style(theme.style(Role::TextPrimary)),
            geometry.dirty,
        );
        bands.push(RowContentBand::semantic(geometry.dirty, Role::TextPrimary));
    }

    let adjustment = draft.adjustment(row);
    let value = if action.is_some() {
        RowValuePaint::empty()
    } else {
        render_row_value(frame, geometry.trailing, context, row, adjustment, enabled)
    };
    bands.extend(value.bands);
    let primary = action.map_or(geometry.primary, |action| action.rect);
    let primary_role = action.map_or(Role::TextPrimary, |action| action.role);
    let mut tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary)
        .with_primary_role(primary_role)
        .with_provisional(geometry.navigation)
        .with_status(geometry.status);
    if checked.is_some() {
        tracks = tracks.with_checked(geometry.checked, "x");
    }
    let Some(painted_row) = super::controls::paint_row_interaction(
        frame,
        row_area,
        tracks,
        &bands,
        ControlSurface::Overlay,
        base_state,
        theme,
    ) else {
        return;
    };
    if painted_row != row_area {
        return;
    }
    if list_active {
        let projected_adjustments = value.adjustment_targets.map(|targets| {
            targets.map(|target| {
                paint_and_record_adjustment(frame, hits, draft.token, row, target, theme)
                    .expect("validated adjustment geometry remains paintable")
            })
        });
        let target = InteractionTarget::SettingsRow {
            token: draft.token,
            row,
        };
        let spec = interaction_spec(enabled);
        let pointer = pointer_presentation(hits.projected_pointer(
            HitLayer::Overlay,
            row_area,
            row_area,
            &target,
            spec,
        ));
        let painted_row = super::controls::paint_row_interaction(
            frame,
            row_area,
            tracks,
            &bands,
            ControlSurface::Overlay,
            base_state.with_pointer(pointer),
            theme,
        )
        .expect("validated row geometry remains paintable");
        debug_assert_eq!(painted_row, row_area);
        if let Some(projected_adjustments) = projected_adjustments {
            for projected in projected_adjustments {
                let painted = paint_adjustment_pointer(frame, projected, theme)
                    .expect("validated adjustment geometry remains paintable");
                debug_assert_eq!(painted, projected.target.rect);
            }
        }
        if let Some(action) = action {
            let painted = repaint_pressed_row_action(frame, action, enabled, pointer, theme)
                .expect("validated row action geometry remains paintable");
            debug_assert_eq!(painted, action.rect);
        }
        hits.record_interaction(HitLayer::Overlay, row_area, painted_row, target, spec);
    }
}

#[derive(Clone, Copy)]
struct AdjustmentTarget {
    rect: Rect,
    forward: bool,
    enabled: bool,
}

#[derive(Clone, Copy)]
struct ProjectedAdjustment {
    target: AdjustmentTarget,
    pointer: PointerPresentation,
}

fn paint_and_record_adjustment(
    frame: &mut Frame<'_>,
    hits: &mut HitMapBuilder,
    token: crate::native_settings::Token,
    row: Row,
    target: AdjustmentTarget,
    theme: &Theme,
) -> Option<ProjectedAdjustment> {
    let interaction_target = InteractionTarget::SettingsControl {
        token,
        control: Control::Adjust {
            row,
            forward: target.forward,
        },
    };
    let spec = interaction_spec(target.enabled);
    let pointer = pointer_presentation(hits.projected_pointer(
        HitLayer::Overlay,
        target.rect,
        target.rect,
        &interaction_target,
        spec,
    ));
    let projected = ProjectedAdjustment { target, pointer };
    let painted = paint_adjustment_pointer(frame, projected, theme)?;
    if painted != target.rect {
        return None;
    }
    hits.record_interaction(
        HitLayer::Overlay,
        target.rect,
        painted,
        interaction_target,
        spec,
    );
    Some(projected)
}

fn paint_adjustment_pointer(
    frame: &mut Frame<'_>,
    projected: ProjectedAdjustment,
    theme: &Theme,
) -> Option<Rect> {
    super::controls::paint_pointer_target(
        frame,
        projected.target.rect,
        ControlSurface::Overlay,
        projected.pointer,
        if projected.target.enabled {
            Role::TextPrimary
        } else {
            Role::TextDisabled
        },
        if projected.target.enabled {
            Availability::Enabled
        } else {
            Availability::Disabled
        },
        theme,
    )
}

struct RowValuePaint {
    adjustment_targets: Option<[AdjustmentTarget; 2]>,
    bands: Vec<RowContentBand>,
}

impl RowValuePaint {
    fn static_band(rect: Rect, role: Role) -> Self {
        Self {
            adjustment_targets: None,
            bands: vec![RowContentBand::semantic(rect, role)],
        }
    }

    fn empty() -> Self {
        Self {
            adjustment_targets: None,
            bands: Vec::new(),
        }
    }
}

fn settings_value(locale: Locale, draft: DraftView<'_>, row: Row) -> String {
    let value = locale.settings_value(draft, row);
    let Some(metadata) = setting_for_row(row) else {
        return value;
    };
    if let SettingControl::BoundedInteger { unit, .. } = metadata.control {
        locale.settings_value_with_unit(&value, unit)
    } else {
        value
    }
}

fn row_uses_choice_trigger(row: Row) -> bool {
    setting_for_row(row).is_some_and(|metadata| {
        matches!(
            metadata.control,
            SettingControl::FiniteChoice | SettingControl::Catalog
        )
    }) || row == Row::CommandType
}

fn row_uses_editor(row: Row) -> bool {
    setting_for_row(row).is_some_and(|metadata| matches!(metadata.control, SettingControl::Color))
        || matches!(row, Row::ProfileName | Row::Executable | Row::ArgumentValue)
}

fn pad_display(value: &str, width: usize, ellipsis: &str) -> String {
    let mut value = truncate_display(value, width, ellipsis);
    value.push_str(&" ".repeat(width.saturating_sub(display_width(&value))));
    value
}

fn render_row_value(
    frame: &mut Frame<'_>,
    area: Rect,
    context: RowContext<'_>,
    row: Row,
    adjustment: Option<Adjustment>,
    enabled: bool,
) -> RowValuePaint {
    let RowContext {
        draft,
        locale,
        theme,
        ..
    } = context;
    let raw_value = settings_value(locale, draft, row);
    let value = sanitize_terminal_text(&raw_value);
    if let Some(adjustment) = adjustment
        && area.width >= 9
    {
        let value_area = Rect::new(area.x + 4, area.y, area.width - 8, 1);
        frame.render_widget(
            Paragraph::new("[-]").style(theme.style(if enabled && adjustment.backward {
                Role::TextPrimary
            } else {
                Role::TextDisabled
            })),
            Rect::new(area.x, area.y, 3, 1),
        );
        frame.render_widget(
            Paragraph::new(pad_display(
                &value,
                usize::from(value_area.width),
                theme.glyphs.ellipsis,
            ))
            .style(theme.style(Role::TextSecondary)),
            value_area,
        );
        frame.render_widget(
            Paragraph::new("[+]").style(theme.style(if enabled && adjustment.forward {
                Role::TextPrimary
            } else {
                Role::TextDisabled
            })),
            Rect::new(area.right() - 3, area.y, 3, 1),
        );
        let backward = Rect::new(area.x, area.y, 3, 1);
        let forward = Rect::new(area.right() - 3, area.y, 3, 1);
        return RowValuePaint {
            adjustment_targets: Some([
                AdjustmentTarget {
                    rect: backward,
                    forward: false,
                    enabled: enabled && adjustment.backward,
                },
                AdjustmentTarget {
                    rect: forward,
                    forward: true,
                    enabled: enabled && adjustment.forward,
                },
            ]),
            bands: vec![
                RowContentBand::semantic(
                    backward,
                    if enabled && adjustment.backward {
                        Role::TextPrimary
                    } else {
                        Role::TextDisabled
                    },
                ),
                RowContentBand::semantic(value_area, Role::TextSecondary),
                RowContentBand::semantic(
                    forward,
                    if enabled && adjustment.forward {
                        Role::TextPrimary
                    } else {
                        Role::TextDisabled
                    },
                ),
            ],
        };
    }
    if draft.checked(row).is_some() {
        return RowValuePaint::empty();
    }
    let width = usize::from(area.width);
    let text = if row_uses_choice_trigger(row) && width >= 6 {
        format!(
            "[ {} > ]",
            pad_display(&value, width - 6, theme.glyphs.ellipsis)
        )
    } else if row_uses_editor(row) && width >= 4 {
        format!(
            "[ {} ]",
            pad_display(&value, width - 4, theme.glyphs.ellipsis)
        )
    } else {
        pad_display(&value, width, theme.glyphs.ellipsis)
    };
    frame.render_widget(
        Paragraph::new(text).style(theme.style(Role::TextSecondary)),
        area,
    );
    RowValuePaint::static_band(area, Role::TextSecondary)
}

fn render_editor(
    frame: &mut Frame<'_>,
    area: Rect,
    row: Row,
    text: &str,
    cursor: usize,
    locale: Locale,
    theme: &Theme,
) {
    let label_width = usize::from(area.width / 3).max(1);
    let label = truncate_display(
        &sanitize_terminal_text(locale.settings_row(row)),
        label_width,
        theme.glyphs.ellipsis,
    );
    let label = format!("{label}: ");
    let prefix_width = display_width(&label).saturating_add(1);
    let Some(value_width) = usize::from(area.width).checked_sub(prefix_width.saturating_add(1))
    else {
        return;
    };
    if value_width == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(label, theme.style(Role::TextSecondary)),
            Span::styled("[", theme.style(Role::AccentFocus)),
        ])),
        area,
    );
    frame.buffer_mut().set_string(
        area.right() - 1,
        area.y,
        "]",
        theme.style(Role::AccentFocus),
    );
    let projection = project_terminal_text(text);
    let displayed = projection.display();
    let cursor_column = display_width(&displayed[..projection.cursor_boundary(cursor)]);
    let left = cursor_column.saturating_sub(value_width.saturating_sub(1));
    let value_area = Rect::new(
        area.x + u16::try_from(prefix_width).unwrap_or(area.width),
        area.y,
        u16::try_from(value_width).unwrap_or(u16::MAX),
        1,
    );
    frame.render_widget(
        Paragraph::new(displayed.as_ref())
            .scroll((0, left.min(usize::from(u16::MAX)) as u16))
            .style(theme.style(Role::TextPrimary)),
        value_area,
    );
    frame.set_cursor_position((
        value_area.x + u16::try_from(cursor_column - left).unwrap_or(0),
        value_area.y,
    ));
}

fn render_state_facts(
    frame: &mut Frame<'_>,
    area: Rect,
    draft: DraftView<'_>,
    locale: Locale,
    theme: &Theme,
) {
    let facts = locale.settings_state_facts(draft);
    let (glyph, glyph_role, text) = if draft.invalid && draft.text.is_some() {
        (
            "!",
            Role::StatusError,
            format!("{} · {facts}", locale.settings_detail(draft)),
        )
    } else if draft.conflict || draft.reconciliation.is_some() {
        ("!", Role::StatusWarning, facts)
    } else if draft.dirty {
        ("~", Role::TextPrimary, facts)
    } else {
        (" ", Role::TextSecondary, facts)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(glyph, theme.style(glyph_role)),
            Span::raw(" "),
            Span::styled(text, theme.style(Role::TextSecondary)),
        ])),
        area,
    );
}
