//! Local entry after a known canonical Opened receipt, using existing native membership and attachment owners.
use super::{NativeUi, WorktreeNotice};
use crate::terminal_session::{
    state::{OperationDeadlines, SessionPhaseKind},
    ui::{ActionBuildError, NativeProjectionStatus, finish_action},
};
use orchbee_workbench_protocol::{Snapshot, WorkbenchWorktreeOutcome, WorktreeOperationStatus};
use std::time::{Duration, Instant};
pub(super) struct Entry {
    member: crate::terminal_session::NativeMemberTarget,
    binding: crate::terminal_session::workbench_projection::NativeTerminalBinding,
    directory: orchbee_workbench_protocol::WorkbenchDirectoryId,
    location: crate::terminal_session::NativeContextLocation,
    minimum_revision: orchbee_workbench_protocol::WorkbenchRevision,
    stage: EntryStage,
    deadline: Instant,
}
enum EntryStage {
    Refresh,
    RefreshRequested,
    Context(Option<u64>),
    Tab,
    Panel,
    Frame,
}
mod membership;

impl NativeUi {
    pub(super) fn begin_worktree_entry(&mut self, snapshot: Option<Snapshot>) {
        let Some(op) = self.worktrees.launcher.operation() else {
            return;
        };
        if !op.focus_allowed || !self.worktrees.launcher.visible() {
            return;
        }
        let Some(snapshot) = snapshot else {
            self.worktrees
                .launcher
                .entry_notice(WorktreeNotice::EntryFailed);
            return;
        };
        let Some(entry) = entry(op, &snapshot, self.clock.now()) else {
            self.worktrees
                .launcher
                .entry_notice(WorktreeNotice::EntryFailed);
            return;
        };
        self.worktrees
            .launcher
            .entry_notice(WorktreeNotice::EntryPending);
        self.worktrees.entry = Some(entry);
    }
    fn request_workbench_refresh(&mut self) {
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref().cloned(), self.state.as_mut())
        else {
            return;
        };
        let deadline = self.worktrees.entry.as_ref().map_or(
            self.clock.now() + Duration::from_secs(2),
            |entry| {
                entry
                    .deadline
                    .min(self.clock.now() + Duration::from_secs(2))
            },
        );
        let result = mailbox.try_enqueue_refresh_for(
            state,
            deadline,
            |state, generation| {
                state
                    .start_pending_refresh(generation)
                    .then_some(())
                    .ok_or(ActionBuildError::Busy)
            },
            finish_action,
        );
        if result.is_err() {
            self.worktrees.entry = None;
            self.worktrees
                .launcher
                .entry_notice(WorktreeNotice::EntryFailed);
        }
    }
    pub(super) fn drive_worktree_entry(&mut self) -> bool {
        let Some(mut entry) = self.worktrees.entry.take() else {
            return false;
        };
        if self.clock.now() >= entry.deadline || self.retirement_failure.is_some() {
            self.worktrees
                .launcher
                .entry_notice(WorktreeNotice::EntryFailed);
            return true;
        }
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let phase = state.phase_kind();
        if state.pending_refresh_generation().is_some()
            || state.context_selecting()
            || phase == SessionPhaseKind::Switching
        {
            self.worktrees.entry = Some(entry);
            return false;
        }
        let target = entry.location.clone();
        if !matches!(
            entry.stage,
            EntryStage::Refresh | EntryStage::RefreshRequested
        ) && state.cache().location() != &target
        {
            self.worktrees
                .launcher
                .entry_notice(WorktreeNotice::EntryFailed);
            return true;
        }
        match entry.stage {
            EntryStage::Refresh | EntryStage::RefreshRequested => {
                let valid = self.worktrees.launcher.visible()
                    && self
                        .worktrees
                        .launcher
                        .operation()
                        .is_some_and(|op| op.focus_allowed)
                    && state.worktree_origin().is_some_and(|current| {
                        self.worktrees
                            .launcher
                            .operation()
                            .is_some_and(|op| op.plan.origin.unchanged(&current))
                    });
                if !valid {
                    self.worktrees
                        .launcher
                        .entry_notice(WorktreeNotice::EntryFailed);
                    return true;
                }
                // A publication refresh may already be in flight when Opened arrives.
                // Await that owner, then request at most one missing membership snapshot.
                let membership_ready = state.cache().revision() == entry.minimum_revision
                    && state
                        .cache()
                        .navigation()
                        .iter()
                        .any(|c| c.location == target);
                if !membership_ready {
                    if matches!(entry.stage, EntryStage::RefreshRequested) {
                        self.worktrees
                            .launcher
                            .entry_notice(WorktreeNotice::EntryFailed);
                    } else {
                        entry.stage = EntryStage::RefreshRequested;
                        self.worktrees.entry = Some(entry);
                        self.request_workbench_refresh();
                    }
                    return true;
                }
                let Some(index) = state
                    .cache()
                    .navigation()
                    .iter()
                    .position(|c| c.location == target)
                else {
                    self.worktrees
                        .launcher
                        .entry_notice(WorktreeNotice::EntryFailed);
                    return true;
                };
                let generation = if state.cache().location() != &target {
                    let Some(generation) = self.select_context_at_generation(index) else {
                        self.worktrees
                            .launcher
                            .entry_notice(WorktreeNotice::EntryFailed);
                        return true;
                    };
                    Some(generation)
                } else {
                    None
                };
                self.worktrees.launcher.entry_started();
                entry.stage = EntryStage::Context(generation);
            }
            EntryStage::Context(_) => {
                if state.cache().location() != &target {
                    self.worktrees
                        .launcher
                        .entry_notice(WorktreeNotice::EntryFailed);
                    return true;
                }
                let Some(index) = state.cache().tab_index(&entry.member.tab) else {
                    self.worktrees
                        .launcher
                        .entry_notice(WorktreeNotice::EntryFailed);
                    return true;
                };
                if state
                    .active()
                    .is_none_or(|active| active.key().anchor().tab_id() != &entry.member.tab)
                {
                    let Ok(position) = u8::try_from(index + 1) else {
                        return true;
                    };
                    self.select_tab_for_panel(position, entry.member.panel.clone());
                }
                entry.stage = EntryStage::Tab;
            }
            EntryStage::Tab => {
                if state
                    .active()
                    .is_none_or(|active| active.key().anchor().tab_id() != &entry.member.tab)
                {
                    self.worktrees
                        .launcher
                        .entry_notice(WorktreeNotice::EntryFailed);
                    return true;
                }
                if state.focus_panel_id() != Some(&entry.member.panel) {
                    self.move_panel_focus_to(&entry.member.panel);
                }
                entry.stage = EntryStage::Panel;
            }
            EntryStage::Panel => {
                if state.focus_panel_id() != Some(&entry.member.panel) {
                    self.worktrees
                        .launcher
                        .entry_notice(WorktreeNotice::EntryFailed);
                    return true;
                }
                if self.view().focused_member().as_ref() != Some(&entry.member) {
                    self.select_presentation_in_active_tab(
                        &entry.member.panel,
                        &entry.member.presentation,
                    );
                    if !self
                        .state
                        .as_ref()
                        .is_some_and(|state| state.phase_kind() == SessionPhaseKind::Switching)
                    {
                        self.worktrees
                            .launcher
                            .entry_notice(WorktreeNotice::EntryFailed);
                        return true;
                    }
                }
                entry.stage = EntryStage::Frame;
            }
            EntryStage::Frame => {
                if phase == SessionPhaseKind::Ready
                    && self.view().focused_member().as_ref() == Some(&entry.member)
                    && self.view().pane_projection_status(&entry.member.panel)
                        == NativeProjectionStatus::FrameAvailable
                {
                    self.worktrees.launcher.entry_notice(WorktreeNotice::Opened);
                    return true;
                }
                if matches!(
                    phase,
                    SessionPhaseKind::SelectedWithoutAttachment
                        | SessionPhaseKind::RetryableAttachUnavailable
                        | SessionPhaseKind::Retired
                ) {
                    self.worktrees
                        .launcher
                        .entry_notice(WorktreeNotice::EntryFailed);
                    return true;
                }
                self.worktrees.entry = Some(entry);
                return false;
            }
        }
        self.worktrees.entry = Some(entry);
        true
    }
}
fn entry(
    op: &crate::terminal_session::state::worktree_launch::Operation,
    snapshot: &Snapshot,
    now: Instant,
) -> Option<Entry> {
    let (candidate, directory) = target_context(op, snapshot)?;
    let tab = candidate.tabs().first()?;
    let panel = tab.panel(0)?;
    let binding = panel.primary().binding()?.clone();
    Some(Entry {
        member: crate::terminal_session::NativeMemberTarget {
            tab: tab.tab_id().clone(),
            panel: panel.panel_id().clone(),
            presentation: panel.primary().presentation_id().clone(),
        },
        binding,
        directory,
        location: candidate.location().clone(),
        minimum_revision: snapshot.revision,
        stage: EntryStage::Refresh,
        deadline: OperationDeadlines::from_sample(now)?.total(),
    })
}

fn target_context(
    op: &crate::terminal_session::state::worktree_launch::Operation,
    snapshot: &Snapshot,
) -> Option<(
    crate::terminal_session::workbench_projection::NativeContextTabs,
    orchbee_workbench_protocol::WorkbenchDirectoryId,
)> {
    let WorkbenchWorktreeOutcome::Available {
        project_id, status, ..
    } = op.outcome.as_ref()?
    else {
        return None;
    };
    let WorktreeOperationStatus::Opened {
        workspace_id,
        checkout,
        reused,
        terminal_outcome,
    } = status.as_ref()
    else {
        return None;
    };
    if !*reused
        && !matches!(
            terminal_outcome,
            Some(orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. })
        )
    {
        return None;
    }
    if snapshot.site_id != *op.plan.origin.cache.site_id() {
        return None;
    }
    let project = snapshot
        .navigation
        .projects
        .iter()
        .find(|p| &p.project_id == project_id)?;
    let workspace = project
        .workspaces
        .iter()
        .find(|w| &w.workspace_id == workspace_id)?;
    let directory = project
        .directories
        .iter()
        .find(|d| Some(&d.directory_id) == workspace.primary_directory_id.as_ref())?;
    if !orchbee_workbench_protocol::same_windows_path_spelling(
        directory.path.as_str(),
        checkout.path.as_str(),
    ) || directory.worker_id != op.plan.discovery.worker
    {
        return None;
    }
    let location = crate::terminal_session::NativeContextLocation::Workspace {
        project_id: project_id.clone(),
        workspace_id: workspace_id.clone(),
        terminal_context_id: workspace.terminal_context_id.clone(),
    };
    let candidate = crate::terminal_session::workbench_projection::project_native_context_tabs(
        snapshot, &location,
    )
    .ok()?;
    Some((candidate, directory.directory_id.clone()))
}

#[cfg(test)]
mod tests;
