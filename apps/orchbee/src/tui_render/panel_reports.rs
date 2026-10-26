//! Panel titles and their Agent state come from the exact-scope report observation.
use crate::{terminal_text::sanitize_terminal_text, workbench_client::PanelReportView};
use orchbee_workbench_protocol::{MetadataState, PanelAgentState};

/// Custom titles win over guarded metadata titles, then the creation label. Metadata tokens follow
/// as literal key/value text.
pub(super) fn title(
    custom_label: Option<&str>,
    display_label: &str,
    prefix: &str,
    view: PanelReportView<'_>,
    separator: &str,
) -> String {
    let row = match view {
        PanelReportView::Ready(row) => Some(row),
        _ => None,
    };
    let custom = custom_label
        .or_else(|| row.and_then(|row| row.metadata.title.as_ref().map(|v| v.as_str())));
    let mut title = if let Some(title) = custom {
        sanitize_terminal_text(title).into_owned()
    } else if !display_label.is_empty() && display_label.chars().all(|c| c.is_ascii_digit()) {
        // Creation labels survive layout reordering and sibling removal.
        format!("{prefix} {display_label}")
    } else {
        sanitize_terminal_text(display_label).into_owned()
    };
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
    Some(match report.map_or(PanelAgentState::Unknown, |r| r.state) {
        PanelAgentState::Idle => MetadataState::Idle,
        PanelAgentState::Working => MetadataState::Working,
        PanelAgentState::Done => MetadataState::Done,
        PanelAgentState::Error => MetadataState::Error,
        PanelAgentState::Blocked => MetadataState::Blocked,
        PanelAgentState::Unknown => MetadataState::Unknown,
    })
}

#[cfg(test)]
mod tests;
