//! The Agent board overlay: canonical session rows with attention,
//! status, freshness, and resolved Workbench location — plus the hit
//! regions that let pointer presses re-resolve by stable identity.

use ratatui::text::Span;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    text::{Line, Text},
    widgets::{Clear, Paragraph},
};

use crate::agent_board::{
    AgentBoardControl, AgentBoardDraw, AgentBoardRead, AgentBoardRow, AgentBoardView,
    PresentationPlacement,
};
use crate::agent_board::{AgentStartCatalogStatus, AgentStartPhase};
use crate::localization::{Locale, ShellText};
use crate::terminal_text::{display_width, sanitize_terminal_text, truncate_display};
use crate::tui_hit_map::{
    HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
    ProjectedPointer,
};
use crate::tui_layout::place_overlay;
use crate::tui_theme::{GlyphMode, Role, Theme};
use orchbee_agent_observation_protocol::{
    AgentLifecycle, AgentObservationFreshness, AgentSemanticStatus,
};

/// Rows the board reserves outside its scrollable row window: one status
/// or notice line, the keyboard hint and the action buttons.
const BOARD_CHROME_ROWS: u16 = 3;
const BOARD_WIDTH: u16 = 92;
const BOARD_MAX_HEIGHT: u16 = 24;
const EMPTY_GUIDANCE_HEIGHT: u16 = 10;
const START_NAME_VALUE_COLUMNS: usize = 32;

/// Paints the open board inside `area` and records its hit regions in the
/// same pass. Geometry refusal carries the exact open episode; success
/// carries the owner's complete visible-window witness.
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ShellText,
    locale: Locale,
    board: &AgentBoardView<'_>,
    (theme, working_frame): (&Theme, usize),
    hits: &mut HitMapBuilder,
) -> AgentBoardDraw {
    let rows = board.rows();
    let read = board.read();
    let has_empty_guidance = rows.is_empty() && matches!(read, AgentBoardRead::Ready { .. });
    let start_supported = board.start_catalog_status() != AgentStartCatalogStatus::Unsupported;
    let start_mode = start_supported
        && rows.is_empty()
        && matches!(read, AgentBoardRead::Ready { complete: true, .. });

    let wanted = if start_mode {
        BOARD_MAX_HEIGHT
    } else if has_empty_guidance {
        EMPTY_GUIDANCE_HEIGHT
    } else {
        u16::try_from(rows.len() + usize::from(BOARD_CHROME_ROWS) + 2)
            .unwrap_or(BOARD_MAX_HEIGHT)
            .min(BOARD_MAX_HEIGHT)
    };
    let Some(area) = place_overlay(area, BOARD_WIDTH, wanted.max(8)) else {
        return AgentBoardDraw::NotPresented(board.episode());
    };
    if area.height < 8 {
        return AgentBoardDraw::NotPresented(board.episode());
    };
    let block = crate::tui_render::overlay_block(text.board.title, area.width, theme);
    let inner = block.inner(area);
    let [body, footer] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(BOARD_CHROME_ROWS)]).areas(inner);
    let [read_row, hint_row, action_row] =
        Layout::vertical([Constraint::Length(1); 3]).areas(footer);
    let key_glyphs = match theme.glyphs.mode() {
        GlyphMode::Unicode => crate::client_view::KeyLabelGlyphs::Unicode,
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => crate::client_view::KeyLabelGlyphs::Ascii,
    };
    let label_style = crate::client_view::LabelStyle::for_host();
    let format_key =
        |binding| crate::client_view::format_binding_with_glyphs(&binding, label_style, key_glyphs);
    let navigation = board.navigation_bindings();
    let navigation_key = format!(
        "{}, {}",
        crate::client_view::join_key_labels(
            &[format_key(navigation[0]), format_key(navigation[1])],
            label_style,
            "/",
        ),
        crate::client_view::join_key_labels(
            &[format_key(navigation[2]), format_key(navigation[3])],
            label_style,
            "/",
        ),
    );
    let close_key = format_key(board.close_binding());
    let jump_key = format_key(board.jump_binding());
    let refresh_key = format_key(board.refresh_binding());
    let start_accept_key = format_key(board.start_accept_binding());
    let start_provider_key = {
        let bindings = board.start_provider_bindings();
        crate::client_view::join_key_labels(
            &[format_key(bindings[0]), format_key(bindings[1])],
            label_style,
            "/",
        )
    };
    let start_edit_key = format_key(board.start_edit_binding());
    let start_cancel_key = format_key(board.start_cancel_binding());
    let hints = if start_mode {
        let mut hints = vec![super::controls::KeyHint::new(
            &close_key,
            text.board.close_hint,
        )];
        if board.start_running() {
            hints.push(super::controls::KeyHint::new(
                &start_cancel_key,
                text.board.start_cancel_hint,
            ));
        } else {
            hints.extend([
                super::controls::KeyHint::new(&start_accept_key, text.board.start_button),
                super::controls::KeyHint::new(&start_provider_key, text.board.start_provider_hint),
                super::controls::KeyHint::new(&start_edit_key, text.board.start_edit_hint),
            ]);
        }
        hints
    } else {
        vec![
            super::controls::KeyHint::new(&close_key, text.board.close_hint),
            super::controls::KeyHint::new(&jump_key, text.board.jump_hint),
            super::controls::KeyHint::new(&refresh_key, text.board.refresh_hint),
            super::controls::KeyHint::new(&navigation_key, text.board.select_hint),
        ]
    };

    if super::controls::measure_overlay_hints(&hints, theme).is_none()
        || super::controls::measure_overlay_hints(&hints[..1], theme)
            .is_none_or(|width| width > usize::from(hint_row.width))
    {
        return AgentBoardDraw::NotPresented(board.episode());
    }
    if !rows.is_empty() && row_minimum_width(theme).is_none_or(|minimum| minimum > body.width) {
        return AgentBoardDraw::NotPresented(board.episode());
    }
    let viewport_rows = usize::from(body.height);
    let Some(draw) = board.actual_draw(viewport_rows) else {
        return AgentBoardDraw::NotPresented(board.episode());
    };
    let start_layout = if start_mode {
        let Some(layout) = StartFormLayout::resolve(frame.area(), body, text, board, theme) else {
            return AgentBoardDraw::NotPresented(board.episode());
        };
        Some(layout)
    } else {
        None
    };
    let default_primary = if draw.selected().is_some() {
        (text.board.open_button, !board.jumping(), board.jumping())
    } else {
        (
            text.board.refresh_button,
            true,
            matches!(board.read(), AgentBoardRead::InFlight),
        )
    };
    let (primary_label, primary_enabled, primary_pending) = if start_mode {
        let cancelling = board.start_phase() == Some(AgentStartPhase::Cancelling);
        if board.start_running() {
            (text.board.start_cancel_button, !cancelling, cancelling)
        } else {
            (text.board.start_button, board.can_start_agent(), false)
        }
    } else {
        default_primary
    };

    let primary = super::controls::ActionSpec::new(
        primary_label,
        super::controls::ActionKind::Primary,
        super::controls::ControlSurface::Overlay,
        interaction_state(
            primary_enabled,
            false,
            primary_pending,
            super::controls::PointerPresentation::Rest,
        ),
    )
    .with_pending_label(primary_label);
    if super::controls::action_rect(frame.area(), action_row, primary, theme).is_none() {
        return AgentBoardDraw::NotPresented(board.episode());
    }

    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    // A presented board owns the press domain: rows select, the surface
    // swallows and scrolls, and an outside press resolves as backdrop.
    hits.start_modal();
    let _ = render_header_close(frame, area, &draw, theme, hits);

    hits.record(
        HitLayer::OverlaySurface,
        body,
        HitTarget::ListScroll(crate::tui_hit_map::ListScrollTarget::AgentBoard(
            draw.token().clone(),
        )),
    );
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    if let Some(layout) = start_layout.as_ref() {
        render_start_form(frame, text, board, &draw, layout, theme, hits);
    } else if rows.is_empty() {
        render_empty_state(frame, body, text, locale, read, start_supported, theme);
    } else {
        render_rows(
            frame,
            body,
            text,
            locale,
            board,
            &draw,
            (theme, working_frame),
            hits,
        );
    }

    let (status, status_role) = if let Some(failure) = board.notice() {
        (
            locale.agent_board_notice(failure).to_owned(),
            Role::StatusWarning,
        )
    } else if board.jumping() {
        (text.board.jumping.to_owned(), Role::StatusPending)
    } else {
        (
            match read {
                AgentBoardRead::Ready {
                    complete: false, ..
                } => text.board.incomplete,
                AgentBoardRead::InFlight => text.board.in_flight,
                AgentBoardRead::Failed(_) => locale.agent_board_read_line(read),
                AgentBoardRead::Unread | AgentBoardRead::Ready { .. } => "",
            }
            .to_owned(),
            Role::TextSecondary,
        )
    };
    let (status, status_role) = if start_mode || board.start_phase().is_some() {
        start_status(text, board, theme)
    } else {
        (status, status_role)
    };
    frame.render_widget(
        Paragraph::new(truncate_display(
            &status,
            usize::from(read_row.width),
            theme.glyphs.ellipsis,
        ))
        .style(theme.style(status_role)),
        read_row,
    );
    let hint_rect = super::controls::render_overlay_hints(frame, hint_row, &hints, theme);
    debug_assert!(
        hint_rect.is_some(),
        "preflighted exit hint remains paintable"
    );
    if start_mode {
        render_start_footer_actions(frame, action_row, text, board, &draw, theme, hits);
    } else {
        render_footer_actions(frame, action_row, text, board, &draw, theme, hits);
    }

    AgentBoardDraw::Presented(draw)
}

fn render_empty_state(
    frame: &mut Frame<'_>,
    body: Rect,
    text: ShellText,
    locale: Locale,
    read: AgentBoardRead,
    start_supported: bool,
    theme: &Theme,
) {
    // No completed-read rows exist — surface the honest read state
    // (unread, in-flight, denied, …) or the "zero sessions" verdict.
    let mut empty_lines = if let AgentBoardRead::Ready { complete, .. } = read {
        vec![Line::from(if complete {
            text.board.empty
        } else {
            text.board.empty_partial
        })]
    } else {
        vec![Line::from(locale.agent_board_read_line(read))]
    };
    if !start_supported && matches!(read, AgentBoardRead::Ready { complete: true, .. }) {
        empty_lines.push(Line::from(text.board.start_platform_unsupported));
    }
    frame.render_widget(
        Paragraph::new(crate::terminal_text::wrap_text(
            &Text::from(empty_lines),
            body.width,
        ))
        .alignment(Alignment::Center)
        .style(theme.style(Role::TextSecondary)),
        body,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_rows(
    frame: &mut Frame<'_>,
    body: Rect,
    text: ShellText,
    locale: Locale,
    board: &AgentBoardView<'_>,
    draw: &crate::agent_board::AgentBoardActualDraw,
    (theme, working_frame): (&Theme, usize),
    hits: &mut HitMapBuilder,
) {
    let rows = board.rows();
    let viewport_rows = usize::from(body.height);
    let scroll = draw.token().scroll().min(rows.len().saturating_sub(1));
    let selection = board.selection();
    let end = (scroll + viewport_rows).min(rows.len());
    for (index, row) in rows[scroll..end].iter().enumerate() {
        let y = body
            .y
            .saturating_add(u16::try_from(index).unwrap_or(u16::MAX));
        let row_area = Rect::new(body.x, y, body.width, 1);
        if row_area.is_empty() {
            break;
        }
        let target = draw.row_target(row.session_id().clone());
        render_row(
            frame,
            row_area,
            text,
            locale,
            board,
            row,
            index + scroll == selection,
            target,
            (theme, working_frame),
            hits,
        );
    }
}

fn pointer_presentation(pointer: ProjectedPointer) -> super::controls::PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => super::controls::PointerPresentation::Rest,
        ProjectedPointer::Hovered => super::controls::PointerPresentation::Hovered,
        ProjectedPointer::Pressed => super::controls::PointerPresentation::Pressed,
    }
}

struct StartFormLayout {
    empty: Rect,
    explanation: Rect,
    catalog: Rect,
    name: Option<Rect>,
    providers: Vec<(usize, Rect, String)>,
}

impl StartFormLayout {
    fn resolve(
        bounds: Rect,
        body: Rect,
        text: ShellText,
        board: &AgentBoardView<'_>,
        theme: &Theme,
    ) -> Option<Self> {
        if body.height < 4 {
            return None;
        }
        let row = |y| (y < body.bottom()).then_some(Rect::new(body.x, y, body.width, 1));
        let empty = row(body.y)?;
        let explanation = row(body.y.checked_add(1)?)?;
        let catalog = row(body.y.checked_add(2)?)?;
        let name_width = display_width(text.board.start_name)
            .checked_add(2)?
            .checked_add(START_NAME_VALUE_COLUMNS.checked_add(4)?)?;
        let name_fits = name_width <= usize::from(body.width);
        if !name_fits || body.height < 6 {
            return None;
        }
        let selected = board
            .start_providers()
            .iter()
            .position(|provider| Some(*provider) == board.selected_start_provider())
            .unwrap_or(0);
        let mut providers = Vec::new();
        let mut y = catalog.y + 1;
        let mut x = body.x;
        // A bounded moving window keeps the selected provider visible at every
        // width; Up/Down reaches all canonical providers, including manual choices.
        for (index, provider) in board
            .start_providers()
            .iter()
            .enumerate()
            .skip(selected)
            .take(5)
        {
            let label = (*provider).to_owned();
            let remaining = Rect::new(x, y, body.right().saturating_sub(x), 1);
            let rect = provider_choice_rect(bounds, remaining, &label, theme).or_else(|| {
                y += 1;
                x = body.x;
                provider_choice_rect(bounds, Rect::new(x, y, body.width, 1), &label, theme)
            })?;
            if y > catalog.y + 2 {
                break;
            }
            x = rect.right().saturating_add(1);
            providers.push((index, rect, label));
        }
        Some(Self {
            empty,
            explanation,
            catalog,
            name: Some(row(catalog.y + 3)?),
            providers,
        })
    }
}

fn provider_choice_rect(bounds: Rect, area: Rect, label: &str, theme: &Theme) -> Option<Rect> {
    let width = display_width(label)
        .checked_add(display_width(theme.glyphs.status_bullet))?
        .checked_add(5)?;
    let width = u16::try_from(width).ok()?;
    if area.height == 0 || width > area.width {
        return None;
    }
    let rect = Rect::new(area.x, area.y, width, 1);
    (bounds.intersection(rect) == rect).then_some(rect)
}

#[allow(clippy::too_many_arguments)]
fn render_start_form(
    frame: &mut Frame<'_>,
    text: ShellText,
    board: &AgentBoardView<'_>,
    draw: &crate::agent_board::AgentBoardActualDraw,
    layout: &StartFormLayout,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let heading = text.board.empty;
    frame.render_widget(
        Paragraph::new(truncate_display(
            heading,
            usize::from(layout.empty.width),
            theme.glyphs.ellipsis,
        ))
        .style(theme.style(Role::TextPrimary)),
        layout.empty,
    );
    frame.render_widget(
        Paragraph::new(truncate_display(
            text.board.empty_start,
            usize::from(layout.explanation.width),
            theme.glyphs.ellipsis,
        ))
        .style(theme.style(Role::TextSecondary)),
        layout.explanation,
    );
    let detected = board.detected_start_providers();
    let catalog = if detected.is_empty() {
        match board.start_catalog_status() {
            AgentStartCatalogStatus::Loading => text.board.start_checking.to_owned(),
            AgentStartCatalogStatus::Unavailable => text.board.start_catalog_unavailable.to_owned(),
            AgentStartCatalogStatus::Ready => text.board.start_none_found.to_owned(),
            AgentStartCatalogStatus::Unsupported => {
                text.board.start_platform_unsupported.to_owned()
            }
        }
    } else {
        let pending = match board.start_catalog_status() {
            AgentStartCatalogStatus::Loading => format!(" · {}", text.board.start_checking),
            AgentStartCatalogStatus::Unavailable => {
                format!(" · {}", text.board.start_catalog_unavailable)
            }
            AgentStartCatalogStatus::Ready => String::new(),
            AgentStartCatalogStatus::Unsupported => {
                format!(" · {}", text.board.start_platform_unsupported)
            }
        };
        format!(
            "{}: {}{pending}",
            text.board.start_providers,
            detected.join(", ")
        )
    };
    frame.render_widget(
        Paragraph::new(truncate_display(
            &catalog,
            usize::from(layout.catalog.width),
            theme.glyphs.ellipsis,
        ))
        .style(theme.style(Role::TextSecondary)),
        layout.catalog,
    );
    for (index, rect, label) in &layout.providers {
        let provider = board.start_providers()[*index];
        let enabled = board.start_draft_editable();
        let selected = board.selected_start_provider() == Some(provider);
        let target = draw.control_target(AgentBoardControl::StartProvider(*index));
        let interaction = InteractionTarget::AgentBoardControl(target);
        let spec = if enabled {
            InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
        } else {
            InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
        };
        let pointer = pointer_presentation(hits.projected_pointer(
            HitLayer::Overlay,
            *rect,
            *rect,
            &interaction,
            spec,
        ));
        let Some(painted) =
            render_provider_choice(frame, *rect, label, selected, enabled, pointer, theme)
        else {
            continue;
        };
        debug_assert_eq!(painted, *rect);
        hits.record_interaction(HitLayer::Overlay, painted, painted, interaction, spec);
    }
    if let Some(name) = layout.name {
        render_start_name(frame, name, text, board, theme);
    }
}

fn render_provider_choice(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    selected: bool,
    enabled: bool,
    pointer: super::controls::PointerPresentation,
    theme: &Theme,
) -> Option<Rect> {
    use super::controls::{
        Availability, CheckedState, ControlSurface, CurrentState, FocusState, InteractionState,
        NavigationState, OperationState, RowFocusCue, RowTracks, paint_row_interaction,
    };
    let rect = provider_choice_rect(frame.area(), area, label, theme)?;
    let checked_width = u16::try_from(display_width(theme.glyphs.status_bullet)).ok()?;
    let checked = Rect::new(rect.x + 2, rect.y, checked_width, 1);
    let primary = Rect::new(
        checked.right() + 1,
        rect.y,
        rect.width - checked_width - 5,
        1,
    );
    let role = if !enabled {
        Role::TextDisabled
    } else if selected {
        Role::AccentSelection
    } else {
        Role::TextPrimary
    };
    frame.render_widget(
        Paragraph::new(format!(
            "[ {} {label} ]",
            " ".repeat(usize::from(checked_width)),
        ))
        .style(theme.style(role)),
        rect,
    );
    paint_row_interaction(
        frame,
        rect,
        RowTracks::new(RowFocusCue::EnclosingFrame, primary)
            .with_primary_role(role)
            .with_checked(checked, theme.glyphs.status_bullet),
        &[],
        ControlSurface::Overlay,
        InteractionState::new(
            FocusState::Unfocused,
            pointer,
            NavigationState::None,
            CurrentState::NotCurrent,
            if selected {
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
        ),
        theme,
    )
}

fn render_start_name(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ShellText,
    board: &AgentBoardView<'_>,
    theme: &Theme,
) {
    let label = sanitize_terminal_text(text.board.start_name);
    let value = if board.start_name().is_empty() {
        text.board.start_name_placeholder
    } else {
        board.start_name()
    };
    let value = truncate_display(value, START_NAME_VALUE_COLUMNS, theme.glyphs.ellipsis);
    let padding = START_NAME_VALUE_COLUMNS.saturating_sub(display_width(&value));
    let value_role = if board.start_name().is_empty() {
        Role::TextDisabled
    } else {
        Role::TextPrimary
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(label.to_string(), theme.style(Role::TextSecondary)),
            Span::raw("  "),
            Span::styled("[ ", theme.style(Role::AccentFocus)),
            Span::styled(value, theme.style(value_role)),
            Span::raw(" ".repeat(padding)),
            Span::styled(" ]", theme.style(Role::AccentFocus)),
        ])),
        area,
    );
    if board.start_draft_editable() {
        let x = area
            .x
            .saturating_add(u16::try_from(display_width(&label)).unwrap_or(u16::MAX))
            .saturating_add(4)
            .saturating_add(u16::try_from(board.start_name_cursor()).unwrap_or(u16::MAX));
        if x < area.right() {
            frame.set_cursor_position((x, area.y));
        }
    }
}

fn start_status(text: ShellText, board: &AgentBoardView<'_>, theme: &Theme) -> (String, Role) {
    let result = match board.start_phase() {
        Some(AgentStartPhase::WaitingInput) => {
            (text.board.start_waiting_input, Role::StatusWarning)
        }
        Some(AgentStartPhase::Unconfirmed) => (text.board.start_unconfirmed, Role::StatusWarning),
        Some(AgentStartPhase::Starting) => (text.board.start_starting, Role::StatusPending),
        Some(AgentStartPhase::Cancelling) => (text.board.start_cancelling, Role::StatusPending),
        Some(AgentStartPhase::Started) => (text.board.start_started, Role::StatusSuccess),
        Some(AgentStartPhase::Failed) => (text.board.start_failed, Role::StatusError),
        Some(AgentStartPhase::Unknown) => (text.board.start_unknown, Role::StatusWarning),
        Some(AgentStartPhase::Cancelled) => (text.board.start_cancelled, Role::StatusWarning),
        None if orchbee_agent_observation_protocol::AgentName::parse(board.start_name())
            .is_err() =>
        {
            (text.board.start_name_invalid, Role::StatusInfo)
        }
        None => ("", Role::TextSecondary),
    };
    let identity = board.start_operation_identity().map(|(name, provider)| {
        format!(
            "  {} {} {}",
            name.as_str(),
            theme.glyphs.middle_dot,
            provider.as_str()
        )
    });
    (
        format!("{}{}", result.0, identity.unwrap_or_default()),
        result.1,
    )
}

fn render_start_footer_actions(
    frame: &mut Frame<'_>,
    row: Rect,
    text: ShellText,
    board: &AgentBoardView<'_>,
    draw: &crate::agent_board::AgentBoardActualDraw,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    use super::controls::{ActionKind, ActionSpec, ControlSurface, PointerPresentation};
    let cancelling = board.start_phase() == Some(AgentStartPhase::Cancelling);
    let controls = if board.start_running() {
        vec![(
            AgentBoardControl::CancelStart,
            text.board.start_cancel_button,
            ActionKind::Secondary,
            !cancelling,
            cancelling,
        )]
    } else {
        vec![
            (
                AgentBoardControl::Start,
                text.board.start_button,
                ActionKind::Primary,
                board.can_start_agent(),
                false,
            ),
            (
                AgentBoardControl::Refresh,
                text.board.refresh_button,
                ActionKind::Secondary,
                true,
                matches!(board.read(), AgentBoardRead::InFlight),
            ),
        ]
    };
    let control_count = controls.len();
    let mut remaining = row;
    for (index, (control, label, kind, enabled, pending)) in controls.into_iter().enumerate() {
        let rest = ActionSpec::new(
            label,
            kind,
            ControlSurface::Overlay,
            interaction_state(enabled, false, pending, PointerPresentation::Rest),
        )
        .with_pending_label(label);
        let Some(rect) = super::controls::action_rect(frame.area(), remaining, rest, theme) else {
            break;
        };
        let target = draw.control_target(control);
        let interaction = InteractionTarget::AgentBoardControl(target);
        let spec = if enabled {
            InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
        } else {
            InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
        };
        let pointer = pointer_presentation(hits.projected_pointer(
            HitLayer::Overlay,
            rect,
            rect,
            &interaction,
            spec,
        ));
        let action = ActionSpec::new(
            label,
            kind,
            ControlSurface::Overlay,
            interaction_state(enabled, false, pending, pointer),
        )
        .with_pending_label(label);
        let Some(painted) = super::controls::render_action(frame, remaining, action, theme) else {
            break;
        };
        if painted != rect {
            break;
        }
        hits.record_interaction(HitLayer::Overlay, painted, painted, interaction, spec);
        if index + 1 < control_count {
            let Some(next_x) = painted.right().checked_add(2) else {
                break;
            };
            remaining = Rect::new(
                next_x,
                row.y,
                row.right().saturating_sub(next_x),
                row.height,
            );
        }
    }
}

fn row_minimum_width(theme: &Theme) -> Option<u16> {
    use orchbee_workbench_protocol::MetadataState;
    let glyph_width = [
        MetadataState::Blocked,
        MetadataState::Working,
        MetadataState::Done,
        MetadataState::Error,
        MetadataState::Idle,
        MetadataState::Unknown,
    ]
    .into_iter()
    .map(|state| display_width(theme.agent_glyph(state)).checked_add(1))
    .collect::<Option<Vec<_>>>()?
    .into_iter()
    .max()?;
    let width = display_width(theme.glyphs.navigation_marker)
        .max(1)
        .checked_add(glyph_width)?
        .checked_add(display_width(theme.glyphs.status_bullet).max(1))?
        .checked_add(1)?;
    u16::try_from(width).ok()
}

fn interaction_state(
    enabled: bool,
    selected: bool,
    pending: bool,
    pointer: super::controls::PointerPresentation,
) -> super::controls::InteractionState {
    use super::controls::{
        Availability, CheckedState, CurrentState, FocusState, NavigationState, OperationState,
    };
    super::controls::InteractionState::new(
        FocusState::Unfocused,
        pointer,
        if selected {
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
        if pending {
            OperationState::Pending
        } else {
            OperationState::Idle
        },
    )
}

fn render_header_close(
    frame: &mut Frame<'_>,
    area: Rect,
    draw: &crate::agent_board::AgentBoardActualDraw,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    use super::controls::{Availability, ControlSurface, IconActionGlyph, IconActionSpec};
    let target = draw.control_target(AgentBoardControl::Close);
    let interaction = InteractionTarget::AgentBoardControl(target);
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let rect = super::controls::frame_close_rect(frame.area(), area)?;
    let pointer = pointer_presentation(hits.projected_pointer(
        HitLayer::Overlay,
        rect,
        rect,
        &interaction,
        spec,
    ));
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
    hits.record_interaction(HitLayer::Overlay, painted, painted, interaction, spec);
    Some(painted)
}

fn render_footer_actions(
    frame: &mut Frame<'_>,
    row: Rect,
    text: ShellText,
    board: &AgentBoardView<'_>,
    draw: &crate::agent_board::AgentBoardActualDraw,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    use super::controls::{ActionKind, ActionSpec, ControlSurface, PointerPresentation};
    let controls = if draw.selected().is_some() {
        [
            (
                AgentBoardControl::Open,
                text.board.open_button,
                ActionKind::Primary,
                !board.jumping(),
                board.jumping(),
            ),
            (
                AgentBoardControl::Refresh,
                text.board.refresh_button,
                ActionKind::Secondary,
                true,
                matches!(board.read(), AgentBoardRead::InFlight),
            ),
        ]
    } else {
        [
            (
                AgentBoardControl::Refresh,
                text.board.refresh_button,
                ActionKind::Primary,
                true,
                matches!(board.read(), AgentBoardRead::InFlight),
            ),
            (
                AgentBoardControl::Open,
                text.board.open_button,
                ActionKind::Secondary,
                false,
                board.jumping(),
            ),
        ]
    };
    let control_count = controls.len();
    let mut remaining = row;
    for (index, (control, label, kind, enabled, pending)) in controls.into_iter().enumerate() {
        let rest = ActionSpec::new(
            label,
            kind,
            ControlSurface::Overlay,
            interaction_state(enabled, false, pending, PointerPresentation::Rest),
        )
        .with_pending_label(label);
        let Some(rect) = super::controls::action_rect(frame.area(), remaining, rest, theme) else {
            break;
        };
        let target = draw.control_target(control);
        let interaction = InteractionTarget::AgentBoardControl(target);
        let spec = if enabled {
            InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
        } else {
            InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
        };
        let pointer = pointer_presentation(hits.projected_pointer(
            HitLayer::Overlay,
            rect,
            rect,
            &interaction,
            spec,
        ));
        let action = ActionSpec::new(
            label,
            kind,
            ControlSurface::Overlay,
            interaction_state(enabled, false, pending, pointer),
        )
        .with_pending_label(label);
        let Some(painted) = super::controls::render_action(frame, remaining, action, theme) else {
            break;
        };
        if painted != rect {
            break;
        }
        hits.record_interaction(HitLayer::Overlay, painted, painted, interaction, spec);
        if index + 1 < control_count {
            let Some(next_x) = painted.right().checked_add(2) else {
                break;
            };
            remaining = Rect::new(
                next_x,
                row.y,
                row.right().saturating_sub(next_x),
                row.height,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_row(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ShellText,
    locale: Locale,
    board: &AgentBoardView<'_>,
    row: &AgentBoardRow,
    selected: bool,
    target: crate::agent_board::AgentBoardRowTarget,
    (theme, working_frame): (&Theme, usize),
    hits: &mut HitMapBuilder,
) {
    use super::controls::{ControlSurface, RowContentBand, RowFocusCue, RowTracks};
    use orchbee_workbench_protocol::MetadataState;
    let state = match board.display_status(row) {
        AgentSemanticStatus::NeedsAttention { .. } => MetadataState::Blocked,
        AgentSemanticStatus::Working { .. } => MetadataState::Working,
        AgentSemanticStatus::Succeeded { .. } => MetadataState::Done,
        AgentSemanticStatus::Failed { .. }
        | AgentSemanticStatus::Canceled { .. }
        | AgentSemanticStatus::Interrupted { .. } => MetadataState::Error,
        AgentSemanticStatus::Idle { .. } => MetadataState::Idle,
        AgentSemanticStatus::Unknown { .. } | AgentSemanticStatus::Ended { .. } => {
            MetadataState::Unknown
        }
    };
    let status = locale.agent_status(board.display_status(row));
    let freshness = match row.freshness() {
        AgentObservationFreshness::Fresh => "",
        AgentObservationFreshness::Stale => text.board.stale,
        AgentObservationFreshness::Disconnected => text.board.disconnected,
    };
    let ended = row.lifecycle() == AgentLifecycle::Ended;
    // The opaque session id stays internal: selection and jumps use it, but the
    // row names the session by its terminal's Tab-strip name, then provider and
    // Workspace, and an unplaced session by a localized placeholder.
    let provider = sanitize_terminal_text(row.provider_id().as_str());
    let separator = format!(" {} ", theme.glyphs.middle_dot);
    let placed = row
        .attachments()
        .iter()
        .find(|attachment| attachment.navigable)
        .and_then(|attachment| Some((attachment, board.placement(attachment)?)));
    let placement = placed.map(|(_, placement)| placement);
    let names =
        placed.map(|(attachment, placement)| board.placement_names(attachment, placement, locale));
    let title = names
        .as_ref()
        .map(|(title, _)| sanitize_terminal_text(title.trim()).into_owned())
        .unwrap_or_default();
    let location = placement.zip(names.as_ref()).map_or_else(
        || text.board.no_panel.to_owned(),
        |(placement, (_, tab))| location_label(text, placement, tab, &separator, &title),
    );
    let detail = [title, provider.into_owned(), freshness.to_owned(), location]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(&separator);
    let navigation_width =
        u16::try_from(display_width(theme.glyphs.navigation_marker).max(1)).unwrap_or(u16::MAX);
    let glyph_text = format!(
        "{} ",
        super::sidebar_text::state_glyph(state, theme, working_frame)
    );
    let glyph_width = u16::try_from(display_width(&glyph_text)).unwrap_or(u16::MAX);
    let unread_width =
        u16::try_from(display_width(theme.glyphs.status_bullet).max(1)).unwrap_or(u16::MAX);
    let Some(primary_width) = area
        .width
        .checked_sub(navigation_width)
        .and_then(|width| width.checked_sub(glyph_width))
        .and_then(|width| width.checked_sub(unread_width))
        .filter(|width| *width > 0)
    else {
        return;
    };
    let navigation = Rect::new(area.x, area.y, navigation_width, 1);
    let glyph = Rect::new(navigation.right(), area.y, glyph_width, 1);
    let primary = Rect::new(glyph.right(), area.y, primary_width, 1);
    let unread = Rect::new(primary.right(), area.y, unread_width, 1);
    let line = truncate_display(
        &format!("{status}{separator}{detail}"),
        usize::from(primary.width),
        theme.glyphs.ellipsis,
    );
    let primary_role = if ended {
        Role::TextSecondary
    } else {
        Role::TextPrimary
    };
    frame.render_widget(
        Paragraph::new(glyph_text).style(theme.agent_style(state, working_frame)),
        glyph,
    );
    frame.render_widget(
        Paragraph::new(line).style(theme.style(primary_role)),
        primary,
    );
    let unread_visible = board.unread(row);
    if unread_visible {
        frame.render_widget(
            Paragraph::new(theme.glyphs.status_bullet).style(theme.style(Role::StatusUnread)),
            unread,
        );
    }
    let interaction = InteractionTarget::AgentBoardRow(target);
    let spec = InteractionSpec::enabled(PrimaryContract::SelectOnPress);
    let pointer = pointer_presentation(hits.projected_pointer(
        HitLayer::Overlay,
        area,
        area,
        &interaction,
        spec,
    ));
    let mut bands = vec![RowContentBand::explicit(glyph)];
    if unread_visible {
        bands.push(RowContentBand::semantic(unread, Role::StatusUnread));
    }
    let tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary)
        .with_primary_role(primary_role)
        .with_provisional(navigation);
    let Some(painted) = super::controls::paint_row_interaction(
        frame,
        area,
        tracks,
        &bands,
        ControlSurface::Overlay,
        interaction_state(true, selected, false, pointer),
        theme,
    ) else {
        return;
    };
    debug_assert_eq!(painted, area);
    hits.record_interaction(HitLayer::Overlay, area, painted, interaction, spec);
}

/// The workspace/project context and tab for the row's location cell —
/// display only; navigation never reads it back. A tab name that repeats the
/// row title is omitted. The separator is the active theme's host-safe glyph form.
fn location_label(
    text: ShellText,
    placement: &PresentationPlacement,
    tab: &str,
    separator: &str,
    title: &str,
) -> String {
    let context = placement
        .workspace_label
        .clone()
        .or_else(|| placement.project_label.clone())
        .unwrap_or_else(|| text.board.no_workspace.to_owned());
    let context = sanitize_terminal_text(&context);
    let tab = sanitize_terminal_text(tab.trim());
    if tab.is_empty() || tab == title {
        context.into_owned()
    } else {
        format!("{context}{separator}{tab}")
    }
}
