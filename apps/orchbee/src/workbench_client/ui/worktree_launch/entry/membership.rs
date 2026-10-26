//! Exact target proof joins accepted refreshes and fences context installation before cleanup.
use super::{Entry, EntryStage, NativeUi, WorktreeNotice, target_context};
use crate::workbench_client::state::PreparedRefresh;
use orchbee_workbench_protocol::{Snapshot, WorkbenchRevision};

fn matches_target(
    expected: &Entry,
    op: &crate::workbench_client::state::worktree_launch::Operation,
    snapshot: &Snapshot,
) -> bool {
    let Some((candidate, directory)) = target_context(op, snapshot) else {
        return false;
    };
    if candidate.location() != &expected.location || directory != expected.directory {
        return false;
    }
    candidate
        .tab_index(&expected.member.tab)
        .and_then(|index| candidate.tabs().get(index))
        .and_then(|tab| {
            tab.panel_index(&expected.member.panel)
                .and_then(|index| tab.panel(index))
        })
        .is_some_and(|panel| {
            panel.primary().presentation_id() == &expected.member.presentation
                && panel.primary().binding() == Some(&expected.binding)
        })
}
impl NativeUi {
    pub(in crate::workbench_client::ui) fn observe_worktree_entry_snapshot(
        &mut self,
        snapshot: &Snapshot,
    ) {
        let Some(expected) = self.worktrees.entry.as_ref() else {
            return;
        };
        if !matches!(
            expected.stage,
            EntryStage::Refresh | EntryStage::RefreshRequested
        ) || snapshot.revision < expected.minimum_revision
        {
            return;
        }
        let valid = self
            .worktrees
            .launcher
            .operation()
            .is_some_and(|op| matches_target(expected, op, snapshot));
        if valid {
            if let Some(expected) = self.worktrees.entry.as_mut() {
                expected.minimum_revision = snapshot.revision;
            }
        } else {
            self.worktrees.entry = None;
            self.worktrees
                .launcher
                .entry_notice(WorktreeNotice::EntryFailed);
        }
    }

    pub(in crate::workbench_client::ui) fn fence_worktree_context_candidate(
        &self,
        mut prepared: PreparedRefresh,
        snapshot: Option<&Snapshot>,
        generation: u64,
        baseline_revision: WorkbenchRevision,
    ) -> PreparedRefresh {
        let Some(expected) = self.worktrees.entry.as_ref() else {
            return prepared;
        };
        if !matches!(expected.stage, EntryStage::Context(Some(owned)) if owned == generation)
            || !self
                .state
                .as_ref()
                .is_some_and(|state| state.context_selecting())
        {
            return prepared;
        }
        let PreparedRefresh::Membership {
            generation: prepared_generation,
            prepared: candidate,
        } = &mut prepared
        else {
            return prepared;
        };
        if *prepared_generation != generation {
            return prepared;
        }
        let valid = snapshot
            .filter(|snapshot| snapshot.revision >= expected.minimum_revision)
            .and_then(|snapshot| {
                self.worktrees
                    .launcher
                    .operation()
                    .map(|op| matches_target(expected, op, snapshot))
            })
            .unwrap_or(false);
        if valid && candidate.retarget_selected_member(&expected.location, &expected.member) {
            prepared
        } else {
            PreparedRefresh::DefiniteFailure {
                generation,
                baseline_revision,
            }
        }
    }
}
