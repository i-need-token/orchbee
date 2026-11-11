//! Panel titles and their Agent state come from the exact-scope report observation.
use crate::{terminal_text::sanitize_terminal_text, workbench_client::PanelReportView};
use orchbee_workbench_protocol::{MetadataState, PanelAgentState};

/// The shared Panel name already resolves custom, guarded metadata and automatic precedence.
/// Tokens follow as literal key/value text.
pub(super) fn title(display_label: &str, view: PanelReportView<'_>, separator: &str) -> String {
    let row = match view {
        PanelReportView::Ready(row) => Some(row),
        _ => None,
    };
    let mut title = sanitize_terminal_text(display_label).into_owned();
    for (key, value) in row.iter().flat_map(|row| row.tokens.as_map()) {
        title.push_str(&sanitize_terminal_text(&format!(
            " {separator} {}={}",
            key.as_str(),
            value.as_str()
        )));
    }
    title
}

/// The reported Agent's state. Loading, unavailable and Agent-less rounds show no state.
pub(super) fn agent_state(view: PanelReportView<'_>) -> Option<MetadataState> {
    let PanelReportView::Ready(row) = view else {
        return None;
    };
    let report = row.report.as_ref();
    row.metadata
        .display_agent
        .as_ref()
        .map(|v| v.as_str())
        .or_else(|| report.and_then(|r| r.agent.as_deref()))?;
    Some(
        match report.map_or(PanelAgentState::Unknown, |r| r.display_state()) {
            PanelAgentState::Idle => MetadataState::Idle,
            // Opening shares the working glyph; only its state word differs.
            PanelAgentState::Working | PanelAgentState::Opening => MetadataState::Working,
            PanelAgentState::Done => MetadataState::Done,
            PanelAgentState::Error => MetadataState::Error,
            PanelAgentState::Blocked => MetadataState::Blocked,
            PanelAgentState::Unknown => MetadataState::Unknown,
        },
    )
}

#[cfg(test)]
mod tests;
