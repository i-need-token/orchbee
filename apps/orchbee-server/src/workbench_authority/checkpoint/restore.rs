use std::{collections::BTreeMap, fmt};

use orchbee_workbench_protocol::{
    ContractError, PanelNode, Presentation, PresentationBinding, SiteId, Snapshot,
    TerminalPresentationId, WorkbenchDirectoryPath, WorkbenchPanelId, WorkbenchProtocolDocument,
    WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId, WorkerId, serialize_document,
};

use super::WorkbenchAuthority;
use crate::workbench_projection::{PreparedProjectionAdvance, ProjectionAdvanceError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkbenchRestoreError {
    Unavailable,
    ForeignWorker,
    TargetChanged,
    AlreadyBound,
    RevisionExhausted,
    InvalidProjection(ContractError),
}

impl fmt::Display for WorkbenchRestoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("Workbench restoration is unavailable"),
            Self::ForeignWorker => {
                formatter.write_str("restore target belongs to a different Worker")
            }
            Self::TargetChanged => {
                formatter.write_str("restore target membership or directory changed")
            }
            Self::AlreadyBound => {
                formatter.write_str("restore target already has a terminal binding")
            }
            Self::RevisionExhausted => formatter.write_str("Workbench revision is exhausted"),
            Self::InvalidProjection(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WorkbenchRestoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidProjection(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkbenchRestoreTarget {
    site_id: SiteId,
    local_worker: WorkerId,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    working_directory: Option<WorkbenchDirectoryPath>,
    restored_directory: Option<WorkbenchDirectoryPath>,
    agent_session: Option<orchbee_workbench_protocol::PanelAgentSession>,
}

impl WorkbenchRestoreTarget {
    #[must_use]
    pub fn agent_session(&self) -> Option<&orchbee_workbench_protocol::PanelAgentSession> {
        self.agent_session.as_ref()
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
    pub fn working_directory(&self) -> Option<&WorkbenchDirectoryPath> {
        self.restored_directory
            .as_ref()
            .or(self.working_directory.as_ref())
    }
}

pub struct PreparedWorkbenchRestoreBinding<'authority> {
    authority: &'authority mut WorkbenchAuthority,
    advance: PreparedProjectionAdvance,
    revision: WorkbenchRevision,
}

impl PreparedWorkbenchRestoreBinding<'_> {
    pub fn commit(self) -> WorkbenchRevision {
        self.authority.projection.commit_prepared(self.advance);
        self.revision
    }
}

impl WorkbenchAuthority {
    /// Suppress a duplicate resume only while its exact restore target is still unbound.
    /// No published topology or live report is changed.
    pub fn suppress_duplicate_restore_reference(
        &mut self,
        target: &mut WorkbenchRestoreTarget,
    ) -> Result<(), WorkbenchRestoreError> {
        if !self
            .restoration_targets(&target.local_worker)?
            .contains(target)
        {
            return Err(WorkbenchRestoreError::TargetChanged);
        }
        self.pending_agent_sessions.remove(&target.presentation_id);
        target.agent_session = None;
        Ok(())
    }

    pub fn restoration_targets(
        &self,
        local_worker: &WorkerId,
    ) -> Result<Vec<WorkbenchRestoreTarget>, WorkbenchRestoreError> {
        if self.retired {
            return Err(WorkbenchRestoreError::Unavailable);
        }
        let snapshot = self.snapshot();
        let directories = context_directories(snapshot, local_worker)?;
        let mut targets = Vec::new();
        for context in &snapshot.terminal_contexts {
            let working_directory = directories
                .get(&context.terminal_context_id)
                .ok_or(WorkbenchRestoreError::TargetChanged)?;
            for tab in &context.tabs {
                super::super::terminal_membership::visit_panel_presentations(
                    &tab.root,
                    &mut |panel_id, presentation| {
                        if presentation.binding.is_none() {
                            targets.push(WorkbenchRestoreTarget {
                                site_id: snapshot.site_id.clone(),
                                local_worker: local_worker.clone(),
                                terminal_context_id: context.terminal_context_id.clone(),
                                tab_id: tab.tab_id.clone(),
                                panel_id: panel_id.clone(),
                                presentation_id: presentation.presentation_id.clone(),
                                working_directory: working_directory.clone(),
                                restored_directory: self
                                    .restart_directories
                                    .get(&presentation.presentation_id)
                                    .cloned(),
                                agent_session: self
                                    .pending_agent_sessions
                                    .get(&presentation.presentation_id)
                                    .cloned(),
                            });
                        }
                    },
                );
            }
        }
        Ok(targets)
    }

    /// The runtime composes this effect-free projection preparation with a fresh terminal
    /// reservation; dispatch happens only after both owners have committed under one lock.
    pub fn prepare_restored_terminal(
        &mut self,
        target: &WorkbenchRestoreTarget,
        binding: PresentationBinding,
    ) -> Result<PreparedWorkbenchRestoreBinding<'_>, WorkbenchRestoreError> {
        if self.retired {
            return Err(WorkbenchRestoreError::Unavailable);
        }
        if self.site_id() != &target.site_id {
            return Err(WorkbenchRestoreError::TargetChanged);
        }
        if binding.worker_id != target.local_worker {
            return Err(WorkbenchRestoreError::ForeignWorker);
        }
        if self.pending_agent_sessions.get(&target.presentation_id) != target.agent_session.as_ref()
        {
            return Err(WorkbenchRestoreError::TargetChanged);
        }
        if self.restart_directories.get(&target.presentation_id)
            != target.restored_directory.as_ref()
        {
            return Err(WorkbenchRestoreError::TargetChanged);
        }
        let current = self.snapshot();
        let directories = context_directories(current, &target.local_worker)?;
        if directories.get(&target.terminal_context_id) != Some(&target.working_directory) {
            return Err(WorkbenchRestoreError::TargetChanged);
        }
        let mut candidate = current.clone();
        let tab = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| context.terminal_context_id == target.terminal_context_id)
            .and_then(|context| {
                context
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.tab_id == target.tab_id)
            })
            .ok_or(WorkbenchRestoreError::TargetChanged)?;
        let presentation =
            find_presentation(&mut tab.root, target).ok_or(WorkbenchRestoreError::TargetChanged)?;
        if presentation.binding.is_some() {
            return Err(WorkbenchRestoreError::AlreadyBound);
        }
        presentation.binding = Some(binding);
        let revision = WorkbenchRevision::new(
            current
                .revision
                .value()
                .checked_add(1)
                .ok_or(WorkbenchRestoreError::RevisionExhausted)?,
        );
        candidate.revision = revision;
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate))
            .map_err(WorkbenchRestoreError::InvalidProjection)?;
        let advance = self.projection.prepare_advance(&encoded).map_err(|error| {
            WorkbenchRestoreError::InvalidProjection(match error {
                ProjectionAdvanceError::Contract(error) => error,
                ProjectionAdvanceError::SiteMismatch
                | ProjectionAdvanceError::NonIncreasingRevision
                | ProjectionAdvanceError::PublicationInvariant => ContractError::InvalidDocument,
            })
        })?;
        Ok(PreparedWorkbenchRestoreBinding {
            authority: self,
            advance,
            revision,
        })
    }
}

fn context_directories(
    snapshot: &Snapshot,
    local_worker: &WorkerId,
) -> Result<
    BTreeMap<WorkbenchTerminalContextId, Option<WorkbenchDirectoryPath>>,
    WorkbenchRestoreError,
> {
    let mut directories = BTreeMap::new();
    if snapshot
        .navigation
        .directories
        .iter()
        .any(|directory| directory.worker_id != *local_worker)
    {
        return Err(WorkbenchRestoreError::ForeignWorker);
    }
    for workspace in &snapshot.navigation.workspaces {
        let directory = workspace
            .primary_directory_id
            .as_ref()
            .map(|id| {
                snapshot
                    .navigation
                    .directory(id)
                    .map(|directory| directory.path.clone())
                    .ok_or(WorkbenchRestoreError::TargetChanged)
            })
            .transpose()?;
        if directories
            .insert(workspace.terminal_context_id.clone(), directory)
            .is_some()
        {
            return Err(WorkbenchRestoreError::TargetChanged);
        }
    }
    Ok(directories)
}

fn find_presentation<'node>(
    node: &'node mut PanelNode,
    target: &WorkbenchRestoreTarget,
) -> Option<&'node mut Presentation> {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
            ..
        } => {
            if *panel_id != target.panel_id {
                return None;
            }
            std::iter::once(primary_presentation)
                .chain(child_presentations)
                .find(|presentation| presentation.presentation_id == target.presentation_id)
        }
        PanelNode::Split { children, .. } => children
            .iter_mut()
            .find_map(|child| find_presentation(&mut child.node, target)),
    }
}
