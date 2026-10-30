//! Copy cursor and search/status projection; no content, selection or effect authority.
use crate::{
    client_view::{
        ClientView, KeyLabelGlyphs, LabelStyle, copy_footer_bindings, format_binding_with_glyphs,
        join_key_labels,
    },
    localization::Locale,
    terminal_copy::CopySearchDirection,
    terminal_copy_mode::{CopyControl, CopyModeStatus, CopyModeView},
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget, InteractionAvailability, ProjectedPointer},
    tui_theme::{GlyphMode, Role, Theme},
};
use ratatui::{Frame, layout::Rect, style::Modifier, widgets::Clear};
use unicode_segmentation::UnicodeSegmentation;

use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    IconActionGlyph, IconActionSpec, InteractionState, KeyHint, NavigationState, OperationState,
    PointerPresentation, RowContentBand, RowFocusCue, RowTracks,
};

mod content;
pub(super) use content::paint_surface;

pub(super) fn render_controls(
    frame: &mut Frame<'_>,
    view: &ClientView,
    mode: CopyModeView<'_>,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let area = super::controls::mode_bar(frame.area(), view.sidebar_surface());
    let Some(token) = mode.pointer else {
        return;
    };
    if area.is_empty() {
        return;
    }
    hits.cover_chrome(area);
    frame.render_widget(Clear, area);
    frame
        .buffer_mut()
        .set_style(area, theme.style(Role::SurfaceChrome));

    let Some(close_area) = super::controls::close_rect(frame.area(), area) else {
        return;
    };
    let Some(close) = paint_icon_control(
        frame,
        close_area,
        IconActionGlyph::Close,
        HitTarget::CopyControl {
            token,
            control: CopyControl::Exit,
        },
        true,
        theme,
        hits,
    ) else {
        return;
    };
    let mut remaining = Rect::new(
        area.x,
        area.y,
        close.x.saturating_sub(area.x.saturating_add(1)),
        1,
    );
    let ready = mode.status == CopyModeStatus::Ready;

    if let Some(search) = mode.search {
        if let Some(rect) = paint_action_control(
            frame,
            remaining,
            locale.copy_find_label(),
            ActionKind::Primary,
            HitTarget::CopyControl {
                token,
                control: CopyControl::SubmitSearch,
            },
            ready,
            theme,
            hits,
        ) {
            remaining = after_control(rect, close.x);
        }
        if let Some(rect) = paint_action_control(
            frame,
            remaining,
            locale.copy_back_label(),
            ActionKind::Secondary,
            HitTarget::CopyControl {
                token,
                control: CopyControl::CancelSearch,
            },
            true,
            theme,
            hits,
        ) {
            remaining = after_control(rect, close.x);
        }
        paint_search_direction_and_reverse(
            frame,
            remaining,
            search.direction,
            locale,
            token,
            ready,
            theme,
            hits,
        );
    } else {
        if view
            .capabilities()
            .supports(crate::tui_capabilities::Capability::Clipboard)
            && let Some(rect) = paint_action_control(
                frame,
                remaining,
                locale.copy_action_label(),
                ActionKind::Primary,
                HitTarget::CopyControl {
                    token,
                    control: CopyControl::Copy,
                },
                ready,
                theme,
                hits,
            )
        {
            remaining = after_control(rect, close.x);
        }
        if let Some(rect) = paint_boolean_control(
            frame,
            remaining,
            locale.copy_select_label(),
            mode.selecting,
            HitTarget::CopyControl {
                token,
                control: CopyControl::ToggleSelection,
            },
            ready,
            theme,
            hits,
        ) {
            remaining = after_control(rect, close.x);
        }
        if let Some(rect) = paint_action_control(
            frame,
            remaining,
            locale.copy_search_label(),
            ActionKind::Secondary,
            HitTarget::CopyControl {
                token,
                control: CopyControl::Search,
            },
            ready,
            theme,
            hits,
        ) {
            remaining = after_control(rect, close.x);
        }
        for (glyph, reverse) in [
            (IconActionGlyph::Previous, true),
            (IconActionGlyph::Next, false),
        ] {
            if let Some(rect) = paint_icon_control(
                frame,
                remaining,
                glyph,
                HitTarget::CopyControl {
                    token,
                    control: CopyControl::RepeatSearch { reverse },
                },
                ready,
                theme,
                hits,
            ) {
                remaining = after_control(rect, close.x);
            }
        }
        let _ = paint_action_control(
            frame,
            remaining,
            locale.copy_help_label(),
            ActionKind::Secondary,
            HitTarget::CopyControl {
                token,
                control: CopyControl::Help,
            },
            true,
            theme,
            hits,
        );
    }
    hits.record(HitLayer::Chrome, area, HitTarget::OverlaySurface);
}

fn after_control(rect: Rect, close_x: u16) -> Rect {
    let x = rect.right().saturating_add(1);
    Rect::new(x, rect.y, close_x.saturating_sub(x.saturating_add(1)), 1)
}

#[allow(clippy::too_many_arguments)]
fn paint_action_control(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    kind: ActionKind,
    target: HitTarget,
    enabled: bool,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    let availability = availability(enabled);
    let rest = action_spec(label, kind, enabled, PointerPresentation::Rest);
    let rect = super::controls::action_rect(frame.area(), area, rest, theme)?;
    let pointer = pointer_presentation(hits.projected_modal_pointer(
        HitLayer::Chrome,
        rect,
        &target,
        availability,
    ));
    let painted = super::controls::render_action(
        frame,
        area,
        action_spec(label, kind, enabled, pointer),
        theme,
    )?;
    if painted != rect {
        return None;
    }
    hits.record_modal_control(HitLayer::Chrome, rect, target, availability);
    Some(rect)
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
        ControlSurface::Chrome,
        interaction_state(enabled, pointer, false),
    )
}

#[allow(clippy::too_many_arguments)]
fn paint_icon_control(
    frame: &mut Frame<'_>,
    area: Rect,
    glyph: IconActionGlyph,
    target: HitTarget,
    enabled: bool,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    let rect = super::controls::icon_action_rect(frame.area(), area)?;
    let interaction_availability = availability(enabled);
    let pointer = pointer_presentation(hits.projected_modal_pointer(
        HitLayer::Chrome,
        rect,
        &target,
        interaction_availability,
    ));
    let painted = super::controls::render_icon_action(
        frame,
        area,
        IconActionSpec::new(
            glyph,
            ControlSurface::Chrome,
            value_availability(enabled),
            pointer,
        ),
        theme,
    )?;
    if painted != rect {
        return None;
    }
    hits.record_modal_control(HitLayer::Chrome, rect, target, interaction_availability);
    Some(rect)
}

#[allow(clippy::too_many_arguments)]
fn paint_boolean_control(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    checked: bool,
    target: HitTarget,
    enabled: bool,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    if area.height == 0 {
        return None;
    }
    let label = sanitize_terminal_text(label);
    let label_width = display_width(&label);
    if label_width == 0 {
        return None;
    }
    let width = u16::try_from(label_width.checked_add(4)?).ok()?;
    if width > area.width {
        return None;
    }
    let rect = Rect::new(area.x, area.y, width, 1);
    if frame.area().intersection(rect) != rect {
        return None;
    }
    let interaction_availability = availability(enabled);
    let pointer = pointer_presentation(hits.projected_modal_pointer(
        HitLayer::Chrome,
        rect,
        &target,
        interaction_availability,
    ));
    frame.buffer_mut().set_string(
        rect.x,
        rect.y,
        format!("[ ] {label}"),
        theme.style(if enabled {
            Role::TextPrimary
        } else {
            Role::TextDisabled
        }),
    );
    let checked_track = Rect::new(rect.x + 1, rect.y, 1, 1);
    let primary = Rect::new(rect.x + 4, rect.y, width - 4, 1);
    let tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary)
        .with_primary_role(Role::TextPrimary)
        .with_checked(checked_track, "x");
    let bands = [
        RowContentBand::semantic(Rect::new(rect.x, rect.y, 1, 1), Role::TextSecondary),
        RowContentBand::semantic(Rect::new(rect.x + 2, rect.y, 2, 1), Role::TextSecondary),
    ];
    let painted = super::controls::paint_row_interaction(
        frame,
        rect,
        tracks,
        &bands,
        ControlSurface::Chrome,
        interaction_state(enabled, pointer, checked),
        theme,
    )?;
    if painted != rect {
        return None;
    }
    hits.record_modal_control(HitLayer::Chrome, rect, target, interaction_availability);
    Some(rect)
}

#[allow(clippy::too_many_arguments)]
fn paint_search_direction_and_reverse(
    frame: &mut Frame<'_>,
    area: Rect,
    direction: CopySearchDirection,
    locale: Locale,
    token: crate::terminal_copy_mode::CopyModeToken,
    enabled: bool,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let reverse_area = Rect::new(
        area.x.saturating_add(1),
        area.y,
        area.width.saturating_sub(1),
        1,
    );
    let label = locale.copy_reverse_label();
    let spec = action_spec(
        label,
        ActionKind::Secondary,
        enabled,
        PointerPresentation::Rest,
    );
    if super::controls::action_rect(frame.area(), reverse_area, spec, theme).is_none() {
        return;
    }
    let glyph = match direction {
        CopySearchDirection::Forward => ">",
        CopySearchDirection::Backward => "<",
    };
    frame
        .buffer_mut()
        .set_string(area.x, area.y, glyph, theme.style(Role::TextSecondary));
    let _ = paint_action_control(
        frame,
        reverse_area,
        label,
        ActionKind::Secondary,
        HitTarget::CopyControl {
            token,
            control: CopyControl::ReverseSearch,
        },
        enabled,
        theme,
        hits,
    );
}

const fn interaction_state(
    enabled: bool,
    pointer: PointerPresentation,
    checked: bool,
) -> InteractionState {
    InteractionState::new(
        FocusState::Unfocused,
        pointer,
        NavigationState::None,
        CurrentState::NotCurrent,
        if checked {
            CheckedState::Checked
        } else {
            CheckedState::Unchecked
        },
        value_availability(enabled),
        OperationState::Idle,
    )
}

const fn value_availability(enabled: bool) -> Availability {
    if enabled {
        Availability::Enabled
    } else {
        Availability::Disabled
    }
}

const fn availability(enabled: bool) -> InteractionAvailability {
    if enabled {
        InteractionAvailability::Enabled
    } else {
        InteractionAvailability::Disabled
    }
}

const fn pointer_presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
}

pub(super) fn paint_cursor(
    frame: &mut Frame<'_>,
    area: Rect,
    mode: CopyModeView<'_>,
    theme: &Theme,
) {
    let Some(cursor) = mode.cursor.filter(|_| mode.search.is_none()) else {
        return;
    };
    if cursor.column >= area.width || cursor.row >= area.height {
        return;
    }
    let position = (area.x + cursor.column, area.y + cursor.row);
    if let Some(cell) = frame.buffer_mut().cell_mut(position) {
        cell.set_style(
            theme
                .style(Role::TextInverse)
                .bg(theme.color(Role::AccentFocus))
                .add_modifier(Modifier::BOLD),
        );
    }
    frame.set_cursor_position(position);
}

struct ResolvedFooterHint {
    key: String,
    label: &'static str,
}

fn resolved_footer_hints(
    searching: bool,
    clipboard_supported: bool,
    locale: Locale,
    theme: &Theme,
) -> Vec<ResolvedFooterHint> {
    let key_glyphs = match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    };
    let style = LabelStyle::for_host();
    copy_footer_bindings(searching)
        .iter()
        .filter(|hint| {
            clipboard_supported || hint.label() != crate::client_view::CopyHintLabel::Copy
        })
        .map(|hint| {
            let labels = hint
                .bindings()
                .map(|binding| format_binding_with_glyphs(&binding, style, key_glyphs))
                .collect::<Vec<_>>();
            ResolvedFooterHint {
                key: join_key_labels(&labels, style, "/"),
                label: locale.copy_hint_label(hint.label()),
            }
        })
        .collect()
}

fn key_hints(resolved: &[ResolvedFooterHint]) -> Vec<KeyHint<'_>> {
    resolved
        .iter()
        .map(|hint| KeyHint::new(&hint.key, hint.label))
        .collect()
}

fn visible_hint_prefix(
    resolved: &[ResolvedFooterHint],
    budget: usize,
    theme: &Theme,
) -> Option<(usize, usize)> {
    let hints = key_hints(resolved);
    (1..=hints.len()).rev().find_map(|count| {
        let width = super::controls::measure_key_hints(&hints[..count], theme)?;
        (width <= budget).then_some((count, width))
    })
}

pub(super) fn render_status(
    frame: &mut Frame<'_>,
    area: Rect,
    mode: CopyModeView<'_>,
    locale: Locale,
    notice: Option<&str>,
    theme: &Theme,
    clipboard_supported: bool,
) {
    if area.is_empty() {
        return;
    }
    frame.render_widget(Clear, area);
    frame
        .buffer_mut()
        .set_style(area, theme.style(Role::SurfaceChrome));

    let resolved = resolved_footer_hints(mode.search.is_some(), clipboard_supported, locale, theme);
    let all_hints = key_hints(&resolved);
    let Some(exit_width) = super::controls::measure_key_hints(&all_hints[..1], theme) else {
        return;
    };
    let requested_label = locale.copy_mode_label(mode.status, mode.selecting);
    let fallback_label = locale.copy_mode_label(CopyModeStatus::Ready, false);
    let notice_width = notice.map_or(0, |notice| {
        warning_marker_width(theme).saturating_add(display_width(notice))
    });
    let body_reserve = if mode.search.is_some() {
        9_usize.saturating_add(if notice.is_some() {
            notice_width.min(12).saturating_add(1)
        } else {
            0
        })
    } else {
        notice_width
    };
    let requested_prefix_width = display_width(requested_label).saturating_add(3);
    let label = if requested_prefix_width
        .saturating_add(body_reserve)
        .saturating_add(1)
        .saturating_add(exit_width)
        <= usize::from(area.width)
    {
        requested_label
    } else {
        fallback_label
    };
    let prefix = format!("{label} {} ", theme.glyphs.middle_dot);
    let prefix_width = display_width(&prefix);
    let content_reserve = prefix_width.saturating_add(body_reserve);
    let desired_budget = usize::from(area.width).saturating_sub(content_reserve.saturating_add(1));
    let fallback_budget = usize::from(area.width).saturating_sub(prefix_width.saturating_add(1));
    let Some((hint_count, hint_width)) = visible_hint_prefix(&resolved, desired_budget, theme)
        .or_else(|| (exit_width <= fallback_budget).then_some((1, exit_width)))
    else {
        return;
    };
    let hint_width = u16::try_from(hint_width).expect("footer hint width is bounded by u16 area");
    let hint_area = Rect::new(area.right() - hint_width, area.y, hint_width, 1);
    let content_right = hint_area.x.saturating_sub(1);
    let content_width = usize::from(content_right.saturating_sub(area.x));
    let label_width = prefix_width.min(content_width);
    frame.buffer_mut().set_stringn(
        area.x,
        area.y,
        truncate_display(&prefix, label_width, ""),
        label_width,
        theme.style(Role::StatusInfo),
    );

    let body_x = area.x + u16::try_from(label_width).expect("status width is u16");
    let body_width = content_right.saturating_sub(body_x);
    if let Some(search) = mode.search {
        paint_search_status(
            frame,
            Rect::new(body_x, area.y, body_width, 1),
            search,
            notice,
            theme,
        );
    } else if let Some(notice) = notice {
        paint_warning_notice(
            frame,
            Rect::new(body_x, area.y, body_width, 1),
            notice,
            theme,
        );
    }

    let rendered =
        super::controls::render_key_hints(frame, hint_area, &all_hints[..hint_count], theme);
    debug_assert_eq!(rendered, Some(hint_area));
}

fn paint_search_status(
    frame: &mut Frame<'_>,
    area: Rect,
    search: crate::terminal_copy_mode::CopyModeSearchView<'_>,
    notice: Option<&str>,
    theme: &Theme,
) {
    if area.width <= 1 {
        return;
    }
    let direction = match search.direction {
        CopySearchDirection::Forward => "/",
        CopySearchDirection::Backward => "?",
    };
    frame
        .buffer_mut()
        .set_string(area.x, area.y, direction, theme.style(Role::HintKey));

    let total = usize::from(area.width - 1);
    let warning_width = notice.map_or(0, |notice| {
        warning_marker_width(theme)
            .saturating_add(display_width(notice))
            .min(total.saturating_sub(6))
            .min(24)
    });
    let warning_seam = usize::from(warning_width > 0);
    let draft_columns = total.saturating_sub(warning_width.saturating_add(warning_seam));
    let (draft, cursor) = search_window(search.draft, search.cursor, draft_columns);
    frame.buffer_mut().set_stringn(
        area.x + 1,
        area.y,
        draft,
        draft_columns,
        theme.style(Role::TextPrimary),
    );
    frame.set_cursor_position((area.x + 1 + cursor, area.y));

    if let Some(notice) = notice.filter(|_| warning_width > 0) {
        let warning_width = u16::try_from(warning_width).expect("warning width is within area");
        let warning_area = Rect::new(area.right() - warning_width, area.y, warning_width, 1);
        paint_warning_notice(frame, warning_area, notice, theme);
    }
}

fn paint_warning_notice(frame: &mut Frame<'_>, area: Rect, notice: &str, theme: &Theme) {
    if area.is_empty() {
        return;
    }
    let marker = format!("{} ", theme.glyphs.status_bullet);
    let marker_width = warning_marker_width(theme);
    let (text_x, text_width) = if marker_width < usize::from(area.width) {
        frame.buffer_mut().set_stringn(
            area.x,
            area.y,
            marker,
            marker_width,
            theme.style(Role::StatusWarning),
        );
        let marker_width = u16::try_from(marker_width).expect("status marker fits its area");
        (
            area.x + marker_width,
            usize::from(area.width - marker_width),
        )
    } else {
        (area.x, usize::from(area.width))
    };
    let text = truncate_display(notice, text_width, theme.glyphs.ellipsis);
    frame.buffer_mut().set_stringn(
        text_x,
        area.y,
        text,
        text_width,
        theme.style(Role::TextPrimary),
    );
}

fn warning_marker_width(theme: &Theme) -> usize {
    display_width(theme.glyphs.status_bullet).saturating_add(1)
}

/// Horizontally scroll at grapheme boundaries, reserving one cell for the insertion caret.
fn search_window(draft: &str, cursor: usize, columns: usize) -> (String, u16) {
    if columns == 0 {
        return (String::new(), 0);
    }
    let before = &draft[..cursor];
    let mut start = 0;
    let mut width = display_width(before);
    for (offset, grapheme) in before.grapheme_indices(true) {
        if width < columns {
            break;
        }
        width = width.saturating_sub(display_width(grapheme));
        start = offset + grapheme.len();
    }
    (
        truncate_display(&draft[start..], columns, ""),
        u16::try_from(width).expect("caret is within u16 status width"),
    )
}

#[cfg(test)]
mod tests;
