use super::{WorkbenchAuthority, identity};
use orchbee_workbench_protocol::{
    PanelNode, Presentation, PresentationBinding, Snapshot, Tab, TerminalContext,
    TerminalPresentationId, WorkbenchPanelId, WorkbenchTabId, WorkbenchTerminalContextId,
};

pub(super) fn visit_panel_presentations(
    node: &PanelNode,
    visitor: &mut impl FnMut(&WorkbenchPanelId, &Presentation),
) {
    match node {
        PanelNode::Panel {
            right_click: _,
            child_presentations,
            panel_id,
            primary_presentation,
        } => {
            visitor(panel_id, primary_presentation);
            for presentation in child_presentations {
                visitor(panel_id, presentation);
            }
        }
        PanelNode::Split { children, .. } => {
            for child in children.iter() {
                visit_panel_presentations(&child.node, visitor);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkbenchTerminalInvariantError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkbenchTerminalTab {
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    binding: PresentationBinding,
    primary: bool,
}

impl WorkbenchTerminalTab {
    #[must_use]
    pub fn tab_id(&self) -> &WorkbenchTabId {
        &self.tab_id
    }

    #[must_use]
    pub fn panel_id(&self) -> &WorkbenchPanelId {
        &self.panel_id
    }

    #[must_use]
    pub fn presentation_id(&self) -> &TerminalPresentationId {
        &self.presentation_id
    }

    #[must_use]
    pub fn binding(&self) -> &PresentationBinding {
        &self.binding
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkbenchTerminalMembership {
    terminal_context_id: WorkbenchTerminalContextId,
    members: Vec<WorkbenchTerminalTab>,
}

impl WorkbenchTerminalMembership {
    #[must_use]
    pub fn terminal_context_id(&self) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }

    #[must_use]
    pub fn members(&self) -> &[WorkbenchTerminalTab] {
        &self.members
    }

    pub fn members_for_tab(
        &self,
        tab_id: &WorkbenchTabId,
    ) -> impl Iterator<Item = &WorkbenchTerminalTab> {
        self.members
            .iter()
            .filter(move |member| member.tab_id() == tab_id)
    }

    /// Resolves the bound primary Presentation under `panel_id` inside `tab_id`.
    /// An unbound primary never resolves to one of its bound children.
    #[must_use]
    pub fn member_for_panel(
        &self,
        tab_id: &WorkbenchTabId,
        panel_id: &WorkbenchPanelId,
    ) -> Option<&WorkbenchTerminalTab> {
        self.members.iter().find(|member| {
            member.primary && member.tab_id() == tab_id && member.panel_id() == panel_id
        })
    }

    #[must_use]
    /// Whether the context has no bound runtime members; unbound Panels and Tabs may remain.
    pub const fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// The post-publication membership of a removed Workspace Context: the
    /// identity stays addressable for evidence, the member set is canonically
    /// empty because the Context itself left the projection.
    pub(crate) fn removed_workspace(context_id: WorkbenchTerminalContextId) -> Self {
        Self {
            terminal_context_id: context_id,
            members: Vec::new(),
        }
    }
}

impl WorkbenchAuthority {
    /// Complete bound membership for registration, structural operations and Close.
    pub fn presentation_membership(
        &self,
        context_id: &WorkbenchTerminalContextId,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
        presentation_membership(self.projection.snapshot(), context_id)
    }

    pub fn terminal_membership(
        &self,
        context_id: &WorkbenchTerminalContextId,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
        terminal_membership(self.projection.snapshot(), context_id)
    }

    pub fn terminal_tree_membership(
        &self,
        context_id: &WorkbenchTerminalContextId,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
        tree_terminal_membership(self.projection.snapshot(), context_id)
    }

    pub fn retire_terminal_membership(&mut self) {
        self.retired = true;
    }
}

pub(super) struct SimpleTabIds {
    pub(super) tab_id: WorkbenchTabId,
    pub(super) panel_id: WorkbenchPanelId,
    pub(super) presentation_id: TerminalPresentationId,
}

pub(super) fn parse_tab_ids(last_identity: u128) -> Option<SimpleTabIds> {
    Some(SimpleTabIds {
        tab_id: WorkbenchTabId::parse(&identity(last_identity.checked_sub(2)?)).ok()?,
        panel_id: WorkbenchPanelId::parse(&identity(last_identity.checked_sub(1)?)).ok()?,
        presentation_id: TerminalPresentationId::parse(&identity(last_identity)).ok()?,
    })
}

#[cfg(test)]
pub(super) fn append_simple_tab(
    context: &mut TerminalContext,
    ids: &SimpleTabIds,
    label: &str,
    automatic_label: bool,
    binding: PresentationBinding,
) {
    context.tabs.push(orchbee_workbench_protocol::Tab {
        automatic_label,
        tab_id: ids.tab_id.clone(),
        label: label.to_owned(),
        group_id: None,
        pinned: false,
        root: PanelNode::Panel {
            right_click: Default::default(),
            panel_id: ids.panel_id.clone(),
            primary_presentation: Presentation {
                custom_label: None,
                presentation_id: ids.presentation_id.clone(),
                label: label.to_owned(),
                binding: Some(binding),
                launch_failure: None,
            },
            child_presentations: Vec::new(),
        },
    });
}

pub(super) fn terminal_membership(
    snapshot: &Snapshot,
    context_id: &WorkbenchTerminalContextId,
) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
    validate_workspace_context(snapshot, context_id)?;
    let (_context, members) = tree_membership_context(snapshot, context_id)?;
    Ok(WorkbenchTerminalMembership {
        terminal_context_id: context_id.clone(),
        members,
    })
}

pub(super) fn presentation_membership(
    snapshot: &Snapshot,
    context_id: &WorkbenchTerminalContextId,
) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
    validate_workspace_context(snapshot, context_id)?;
    let mut matching = snapshot
        .terminal_contexts
        .iter()
        .filter(|context| &context.terminal_context_id == context_id);
    let context = matching.next().ok_or(WorkbenchTerminalInvariantError)?;
    if matching.next().is_some() {
        return Err(WorkbenchTerminalInvariantError);
    }
    let mut members = Vec::new();
    for tab in &context.tabs {
        collect_tree_members(tab, &tab.root, &mut members, false)?;
    }
    Ok(WorkbenchTerminalMembership {
        terminal_context_id: context_id.clone(),
        members,
    })
}

pub(super) fn tree_terminal_membership(
    snapshot: &Snapshot,
    context_id: &WorkbenchTerminalContextId,
) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
    presentation_membership(snapshot, context_id)
}

fn tree_membership_context<'a>(
    snapshot: &'a Snapshot,
    context_id: &WorkbenchTerminalContextId,
) -> Result<(&'a TerminalContext, Vec<WorkbenchTerminalTab>), WorkbenchTerminalInvariantError> {
    let mut matching = snapshot
        .terminal_contexts
        .iter()
        .filter(|context| &context.terminal_context_id == context_id);
    let context = matching.next().ok_or(WorkbenchTerminalInvariantError)?;
    if matching.next().is_some() || !context.groups.is_empty() {
        return Err(WorkbenchTerminalInvariantError);
    }

    // The projection already validates the protocol's per-context collection bound.
    for tab in &context.tabs {
        if tab.pinned || tab.group_id.is_some() {
            return Err(WorkbenchTerminalInvariantError);
        }
    }
    let mut members = Vec::new();
    for tab in &context.tabs {
        collect_tree_members(tab, &tab.root, &mut members, true)?;
    }
    Ok((context, members))
}

fn validate_workspace_context(
    snapshot: &Snapshot,
    context_id: &WorkbenchTerminalContextId,
) -> Result<(), WorkbenchTerminalInvariantError> {
    if snapshot
        .navigation
        .workspaces
        .iter()
        .any(|workspace| &workspace.terminal_context_id == context_id)
    {
        Ok(())
    } else {
        Err(WorkbenchTerminalInvariantError)
    }
}

fn collect_tree_members(
    tab: &Tab,
    node: &PanelNode,
    members: &mut Vec<WorkbenchTerminalTab>,
    require_binding: bool,
) -> Result<(), WorkbenchTerminalInvariantError> {
    match node {
        PanelNode::Panel {
            right_click: _,
            child_presentations,
            panel_id,
            primary_presentation,
        } => {
            for (index, presentation) in std::iter::once(primary_presentation)
                .chain(child_presentations)
                .enumerate()
            {
                let Some(binding) = &presentation.binding else {
                    if require_binding {
                        return Err(WorkbenchTerminalInvariantError);
                    }
                    continue;
                };
                members.push(WorkbenchTerminalTab {
                    tab_id: tab.tab_id.clone(),
                    panel_id: panel_id.clone(),
                    presentation_id: presentation.presentation_id.clone(),
                    binding: binding.clone(),
                    primary: index == 0,
                });
            }
        }
        PanelNode::Split { children, .. } => {
            for child in children.iter() {
                collect_tree_members(tab, &child.node, members, require_binding)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
