//! The Agent board overlay: canonical session rows with attention,
//! status, freshness, and resolved Workbench location — plus the hit
//! regions that let pointer presses re-resolve by stable identity.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    text::Line,
    widgets::{Clear, Paragraph, Wrap},
};

use crate::{
    localization::{Locale, ShellText},
    terminal_session::{
        AgentBoardControl, AgentBoardDraw, AgentBoardRead, AgentBoardRow, NativeAgentBoardView,
        PresentationPlacement,
    },
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_layout::place_overlay,
    tui_pointer::{
        HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
        ProjectedPointer,
    },
    tui_theme::{GlyphMode, Role, Theme},
};
use orchbee_agent_observation_protocol::{
    AgentLifecycle, AgentObservationFreshness, AgentSemanticStatus,
};

/// Rows the board reserves outside its scrollable row window: one status
/// or notice line, the keyboard hint and the action buttons.
const BOARD_CHROME_ROWS: u16 = 3;
const BOARD_WIDTH: u16 = 92;
const BOARD_MAX_HEIGHT: u16 = 24;
const EMPTY_GUIDANCE_HEIGHT: u16 = 10;

/// Paints the open board inside `area` and records its hit regions in the
/// same pass. Geometry refusal carries the exact open episode; success
/// carries the owner's complete visible-window witness.
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    text: ShellText,
    locale: Locale,
    board: &NativeAgentBoardView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> AgentBoardDraw {
    let rows = board.rows();
    let read = board.read();
    let has_empty_guidance =
        cfg!(windows) && rows.is_empty() && matches!(read, AgentBoardRead::Ready { .. });
    let wanted = if has_empty_guidance {
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
    let block = crate::tui_render::overlay_block(text.board.title, theme, true);
    let inner = block.inner(area);
    let [body, footer] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(BOARD_CHROME_ROWS)]).areas(inner);
    let [read_row, hint_row, action_row] =
        Layout::vertical([Constraint::Length(1); 3]).areas(footer);
    let key_glyphs = match theme.glyphs.mode() {
        GlyphMode::Unicode => crate::client_view::KeyLabelGlyphs::Unicode,
        #[cfg(test)]
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
    let hints = [
        super::controls::KeyHint::new(&close_key, text.board.close_hint),
        super::controls::KeyHint::new(&jump_key, text.board.jump_hint),
        super::controls::KeyHint::new(&refresh_key, text.board.refresh_hint),
        super::controls::KeyHint::new(&navigation_key, text.board.select_hint),
    ];
    if super::controls::measure_key_hints(&hints, theme).is_none()
        || super::controls::measure_key_hints(&hints[..1], theme)
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
    let (primary_label, primary_enabled, primary_pending) = if draw.selected().is_some() {
        (text.board.open_button, !board.jumping(), board.jumping())
    } else {
        (
            text.board.refresh_button,
            true,
            matches!(board.read(), AgentBoardRead::InFlight),
        )
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
        HitTarget::ListScroll(crate::tui_pointer::ListScrollTarget::AgentBoard(
            draw.token().clone(),
        )),
    );
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    if rows.is_empty() {
        // No completed-read rows exist — surface the honest read state
        // (unread, in-flight, denied, …) or the "zero sessions" verdict.
        let empty_lines = if let AgentBoardRead::Ready { complete, .. } = read {
            let mut lines = vec![Line::from(if complete {
                text.board.empty
            } else {
                text.board.empty_partial
            })];
            if cfg!(windows) {
                lines.push(Line::from(text.board.empty_start));
                lines.push(Line::from(text.board.empty_start_command));
            }
            lines
        } else {
            vec![Line::from(locale.agent_board_read_line(read))]
        };
        frame.render_widget(
            Paragraph::new(empty_lines)
                .alignment(Alignment::Center)
                .style(theme.style(Role::TextSecondary))
                .wrap(Wrap { trim: true }),
            body,
        );
    } else {
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
                theme,
                hits,
            );
        }
    }

    let (status, status_role) = if let Some(failure) = board.notice() {
        (locale.agent_board_notice(failure), Role::StatusWarning)
    } else if board.jumping() {
        (text.board.jumping, Role::StatusPending)
    } else {
        (
            match read {
                AgentBoardRead::Ready {
                    complete: false, ..
                } => text.board.incomplete,
                AgentBoardRead::InFlight => text.board.in_flight,
                AgentBoardRead::Failed(_) => locale.agent_board_read_line(read),
                AgentBoardRead::Unread | AgentBoardRead::Ready { .. } => "",
            },
            Role::TextSecondary,
        )
    };
    frame.render_widget(
        Paragraph::new(truncate_display(
            status,
            usize::from(read_row.width),
            theme.glyphs.ellipsis,
        ))
        .style(theme.style(status_role)),
        read_row,
    );
    let hint_rect = super::controls::render_key_hints(frame, hint_row, &hints, theme);
    debug_assert!(
        hint_rect.is_some(),
        "preflighted exit hint remains paintable"
    );
    render_footer_actions(frame, action_row, text, board, &draw, theme, hits);
    AgentBoardDraw::Presented(draw)
}

fn pointer_presentation(pointer: ProjectedPointer) -> super::controls::PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => super::controls::PointerPresentation::Rest,
        ProjectedPointer::Hovered => super::controls::PointerPresentation::Hovered,
        ProjectedPointer::Pressed => super::controls::PointerPresentation::Pressed,
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
    draw: &crate::terminal_session::AgentBoardActualDraw,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    use super::controls::{Availability, ControlSurface, IconActionGlyph, IconActionSpec};
    let target = draw.control_target(AgentBoardControl::Close);
    let interaction = InteractionTarget::AgentBoardControl(target);
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let rect = super::controls::close_rect(frame.area(), area)?;
    let pointer = pointer_presentation(hits.projected_pointer(
        HitLayer::Overlay,
        rect,
        rect,
        &interaction,
        spec,
    ));
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
    hits.record_interaction(HitLayer::Overlay, painted, painted, interaction, spec);
    Some(painted)
}

fn render_footer_actions(
    frame: &mut Frame<'_>,
    row: Rect,
    text: ShellText,
    board: &NativeAgentBoardView<'_>,
    draw: &crate::terminal_session::AgentBoardActualDraw,
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
    board: &NativeAgentBoardView<'_>,
    row: &AgentBoardRow,
    selected: bool,
    target: crate::terminal_session::AgentBoardRowTarget,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    use super::controls::{ControlSurface, RowContentBand, RowFocusCue, RowTracks};
    use orchbee_workbench_protocol::MetadataState;
    let state = match row.status() {
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
    let status = locale.agent_status(row.status());
    let freshness = match row.freshness() {
        AgentObservationFreshness::Fresh => "",
        AgentObservationFreshness::Stale => text.board.stale,
        AgentObservationFreshness::Disconnected => text.board.disconnected,
    };
    let ended = row.lifecycle() == AgentLifecycle::Ended;
    // The row's identity is the opaque session id — shown verbatim, never
    // replaced by a label or a position.
    let session = sanitize_terminal_text(row.session_id().as_str());
    let provider = sanitize_terminal_text(row.provider_id().as_str());
    let separator = format!(" {} ", theme.glyphs.middle_dot);
    let location = row
        .attachments()
        .iter()
        .find(|attachment| attachment.navigable)
        .and_then(|attachment| board.placement(attachment))
        .map_or_else(String::new, |placement| {
            location_label(text, placement, &separator)
        });
    let detail = [
        session.into_owned(),
        provider.into_owned(),
        freshness.to_owned(),
        location,
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(&separator);
    let navigation_width =
        u16::try_from(display_width(theme.glyphs.navigation_marker).max(1)).unwrap_or(u16::MAX);
    let glyph_text = format!("{} ", theme.agent_glyph(state));
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
        Paragraph::new(glyph_text).style(theme.style(Theme::agent_role(state))),
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
/// display only; navigation never reads it back. The separator is the active
/// theme's host-safe glyph form.
fn location_label(text: ShellText, placement: &PresentationPlacement, separator: &str) -> String {
    let context = placement
        .workspace_label
        .clone()
        .or_else(|| placement.project_label.clone())
        .unwrap_or_else(|| text.board.standalone.to_owned());
    format!(
        "{}{separator}{}",
        sanitize_terminal_text(&context),
        sanitize_terminal_text(&placement.tab_label)
    )
}
