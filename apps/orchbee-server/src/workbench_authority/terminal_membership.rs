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

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.members.is_empty()
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
    pub fn terminal_membership(
        &self,
        context_id: &WorkbenchTerminalContextId,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
        terminal_membership(self.projection.snapshot(), context_id)
    }

    pub fn standalone_membership(
        &self,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
        standalone_membership(self.projection.snapshot())
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
        let membership = match self.standalone_membership() {
            Ok(membership) => membership,
            Err(_error) => return self.retire_membership(),
        };
        if !membership.is_empty() {
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
        append_simple_tab(context, &ids, "1", binding.clone());
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
    binding: PresentationBinding,
) {
    context.tabs.push(Tab {
        tab_id: ids.tab_id.clone(),
        label: label.to_owned(),
        group_id: None,
        pinned: false,
        root: PanelNode::Panel {
            panel_id: ids.panel_id.clone(),
            primary_presentation: Presentation {
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
    let context_kind = if context_id == &snapshot.navigation.standalone_terminal_context_id {
        ContextKind::Standalone
    } else if snapshot.navigation.projects.iter().any(|project| {
        project
            .workspaces
            .iter()
            .any(|workspace| &workspace.terminal_context_id == context_id)
    }) {
        ContextKind::Workspace
    } else {
        return Err(WorkbenchTerminalInvariantError);
    };
    let (_context, members) = legacy_single_leaf_context(snapshot, context_id)?;
    Ok(WorkbenchTerminalMembership {
        terminal_context_id: context_id.clone(),
        context_kind,
        members,
    })
}

pub(super) fn strict_simple_context<'a>(
    snapshot: &'a Snapshot,
    context_id: &WorkbenchTerminalContextId,
) -> Result<&'a TerminalContext, WorkbenchTerminalInvariantError> {
    let (context, _members) = legacy_single_leaf_context(snapshot, context_id)?;
    Ok(context)
}

fn legacy_single_leaf_context<'a>(
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
    let members = context
        .tabs
        .iter()
        .map(legacy_single_leaf_member)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((context, members))
}

fn legacy_single_leaf_member(
    tab: &Tab,
) -> Result<WorkbenchTerminalTab, WorkbenchTerminalInvariantError> {
    let mut count = 0;
    let mut first = None;
    visit_panel_presentations(&tab.root, &mut |panel_id, presentation| {
        count += 1;
        if first.is_none() {
            first = Some((
                panel_id.clone(),
                presentation.presentation_id.clone(),
                presentation.binding.clone(),
            ));
        }
    });
    if count != 1 {
        return Err(WorkbenchTerminalInvariantError);
    }
    let (panel_id, presentation_id, binding) = first.ok_or(WorkbenchTerminalInvariantError)?;
    Ok(WorkbenchTerminalTab {
        tab_id: tab.tab_id.clone(),
        panel_id,
        presentation_id,
        binding: binding.ok_or(WorkbenchTerminalInvariantError)?,
    })
}

fn membership_failure(failure: super::PlanFailure) -> StandaloneIntentError {
    match failure {
        super::PlanFailure::Rejected(reason) => StandaloneIntentError::Rejected(reason),
        super::PlanFailure::Internal => StandaloneIntentError::Unavailable,
    }
}

#[cfg(test)]
mod tests;
