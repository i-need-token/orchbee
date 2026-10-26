//! Source membership and display labels never supply physical execution authority.
use orchbee_workbench_protocol::{
    PanelNode, PluginInvocationContext, Snapshot, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandUnavailableReason as Failure,
};

pub(super) fn resolve(
    snapshot: &Snapshot,
    command: &WorkbenchCommand,
    legacy: PluginInvocationContext,
) -> Result<PluginInvocationContext, Failure> {
    let options = match &command.action {
        WorkbenchAction::TabCreate { options, .. }
        | WorkbenchAction::PanelSplit { options, .. } => options,
        // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
        _ => return Err(Failure::ServiceUnavailable),
    };
    let Some(supplied) = &options.plugin_context else {
        return Ok(legacy);
    };
    observed(snapshot, &command.request_id, supplied)
}

pub(super) fn observed(
    snapshot: &Snapshot,
    request_id: &orchbee_workbench_protocol::WorkbenchRequestId,
    supplied: &PluginInvocationContext,
) -> Result<PluginInvocationContext, Failure> {
    let invalid = Failure::ServiceUnavailable;
    supplied.validate().map_err(|_| invalid)?;
    let mut context = supplied.clone();
    let workspace = context
        .workspace_id
        .as_ref()
        .map(|id| {
            snapshot
                .navigation
                .workspaces
                .iter()
                .find(|w| w.workspace_id.as_str() == id)
                .ok_or(invalid)
        })
        .transpose()?;
    let tab = context
        .tab_id
        .as_ref()
        .map(|id| {
            snapshot
                .terminal_contexts
                .iter()
                .flat_map(|c| c.tabs.iter().map(move |t| (c, t)))
                .find(|(_, t)| t.tab_id.as_str() == id)
                .ok_or(invalid)
        })
        .transpose()?;
    if let Some((parent, _)) = tab
        && workspace.is_some_and(|w| w.terminal_context_id != parent.terminal_context_id)
    {
        return Err(invalid);
    }
    if let Some(id) = &context.focused_pane_id {
        let (_, tab) = tab.ok_or(invalid)?;
        if !contains(&tab.root, id) {
            return Err(invalid);
        }
    } else if context.focused_pane_cwd.is_some()
        || context.focused_pane_agent.is_some()
        || context.focused_pane_status.is_some()
        || context.selected_text.is_some()
    {
        return Err(invalid);
    }
    if context.worktree.is_some() && workspace.is_none() {
        return Err(invalid);
    }
    context.workspace_label = workspace.map(|w| w.label.clone());
    context.tab_label = tab.map(|(_, t)| t.label.clone());
    context.invocation_source = Some("plugin-pane".into());
    context.correlation_id = Some(request_id.to_string());
    Ok(context)
}
fn contains(node: &PanelNode, id: &str) -> bool {
    match node {
        PanelNode::Panel { panel_id, .. } => panel_id.as_str() == id,
        PanelNode::Split { children, .. } => children.iter().any(|slot| contains(&slot.node, id)),
    }
}
