//! Keep status readable beside literal, bounded Panel metadata without consuming terminal rows.
use crate::{
    localization::Locale,
    terminal_session::PanelReportView,
    terminal_text::{display_width, sanitize_terminal_text, truncate_display},
    tui_theme::{Role, Theme},
};
use orchbee_workbench_protocol::{MetadataState, PanelAgentState};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

pub(super) fn title(
    panel: &crate::terminal_session::NativePresentation,
    ordinal: usize,
    prefix: &str,
    view: PanelReportView<'_>,
    theme: &Theme,
) -> String {
    let title = panel.custom_label().or_else(|| match view {
        PanelReportView::Ready(row) => row.metadata.title.as_ref().map(|v| v.as_str()),
        _ => None,
    });
    if let Some(title) = title {
        format!(
            " {} {} {} ",
            sanitize_terminal_text(title),
            theme.glyphs.middle_dot,
            ordinal
        )
    } else {
        format!(
            " {prefix} {ordinal} {} {} ",
            theme.glyphs.middle_dot,
            sanitize_terminal_text(panel.display_label())
        )
    }
}

pub(super) fn header(
    mut title: String,
    view: PanelReportView<'_>,
    width: usize,
    locale: Locale,
    theme: &Theme,
    title_style: Style,
    unread: bool,
) -> Line<'static> {
    let mut symbol = None;
    let (state, agent, role) = match view {
        PanelReportView::Pending => (
            locale.panel_report_loading().to_owned(),
            None,
            Role::StatusPending,
        ),
        PanelReportView::Unavailable => (
            locale.panel_report_unavailable().to_owned(),
            None,
            Role::StatusWarning,
        ),
        PanelReportView::Ready(row) => {
            for (key, value) in row.tokens.as_map() {
                title.push_str(&format!(
                    " {} {}={}",
                    theme.glyphs.middle_dot,
                    key.as_str(),
                    value.as_str()
                ));
            }
            let report = row.report.as_ref();
            let agent = row
                .metadata
                .display_agent
                .as_ref()
                .map(|v| v.as_str())
                .or_else(|| report.and_then(|r| r.agent.as_deref()));
            if agent.is_none() {
                return Line::from(Span::styled(
                    truncate_display(
                        &sanitize_terminal_text(&title),
                        width,
                        theme.glyphs.ellipsis,
                    ),
                    title_style,
                ));
            }
            let status = report.map_or(PanelAgentState::Unknown, |r| r.state);
            let (label, role) = match status {
                PanelAgentState::Idle => (MetadataState::Idle, Role::TextSecondary),
                PanelAgentState::Working => (MetadataState::Working, Role::StatusWorking),
                PanelAgentState::Done => (MetadataState::Done, Role::StatusSuccess),
                PanelAgentState::Error => (MetadataState::Error, Role::StatusError),
                PanelAgentState::Blocked => (MetadataState::Blocked, Role::StatusWarning),
                PanelAgentState::Unknown => (MetadataState::Unknown, Role::TextSecondary),
            };
            symbol = Some(super::sidebar_text::state_glyph(label, theme));
            (
                row.metadata.state_labels.get(&label).map_or_else(
                    || locale.panel_report_state(status).to_owned(),
                    |v| v.as_str().to_owned(),
                ),
                agent,
                role,
            )
        }
    };
    let prefix_width = if symbol.is_some() { width.min(2) } else { 0 };
    let unread_width = if symbol.is_some() && width >= 4 { 2 } else { 0 };
    let width = width.saturating_sub(prefix_width + unread_width);
    let badge_width = width / 2;
    let state = truncate_display(
        &sanitize_terminal_text(&state),
        badge_width,
        theme.glyphs.ellipsis,
    );
    let state_width = display_width(&state);
    let suffix = agent.map_or_else(String::new, |agent| {
        format!(
            " {} {}",
            theme.glyphs.middle_dot,
            sanitize_terminal_text(agent)
        )
    });
    let suffix = truncate_display(
        &suffix,
        badge_width.saturating_sub(state_width),
        theme.glyphs.ellipsis,
    );
    let used = state_width + display_width(&suffix);
    let gap = usize::from(used > 0 && width > used);
    let title = truncate_display(
        &sanitize_terminal_text(&title),
        width.saturating_sub(used + gap),
        theme.glyphs.ellipsis,
    );
    let padding = width.saturating_sub(display_width(&title) + used);
    let mut spans = vec![
        Span::styled(title, title_style),
        Span::raw(" ".repeat(padding)),
        Span::styled(state, theme.style(role)),
        Span::styled(suffix, theme.style(Role::TextSecondary)),
    ];
    if let Some(symbol) = symbol {
        spans.insert(
            0,
            Span::styled(
                truncate_display(&format!("{symbol} "), prefix_width, ""),
                theme.style(role),
            ),
        );
    }
    if unread_width > 0 {
        spans.push(Span::styled(
            if unread {
                format!(" {}", theme.glyphs.status_bullet)
            } else {
                "  ".into()
            },
            theme.style(Role::StatusUnread),
        ));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests;
