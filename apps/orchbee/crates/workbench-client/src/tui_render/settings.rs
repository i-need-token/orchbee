//! Scrollable common settings; only controls from the last successful frame authorize input.
use crate::{
    client_view::{Binding, KeyLabelGlyphs, LabelStyle, format_binding_with_glyphs},
    localization::Locale,
    native_settings::{
        Adjustment, Choice, Control, DraftView, Entry, Page, Row, SettingControl, SettingsDraw,
        SettingsHintLabel, View, footer_bindings, setting_for_row,
    },
    terminal_text::{
        display_width, project_terminal_text, sanitize_terminal_text, truncate_display,
    },
    tui_hit_map::{
        HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
        ProjectedPointer,
    },
    tui_theme::{GlyphMode, Role, Theme},
    workbench_client::ModalPresentation,
};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    text::{Line, Span, Text},
    widgets::{Clear, Padding, Paragraph},
};

use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    IconActionGlyph, IconActionSpec, InteractionState, KeyHint, NavigationState, OperationState,
    PointerPresentation, QueryField, RowContentBand, RowFocusCue, RowTracks, measure_overlay_hints,
    render_divider, render_query_field,
};

#[cfg(test)]
mod tests;

/// One blank row separates the heading edge from the search slot; one column pads each side.
const FRAME_PADDING: Padding = Padding::new(1, 1, 1, 0);
/// Short frames give up the top blank row before any setting, detail, or footer row.
const COMPACT_FRAME_PADDING: Padding = Padding::horizontal(1);
/// The shortest frame that keeps its top blank row together with the detail row.
const PADDED_FRAME_HEIGHT: u16 = 15;

pub(super) fn render(
    frame: &mut Frame<'_>,
    content: Rect,
    view: View<'_>,
    pending: bool,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> ModalPresentation<SettingsDraw> {
    let theme = &theme.paired_pointer_surfaces();
    let Some(layout) = SettingsLayout::resolve(frame.area(), content, view, locale, theme) else {
        return ModalPresentation::NotPresented;
    };
    hits.start_settings();
    let title = match view {
        View::Draft(draft)
            if draft.text.is_some() || draft.picker.is_some() || draft.page == Page::Profiles =>
        {
            format!(
                "{} > {}",
                locale.settings_title(),
                if draft.page == Page::Profiles && draft.text.is_none() {
                    locale.settings_category_name(draft.selected)
                } else {
                    locale.settings_row(draft.selected)
                }
            )
        }
        _ => locale.settings_title().into(),
    };
    let block = super::titled_overlay(theme, layout.padding, &title, layout.area.width);
    frame.render_widget(Clear, layout.area);
    frame.render_widget(block, layout.area);
    let painted_close = render_close(frame, layout.close, view, theme, hits);
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
                Paragraph::new(super::modal::status_line(
                    locale.settings_saving_fact(),
                    Role::StatusInfo,
                    theme,
                )),
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
                Paragraph::new(super::modal::status_line(
                    locale.settings_pending(matches!(view, View::Saving { .. })),
                    Role::StatusPending,
                    theme,
                )),
                Rect::new(layout.inner.x, y, layout.inner.width, 1),
            );
        }
        SettingsDraw::surface(view.token())
    };
    if !is_confirmation(view) {
        let painted_hints = render_footer_hints(frame, layout.hints, view, locale, theme);
        debug_assert!(painted_hints.is_some());
    }
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
    padding: Padding,
    inner: Rect,
    close: Rect,
    hints: Rect,
    actions: Rect,
    footer_actions: FooterActionGeometry,
    draft: Option<DraftLayout>,
}

#[derive(Clone, Copy)]
struct DraftLayout {
    regions: DraftRegions,
    draw: SettingsDraw,
}

impl SettingsLayout {
    fn resolve(
        bounds: Rect,
        content: Rect,
        view: View<'_>,
        locale: Locale,
        theme: &Theme,
    ) -> Option<Self> {
        let theme = &theme.paired_pointer_surfaces();
        if content.width < 36 || content.height < 10 || bounds.intersection(content) != content {
            return None;
        }
        let width = content.width.min(92);
        let height = content.height.min(30);
        let area = Rect::new(
            content.x + (content.width - width) / 2,
            content.y + (content.height - height) / 2,
            width,
            height,
        );
        if bounds.intersection(area) != area {
            return None;
        }
        let padding = if area.height >= PADDED_FRAME_HEIGHT {
            FRAME_PADDING
        } else {
            COMPACT_FRAME_PADDING
        };
        let inner = super::overlay_surface(theme, padding).inner(area);
        if inner.width == 0 || inner.height < 6 || bounds.intersection(inner) != inner {
            return None;
        }
        let close = super::controls::frame_close_rect(bounds, area)?;
        let mut action_height =
            if matches!(view, View::Draft(draft) if !is_child(draft)) && inner.width < 70 {
                2
            } else {
                1
            };
        if matches!(view, View::Draft(draft) if is_child(draft))
            && FooterActionGeometry::resolve(
                bounds,
                Rect::new(inner.x, inner.bottom().checked_sub(1)?, inner.width, 1),
                view,
                locale,
                theme,
            )
            .is_none()
        {
            action_height = 2;
        }
        let hints = Rect::new(
            inner.x,
            inner.bottom().checked_sub(action_height + 1)?,
            inner.width,
            1,
        );
        if !is_confirmation(view) {
            footer_hints_admitted(bounds, hints, view, locale, theme)?;
        }
        let actions = Rect::new(
            inner.x,
            inner.bottom().checked_sub(action_height)?,
            inner.width,
            action_height,
        );
        let footer_actions = FooterActionGeometry::resolve(bounds, actions, view, locale, theme)?;
        let draft = match view {
            View::Draft(draft) => {
                let regions = DraftRegions::resolve(inner, action_height)?;
                let visible = usize::from(regions.list.height);
                let draw = if let Some(picker) = draft.picker {
                    let visible = usize::from(regions.facts.y.saturating_sub(inner.y + 3))
                        .min(Choice::options(picker.row()).len());
                    SettingsDraw::picker(draft.token, picker.first_visible(visible), visible)
                } else if draft.text.is_some() || draft.confirmation.is_some() {
                    SettingsDraw::surface(draft.token)
                } else {
                    draft.presentation(visible)
                };
                if !draft_geometry_admitted(bounds, inner, regions, draft, draw, locale, theme) {
                    return None;
                }
                Some(DraftLayout { regions, draw })
            }
            View::Loading { .. } | View::Saving { .. } => None,
        };
        Some(Self {
            area,
            padding,
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

fn format_settings_bindings(bindings: impl Iterator<Item = Binding>, theme: &Theme) -> String {
    let key_glyphs = match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    };
    bindings
        .map(|binding| format_binding_with_glyphs(&binding, LabelStyle::for_host(), key_glyphs))
        .collect::<Vec<_>>()
        .join("/")
}

fn is_confirmation(view: View<'_>) -> bool {
    matches!(view, View::Draft(draft) if draft.confirmation.is_some())
}

fn is_child(draft: DraftView<'_>) -> bool {
    draft.text.is_some()
        || draft.confirmation.is_some()
        || draft.searching
        || draft.picker.is_some()
}

fn child_shortcuts(view: View<'_>, theme: &Theme) -> Option<[String; 2]> {
    matches!(view, View::Draft(draft) if is_child(draft)).then(|| {
        [
            SettingsHintLabel::Cancel,
            if is_confirmation(view) {
                SettingsHintLabel::Confirm
            } else {
                SettingsHintLabel::Accept
            },
        ]
        .map(|label| {
            let hint = footer_bindings(view)
                .iter()
                .find(|hint| hint.label() == label)
                .expect("child owner exposes cancel and accept bindings");
            format_settings_bindings(hint.bindings(), theme)
        })
    })
}

fn exit_shortcut(view: View<'_>, theme: &Theme) -> String {
    let hint = footer_bindings(view)
        .first()
        .expect("settings owner exposes exit first");
    format_settings_bindings(hint.bindings(), theme)
}

fn resolved_footer_hints(view: View<'_>, locale: Locale, theme: &Theme) -> Vec<ResolvedFooterHint> {
    footer_bindings(view)
        .iter()
        .filter(|hint| {
            !matches!(
                hint.label(),
                SettingsHintLabel::Close
                    | SettingsHintLabel::Back
                    | SettingsHintLabel::Cancel
                    | SettingsHintLabel::Confirm
                    | SettingsHintLabel::Accept
                    | SettingsHintLabel::Save
            )
        })
        .map(|hint| ResolvedFooterHint {
            key: format_settings_bindings(hint.bindings(), theme),
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
    if hints.is_empty() {
        return Some(());
    }
    let _ = measure_overlay_hints(&hints, theme)?;
    let first = measure_overlay_hints(hints.get(..1)?, theme)?;
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
    if hints.is_empty() {
        return Some(Rect::new(area.x, area.y, 0, 1));
    }
    super::controls::render_overlay_hints(frame, area, &hints, theme)
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
    view: View<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    let target = InteractionTarget::SettingsControl {
        token: view.token(),
        control: Control::Close,
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
    )?;
    if painted != rect {
        return None;
    }
    if view.allows_backdrop_close() {
        hits.record_overlay_dismissal(&target);
    }
    hits.record_interaction(HitLayer::Overlay, painted, painted, target, spec);
    Some(painted)
}

struct ActionPaint<'a> {
    area: Rect,
    rect: Rect,
    label: &'a str,
    shortcut: Option<&'a str>,
    kind: ActionKind,
    enabled: bool,
    target: InteractionTarget,
    focused: bool,
}

#[derive(Clone, Copy)]
enum FooterActionGeometry {
    Root {
        reset: Rect,
        reload: Rect,
        close: Rect,
        save: Rect,
    },
    Profiles {
        reset: Rect,
        back: Rect,
    },
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
        let child = matches!(view, View::Draft(draft) if is_child(draft));
        if let View::Draft(draft) = view
            && !child
        {
            let rect_for = |x, y, label, kind, shortcut: Option<&str>| {
                let area = Rect::new(x, y, row.right().checked_sub(x)?, 1);
                let mut spec = action_spec(label, kind, true, PointerPresentation::Rest);
                if let Some(shortcut) = shortcut {
                    spec = spec.with_shortcut(shortcut);
                }
                super::controls::action_rect(bounds, area, spec, theme)
            };
            let exit = exit_shortcut(view, theme);
            if draft.page == Page::Profiles {
                let reset = rect_for(
                    row.x,
                    row.y,
                    locale.settings_row(Row::ResetProfiles),
                    ActionKind::Secondary,
                    None,
                )?;
                let (x, y) = if row.height > 1 {
                    (row.x, row.y + 1)
                } else {
                    (reset.right() + 2, row.y)
                };
                let back = rect_for(
                    x,
                    y,
                    locale.settings_back_button(),
                    ActionKind::Secondary,
                    Some(&exit),
                )?;
                return Some(Self::Profiles { reset, back });
            }
            let reset = rect_for(
                row.x,
                row.y,
                locale.settings_row_action(Row::ResetAll)?,
                ActionKind::Secondary,
                None,
            )?;
            let reload = rect_for(
                reset.right() + 2,
                row.y,
                locale.settings_row_action(Row::Reload)?,
                ActionKind::Secondary,
                None,
            )?;
            let (x, y) = if row.height > 1 {
                (row.x, row.y + 1)
            } else {
                (reload.right() + 2, row.y)
            };
            let close_label = if draft.query.is_empty() {
                locale.settings_close_button()
            } else {
                locale.settings_back_button()
            };
            let close = rect_for(x, y, close_label, ActionKind::Secondary, Some(&exit))?;
            let shortcut = save_shortcut(view, row.width, theme);
            let save = rect_for(
                close.right() + 2,
                y,
                locale.settings_row_action(Row::Save)?,
                ActionKind::Primary,
                shortcut.as_deref(),
            )?;
            return Some(Self::Root {
                reset,
                reload,
                close,
                save,
            });
        }
        if child {
            let accept_enabled =
                !matches!(view, View::Draft(draft) if draft.text.is_some() && draft.invalid);
            let mut first_spec = action_spec(
                locale.settings_cancel(),
                ActionKind::Secondary,
                true,
                PointerPresentation::Rest,
            );
            let mut second_spec = action_spec(
                locale.settings_accept_button(),
                ActionKind::Primary,
                accept_enabled,
                PointerPresentation::Rest,
            );
            let shortcuts = child_shortcuts(view, theme);
            if let Some(shortcuts) = shortcuts.as_ref() {
                first_spec = first_spec.with_shortcut(&shortcuts[0]);
                second_spec = second_spec.with_shortcut(&shortcuts[1]);
            }
            let first = super::controls::action_rect(bounds, row, first_spec, theme)?;
            let (second_x, second_y) = if row.height > 1 {
                (row.x, row.y.checked_add(1)?)
            } else {
                (first.right().checked_add(2)?, row.y)
            };
            let second_area = Rect::new(second_x, second_y, row.right().checked_sub(second_x)?, 1);
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
            let shortcut = exit_shortcut(view, theme);
            let spec = action_spec(
                label,
                ActionKind::Secondary,
                true,
                PointerPresentation::Rest,
            )
            .with_shortcut(&shortcut);
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
    let mut state = rest_state(action.enabled).with_pointer(pointer);
    if action.focused {
        state = InteractionState::new(
            FocusState::Focused,
            pointer,
            NavigationState::None,
            CurrentState::NotCurrent,
            CheckedState::Unchecked,
            if action.enabled {
                Availability::Enabled
            } else {
                Availability::Disabled
            },
            OperationState::Idle,
        );
    }
    let mut action_spec =
        ActionSpec::new(action.label, action.kind, ControlSurface::Overlay, state);
    if let Some(shortcut) = action.shortcut {
        action_spec = action_spec.with_shortcut(shortcut);
    }
    let painted = super::controls::render_action(frame, action.area, action_spec, theme)?;
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
    if let View::Draft(draft) = view {
        let exit = exit_shortcut(view, theme);
        let save_key = save_shortcut(view, row.width, theme);
        let buttons = match geometry {
            FooterActionGeometry::Root {
                reset,
                reload,
                close,
                save,
            } => vec![
                (
                    reset,
                    Some(Row::ResetAll),
                    locale.settings_row_action(Row::ResetAll).unwrap(),
                    None,
                    Control::Close,
                ),
                (
                    reload,
                    Some(Row::Reload),
                    locale.settings_row_action(Row::Reload).unwrap(),
                    None,
                    Control::Close,
                ),
                (
                    close,
                    None,
                    if draft.query.is_empty() {
                        locale.settings_close_button()
                    } else {
                        locale.settings_back_button()
                    },
                    Some(exit.as_str()),
                    if draft.query.is_empty() {
                        Control::Close
                    } else {
                        Control::Back
                    },
                ),
                (
                    save,
                    Some(Row::Save),
                    locale.settings_row_action(Row::Save).unwrap(),
                    save_key.as_deref(),
                    Control::Close,
                ),
            ],
            FooterActionGeometry::Profiles { reset, back } => vec![
                (
                    reset,
                    Some(Row::ResetProfiles),
                    locale.settings_row(Row::ResetProfiles),
                    None,
                    Control::Back,
                ),
                (
                    back,
                    Some(Row::Back),
                    locale.settings_back_button(),
                    Some(exit.as_str()),
                    Control::Back,
                ),
            ],
            _ => Vec::new(),
        };
        if !buttons.is_empty() {
            for (rect, setting, label, shortcut, control) in buttons {
                let target = setting.map_or(
                    InteractionTarget::SettingsControl {
                        token: draft.token,
                        control,
                    },
                    |row| InteractionTarget::SettingsRow {
                        token: draft.token,
                        row,
                    },
                );
                let painted = render_interactive_action(
                    frame,
                    ActionPaint {
                        area: rect,
                        rect,
                        label,
                        shortcut,
                        kind: if setting == Some(Row::Save) {
                            ActionKind::Primary
                        } else {
                            ActionKind::Secondary
                        },
                        enabled: setting.is_none_or(|row| draft.enabled(row)),
                        target,
                        focused: setting == Some(draft.selected),
                    },
                    theme,
                    hits,
                );
                debug_assert_eq!(painted, Some(rect));
            }
            return;
        }
    }
    let child = matches!(view, View::Draft(draft) if is_child(draft));
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
        let shortcuts = child_shortcuts(view, theme);
        let accept_enabled =
            !matches!(view, View::Draft(draft) if draft.text.is_some() && draft.invalid);
        let token = view.token();
        let Some(first) = render_interactive_action(
            frame,
            ActionPaint {
                area: row,
                rect: first,
                label: labels[0],
                shortcut: shortcuts.as_ref().map(|keys| keys[0].as_str()),
                kind: ActionKind::Secondary,
                enabled: true,
                target: InteractionTarget::SettingsControl {
                    token,
                    control: Control::Back,
                },
                focused: false,
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
                shortcut: shortcuts.as_ref().map(|keys| keys[1].as_str()),
                kind: ActionKind::Primary,
                enabled: accept_enabled,
                target: InteractionTarget::SettingsControl {
                    token,
                    control: Control::Accept,
                },
                focused: false,
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
    let shortcut = exit_shortcut(view, theme);
    let painted = render_interactive_action(
        frame,
        ActionPaint {
            area: row,
            rect,
            label,
            shortcut: Some(&shortcut),
            kind: ActionKind::Secondary,
            enabled: true,
            target: InteractionTarget::SettingsControl {
                token: view.token(),
                control,
            },
            focused: false,
        },
        theme,
        hits,
    );
    debug_assert_eq!(painted, Some(rect));
}

fn save_shortcut(view: View<'_>, width: u16, theme: &Theme) -> Option<String> {
    (width >= 40)
        .then(|| {
            footer_bindings(view)
                .iter()
                .find(|hint| hint.label() == SettingsHintLabel::Save)
                .map(|hint| format_settings_bindings(hint.bindings(), theme))
        })
        .flatten()
}

fn render_draft(
    frame: &mut Frame<'_>,
    inner: Rect,
    draft: DraftView<'_>,
    layout: DraftLayout,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> SettingsDraw {
    let DraftLayout { regions, draw } = layout;
    if draft.text.is_some() || draft.confirmation.is_some() || draft.picker.is_some() {
        render_child(frame, inner, regions.facts.y, draft, locale, theme, hits);
        render_state_facts(frame, regions.facts, draft, locale, theme);
        return draw;
    }
    let entries = draft.entries();
    let start = draw.first_visible().unwrap_or(0);
    let visible = draw.visible_rows().unwrap_or(0);
    let context = RowContext {
        draft,
        list_active: true,
        locale,
        theme,
    };
    if !draft.searching {
        hits.record(
            HitLayer::OverlaySurface,
            regions.list,
            HitTarget::ListScroll(crate::tui_hit_map::ListScrollTarget::Settings(draft.token)),
        );
    }
    if draft.page == Page::General {
        render_search(
            frame,
            Rect::new(inner.x, inner.y, inner.width, 1),
            draft,
            locale,
            theme,
            hits,
        );
    } else {
        frame.render_widget(
            Paragraph::new(super::modal::title_line(
                locale.settings_category_name(draft.selected),
                theme,
            )),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
    }
    render_divider(
        frame,
        Rect::new(inner.x, inner.y + 1, inner.width, 1),
        theme,
    );
    for (offset, entry) in entries
        .iter()
        .skip(start)
        .take(visible)
        .copied()
        .enumerate()
    {
        let row_area = Rect::new(
            regions.list.x,
            regions.list.y + u16::try_from(offset).expect("bounded visible rows"),
            regions.list.width.saturating_sub(1),
            1,
        );
        match entry {
            Entry::Setting(row) => render_setting_row(frame, row_area, context, row, hits),
            Entry::Heading(category) => render_section(
                frame,
                row_area,
                locale.settings_category_name(category),
                theme,
            ),
        }
    }
    if entries.is_empty() {
        frame.render_widget(
            Paragraph::new(crate::terminal_text::wrap_text(
                &Text::from(locale.settings_no_results()),
                regions.list.width,
            ))
            .style(theme.style(Role::TextSecondary)),
            regions.list,
        );
    }
    if entries.len() > usize::from(regions.list.height) {
        let height = usize::from(regions.list.height);
        let thumb = start.saturating_mul(height.saturating_sub(1))
            / entries.len().saturating_sub(height).max(1);
        frame.render_widget(
            Paragraph::new(theme.glyphs.scrollbar_thumb).style(theme.style(Role::TextSecondary)),
            Rect::new(
                regions.list.right() - 1,
                regions.list.y + u16::try_from(thumb).expect("bounded thumb"),
                1,
                1,
            ),
        );
    }
    render_divider(frame, regions.separator, theme);
    if let Some(detail) = regions.detail {
        let text = draft_description(draft, locale, theme);
        frame.render_widget(
            Paragraph::new(truncate_display(
                &text,
                usize::from(detail.width),
                theme.glyphs.ellipsis,
            ))
            .style(theme.style(Role::TextSecondary)),
            detail,
        );
    }
    render_state_facts(frame, regions.facts, draft, locale, theme);
    draw
}

#[derive(Clone, Copy)]
struct DraftRegions {
    list: Rect,
    separator: Rect,
    detail: Option<Rect>,
    facts: Rect,
}

impl DraftRegions {
    fn resolve(inner: Rect, actions: u16) -> Option<Self> {
        let facts_y = inner.bottom().checked_sub(actions + 2)?;
        let with_detail = inner.height >= 12;
        let separator_y = facts_y.checked_sub(if with_detail { 2 } else { 1 })?;
        let list_y = inner.y.checked_add(2)?;
        let height = separator_y.checked_sub(list_y)?;
        if height == 0 {
            return None;
        }
        Some(Self {
            list: Rect::new(inner.x, list_y, inner.width, height),
            separator: Rect::new(inner.x, separator_y, inner.width, 1),
            detail: with_detail.then(|| Rect::new(inner.x, separator_y + 1, inner.width, 1)),
            facts: Rect::new(inner.x, facts_y, inner.width, 1),
        })
    }
}

fn render_section(frame: &mut Frame<'_>, area: Rect, label: &str, theme: &Theme) {
    render_divider(frame, area, theme);
    let label = format!(
        " {} ",
        truncate_display(
            label,
            usize::from(area.width).saturating_sub(2),
            theme.glyphs.ellipsis
        )
    );
    let label_width = u16::try_from(display_width(&label))
        .unwrap_or(area.width)
        .min(area.width);
    frame.render_widget(
        Paragraph::new(label).style(theme.style(Role::TextSecondary)),
        Rect::new(area.x, area.y, label_width, 1),
    );
}

fn render_search(
    frame: &mut Frame<'_>,
    area: Rect,
    draft: DraftView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let field = if !draft.query.is_empty() {
        let count = locale.settings_search_count(draft.content_rows().len());
        let count_width = u16::try_from(display_width(&count))
            .unwrap_or(area.width)
            .min(area.width);
        let count_area = Rect::new(area.right() - count_width, area.y, count_width, 1);
        frame.render_widget(
            Paragraph::new(count).style(theme.style(Role::TextSecondary)),
            count_area,
        );
        Rect::new(
            area.x,
            area.y,
            area.width.saturating_sub(count_width + 2),
            1,
        )
    } else {
        area
    };
    if draft.searching {
        let _ = render_query_field(
            frame,
            field,
            QueryField::Editing {
                label: locale.settings_search_label(),
                query: draft.query,
                cursor: draft.search_cursor,
            },
            theme,
        );
        return;
    }
    let key = footer_bindings(View::Draft(draft))
        .iter()
        .find(|hint| hint.label() == SettingsHintLabel::Search)
        .map(|hint| format_settings_bindings(hint.bindings(), theme));
    let query = match (draft.query.is_empty(), key.as_deref()) {
        (true, Some(key)) => QueryField::Rest {
            key,
            prompt: locale.settings_search_prompt(),
        },
        _ => QueryField::Retained {
            label: locale.settings_search_label(),
            query: draft.query,
        },
    };
    let _ = render_query_field(frame, field, query, theme);
    let target = InteractionTarget::SettingsControl {
        token: draft.token,
        control: Control::Search,
    };
    let spec = interaction_spec(true);
    let pointer =
        pointer_presentation(hits.projected_pointer(HitLayer::Overlay, area, area, &target, spec));
    super::controls::paint_pointer_target(
        frame,
        area,
        ControlSurface::Overlay,
        pointer,
        Role::TextSecondary,
        Availability::Enabled,
        theme,
    );
    hits.record_interaction(HitLayer::Overlay, area, area, target, spec);
}

fn draft_description(draft: DraftView<'_>, locale: Locale, theme: &Theme) -> String {
    if draft.confirmation.is_some()
        || draft.invalid
        || !draft.enabled(draft.selected)
        || draft.page == Page::Profiles
        || draft.reconciliation == Some(crate::native_settings::Reconciliation::OutcomeUnknown)
    {
        return locale.settings_detail(draft);
    }
    if theme.no_color()
        && matches!(
            draft.selected,
            Row::Theme | Row::TerminalTheme | Row::Foreground | Row::Background
        )
    {
        return locale.settings_no_color().into();
    }
    locale.settings_description(draft.selected).into()
}

fn render_child(
    frame: &mut Frame<'_>,
    inner: Rect,
    bottom: u16,
    draft: DraftView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    frame.render_widget(
        Paragraph::new(super::modal::title_line(
            locale.settings_row(draft.selected),
            theme,
        )),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    let description = draft_description(draft, locale, theme);
    let description_area = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        2.min(bottom.saturating_sub(inner.y + 1)),
    );
    frame.render_widget(
        Paragraph::new(crate::terminal_text::wrap_text(
            &Text::from(description),
            description_area.width,
        ))
        .style(theme.style(Role::TextSecondary)),
        description_area,
    );
    if let Some((text, cursor)) = draft.text {
        render_editor(
            frame,
            Rect::new(inner.x, inner.y + 3, inner.width, 1),
            draft.selected,
            text,
            cursor,
            locale,
            theme,
        );
    }
    if let Some(picker) = draft.picker {
        let choices = Choice::options(picker.row());
        let height = usize::from(bottom.saturating_sub(inner.y + 3));
        let first = picker.first_visible(height);
        hits.record(
            HitLayer::OverlaySurface,
            Rect::new(
                inner.x,
                inner.y + 3,
                inner.width,
                u16::try_from(height).expect("bounded choice viewport"),
            ),
            HitTarget::ListScroll(crate::tui_hit_map::ListScrollTarget::Settings(draft.token)),
        );
        for (index, choice) in choices.iter().copied().enumerate().skip(first).take(height) {
            let area = Rect::new(
                inner.x,
                inner.y + 3 + u16::try_from(index - first).expect("bounded choices"),
                inner.width,
                1,
            );
            render_choice(
                frame,
                area,
                RowContext {
                    draft,
                    list_active: true,
                    locale,
                    theme,
                },
                choice,
                index == picker.cursor(),
                choice == picker.original(),
                hits,
            );
        }
    }
}

fn render_choice(
    frame: &mut Frame<'_>,
    area: Rect,
    context: RowContext<'_>,
    choice: Choice,
    selected: bool,
    checked: bool,
    hits: &mut HitMapBuilder,
) {
    let RowContext {
        draft,
        locale,
        theme,
        ..
    } = context;
    let navigation = Rect::new(area.x, area.y, 1, 1);
    let marker = Rect::new(area.x + 2, area.y, 3, 1);
    let checked_marker = Rect::new(area.x + 3, area.y, 1, 1);
    let primary = Rect::new(area.x + 6, area.y, area.width.saturating_sub(6), 1);
    frame.render_widget(
        Paragraph::new(if checked { "(*)" } else { "( )" })
            .style(theme.style(Role::AccentSelection)),
        marker,
    );
    frame.render_widget(
        Paragraph::new(truncate_display(
            locale.settings_choice(choice),
            usize::from(primary.width),
            theme.glyphs.ellipsis,
        ))
        .style(theme.style(Role::TextPrimary)),
        primary,
    );
    let target = InteractionTarget::SettingsControl {
        token: draft.token,
        control: Control::Choose(choice),
    };
    let spec = interaction_spec(true);
    let pointer =
        pointer_presentation(hits.projected_pointer(HitLayer::Overlay, area, area, &target, spec));
    let state = InteractionState::new(
        if selected {
            FocusState::Focused
        } else {
            FocusState::Unfocused
        },
        pointer,
        if selected {
            NavigationState::Provisional
        } else {
            NavigationState::None
        },
        CurrentState::NotCurrent,
        if checked {
            CheckedState::Checked
        } else {
            CheckedState::Unchecked
        },
        Availability::Enabled,
        OperationState::Idle,
    );
    let tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary)
        .with_provisional(navigation)
        .with_checked(checked_marker, "*");
    let painted = super::controls::paint_row_interaction(
        frame,
        area,
        tracks,
        &[
            RowContentBand::semantic(Rect::new(marker.x, marker.y, 1, 1), Role::TextSecondary),
            RowContentBand::semantic(
                Rect::new(marker.right() - 1, marker.y, 1, 1),
                Role::TextSecondary,
            ),
        ],
        ControlSurface::Overlay,
        state,
        theme,
    );
    if painted == Some(area) {
        hits.record_interaction(HitLayer::Overlay, area, area, target, spec);
    }
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
        let label_x = row.x + 3;
        let status = Rect::new(row.right() - 1, row.y, 1, 1);
        let available = status.x.saturating_sub(label_x);
        let trailing_width = (available / 2)
            .clamp(9, 30)
            .min(available.saturating_sub(2));
        let primary_width = available.saturating_sub(trailing_width).saturating_sub(1);
        if primary_width == 0 || trailing_width < 5 {
            return None;
        }
        let primary = Rect::new(label_x, row.y, primary_width, 1);
        let trailing = Rect::new(primary.right() + 1, row.y, trailing_width, 1);
        let checked_open = Rect::new(trailing.x, row.y, 1, 1);
        let checked = Rect::new(trailing.x + 1, row.y, 1, 1);
        let checked_close = Rect::new(trailing.x + 2, row.y, 1, 1);
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

fn draft_geometry_admitted(
    bounds: Rect,
    inner: Rect,
    regions: DraftRegions,
    draft: DraftView<'_>,
    draw: SettingsDraw,
    locale: Locale,
    theme: &Theme,
) -> bool {
    if draft.text.is_some() || draft.confirmation.is_some() || draft.picker.is_some() {
        if draft.confirmation.is_some() {
            let rows = crate::terminal_text::wrap_text(
                &Text::from(locale.settings_detail(draft)),
                inner.width,
            )
            .lines
            .len();
            return rows <= 2;
        }
        let child_height = regions.facts.y.saturating_sub(inner.y + 3);
        return child_height > 0
            && (draft.text.is_none()
                || editor_geometry_admitted(
                    Rect::new(inner.x, inner.y + 3, inner.width, 1),
                    draft.selected,
                    locale,
                    theme,
                ));
    }
    let Some(start) = draw.first_visible() else {
        return false;
    };
    let Some(visible) = draw.visible_rows() else {
        return false;
    };
    let entries = draft.entries();
    let Some(end) = start.checked_add(visible) else {
        return false;
    };
    if end > entries.len() {
        return false;
    }
    let Some(visible_height) = u16::try_from(visible).ok() else {
        return false;
    };
    let list = Rect::new(
        regions.list.x,
        regions.list.y,
        regions.list.width,
        visible_height,
    );
    if bounds.intersection(list) != list || inner.intersection(list) != list {
        return false;
    }
    for (offset, entry) in entries[start..end].iter().copied().enumerate() {
        let Some(offset) = u16::try_from(offset).ok() else {
            return false;
        };
        let row_area = Rect::new(list.x, list.y + offset, list.width.saturating_sub(1), 1);
        let Entry::Setting(_) = entry else { continue };
        let Some(_) = RowGeometry::new(row_area) else {
            return false;
        };
        if bounds.intersection(row_area) != row_area || inner.intersection(row_area) != row_area {
            return false;
        }
    }
    bounds.intersection(regions.facts) == regions.facts
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
    let selected = list_active && !draft.searching && row == draft.selected;
    let checked = draft.checked(row);
    let base_state = InteractionState::new(
        if selected {
            FocusState::Focused
        } else {
            FocusState::Unfocused
        },
        PointerPresentation::Rest,
        NavigationState::None,
        if selected {
            CurrentState::Current
        } else {
            CurrentState::NotCurrent
        },
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
    let label = truncate_display(
        &sanitize_terminal_text(locale.settings_row(row)),
        usize::from(geometry.primary.width),
        theme.glyphs.ellipsis,
    );
    frame.render_widget(
        Paragraph::new(label).style(theme.style(Role::TextPrimary)),
        geometry.primary,
    );
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
    let value = render_row_value(frame, geometry.trailing, context, row, adjustment, enabled);
    bands.extend(value.bands);
    let mut tracks = RowTracks::new(RowFocusCue::EnclosingFrame, geometry.primary)
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
}

fn settings_value(locale: Locale, draft: DraftView<'_>, row: Row, theme: &Theme) -> String {
    if matches!(row, Row::Foreground | Row::Background)
        && draft.values.terminal_display_theme
            != orchbee_workbench_protocol::TerminalDisplayAppearance::Custom
    {
        return theme
            .terminal_rgb_defaults()
            .map(|(foreground, background)| {
                let rgb = if row == Row::Foreground {
                    foreground
                } else {
                    background
                };
                format!("#{rgb:06x}")
            })
            .unwrap_or_else(|| {
                locale
                    .settings_choice(Choice::TerminalTheme(draft.values.terminal_display_theme))
                    .to_owned()
            });
    }
    let value = locale.settings_value((draft, row));
    if row == Row::Scrollback {
        return value;
    }
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
    let raw_value = settings_value(locale, draft, row, theme);
    let value = sanitize_terminal_text(&raw_value);
    if let Some(adjustment) = adjustment
        && usize::from(area.width) >= display_width(&value).saturating_add(8)
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
        let value_area = Rect::new(area.x + 4, area.y, area.width.saturating_sub(4), 1);
        frame.render_widget(
            Paragraph::new(value).style(theme.style(Role::TextPrimary)),
            value_area,
        );
        return RowValuePaint::static_band(value_area, Role::TextPrimary);
    }
    let width = usize::from(area.width);
    let text = if row_uses_choice_trigger(row) && width >= 6 {
        let value = truncate_display(&value, width - 2, theme.glyphs.ellipsis);
        format!("{value} >")
    } else if row_uses_editor(row) && width >= 4 {
        format!(
            "[ {} ]",
            pad_display(&value, width - 4, theme.glyphs.ellipsis)
        )
    } else {
        pad_display(&value, width, theme.glyphs.ellipsis)
    };
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Right)
            .style(theme.style(Role::TextPrimary)),
        area,
    );
    RowValuePaint::static_band(area, Role::TextPrimary)
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
    render_text_field(frame, area, locale.settings_row(row), text, cursor, theme);
}

fn render_text_field(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    text: &str,
    cursor: usize,
    theme: &Theme,
) {
    let label_width = usize::from(area.width / 3).max(1);
    let label = truncate_display(
        &sanitize_terminal_text(label),
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
    let mut facts = locale.settings_state_facts(draft);
    if display_width(&facts).saturating_add(2) > usize::from(area.width) {
        facts = locale.settings_compact_state(draft).into();
    }
    let (glyph, glyph_role, text) = if draft.invalid {
        (
            "!",
            Role::StatusError,
            if draft.text.is_some() || draft.searching {
                let message = format!("{} · {facts}", locale.settings_invalid_short());
                if display_width(&message).saturating_add(2) <= usize::from(area.width) {
                    message
                } else {
                    locale.settings_invalid_short().into()
                }
            } else {
                facts
            },
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
