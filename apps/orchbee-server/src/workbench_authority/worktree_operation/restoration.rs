//! Identity-preserving replacement of disposed Worktree presentation bindings.
use super::{WorkbenchAuthority, WorktreeOperationPlan};
use crate::{WorkbenchTerminalTab, workbench_projection::PreparedProjectionAdvance};
use orchbee_workbench_protocol::{
    PanelNode, Presentation, PresentationBinding, WorkbenchProtocolDocument, WorkbenchRequestId,
    WorktreeIntent, WorktreeOperationPhase, WorktreeOperationStatus, serialize_document,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreeRebindError {
    Unavailable,
    TargetChanged,
    PreparationFailed,
}

pub struct PreparedWorktreeRebind<'a> {
    authority: &'a mut WorkbenchAuthority,
    projection: PreparedProjectionAdvance,
    operation: WorkbenchRequestId,
    last_identity: u128,
    progress: orchbee_workbench_protocol::WorktreeProgress,
}
impl PreparedWorktreeRebind<'_> {
    /// Session commits the matching physical Create reservation at the same no-fail cut.
    pub fn commit(self) -> orchbee_workbench_protocol::WorktreeProgress {
        self.authority.projection.commit_prepared(self.projection);
        self.authority.last_identity = self.last_identity;
        if let Some(record) = self.authority.worktrees.records.get_mut(&self.operation) {
            record.status = WorktreeOperationStatus::Pending {
                phase: WorktreeOperationPhase::Removing,
                progress: self.progress.clone(),
            };
        }
        self.progress
    }
}

impl WorkbenchAuthority {
    fn worktree_recovery_owns_close(
        &self,
        plan: &WorktreeOperationPlan,
        close: &WorkbenchRequestId,
    ) -> bool {
        !self.retired
            && self.worktree_close_quiesced(plan, close)
            && self
                .worktrees
                .records
                .get(&plan.operation_id)
                .is_some_and(|record| {
                    !record.plan.removal_complete
                        && matches!(
                            record.status,
                            WorktreeOperationStatus::Pending {
                                phase: WorktreeOperationPhase::Removing,
                                ..
                            }
                        )
                })
            && matches!(&plan.intent, WorktreeIntent::Remove { workspace_id, .. }
                if self.terminal_close_matches_workspace(close, workspace_id))
    }

    /// Captured order is stable even as earlier slots receive their fresh bindings.
    pub fn worktree_restore_members(
        &self,
        plan: &WorktreeOperationPlan,
        close: &WorkbenchRequestId,
    ) -> Option<Vec<WorkbenchTerminalTab>> {
        if !self.worktree_recovery_owns_close(plan, close) {
            return None;
        }
        self.terminal_close_captured_members(close)
    }

    pub fn prepare_worktree_rebind(
        &mut self,
        plan: &WorktreeOperationPlan,
        close: &WorkbenchRequestId,
        member: &WorkbenchTerminalTab,
        fresh: PresentationBinding,
    ) -> Result<PreparedWorktreeRebind<'_>, WorktreeRebindError> {
        if !self.worktree_recovery_owns_close(plan, close) {
            return Err(WorktreeRebindError::Unavailable);
        }
        if !self.worktrees.records.get(&plan.operation_id).is_some_and(|record|
            matches!(&record.status, WorktreeOperationStatus::Pending { progress, .. } if *progress == plan.progress)) {
            return Err(WorktreeRebindError::Unavailable);
        }
        let Some(target) = self.terminal_tab_close_target_for_binding(close, member.binding())
        else {
            return Err(WorktreeRebindError::TargetChanged);
        };
        if target.member() != member
            || fresh.worker_id != plan.source.worker_id
            || fresh == *member.binding()
            || self.has_terminal_tab_close(&fresh)
        {
            return Err(WorktreeRebindError::TargetChanged);
        }
        let context_id = target.terminal_context_id().clone();
        let current = self
            .terminal_tree_membership(&context_id)
            .map_err(|_| WorktreeRebindError::TargetChanged)?;
        if !current.members().contains(member) {
            return Err(WorktreeRebindError::TargetChanged);
        }
        let last_identity = super::super::allocate(self.last_identity, 1)
            .map_err(|_| WorktreeRebindError::PreparationFailed)?;
        let presentation = orchbee_workbench_protocol::TerminalPresentationId::parse(
            &super::super::identity(last_identity),
        )
        .map_err(|_| WorktreeRebindError::PreparationFailed)?;
        let mut candidate = self.snapshot().clone();
        let context = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| context.terminal_context_id == context_id)
            .ok_or(WorktreeRebindError::TargetChanged)?;
        let tab = context
            .tabs
            .iter_mut()
            .find(|tab| &tab.tab_id == member.tab_id())
            .ok_or(WorktreeRebindError::TargetChanged)?;
        if !replace_binding(&mut tab.root, member, &fresh, &presentation) {
            return Err(WorktreeRebindError::TargetChanged);
        }
        candidate.revision = self
            .next_revision()
            .map_err(|_| WorktreeRebindError::PreparationFailed)?;
        // Snapshot validation enforces unique terminal bindings across all Contexts.
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate))
            .map_err(|_| WorktreeRebindError::PreparationFailed)?;
        let projection = self
            .projection
            .prepare_advance(&encoded)
            .map_err(|_| WorktreeRebindError::PreparationFailed)?;
        let mut progress = plan.progress.clone();
        if let Some(restoration) = &mut progress.restoration {
            if restoration.attempted >= restoration.total
                || restoration.attempted
                    != restoration.started + restoration.failed + restoration.unknown
            {
                return Err(WorktreeRebindError::Unavailable);
            }
            restoration.attempted += 1;
        }
        Ok(PreparedWorktreeRebind {
            authority: self,
            projection,
            operation: plan.operation_id.clone(),
            last_identity,
            progress,
        })
    }

    /// This stops canonical deletion, not terminal disposal, and never claims shell success.
    pub fn retain_worktree_workspace(
        &mut self,
        plan: &WorktreeOperationPlan,
        close: &WorkbenchRequestId,
    ) -> bool {
        if !self.worktree_recovery_owns_close(plan, close)
            || !self.retain_quiescent_workspace(close)
        {
            return false;
        }
        // The same guard has already proven this retained record exists.
        if let Some(record) = self.worktrees.records.get_mut(&plan.operation_id) {
            record.close_hold = None;
        }
        true
    }
}

fn replace_binding(
    node: &mut PanelNode,
    member: &WorkbenchTerminalTab,
    fresh: &PresentationBinding,
    presentation_id: &orchbee_workbench_protocol::TerminalPresentationId,
) -> bool {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
            ..
        } if panel_id == member.panel_id() => std::iter::once(primary_presentation)
            .chain(child_presentations.iter_mut())
            .any(|presentation| replace_presentation(presentation, member, fresh, presentation_id)),
        PanelNode::Split { children, .. } => children
            .iter_mut()
            .any(|child| replace_binding(&mut child.node, member, fresh, presentation_id)),
        PanelNode::Panel { .. } => false,
    }
}
fn replace_presentation(
    presentation: &mut Presentation,
    member: &WorkbenchTerminalTab,
    fresh: &PresentationBinding,
    presentation_id: &orchbee_workbench_protocol::TerminalPresentationId,
) -> bool {
    if &presentation.presentation_id != member.presentation_id()
        || presentation.binding.as_ref() != Some(member.binding())
    {
        return false;
    }
    presentation.binding = Some(fresh.clone());
    // A failure describes only the binding it was recorded for.
    presentation.launch_failure = None;
    presentation.presentation_id = presentation_id.clone();
    true
}
