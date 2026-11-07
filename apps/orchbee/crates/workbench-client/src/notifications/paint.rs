//! Noncapturing paint of the one visible notification card.
use crate::{
    client_view::notifications::State,
    tui_hit_map::ProjectedPointer,
    tui_theme::{Role, Theme},
};
use orchbee_workbench_protocol::NotificationPosition as Position;
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

/// Paint the one visible card and record its exact painted extent. An Agent card is one row-like
/// action that opens the Agent it announced; a custom card is a noncapturing hit shield. Either
/// way covered controls cannot hover, arm, or activate through the card, while cells outside it
/// keep their ordinary policy. Returns the painted card rectangle only when this frame drew it.
pub fn render(
    frame: &mut Frame<'_>,
    state: &State,
    theme: &Theme,
    hits: &mut crate::tui_hit_map::HitMapBuilder,
) -> Option<Rect> {
    let message = state.visible_message()?;
    let bounds = frame.area();
    // Keep clear of the top chrome row and the last row, where the status chip floats.
    let area = Rect::new(
        bounds.x,
        bounds.y.saturating_add(1),
        bounds.width,
        bounds.height.saturating_sub(2),
    );
    if area.width < 8 || area.height < 3 {
        return None;
    }
    let title = crate::terminal_text::escape(&message.title);
    let body = message.body.as_deref().map(crate::terminal_text::escape);
    let width = crate::terminal_text::display_width(&title)
        .max(
            body.as_deref()
                .map_or(0, crate::terminal_text::display_width),
        )
        .saturating_add(4)
        .clamp(8, 60)
        .min(usize::from(area.width)) as u16;
    let height = if body.is_some() { 4 } else { 3 }.min(area.height);
    let left = matches!(
        message.position.unwrap_or(Position::BottomRight),
        Position::TopLeft | Position::BottomLeft
    );
    let top = matches!(
        message.position.unwrap_or(Position::BottomRight),
        Position::TopLeft | Position::TopRight
    );
    let rect = Rect::new(
        if left { area.x } else { area.right() - width },
        if top { area.y } else { area.bottom() - height },
        width,
        height,
    );
    let action = state.agent_card().map(|(token, _)| {
        (
            crate::tui_hit_map::InteractionTarget::NotificationCard(token),
            crate::tui_hit_map::InteractionSpec::enabled(
                crate::tui_hit_map::PrimaryContract::ActivateOnRelease,
            ),
        )
    });
    let pointer = action
        .as_ref()
        .map_or(ProjectedPointer::Rest, |(target, spec)| {
            hits.projected_pointer(
                crate::tui_hit_map::HitLayer::Notification,
                rect,
                rect,
                target,
                *spec,
            )
        });
    let surface = match pointer {
        ProjectedPointer::Rest => Role::SurfaceOverlay,
        ProjectedPointer::Hovered => Role::SurfaceHover,
        ProjectedPointer::Pressed => Role::SurfacePressed,
    };
    let title_style = match pointer {
        ProjectedPointer::Pressed => theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD),
        ProjectedPointer::Rest | ProjectedPointer::Hovered => theme.style(Role::TextPrimary),
    };
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.style(Role::StatusInfo))
        .style(theme.style(surface));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let clip = |text: &str| {
        crate::terminal_text::truncate_display(
            text,
            usize::from(inner.width),
            theme.glyphs.ellipsis,
        )
    };
    let mut lines = vec![Line::from(Span::styled(clip(&title), title_style))];
    if let Some(body) = body {
        lines.push(Line::from(Span::styled(
            clip(&body),
            theme.style(Role::TextSecondary),
        )));
    }
    frame.render_widget(Paragraph::new(lines).style(theme.style(surface)), inner);
    match action {
        Some((target, spec)) => hits.record_interaction(
            crate::tui_hit_map::HitLayer::Notification,
            rect,
            rect,
            target,
            spec,
        ),
        None => hits.record(
            crate::tui_hit_map::HitLayer::Notification,
            rect,
            crate::tui_hit_map::HitTarget::OverlaySurface,
        ),
    }
    Some(rect)
}
