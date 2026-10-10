use super::{WorkbenchAuthority, allocate, identity};
use crate::workbench_projection::PreparedProjectionAdvance;
use orchbee_workbench_protocol::{
    ContractError, PanelNode, Presentation, PresentationBinding, Snapshot, Tab, TerminalContext,
    TerminalPresentationId, WorkbenchPanelId, WorkbenchProtocolDocument, WorkbenchRejectionReason,
    WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId, serialize_document,
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
pub enum StandaloneIntentError {
    Occupied,
    Rejected(WorkbenchRejectionReason),
    Unavailable,
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
    context_kind: ContextKind,
    members: Vec<WorkbenchTerminalTab>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContextKind {
    Standalone,
    Workspace,
}

impl WorkbenchTerminalMembership {
    #[must_use]
    pub const fn is_standalone(&self) -> bool {
        matches!(self.context_kind, ContextKind::Standalone)
    }

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
            context_kind: ContextKind::Workspace,
            members: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandaloneIntentMetadata {
    revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    binding: PresentationBinding,
}

impl StandaloneIntentMetadata {
    #[must_use]
    pub const fn revision(&self) -> WorkbenchRevision {
        self.revision
    }

    #[must_use]
    pub fn terminal_context_id(&self) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }

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

pub struct PreparedStandaloneIntent<'a> {
    authority: &'a mut WorkbenchAuthority,
    projection: PreparedProjectionAdvance,
    last_identity: u128,
    metadata: StandaloneIntentMetadata,
}

impl PreparedStandaloneIntent<'_> {
    #[must_use]
    pub fn metadata(&self) -> &StandaloneIntentMetadata {
        &self.metadata
    }

    #[must_use]
    pub fn commit(self) -> StandaloneIntentMetadata {
        self.authority.projection.commit_prepared(self.projection);
        self.authority.last_identity = self.last_identity;
        self.metadata
    }
}

impl WorkbenchAuthority {
    /// Read-only Open membership includes every retained primary and child Presentation.
    pub fn standalone_presentation_membership(
        &self,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
        self.presentation_membership(&self.snapshot().navigation.standalone_terminal_context_id)
    }

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

    pub fn standalone_membership(
        &self,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
        standalone_membership(self.projection.snapshot())
    }

    pub fn tree_standalone_membership(
        &self,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
        let snapshot = self.projection.snapshot();
        tree_terminal_membership(
            snapshot,
            &snapshot.navigation.standalone_terminal_context_id,
        )
    }

    pub fn retire_standalone_composition(&mut self) {
        self.retired = true;
    }

    /// Prepares Server-issued terminal membership; this is not a Client command or ledger entry.
    pub fn prepare_standalone_intent(
        &mut self,
        binding: PresentationBinding,
    ) -> Result<PreparedStandaloneIntent<'_>, StandaloneIntentError> {
        if self.retired {
            return Err(StandaloneIntentError::Unavailable);
        }
        let membership = match self.standalone_presentation_membership() {
            Ok(membership) => membership,
            Err(_error) => return self.retire_membership(),
        };
        if !membership.is_empty()
            || self.snapshot().terminal_contexts.iter().any(|context| {
                context.terminal_context_id == *membership.terminal_context_id()
                    && !context.tabs.is_empty()
            })
        {
            return Err(StandaloneIntentError::Occupied);
        }

        let mut candidate = self.projection.snapshot().clone();
        let context_id = candidate.navigation.standalone_terminal_context_id.clone();
        let Some(context) = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| context.terminal_context_id == context_id)
        else {
            return self.retire_membership();
        };
        let last_identity = allocate(self.last_identity, 3).map_err(membership_failure)?;
        let revision = self.next_revision().map_err(membership_failure)?;
        let Some(ids) = parse_tab_ids(last_identity) else {
            return self.retire_membership();
        };
        append_simple_tab(context, &ids, "1", true, binding.clone());
        candidate.revision = revision;
        let encoded = match serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)) {
            Ok(encoded) => encoded,
            Err(ContractError::EncodedValueTooLarge { .. }) => {
                return Err(StandaloneIntentError::Rejected(
                    WorkbenchRejectionReason::ResourceLimit,
                ));
            }
            Err(ContractError::InvalidDocument | ContractError::WrongDocumentKind) => {
                return self.retire_membership();
            }
        };
        let projection = match self.projection.prepare_advance(&encoded) {
            Ok(projection) => projection,
            Err(_error) => return self.retire_membership(),
        };
        Ok(PreparedStandaloneIntent {
            authority: self,
            projection,
            last_identity,
            metadata: StandaloneIntentMetadata {
                revision,
                terminal_context_id: context_id,
                tab_id: ids.tab_id,
                panel_id: ids.panel_id,
                presentation_id: ids.presentation_id,
                binding,
            },
        })
    }

    /// Commits Server-issued terminal membership; this is not a Client command or ledger entry.
    pub fn commit_standalone_intent(
        &mut self,
        binding: PresentationBinding,
    ) -> Result<(), StandaloneIntentError> {
        let _metadata = self.prepare_standalone_intent(binding)?.commit();
        Ok(())
    }

    fn retire_membership<T>(&mut self) -> Result<T, StandaloneIntentError> {
        self.retired = true;
        Err(StandaloneIntentError::Unavailable)
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

pub(super) fn append_simple_tab(
    context: &mut TerminalContext,
    ids: &SimpleTabIds,
    label: &str,
    automatic_label: bool,
    binding: PresentationBinding,
) {
    context.tabs.push(Tab {
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
            },
            child_presentations: Vec::new(),
        },
    });
}

pub(super) fn standalone_membership(
    snapshot: &Snapshot,
) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
    terminal_membership(
        snapshot,
        &snapshot.navigation.standalone_terminal_context_id,
    )
}

pub(super) fn terminal_membership(
    snapshot: &Snapshot,
    context_id: &WorkbenchTerminalContextId,
) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
    let context_kind = terminal_context_kind(snapshot, context_id)?;
    let (_context, members) = legacy_single_leaf_context(snapshot, context_id)?;
    Ok(WorkbenchTerminalMembership {
        terminal_context_id: context_id.clone(),
        context_kind,
        members,
    })
}

pub(super) fn presentation_membership(
    snapshot: &Snapshot,
    context_id: &WorkbenchTerminalContextId,
) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
    let context_kind = terminal_context_kind(snapshot, context_id)?;
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
        context_kind,
        members,
    })
}

pub(super) fn tree_terminal_membership(
    snapshot: &Snapshot,
    context_id: &WorkbenchTerminalContextId,
) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
    presentation_membership(snapshot, context_id)
}

fn legacy_single_leaf_context<'a>(
    snapshot: &'a Snapshot,
    context_id: &WorkbenchTerminalContextId,
) -> Result<(&'a TerminalContext, Vec<WorkbenchTerminalTab>), WorkbenchTerminalInvariantError> {
    let (context, members) = tree_membership_context(snapshot, context_id)?;
    if members.len() != context.tabs.len()
        || context.tabs.iter().any(|tab| {
            members
                .iter()
                .filter(|member| member.tab_id() == &tab.tab_id)
                .count()
                != 1
        })
    {
        return Err(WorkbenchTerminalInvariantError);
    }
    Ok((context, members))
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

fn terminal_context_kind(
    snapshot: &Snapshot,
    context_id: &WorkbenchTerminalContextId,
) -> Result<ContextKind, WorkbenchTerminalInvariantError> {
    if context_id == &snapshot.navigation.standalone_terminal_context_id {
        Ok(ContextKind::Standalone)
    } else if snapshot
        .navigation
        .workspaces
        .iter()
        .any(|workspace| &workspace.terminal_context_id == context_id)
    {
        Ok(ContextKind::Workspace)
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

fn membership_failure(failure: super::PlanFailure) -> StandaloneIntentError {
    match failure {
        super::PlanFailure::Rejected(reason) => StandaloneIntentError::Rejected(reason),
        super::PlanFailure::Internal => StandaloneIntentError::Unavailable,
    }
}

#[cfg(test)]
mod tests;
