//! Verifies creation against its original full canonical baseline, including placement and fresh IDs.
//! The binding and its launched program are Server-chosen, so replay takes both from the candidate.
use super::{
    ClientContextTabs, ClientProjectionDivergence, CreatedTabExpectation,
    ValidatedCreatedProjection, validate_canonical_candidate,
};
use orchbee_workbench_protocol::{
    NewTerminalTab, PanelNode, TabCreationPlacement, create_terminal_tab,
};

pub(in crate::workbench_client) fn validate_created_tab(
    current: &ClientContextTabs,
    expected: &CreatedTabExpectation,
    candidate: ClientContextTabs,
) -> Result<ValidatedCreatedProjection, ClientProjectionDivergence> {
    validate_canonical_candidate(current, &candidate)?;
    if current.revision.value().checked_add(1) != Some(expected.committed_revision.value())
        || candidate.revision < expected.committed_revision
        || current.terminal_context_id() != Some(&expected.terminal_context_id)
        || candidate.terminal_context_id() != Some(&expected.terminal_context_id)
        || current
            .canonical_snapshot()
            .terminal_contexts
            .iter()
            .any(|context| {
                context.tabs.iter().any(|tab| {
                    tab.tab_id == expected.tab_id || contains_identity(&tab.root, expected)
                })
            })
    {
        return Err(ClientProjectionDivergence);
    }
    let created_index = candidate.tab_index(&expected.tab_id);
    if candidate.revision == expected.committed_revision {
        let index = created_index.ok_or(ClientProjectionDivergence)?;
        let created = &candidate
            .canonical_context()
            .ok_or(ClientProjectionDivergence)?
            .tabs[index];
        let PanelNode::Panel {
            primary_presentation,
            ..
        } = &created.root
        else {
            return Err(ClientProjectionDivergence);
        };
        let new_group = matches!(expected.placement, Some(TabCreationPlacement::NewGroup {}))
            .then_some(created.group_id.as_ref())
            .flatten();
        if new_group.is_some_and(|id| {
            current
                .canonical_snapshot()
                .terminal_contexts
                .iter()
                .any(|context| context.groups.iter().any(|group| &group.group_id == id))
        }) {
            return Err(ClientProjectionDivergence);
        }
        let context = create_terminal_tab(
            current
                .canonical_context()
                .ok_or(ClientProjectionDivergence)?,
            expected.placement.as_ref(),
            NewTerminalTab {
                tab_id: expected.tab_id.clone(),
                panel_id: expected.panel_id.clone(),
                presentation_id: expected.presentation_id.clone(),
                label: expected.requested_label.clone(),
                automatic_label: expected.automatic_label,
                binding: primary_presentation
                    .binding
                    .clone()
                    .ok_or(ClientProjectionDivergence)?,
                launch_program: primary_presentation.launch_program.clone(),
            },
            new_group,
        )
        .map_err(|_| ClientProjectionDivergence)?;
        let mut snapshot = current.canonical_snapshot().clone();
        snapshot.revision = expected.committed_revision;
        snapshot.terminal_contexts[current
            .canonical_context
            .ok_or(ClientProjectionDivergence)?] = context;
        let exact = current
            .project_snapshot(&snapshot)
            .map_err(|_| ClientProjectionDivergence)?;
        if candidate != exact {
            return Err(ClientProjectionDivergence);
        }
    } else if let Some(index) = created_index {
        let created = &candidate.tabs[index];
        let label = match &expected.placement {
            Some(TabCreationPlacement::Duplicate { tab_id }) => current
                .tabs
                .iter()
                .find(|tab| tab.tab_id() == tab_id)
                .ok_or(ClientProjectionDivergence)?
                .tab_label(),
            // Only explicit relative placements require the anchor postcondition checked here.
            _ => &expected.requested_label,
        };
        if created.panel_id != expected.panel_id
            || created.presentation_id != expected.presentation_id
            || &created.presentation_label != label
        {
            return Err(ClientProjectionDivergence);
        }
    } else if candidate
        .canonical_snapshot()
        .terminal_contexts
        .iter()
        .any(|context| {
            context
                .tabs
                .iter()
                .any(|tab| contains_identity(&tab.root, expected))
        })
    {
        return Err(ClientProjectionDivergence);
    }
    Ok(ValidatedCreatedProjection {
        candidate,
        created_index,
    })
}

fn contains_identity(node: &PanelNode, expected: &CreatedTabExpectation) -> bool {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
            ..
        } => {
            panel_id == &expected.panel_id
                || primary_presentation.presentation_id == expected.presentation_id
                || child_presentations
                    .iter()
                    .any(|child| child.presentation_id == expected.presentation_id)
        }
        PanelNode::Split { children, .. } => children
            .iter()
            .any(|child| contains_identity(&child.node, expected)),
    }
}
