//! Main-TUI transient notification state and noncapturing paint; transport stays in subscription.
use crate::{
    client_view::ClientView,
    terminal_session::TerminalSessionView,
    tui_theme::{Role, Theme},
};
use orchbee_workbench_protocol::{
    Notification, NotificationPosition as Position, NotificationSound,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use std::time::{Duration, Instant};
mod subscription;
pub(crate) mod system;
pub(crate) mod terminal;
use orchbee_local_site_runtime::{
    WindowsNotificationDelivery as Delivery, WindowsNotificationSettings,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Effect {
    pub delivery: Delivery,
    pub message: Notification,
}
pub(crate) use subscription::Subscription;
#[derive(Clone, Debug, Eq, PartialEq)]
struct Visible {
    message: Notification,
    until: Instant,
    sound_pending: bool,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct State {
    visible: Option<Visible>,
    policy: WindowsNotificationSettings,
    pending: Option<Effect>,
}
impl State {
    pub(crate) fn configure(&mut self, policy: WindowsNotificationSettings) {
        if self.policy != policy {
            self.visible = None;
            self.pending = None;
            self.policy = policy;
        }
    }
    pub(crate) fn card_delivery(&self) -> bool {
        self.policy.delivery == Delivery::Tui
    }
    pub(crate) fn take_effect(&mut self) -> Option<Effect> {
        self.pending.take()
    }
    pub(crate) fn tick(&mut self, now: Instant) -> bool {
        if self.visible.as_ref().is_some_and(|v| now >= v.until) {
            self.visible = None;
            true
        } else {
            false
        }
    }
    pub(crate) fn empty(&self) -> bool {
        self.visible.is_none() && self.pending.is_none()
    }
    pub(crate) fn install(&mut self, mut message: Notification, now: Instant) -> bool {
        if !self.empty() {
            return false;
        }
        if !self.card_delivery() {
            self.pending = Some(Effect {
                delivery: self.policy.delivery,
                message,
            });
            return true;
        }
        message.position = message.position.or(Some(match self.policy.position() {
            "top-left" => Position::TopLeft,
            "top-right" => Position::TopRight,
            "bottom-left" => Position::BottomLeft,
            _ => Position::BottomRight,
        }));
        self.visible = Some(Visible {
            message,
            until: now + Duration::from_secs(5),
            sound_pending: true,
        });
        true
    }
    pub(crate) fn take_sound_after_draw(&mut self) -> NotificationSound {
        match self.visible.as_mut() {
            Some(v) => {
                if std::mem::take(&mut v.sound_pending) {
                    v.message.sound
                } else {
                    NotificationSound::None
                }
            }
            _ => NotificationSound::None,
        }
    }
}
pub(crate) fn available(view: &ClientView, session: Option<TerminalSessionView<'_>>) -> bool {
    !view.machines.open
        && !view.prefix_active()
        && view.overlay().is_none()
        && view.notice().is_none()
        && matches!(session,
        Some(TerminalSessionView::Native(native)) if native.agent_board().is_none()
        && native.close_confirmation().is_none() && native.panel_close_confirmation().is_none()
        && native.rename_draft().is_none() && native.setup_draft().is_none() && native.notice().is_none())
}
pub(crate) fn sound(kind: NotificationSound) {
    let tone = match kind {
        NotificationSound::None => return,
        NotificationSound::Done => winsafe::co::MBP::INFORMATION,
        NotificationSound::Request => winsafe::co::MBP::EXCLAMATION,
    };
    let _ = winsafe::MessageBeep(tone);
}
pub(crate) fn render(frame: &mut Frame<'_>, state: &State, theme: &Theme) -> bool {
    let Some(visible) = &state.visible else {
        return false;
    };
    let bounds = frame.area();
    // Reserve the persistent status row and one top chrome row.
    let area = Rect::new(
        bounds.x,
        bounds.y.saturating_add(1),
        bounds.width,
        bounds.height.saturating_sub(2),
    );
    if area.width < 8 || area.height < 3 {
        return false;
    }
    let title = crate::cli::escape::escape(&visible.message.title);
    let body = visible
        .message
        .body
        .as_deref()
        .map(crate::cli::escape::escape);
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
        visible.message.position.unwrap_or(Position::BottomRight),
        Position::TopLeft | Position::BottomLeft
    );
    let top = matches!(
        visible.message.position.unwrap_or(Position::BottomRight),
        Position::TopLeft | Position::TopRight
    );
    let rect = Rect::new(
        if left { area.x } else { area.right() - width },
        if top { area.y } else { area.bottom() - height },
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.style(Role::StatusInfo))
        .style(theme.style(Role::SurfaceOverlay));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let clip = |text: &str| {
        crate::terminal_text::truncate_display(
            text,
            usize::from(inner.width),
            theme.glyphs.ellipsis,
        )
    };
    let mut lines = vec![Line::from(Span::styled(
        clip(&title),
        theme.style(Role::TextPrimary),
    ))];
    if let Some(body) = body {
        lines.push(Line::from(Span::styled(
            clip(&body),
            theme.style(Role::TextSecondary),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines).style(theme.style(Role::SurfaceOverlay)),
        inner,
    );
    true
}
#[cfg(test)]
mod tests;
