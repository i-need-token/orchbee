//! Exact linked-checkout confirmation and explicit generation-fenced recovery; the Server owns deletion.
use super::addressed::{SidebarDiscovery, SidebarWorktreeTarget};
use super::{Launcher, Operation, Phase, Plan, WorktreeNotice, WorktreeToken, WorktreeView};
use orchbee_workbench_protocol::{
    MAXIMUM_WORKTREE_OPERATION_ATTEMPTS, WorkbenchDirectoryPath, WorkbenchRequestId,
    WorkbenchWorkspaceId, WorkbenchWorktreeOutcome, WorktreeEntryText, WorktreeOperationFailure,
    WorktreeOperationStatus, WorktreeProgress, WorktreeWorkspaceDisposition,
};

/// The Workspace directory and complete terminal scope shown before destructive submission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Removal {
    pub(crate) workspace_id: WorkbenchWorkspaceId,
    pub(crate) path: WorkbenchDirectoryPath,
    label: String,
    branch: Option<String>,
    terminals: usize,
}
impl Removal {
    pub(crate) fn capture(sidebar: &SidebarDiscovery) -> Option<Self> {
        let SidebarWorktreeTarget::Workspace(workspace_id) = &sidebar.addressed.target else {
            return None;
        };
        let cache = &sidebar.addressed.cache;
        let workspace = cache
            .canonical_snapshot()
            .navigation
            .workspace(workspace_id)?;
        let directory = cache.directory()?;
        let entry = sidebar.discovery.entries.iter().find(|entry| {
            entry.linked
                && !entry.checkout.bare
                && matches!(&entry.checkout.path, WorktreeEntryText::Utf8(path)
                    if same_checkout(directory.path.as_str(), path))
                && entry
                    .workspaces
                    .iter()
                    .any(|workspace| &workspace.workspace_id == workspace_id)
        })?;
        if directory.worker_id != sidebar.discovery.worker {
            return None;
        }
        let branch = match &entry.checkout.branch {
            Some(WorktreeEntryText::Utf8(branch)) => Some(branch.clone()),
            None => None,
            // This operation needs UTF-8 text operands; opaque path bytes cannot supply them.
            _ => return None,
        };
        let terminals = cache
            .tabs()
            .iter()
            .map(|tab| {
                (0..tab.panel_count())
                    .filter_map(|index| tab.panel(index))
                    .flat_map(|panel| panel.presentations())
                    .filter(|presentation| presentation.binding().is_some())
                    .count()
            })
            .sum();
        Some(Self {
            workspace_id: workspace_id.clone(),
            path: directory.path.clone(),
            label: workspace.label.clone(),
            branch,
            terminals,
        })
    }
}

/// Destructive identity: POSIX-rooted spellings compare byte for byte; Windows drive spellings
/// use the shared association rule, which leaves verbatim prefixes and dot components unequal.
pub(super) fn same_checkout(directory: &str, checkout: &str) -> bool {
    if directory.starts_with('/') || checkout.starts_with('/') {
        return directory == checkout;
    }
    orchbee_workbench_protocol::same_windows_path_spelling(directory, checkout)
}

fn resumable(op: &Operation) -> bool {
    op.plan.removal.is_some()
        && op.attempt < MAXIMUM_WORKTREE_OPERATION_ATTEMPTS
        && matches!(&op.outcome, Some(WorkbenchWorktreeOutcome::Available {
            project_id, attempt, status,
        }) if project_id == &op.plan.project && *attempt == op.attempt
            && matches!(status.as_ref(), WorktreeOperationStatus::Failed {
                reason, progress, resumable: true,
            } if !matches!(reason, WorktreeOperationFailure::PhysicalUnknown { .. }
                    | WorktreeOperationFailure::TransportUnknown { .. })
                && progress.restoration.as_ref().is_none_or(|receipt| receipt.unknown == 0)))
}

/// The explicit decision a retained removal result is waiting for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Recovery {
    /// Send one Resume for the failed attempt.
    Resume,
    /// Forget the local receipt; Server work and its status remain.
    Abandon,
}

impl Launcher {
    /// Close keeps these settled receipts so W, O or the sidebar can reopen their recovery.
    pub(crate) fn retains_removal(&self) -> bool {
        matches!(&self.phase, Phase::Result(op) if op.plan.removal.is_some()
            && (self.notice == WorktreeNotice::Unknown
                || (matches!(self.notice, WorktreeNotice::Failed | WorktreeNotice::NotSubmitted)
                    && resumable(op))))
    }

    /// A settled removal receipt has no local recovery left once its Workspace is
    /// absent from the canonical snapshot, so it never blocks a new request.
    pub(crate) fn forget_stale_removal(
        &mut self,
        snapshot: &orchbee_workbench_protocol::Snapshot,
    ) -> bool {
        if !matches!(&self.phase, Phase::Result(op) if op.plan.removal.as_ref().is_some_and(
            |target| snapshot.navigation.workspace(&target.workspace_id).is_none()))
        {
            return false;
        }
        self.forget_terminal();
        self.shown = None;
        self.input_frame_live = false;
        true
    }

    fn prepare_recovery(&mut self, token: WorktreeToken, recovery: Recovery) -> bool {
        let eligible = match recovery {
            Recovery::Resume => {
                matches!(
                    self.notice,
                    WorktreeNotice::Failed | WorktreeNotice::NotSubmitted
                ) && matches!(&self.phase, Phase::Result(op) if resumable(op))
            }
            Recovery::Abandon => self.retains_removal(),
        };
        if self.token() != Some(token)
            || self.shown != Some(token)
            || !self.presentation.interactive()
            || !eligible
            || self.advance_token().is_none()
        {
            return false;
        }
        let Phase::Result(op) = std::mem::replace(&mut self.phase, Phase::Idle) else {
            return false;
        };
        self.phase = Phase::Confirm(op, recovery);
        true
    }

    pub(crate) fn prepare_remove_resume(&mut self, token: WorktreeToken) -> bool {
        self.prepare_recovery(token, Recovery::Resume)
    }

    pub(crate) fn prepare_remove_abandon(&mut self, token: WorktreeToken) -> bool {
        self.prepare_recovery(token, Recovery::Abandon)
    }

    pub(crate) fn confirm_remove_resume(&mut self, token: WorktreeToken) -> Option<(Plan, u32)> {
        if self.token() != Some(token)
            || self.shown != Some(token)
            || !self.presentation.interactive()
        {
            return None;
        }
        let Phase::Confirm(op, Recovery::Resume) = &self.phase else {
            return None;
        };
        if !resumable(op) {
            return None;
        }
        let result = (op.plan.clone(), op.attempt);
        let Phase::Confirm(mut op, _) = std::mem::replace(&mut self.phase, Phase::Idle) else {
            return None;
        };
        op.focus_allowed = false;
        self.notice = WorktreeNotice::Pending;
        self.phase = Phase::Resuming(op);
        Some(result)
    }

    /// Forgets only this Client's receipt; `orchbee worktree status` still reads the operation.
    pub(crate) fn confirm_remove_abandon(&mut self, token: WorktreeToken) -> bool {
        if self.token() != Some(token)
            || self.shown != Some(token)
            || !self.presentation.interactive()
            || !matches!(self.phase, Phase::Confirm(_, Recovery::Abandon))
        {
            return false;
        }
        self.phase = Phase::Idle;
        self.visible = false;
        self.shown = None;
        self.input_frame_live = false;
        true
    }

    pub(crate) fn cancel_remove_recovery(&mut self) -> bool {
        if !matches!(self.phase, Phase::Confirm(..)) {
            return false;
        }
        let Phase::Confirm(op, _) = std::mem::replace(&mut self.phase, Phase::Idle) else {
            return false;
        };
        self.phase = Phase::Result(op);
        // Cancel ended the modal decoder's input lease. The restored result is
        // a new incarnation, so its fresh token lets input capture restart.
        if self.advance_token().is_none() {
            self.shown = None;
            self.input_frame_live = false;
        }
        true
    }

    pub(crate) fn resume_settled(
        &mut self,
        operation: &WorkbenchRequestId,
        result: Result<WorkbenchWorktreeOutcome, bool>,
    ) -> bool {
        let Phase::Resuming(op) = &self.phase else {
            return false;
        };
        if &op.plan.operation_id != operation {
            return false;
        }
        let Phase::Resuming(mut op) = std::mem::replace(&mut self.phase, Phase::Idle) else {
            return false;
        };
        if matches!(result, Err(false)) {
            self.notice = WorktreeNotice::NotSubmitted;
            self.phase = Phase::Result(op);
            return true;
        }
        // A possibly handed-off Resume expects the next attempt. Its old failure
        // remains evidence, but cannot authorize another Resume until Read settles it.
        op.attempt += 1;
        self.phase = Phase::Observing(op);
        self.observed(operation, result.map_err(|_| true))
    }
}

impl<'a> WorktreeView<'a> {
    fn removal(&self) -> Option<&'a Removal> {
        match &self.launcher.phase {
            Phase::Draft(draft) => draft.removal.as_ref(),
            Phase::Starting(plan) => plan.removal.as_ref(),
            _ => self.launcher.operation()?.plan.removal.as_ref(),
        }
    }
    /// Exact label, full checkout path, retained branch and affected terminal count.
    pub(crate) fn removal_target(&self) -> Option<(&'a str, &'a str, Option<&'a str>, usize)> {
        let target = self.removal()?;
        Some((
            &target.label,
            target.path.as_str(),
            target.branch.as_deref(),
            target.terminals,
        ))
    }
    /// Only a known current failed attempt can offer explicit recovery.
    pub(crate) fn can_resume_remove(&self) -> bool {
        matches!(
            self.launcher.notice,
            WorktreeNotice::Failed | WorktreeNotice::NotSubmitted
        ) && matches!(&self.launcher.phase, Phase::Result(op) if resumable(op))
    }
    /// A settled receipt that Close would retain can instead be explicitly abandoned.
    pub(crate) fn can_abandon_remove(&self) -> bool {
        self.launcher.retains_removal()
    }
    /// The Workspace a settled, nonresumable failure still holds closing.
    pub(crate) fn held_close(&self) -> Option<&'a str> {
        if self.launcher.notice != WorktreeNotice::Failed || self.can_resume_remove() {
            return None;
        }
        self.removal_failure()?;
        match &self.removal_progress()?.workspace {
            WorktreeWorkspaceDisposition::Closing { workspace_id, .. } => {
                Some(workspace_id.as_str())
            }
            WorktreeWorkspaceDisposition::Unassociated {}
            | WorktreeWorkspaceDisposition::Open { .. }
            | WorktreeWorkspaceDisposition::Closed { .. } => None,
        }
    }
    /// Typed canonical failure; raw Git diagnostics stay outside product copy.
    pub(crate) fn removal_failure(&self) -> Option<&'a WorktreeOperationFailure> {
        let WorkbenchWorktreeOutcome::Available {
            attempt, status, ..
        } = self.launcher.operation()?.outcome.as_ref()?
        else {
            return None;
        };
        if *attempt != self.launcher.operation()?.attempt {
            return None;
        }
        match status.as_ref() {
            WorktreeOperationStatus::Failed { reason, .. } => Some(reason),
            WorktreeOperationStatus::Pending { .. }
            | WorktreeOperationStatus::Opened { .. }
            | WorktreeOperationStatus::Removed { .. } => None,
        }
    }
    /// Retained Workspace and shell-restoration facts for this exact Remove attempt.
    pub(crate) fn removal_progress(&self) -> Option<&'a WorktreeProgress> {
        self.removal()?;
        let op = self.launcher.operation()?;
        let WorkbenchWorktreeOutcome::Available {
            attempt, status, ..
        } = op.outcome.as_ref()?
        else {
            return None;
        };
        if *attempt != op.attempt {
            return None;
        }
        match status.as_ref() {
            WorktreeOperationStatus::Pending { progress, .. }
            | WorktreeOperationStatus::Failed { progress, .. } => Some(progress),
            WorktreeOperationStatus::Opened { .. } | WorktreeOperationStatus::Removed { .. } => {
                None
            }
        }
    }
}
