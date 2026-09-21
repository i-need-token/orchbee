//! The Agent board overlay: canonical session rows with attention,
//! status, freshness, and resolved Workbench location — plus the hit
//! regions that let pointer presses re-resolve by stable identity.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::Modifier,
    text::Line,
    widgets::{Clear, Paragraph},
};

use crate::{
    localization::{Locale, ShellText},
    terminal_session::{
        AgentBoardDraw, AgentBoardRead, AgentBoardRow, NativeAgentBoardView, PresentationPlacement,
    },
    terminal_text::{sanitize_terminal_text, truncate_display},
    tui_layout::place_overlay,
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::{Role, Theme},
};
use orchbee_agent_observation_protocol::{
    AgentLifecycle, AgentObservationFreshness, AgentSemanticStatus,
};

/// Rows the board reserves outside its scrollable row window: one status
/// or notice line plus the footer hint.
const BOARD_CHROME_ROWS: u16 = 2;
const BOARD_WIDTH: u16 = 92;
const BOARD_MAX_HEIGHT: u16 = 24;

/// Paints the open board inside `area` and records its hit regions in the
/// same pass. Returns the receipt verdict — `presented: false` when the
/// overlay cannot fit, so input capture settles on the painted truth.
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
    let wanted = u16::try_from(rows.len() + 3)
        .unwrap_or(BOARD_MAX_HEIGHT)
        .min(BOARD_MAX_HEIGHT);
    let Some(area) = place_overlay(area, BOARD_WIDTH, wanted.max(8)) else {
        return AgentBoardDraw {
            presented: false,
            viewport_rows: 0,
        };
    };
    frame.render_widget(Clear, area);
    let block = crate::tui_render::overlay_block(text.board.title, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    // A presented board owns the press domain: rows select, the surface
    // swallows and scrolls, and an outside press resolves as backdrop.
    hits.mark_overlay_active();
    hits.record(HitLayer::OverlaySurface, area, HitTarget::AgentBoardSurface);

    let [body, footer] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(BOARD_CHROME_ROWS)]).areas(inner);
    let viewport_rows = usize::from(body.height);
    let read = board.read();
    if rows.is_empty() {
        // No completed-read rows exist — surface the honest read state
        // (unread, in-flight, denied, …) or the "zero sessions" verdict.
        let line = if matches!(read, AgentBoardRead::Ready { .. }) {
            text.board.empty
        } else {
            locale.agent_board_read_line(read)
        };
        frame.render_widget(
            Paragraph::new(line)
                .alignment(Alignment::Center)
                .style(theme.style(Role::TextSecondary)),
            body,
        );
    } else {
        let scroll = board.scroll().min(rows.len().saturating_sub(1));
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
            hits.record(
                HitLayer::Overlay,
                row_area,
                HitTarget::AgentBoardRow(row.session_id().clone()),
            );
            render_row(
                frame,
                row_area,
                text,
                locale,
                board,
                row,
                index + scroll == selection,
                theme,
            );
        }
    }

    let footer_line = board.notice().map_or_else(
        || {
            if board.jumping() {
                Line::styled(text.board.jumping, theme.style(Role::TextSecondary))
            } else {
                Line::styled(text.board.footer, theme.style(Role::HintLabel))
            }
        },
        |failure| {
            Line::styled(
                locale.agent_board_notice(failure),
                theme.style(Role::StatusWarning),
            )
        },
    );
    let footer_lines = [
        Line::styled(
            match read {
                AgentBoardRead::Ready {
                    complete: false, ..
                } => text.board.incomplete,
                AgentBoardRead::InFlight => text.board.in_flight,
                AgentBoardRead::Failed(_) => locale.agent_board_read_line(read),
                AgentBoardRead::Unread | AgentBoardRead::Ready { .. } => "",
            },
            theme.style(Role::TextSecondary),
        ),
        footer_line,
    ];
    frame.render_widget(Paragraph::new(footer_lines.to_vec()), footer);
    AgentBoardDraw {
        presented: true,
        viewport_rows,
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
    theme: &Theme,
) {
    let width = usize::from(area.width);
    let attention = matches!(row.status(), AgentSemanticStatus::NeedsAttention { .. });
    let marker = if attention { "!" } else { " " };
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
    let location = row
        .attachments()
        .iter()
        .find(|attachment| attachment.navigable)
        .and_then(|attachment| board.placement(attachment))
        .map_or_else(String::new, |placement| location_label(text, placement));
    let detail = [
        session.into_owned(),
        provider.into_owned(),
        freshness.to_owned(),
        location,
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");
    let line = truncate_display(
        &format!("{marker} {status} · {detail}"),
        width,
        theme.glyphs.ellipsis,
    );
    let style = if selected {
        theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD)
    } else if ended {
        theme.style(Role::TextSecondary)
    } else {
        theme.style(Role::TextPrimary)
    };
    frame.render_widget(Paragraph::new(line).style(style), area);
}

/// `workspace/project context ▸ tab` for the row's location cell —
/// display only; navigation never reads it back.
fn location_label(text: ShellText, placement: &PresentationPlacement) -> String {
    let context = placement
        .workspace_label
        .clone()
        .or_else(|| placement.project_label.clone())
        .unwrap_or_else(|| text.board.standalone.to_owned());
    format!(
        "{} · {}",
        sanitize_terminal_text(&context),
        sanitize_terminal_text(&placement.tab_label)
    )
}
