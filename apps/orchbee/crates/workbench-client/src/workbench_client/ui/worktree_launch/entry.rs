//! Local entry after a known canonical Opened receipt, using existing native membership and attachment owners.
use super::{WorkbenchClientUi, WorktreeNotice};
use crate::workbench_client::ui::{ActionBuildError, ClientProjectionStatus, finish_action};
use crate::workbench_state::{ClientPhaseKind, OperationDeadlines};
use orchbee_workbench_protocol::{
    Snapshot, WorkbenchWorktreeOutcome, WorktreeIntent, WorktreeOperationStatus,
};
use std::time::{Duration, Instant};
pub(super) struct Entry {
    member: crate::workbench_projection::ClientMemberTarget,
    binding: crate::workbench_projection::ClientTerminalBinding,
    directory: Option<orchbee_workbench_protocol::WorkbenchDirectoryId>,
    location: crate::workbench_projection::ClientContextLocation,
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

impl WorkbenchClientUi {
    /// Counts the Agent a create from the Start Agent picker runs in its new Workspace's first
    /// shell among this Client's starts, once per operation, as a new Tab's start counts; a reused
    /// Workspace or a shell that did not start launched no Agent. The Server watches that start
    /// and reports one that did not happen on the Presentation.
    pub(super) fn count_worktree_agent(&mut self) {
        let Some(op) = self.worktrees.launcher.operation() else {
            return;
        };
        let WorktreeIntent::Create {
            agent_start: Some(selector),
            ..
        } = &op.plan.intent
        else {
            return;
        };
        if self.worktrees.counted.as_ref() == Some(&op.plan.operation_id) || !launched_agent(op) {
            return;
        }
        let provider = selector.agent().id();
        self.worktrees.counted = Some(op.plan.operation_id.clone());
        self.board.start_state().note_started(provider);
    }
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
            || phase == ClientPhaseKind::Switching
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
                let membership_ready = state.cache().revision() >= entry.minimum_revision
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
                let Some(index) = state.display_position(&target) else {
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
                        .is_some_and(|state| state.phase_kind() == ClientPhaseKind::Switching)
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
                if phase == ClientPhaseKind::Ready
                    && self.view().focused_member().as_ref() == Some(&entry.member)
                    && self.view().pane_projection_status(&entry.member.panel)
                        == ClientProjectionStatus::FrameAvailable
                {
                    self.worktrees.launcher.entry_notice(WorktreeNotice::Opened);
                    return true;
                }
                if matches!(
                    phase,
                    ClientPhaseKind::SelectedWithoutAttachment
                        | ClientPhaseKind::RetryableAttachUnavailable
                        | ClientPhaseKind::Retired
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
    op: &crate::workbench_state::worktree_launch::Operation,
    snapshot: &Snapshot,
    now: Instant,
) -> Option<Entry> {
    let (candidate, directory) = target_context(op, snapshot)?;
    let tab = candidate.tabs().first()?;
    let panel = tab.panel(0)?;
    let binding = panel.primary().binding()?.clone();
    Some(Entry {
        member: crate::workbench_projection::ClientMemberTarget {
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

/// Whether this operation opened a new Workspace whose first shell started.
fn launched_agent(op: &crate::workbench_state::worktree_launch::Operation) -> bool {
    matches!(
        op.outcome.as_ref(),
        Some(WorkbenchWorktreeOutcome::Available { status, .. })
            if matches!(
                status.as_ref(),
                WorktreeOperationStatus::Opened {
                    reused: false,
                    terminal_outcome: Some(
                        orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. }
                    ),
                    ..
                }
            )
    )
}

fn target_context(
    op: &crate::workbench_state::worktree_launch::Operation,
    snapshot: &Snapshot,
) -> Option<(
    crate::workbench_projection::ClientContextTabs,
    Option<orchbee_workbench_protocol::WorkbenchDirectoryId>,
)> {
    let WorkbenchWorktreeOutcome::Available { status, .. } = op.outcome.as_ref()? else {
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
    if snapshot.session_id != *op.plan.origin.cache.session_id() {
        return None;
    }
    let workspace = snapshot.navigation.workspace(workspace_id)?;
    let directory = snapshot.navigation.workspace_directory(workspace_id);
    // A reused Workspace may stand at the checkout only by following its first terminal there.
    let names_checkout = |path: &orchbee_workbench_protocol::WorkbenchDirectoryPath| {
        orchbee_workbench_protocol::same_directory_spelling(path.as_str(), checkout.path.as_str())
    };
    let at_checkout = directory.is_some_and(|directory| names_checkout(&directory.path))
        || workspace
            .terminal_directory
            .as_ref()
            .is_some_and(names_checkout);
    if !at_checkout
        || directory.is_some_and(|directory| directory.worker_id != op.plan.discovery.worker)
    {
        return None;
    }
    let location = crate::workbench_projection::ClientContextLocation::Workspace {
        workspace_id: workspace_id.clone(),
        terminal_context_id: workspace.terminal_context_id.clone(),
    };
    let candidate =
        crate::workbench_projection::project_native_context_tabs(snapshot, &location).ok()?;
    Some((
        candidate,
        directory.map(|directory| directory.directory_id.clone()),
    ))
}

#[cfg(test)]
mod tests;
