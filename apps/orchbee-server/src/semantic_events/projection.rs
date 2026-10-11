//! Pure classification of committed identity, order, membership and layout changes.
use super::{
    BTreeMap, SemanticEventData, SemanticEventKind, Snapshot, State, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, workspace_index,
};
use orchbee_workbench_protocol::{
    PanelNode, SemanticPane, Tab, WorkbenchPanelId, WorkbenchTabId, Workspace,
};
type Workspaces<'a> = BTreeMap<&'a WorkbenchWorkspaceId, &'a Workspace>;
type Tabs<'a> = BTreeMap<&'a WorkbenchTabId, (&'a WorkbenchTerminalContextId, usize, &'a Tab)>;
type Panes<'a> = BTreeMap<&'a WorkbenchPanelId, (SemanticPane, &'a PanelNode, usize)>;
fn indices(snapshot: &Snapshot) -> (Workspaces<'_>, Tabs<'_>, Panes<'_>) {
    let workspaces = snapshot
        .navigation
        .workspaces
        .iter()
        .map(|workspace| (&workspace.workspace_id, workspace))
        .collect();
    let mut tabs = BTreeMap::new();
    let mut panes = BTreeMap::new();
    for context in &snapshot.terminal_contexts {
        for (index, tab) in context.tabs.iter().enumerate() {
            tabs.insert(&tab.tab_id, (&context.terminal_context_id, index, tab));
            let mut ordinal = 0;
            leaves(
                &context.terminal_context_id,
                &tab.tab_id,
                &tab.root,
                &mut ordinal,
                &mut panes,
            );
        }
    }
    (workspaces, tabs, panes)
}
fn leaves<'a>(
    context: &WorkbenchTerminalContextId,
    tab: &WorkbenchTabId,
    node: &'a PanelNode,
    index: &mut usize,
    result: &mut Panes<'a>,
) {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } => {
            result.insert(
                panel_id,
                (
                    SemanticPane {
                        terminal_context_id: context.clone(),
                        tab_id: tab.clone(),
                        panel_id: panel_id.clone(),
                        presentation_id: primary_presentation.presentation_id.clone(),
                        binding: primary_presentation.binding.clone(),
                        label: primary_presentation.display_label().into(),
                    },
                    node,
                    *index,
                ),
            );
            *index += 1;
        }
        PanelNode::Split { children, .. } => {
            for child in &**children {
                leaves(context, tab, &child.node, index, result);
            }
        }
    }
}
pub(super) fn publish(state: &mut State, before: &Snapshot, after: &Snapshot) {
    use SemanticEventData as D;
    use SemanticEventKind as K;
    let (old_workspaces, old_tabs, old_panes) = indices(before);
    let (new_workspaces, new_tabs, new_panes) = indices(after);
    let old_contexts = workspace_index(before);
    let new_contexts = workspace_index(after);
    let old_tab_ranks = ranks(
        old_tabs
            .iter()
            .filter(|(id, (parent, ..))| new_tabs.get(*id).is_some_and(|(next, ..)| next == parent))
            .map(|(id, (parent, index, ..))| ((*parent).clone(), *index, (*id).clone())),
    );
    let new_tab_ranks = ranks(
        new_tabs
            .iter()
            .filter(|(id, (parent, ..))| {
                old_tabs
                    .get(*id)
                    .is_some_and(|(previous, ..)| previous == parent)
            })
            .map(|(id, (parent, index, ..))| ((*parent).clone(), *index, (*id).clone())),
    );
    let old_pane_ranks = ranks(
        old_panes
            .iter()
            .filter(|(id, (pane, ..))| {
                new_panes
                    .get(*id)
                    .is_some_and(|(next, ..)| next.tab_id == pane.tab_id)
            })
            .map(|(id, (pane, _, index))| (pane.tab_id.clone(), *index, (*id).clone())),
    );
    let new_pane_ranks = ranks(
        new_panes
            .iter()
            .filter(|(id, (pane, ..))| {
                old_panes
                    .get(*id)
                    .is_some_and(|(previous, ..)| previous.tab_id == pane.tab_id)
            })
            .map(|(id, (pane, _, index))| (pane.tab_id.clone(), *index, (*id).clone())),
    );
    let old_order = workspace_order(before);
    let new_order = workspace_order(after);
    let old_survivors: Vec<_> = old_order
        .iter()
        .filter(|id| new_workspaces.contains_key(id))
        .collect();
    let new_survivors: Vec<_> = new_order
        .iter()
        .filter(|id| old_workspaces.contains_key(id))
        .collect();
    let old_workspace_ranks: BTreeMap<_, _> = old_survivors
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect();
    let new_workspace_ranks: BTreeMap<_, _> = new_survivors
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect();
    let revision = after.revision;
    for (id, workspace) in &old_workspaces {
        if !new_workspaces.contains_key(id) {
            state.publish(
                revision,
                Some((*id).clone()),
                K::WorkspaceClosed,
                D::Workspace {
                    workspace: (*workspace).clone(),
                },
            );
        }
    }
    for (id, workspace) in &new_workspaces {
        let data = || D::Workspace {
            workspace: (*workspace).clone(),
        };
        match old_workspaces.get(id) {
            None => {
                state.publish(revision, Some((*id).clone()), K::WorkspaceCreated, data());
            }
            Some(old) => {
                if old.label != workspace.label {
                    state.publish(revision, Some((*id).clone()), K::WorkspaceRenamed, data());
                }
                if old.project_id != workspace.project_id
                    || old_workspace_ranks.get(id) != new_workspace_ranks.get(id)
                {
                    state.publish(revision, Some((*id).clone()), K::WorkspaceMoved, data());
                }
                if old != workspace || old.project_id != workspace.project_id {
                    state.publish(revision, Some((*id).clone()), K::WorkspaceUpdated, data());
                }
            }
        }
    }
    if old_survivors != new_survivors {
        state.publish(
            revision,
            None,
            K::WorkspaceReordered,
            D::WorkspaceOrder {
                workspace_ids: new_order.clone(),
            },
        );
    }
    for (id, (context, index, tab)) in &old_tabs {
        if !new_tabs.contains_key(id) {
            state.publish(
                revision,
                old_contexts.get(*context).cloned(),
                K::TabClosed,
                tab_data(context, *index, tab),
            );
        }
    }
    for (id, (context, index, tab)) in &new_tabs {
        let workspace = new_contexts.get(*context).cloned();
        match old_tabs.get(id) {
            None => {
                state.publish(
                    revision,
                    workspace.clone(),
                    K::TabCreated,
                    tab_data(context, *index, tab),
                );
            }
            Some((old_context, _, old)) => {
                if old.label != tab.label {
                    state.publish(
                        revision,
                        workspace.clone(),
                        K::TabRenamed,
                        tab_data(context, *index, tab),
                    );
                }
                if old_context != context || old_tab_ranks.get(*id) != new_tab_ranks.get(*id) {
                    state.publish(
                        revision,
                        workspace.clone(),
                        K::TabMoved,
                        tab_data(context, *index, tab),
                    );
                }
            }
        }
        if old_tabs
            .get(id)
            .is_none_or(|(_, _, old)| !same_layout(&old.root, &tab.root))
        {
            state.publish(
                revision,
                workspace,
                K::LayoutUpdated,
                D::Layout {
                    terminal_context_id: (*context).clone(),
                    tab_id: (*id).clone(),
                    root: Box::new(tab.root.clone()),
                },
            );
        }
    }
    for (id, (pane, _, _)) in &old_panes {
        if !new_panes.contains_key(id) {
            state.publish(
                revision,
                old_contexts.get(&pane.terminal_context_id).cloned(),
                K::PaneClosed,
                D::Pane { pane: pane.clone() },
            );
        }
    }
    for (id, (pane, node, _)) in &new_panes {
        let workspace = new_contexts.get(&pane.terminal_context_id).cloned();
        match old_panes.get(id) {
            None => {
                state.publish(
                    revision,
                    workspace,
                    K::PaneCreated,
                    D::Pane { pane: pane.clone() },
                );
            }
            Some((old, old_node, _)) => {
                if old.terminal_context_id != pane.terminal_context_id
                    || old.tab_id != pane.tab_id
                    || old_pane_ranks.get(*id) != new_pane_ranks.get(*id)
                {
                    state.publish(
                        revision,
                        workspace.clone(),
                        K::PaneMoved,
                        D::Pane { pane: pane.clone() },
                    );
                }
                if old_node != node {
                    state.publish(
                        revision,
                        workspace,
                        K::PaneUpdated,
                        D::Pane { pane: pane.clone() },
                    );
                }
            }
        }
    }
}
fn tab_data(context: &WorkbenchTerminalContextId, index: usize, tab: &Tab) -> SemanticEventData {
    SemanticEventData::Tab {
        terminal_context_id: context.clone(),
        tab_id: tab.tab_id.clone(),
        label: tab.label.clone(),
        index: u32::try_from(index).expect("canonical Tab bound"),
    }
}
fn ranks<K: Ord, I: Ord>(values: impl Iterator<Item = (K, usize, I)>) -> BTreeMap<I, usize> {
    let mut groups: BTreeMap<K, Vec<(usize, I)>> = BTreeMap::new();
    for (parent, index, id) in values {
        groups.entry(parent).or_default().push((index, id));
    }
    let mut result = BTreeMap::new();
    for mut members in groups.into_values() {
        members.sort_by_key(|(index, _)| *index);
        for (index, (_, id)) in members.into_iter().enumerate() {
            result.insert(id, index);
        }
    }
    result
}
fn workspace_order(snapshot: &Snapshot) -> Vec<WorkbenchWorkspaceId> {
    if snapshot.navigation.workspace_order.is_empty() {
        snapshot
            .navigation
            .workspaces
            .iter()
            .map(|workspace| workspace.workspace_id.clone())
            .collect()
    } else {
        snapshot.navigation.workspace_order.clone()
    }
}
fn same_layout(left: &PanelNode, right: &PanelNode) -> bool {
    match (left, right) {
        (
            PanelNode::Panel { panel_id: left, .. },
            PanelNode::Panel {
                panel_id: right, ..
            },
        ) => left == right,
        (
            PanelNode::Split {
                split_id: left_id,
                orientation: left_axis,
                children: left,
                ratio: left_ratio,
            },
            PanelNode::Split {
                split_id: right_id,
                orientation: right_axis,
                children: right,
                ratio: right_ratio,
            },
        ) => {
            left_id == right_id
                && left_axis == right_axis
                && left_ratio == right_ratio
                && left.iter().zip(right.iter()).all(|(left, right)| {
                    left.slot_id == right.slot_id && same_layout(&left.node, &right.node)
                })
        }
        _ => false,
    }
}
