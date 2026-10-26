use super::{
    SemanticEvent, SemanticEventData, SemanticEventFilter, SemanticEventKind, SemanticEventMatch,
    SemanticEventScope,
};
#[must_use]
pub fn matches_semantic_filter(event: &SemanticEvent, filter: &SemanticEventFilter) -> bool {
    use SemanticEventData as D;
    if let SemanticEventFilter::AgentStatusChanged { target, state } = filter {
        return event.event == SemanticEventKind::PaneAgentStatusChanged
            && matches!(&event.data, D::Agent { pane, report, .. }
                if pane.panel_id == target.panel_id
                && target.presentation_id.as_ref().is_none_or(|id| id == &pane.presentation_id)
                && state.is_none_or(|state| state == report.state));
    }
    let SemanticEventFilter::Event {
        event: kind,
        scope,
        condition,
    } = filter
    else {
        return false;
    };
    if event.event != *kind {
        return false;
    }
    let context = match &event.data {
        D::Workspace { workspace, .. } => Some(&workspace.terminal_context_id),
        D::Tab {
            terminal_context_id,
            ..
        }
        | D::Layout {
            terminal_context_id,
            ..
        } => Some(terminal_context_id),
        D::Pane { pane }
        | D::Exited { pane, .. }
        | D::Agent { pane, .. }
        | D::Output { pane, .. }
        | D::Scroll { pane, .. } => Some(&pane.terminal_context_id),
        D::Client { view, .. } => view
            .selection
            .as_ref()
            .map(|selection| &selection.terminal_context_id),
        D::WorkspaceOrder { .. } | D::WorkspaceMetadata { .. } | D::Worktree { .. } => None,
    };
    let tab = match &event.data {
        D::Tab { tab_id, .. } | D::Layout { tab_id, .. } => Some(tab_id),
        D::Pane { pane }
        | D::Exited { pane, .. }
        | D::Agent { pane, .. }
        | D::Output { pane, .. }
        | D::Scroll { pane, .. } => Some(&pane.tab_id),
        D::Client { view, .. } => view
            .selection
            .as_ref()
            .and_then(|selection| selection.tab_id.as_ref()),
        D::Workspace { .. }
        | D::WorkspaceOrder { .. }
        | D::WorkspaceMetadata { .. }
        | D::Worktree { .. } => None,
    };
    let pane = match &event.data {
        D::Pane { pane }
        | D::Exited { pane, .. }
        | D::Agent { pane, .. }
        | D::Output { pane, .. }
        | D::Scroll { pane, .. } => Some(&pane.panel_id),
        D::Client { view, .. } => view
            .selection
            .as_ref()
            .and_then(|selection| selection.panel_id.as_ref()),
        D::Workspace { .. }
        | D::WorkspaceOrder { .. }
        | D::WorkspaceMetadata { .. }
        | D::Tab { .. }
        | D::Layout { .. }
        | D::Worktree { .. } => None,
    };
    let scoped = match scope {
        SemanticEventScope::All {} => true,
        SemanticEventScope::Workspace { workspace_id } => {
            event.workspace_id.as_ref() == Some(workspace_id)
                || matches!(&event.data,D::WorkspaceOrder {workspace_ids} if workspace_ids.contains(workspace_id))
        }
        SemanticEventScope::TerminalContext {
            terminal_context_id,
        } => context == Some(terminal_context_id),
        SemanticEventScope::Tab { tab_id } => tab == Some(tab_id),
        SemanticEventScope::Pane { panel_id } => pane == Some(panel_id),
        SemanticEventScope::Client { client_id } => {
            matches!(&event.data,D::Client {client_id:actual,..}|D::Scroll {client_id:actual,..} if actual==client_id)
        }
    };
    scoped
        && match condition {
            SemanticEventMatch::Any {} => true,
            SemanticEventMatch::Label { label } => match &event.data {
                D::Workspace { workspace, .. } => workspace.label == label.as_str(),
                D::Tab { label: actual, .. } => actual == label.as_str(),
                D::Pane { pane } => pane.label == label.as_str(),
                // Only these metadata events expose the mutable label queried by this filter.
                D::WorkspaceOrder { .. }
                | D::WorkspaceMetadata { .. }
                | D::Layout { .. }
                | D::Worktree { .. }
                | D::Client { .. }
                | D::Agent { .. }
                | D::Output { .. }
                | D::Exited { .. }
                | D::Scroll { .. } => false,
            },
            SemanticEventMatch::OutputRevision { minimum_revision } => {
                matches!(&event.data,D::Output {content_revision,..} if content_revision>=minimum_revision)
            }
            SemanticEventMatch::Agent { agent } => {
                matches!(&event.data,D::Agent {report,..} if report.agent.as_ref()==Some(agent))
            }
            SemanticEventMatch::AgentState { state } => {
                matches!(&event.data,D::Agent {report,..} if report.state==*state)
            }
        }
}
