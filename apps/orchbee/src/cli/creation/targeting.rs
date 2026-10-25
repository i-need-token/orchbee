//! Pure preflight resolution and post-creation selection guards.
use super::canonical;
use crate::{
    cli::{Failure, FailureKind, results},
    invocation::{CreationCommand, CreationKind, MutationCommand, Resource, TabContext, Target},
};
use orchbee_workbench_protocol::{
    ClientViewRecord, ClientViewSelection, Snapshot, WorkbenchLabel, WorkbenchPanelId,
    WorkbenchTabId, WorkbenchTerminalContextId,
};

pub(super) fn needs_source(command: &CreationCommand) -> bool {
    matches!(
        command.kind,
        CreationKind::Tab { context: None } | CreationKind::Split { target: None, .. }
    )
}

pub(super) fn resolve(
    command: &CreationCommand,
    snapshot: &Snapshot,
    caller: Option<&Target>,
    client: Option<&ClientViewRecord>,
) -> Result<MutationCommand, Failure> {
    let selected = client.and_then(|client| client.view.selection.as_ref());
    let current_panel = selected
        .and_then(|selected| selected.panel_id.as_ref())
        .map(|panel| Target::Id(panel.to_string()));
    let fallback = caller.or(current_panel.as_ref());
    let label = |count: usize| {
        command.label.clone().map(Ok).unwrap_or_else(|| {
            WorkbenchLabel::parse(&(count + 1).to_string())
                .map_err(|_| Failure::new(FailureKind::InternalFailure))
        })
    };
    match &command.kind {
        CreationKind::Tab { context } => {
            let context_id = match context {
                Some(TabContext::Workspace(target)) => {
                    let workspace = results::resolve_or_fail(
                        snapshot.navigation.workspaces.iter().map(|workspace| {
                            (
                                workspace.workspace_id.as_str(),
                                workspace.label.as_str(),
                                workspace,
                            )
                        }),
                        target,
                        Resource::Workspace,
                    )?;
                    &workspace.terminal_context_id
                }
                None => {
                    if let Some(caller) = caller {
                        panel(snapshot, caller)?.1
                    } else {
                        &selected.ok_or_else(unavailable)?.terminal_context_id
                    }
                }
            };
            let context = snapshot
                .terminal_contexts
                .iter()
                .find(|context| &context.terminal_context_id == context_id)
                .ok_or_else(unavailable)?;
            let workspace = snapshot
                .navigation
                .workspaces
                .iter()
                .find(|workspace| &workspace.terminal_context_id == context_id)
                .ok_or_else(unavailable)?;
            let target_context =
                TabContext::Workspace(Target::Id(workspace.workspace_id.to_string()));
            Ok(canonical(
                command,
                Some(target_context),
                None,
                label(context.tabs.len())?,
            ))
        }
        CreationKind::Split { target, .. } => {
            let target = target.as_ref().or(fallback).ok_or_else(unavailable)?;
            let (panel_id, _, tab) = panel(snapshot, target)?;
            let count = results::panel_entries(snapshot)
                .iter()
                .filter(|(_, _, candidate)| candidate.tab_id == *tab)
                .count();
            Ok(canonical(
                command,
                None,
                Some(Target::Id(panel_id.to_string())),
                label(count)?,
            ))
        }
    }
}

fn unavailable() -> Failure {
    Failure::new(FailureKind::NotFound)
}

pub(super) fn panel<'a>(
    snapshot: &'a Snapshot,
    target: &Target,
) -> Result<
    (
        &'a WorkbenchPanelId,
        &'a WorkbenchTerminalContextId,
        &'a WorkbenchTabId,
    ),
    Failure,
> {
    results::resolve_or_fail(
        results::panel_entries(snapshot)
            .into_iter()
            .map(|(panel, context, tab)| {
                (
                    panel.as_str(),
                    "",
                    (panel, &context.terminal_context_id, &tab.tab_id),
                )
            }),
        target,
        Resource::Panel,
    )
}

pub(super) struct Created {
    pub(super) context: WorkbenchTerminalContextId,
    pub(super) tab: WorkbenchTabId,
    pub(super) panel: WorkbenchPanelId,
    pub(super) split: bool,
}

/// Empty views have no terminal poll to carry publication hints. An unchanged
/// empty view can let the pinned Client command drive its own canonical refresh.
pub(super) fn unchanged_empty_selection(
    before: &Option<ClientViewSelection>,
    after: &Option<ClientViewSelection>,
) -> bool {
    before == after
        && before.as_ref().is_some_and(|selection| {
            selection.tab_id.is_none()
                && selection.panel_id.is_none()
                && selection.panel_count == 0
                && !selection.zoomed
        })
}

pub(super) fn unchanged_selection(
    before: &Option<ClientViewSelection>,
    after: &Option<ClientViewSelection>,
    created: &Created,
) -> bool {
    let (Some(before), Some(after)) = (before, after) else {
        return before == after;
    };
    // An empty destination's first Tab may become visible during normal canonical refresh.
    if !created.split && before.terminal_context_id == created.context && before.tab_id.is_none() {
        return before == after
            || (after.terminal_context_id == created.context
                && after.tab_id.as_ref() == Some(&created.tab)
                && after.panel_id.as_ref() == Some(&created.panel)
                && !after.zoomed
                && after.panel_count == 1);
    }
    let expected_count = if created.split
        && before.terminal_context_id == created.context
        && before.tab_id.as_ref() == Some(&created.tab)
    {
        before.panel_count.checked_add(1)
    } else {
        Some(before.panel_count)
    };
    before.terminal_context_id == after.terminal_context_id
        && before.tab_id == after.tab_id
        && before.panel_id == after.panel_id
        && before.zoomed == after.zoomed
        && Some(after.panel_count) == expected_count
}

#[cfg(test)]
mod tests;
