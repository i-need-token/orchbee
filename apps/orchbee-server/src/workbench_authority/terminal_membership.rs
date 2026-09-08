use super::{WorkbenchAuthority, allocate, identity};
use crate::workbench_projection::PreparedProjectionAdvance;
use orchbee_workbench_protocol::{
    ContractError, PanelNode, Presentation, PresentationBinding, Snapshot, Tab, TerminalContext,
    TerminalPresentationId, WorkbenchPanelId, WorkbenchProtocolDocument, WorkbenchRejectionReason,
    WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId, serialize_document,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandaloneIntentError {
    Occupied,
    Rejected(WorkbenchRejectionReason),
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkbenchStandaloneInvariantError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkbenchStandaloneTab {
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    binding: PresentationBinding,
}

impl WorkbenchStandaloneTab {
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
pub struct WorkbenchStandaloneMembership {
    terminal_context_id: WorkbenchTerminalContextId,
    members: Vec<WorkbenchStandaloneTab>,
}

impl WorkbenchStandaloneMembership {
    #[must_use]
    pub fn terminal_context_id(&self) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }

    #[must_use]
    pub fn members(&self) -> &[WorkbenchStandaloneTab] {
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
    pub fn standalone_membership(
        &self,
    ) -> Result<WorkbenchStandaloneMembership, WorkbenchStandaloneInvariantError> {
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
) -> Result<WorkbenchStandaloneMembership, WorkbenchStandaloneInvariantError> {
    let context_id = &snapshot.navigation.standalone_terminal_context_id;
    let mut matching = snapshot
        .terminal_contexts
        .iter()
        .filter(|context| &context.terminal_context_id == context_id);
    let context = matching.next().ok_or(WorkbenchStandaloneInvariantError)?;
    if matching.next().is_some() || !context.groups.is_empty() {
        return Err(WorkbenchStandaloneInvariantError);
    }

    // The projection already validates the protocol's per-context collection bound.
    let mut members = Vec::with_capacity(context.tabs.len());
    for tab in &context.tabs {
        if tab.pinned || tab.group_id.is_some() {
            return Err(WorkbenchStandaloneInvariantError);
        }
        let PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
        } = &tab.root
        else {
            return Err(WorkbenchStandaloneInvariantError);
        };
        if !child_presentations.is_empty() {
            return Err(WorkbenchStandaloneInvariantError);
        }
        let binding = primary_presentation
            .binding
            .clone()
            .ok_or(WorkbenchStandaloneInvariantError)?;
        members.push(WorkbenchStandaloneTab {
            tab_id: tab.tab_id.clone(),
            panel_id: panel_id.clone(),
            presentation_id: primary_presentation.presentation_id.clone(),
            binding,
        });
    }

    Ok(WorkbenchStandaloneMembership {
        terminal_context_id: context_id.clone(),
        members,
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
