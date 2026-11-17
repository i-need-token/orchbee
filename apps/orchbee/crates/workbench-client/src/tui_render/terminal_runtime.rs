//! The rows across the top of each running terminal while the Session's Server replaces a lost
//! Worker or once it gave up, painted over the terminal's last frame as the window paints its
//! status row: the state after the Tab's label, then, while reconnecting, what it means for the
//! programs and for what the person types. Once the Server gave up, the status line explains.
use crate::localization::Locale;
use crate::tui_theme::{Role, Theme};
use crate::workbench_client::{ClientPanel, WorkbenchClientView};
use orchbee_workbench_protocol::TerminalRuntimeState;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Text},
    widgets::{Clear, Paragraph},
};

/// Paints the rows over `content` when `panel` holds a running terminal and the Server does not
/// serve; `state` is the phase status, which names the runtime state then.
pub(super) fn render<'a>(
    frame: &mut Frame<'_>,
    content: Rect,
    panel: Option<&'a ClientPanel>,
    native: &WorkbenchClientView<'a>,
    state: &str,
    locale: Locale,
    theme: &Theme,
) {
    let (role, description) = match native.terminal_runtime() {
        TerminalRuntimeState::Serving => return,
        TerminalRuntimeState::Reconnecting => (
            Role::StatusWarning,
            Some(locale.terminal_reconnecting_description()),
        ),
        TerminalRuntimeState::GivenUp => (Role::StatusError, None),
    };
    let Some(panel) = panel else {
        return;
    };
    // A Panel without a running terminal shows what it shows today: nothing there reaches a
    // Worker.
    if native.retirement().is_some()
        || native.panel_is_unbound(panel.panel_id())
        || native.held_restore(panel).is_some()
        || native.launch_failure(panel).is_some()
        || native.terminal_end(panel.panel_id()).is_some()
    {
        return;
    }
    let mut lines = vec![Line::styled(state.to_owned(), theme.style(role))];
    if let Some(description) = description {
        lines.push(Line::styled(
            description.to_owned(),
            theme.style(Role::TextSecondary),
        ));
    }
    let text = crate::terminal_text::wrap_text(&Text::from(lines), content.width);
    let rows = u16::try_from(text.lines.len()).unwrap_or(u16::MAX);
    let area = Rect {
        height: rows.min(content.height),
        ..content
    };
    if area.is_empty() {
        return;
    }
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(text).style(theme.style(Role::SurfaceOverlay)),
        area,
    );
}
