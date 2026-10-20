//! Resolve explicit destination authority; exported identifiers never route application.
use super::error;
use crate::{
    cli::{Failure, FailureKind, results},
    invocation::{LayoutApplyCommand, LayoutDestination, Resource, TabContext},
};
use orchbee_workbench_protocol::{
    ClientViewRecord, Snapshot, Tab, WorkbenchLabel, WorkbenchTerminalContextId,
};
pub(super) struct Destination {
    pub(super) context: WorkbenchTerminalContextId,
    pub(super) old: Option<Tab>,
    pub(super) label: WorkbenchLabel,
    pub(super) automatic: bool,
    pub(super) focus: bool,
}
pub(super) fn resolve(
    command: &LayoutApplyCommand,
    snapshot: &Snapshot,
    client: Option<&ClientViewRecord>,
) -> Result<Destination, Failure> {
    let selected = client.and_then(|client| client.view.selection.as_ref());
    let (context, old) = match &command.destination {
        LayoutDestination::Replace(target) => {
            let (context, tab) = results::resolve_or_fail(
                snapshot.terminal_contexts.iter().flat_map(|context| {
                    context.tabs.iter().enumerate().map(move |(index, tab)| {
                        (
                            tab.tab_id.as_str(),
                            tab.display_label(index),
                            (context, tab),
                        )
                    })
                }),
                target,
                Resource::Tab,
            )?;
            (context.terminal_context_id.clone(), Some(tab.clone()))
        }
        LayoutDestination::New(context) => {
            let context = match context {
                Some(TabContext::Standalone) => {
                    snapshot.navigation.standalone_terminal_context_id.clone()
                }
                Some(TabContext::Workspace(target)) => results::resolve_or_fail(
                    snapshot.navigation.workspaces.iter().map(|workspace| {
                        (
                            workspace.workspace_id.as_str(),
                            workspace.label.as_str(),
                            workspace,
                        )
                    }),
                    target,
                    Resource::Workspace,
                )?
                .terminal_context_id
                .clone(),
                None => selected
                    .ok_or_else(|| error(FailureKind::NotFound, "destination-unavailable"))?
                    .terminal_context_id
                    .clone(),
            };
            (context, None)
        }
    };
    let destination = snapshot
        .terminal_contexts
        .iter()
        .find(|entry| entry.terminal_context_id == context)
        .ok_or_else(|| error(FailureKind::NotFound, "destination-unavailable"))?;
    let label = command.label.clone().or_else(|| {
        old.as_ref()
            .filter(|tab| !tab.automatic_label)
            .and_then(|tab| WorkbenchLabel::parse(&tab.label).ok())
    });
    let automatic = label.is_none();
    let label = label
        .map(Ok)
        .unwrap_or_else(|| WorkbenchLabel::parse(&(destination.tabs.len() + 1).to_string()))
        .map_err(|_| error(FailureKind::InvalidUsage, "invalid-layout"))?;
    let focus = command.focus
        || old.as_ref().is_some_and(|old| {
            selected.is_some_and(|selection| selection.tab_id.as_ref() == Some(&old.tab_id))
        });
    Ok(Destination {
        context,
        old,
        label,
        automatic,
        focus,
    })
}
