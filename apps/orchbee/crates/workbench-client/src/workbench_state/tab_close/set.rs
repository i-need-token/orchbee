//! One confirmed, frozen Tab set advances through existing single-Tab Close and membership owners.
//! A halt stops new submissions, never cancels or discards a submitted Close's recovery facts. The
//! Server's own following of a first terminal is no outside change: it stops nothing.
use super::{
    ClientTerminalBinding, CloseConfirmation, CloseConfirmationToken, CloseFlightKey, CloseIntent,
    CloseOperation, CloseOperationKind, CloseResult, CloseSelection, CloseState, CloseStateError,
    CloseTarget, PaneKey, WorkbenchLabel, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseStatus, WorkbenchTabId,
};
use crate::exchange_values::workbench::WorkbenchTabClosePrecommit;
use crate::workbench_state::{ClientPhase, PaneRef, WorkbenchClientState};
use orchbee_workbench_protocol::{
    Snapshot, TerminalPresentationId, WorkbenchRejectionReason, WorkbenchTabCloseReadOutcome,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CloseSetKind {
    Selected,
    Other,
    Trailing,
    Group,
}

/// Resolve the GUI menu's exact set; only relative operations protect pinned Tabs.
pub(crate) fn close_set_targets(
    tabs: &[orchbee_workbench_protocol::Tab],
    selected: &[WorkbenchTabId],
    target: &WorkbenchTabId,
    kind: CloseSetKind,
) -> Vec<WorkbenchTabId> {
    let Some(index) = tabs.iter().position(|tab| &tab.tab_id == target) else {
        return Vec::new();
    };
    let in_selection = selected.contains(target);
    let end = if in_selection {
        tabs.iter()
            .rposition(|tab| selected.contains(&tab.tab_id))
            .unwrap_or(index)
    } else {
        index
    };
    tabs.iter()
        .enumerate()
        .filter(|(position, tab)| match kind {
            CloseSetKind::Selected => {
                if in_selection {
                    selected.contains(&tab.tab_id)
                } else {
                    &tab.tab_id == target
                }
            }
            CloseSetKind::Group => {
                tabs[index].group_id.is_some() && tab.group_id == tabs[index].group_id
            }
            CloseSetKind::Other | CloseSetKind::Trailing => {
                !tab.pinned
                    && &tab.tab_id != target
                    && !(in_selection && selected.contains(&tab.tab_id))
                    && (kind != CloseSetKind::Trailing || *position > end)
            }
        })
        .map(|(_, tab)| tab.tab_id.clone())
        .collect()
}

/// Whether `current` is `source` but for what the Server publishes on its own as a first terminal
/// moves: the revision, each Workspace's followed directory and an automatic Workspace name. A
/// close of a first terminal causes exactly that, in a revision of its own and at a moment of its
/// own. Every other difference, whoever made it, is one the frozen set did not agree to.
pub(crate) fn close_set_premises_match(current: &Snapshot, source: &Snapshot) -> bool {
    let mut followed = source.clone();
    followed.revision = current.revision;
    for workspace in &mut followed.navigation.workspaces {
        let Some(moved) = current
            .navigation
            .workspaces
            .iter()
            .find(|moved| moved.workspace_id == workspace.workspace_id)
        else {
            return false;
        };
        workspace
            .terminal_directory
            .clone_from(&moved.terminal_directory);
        if workspace.automatic_label && moved.automatic_label {
            workspace.label.clone_from(&moved.label);
        }
    }
    &followed == current
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LocalFocus {
    desired: Option<PaneKey>,
    attached: Option<(PaneRef, ClientTerminalBinding)>,
    presentation: Option<(TerminalPresentationId, Option<ClientTerminalBinding>)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CloseSetCapture {
    first: CloseSelection,
    source: Arc<Snapshot>,
    tabs: Vec<WorkbenchTabId>,
    labels: Vec<(usize, String)>,
    focus: LocalFocus,
    marked: Vec<WorkbenchTabId>,
}

impl CloseSetCapture {
    pub(super) fn first(&self) -> &CloseSelection {
        &self.first
    }
    pub(crate) fn count(&self) -> usize {
        self.tabs.len()
    }
    pub(super) fn tabs(&self) -> &[WorkbenchTabId] {
        &self.tabs
    }
    #[cfg(test)]
    pub(crate) fn labels(&self) -> &[(usize, String)] {
        &self.labels
    }
    /// Each closing Tab's captured position and name, with the identity that finds it live.
    pub(crate) fn entries(&self) -> impl Iterator<Item = (usize, &str, &WorkbenchTabId)> {
        self.labels
            .iter()
            .zip(&self.tabs)
            .map(|((position, label), id)| (*position, label.as_str(), id))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Step {
    Ready,
    Waiting(CloseFlightKey),
    Verifying {
        key: CloseFlightKey,
        revision: WorkbenchRevision,
    },
    Halted {
        awaiting: Option<CloseFlightKey>,
    },
    Complete,
}

pub(super) struct CloseSetRun {
    capture: CloseSetCapture,
    source: Arc<Snapshot>,
    focus: LocalFocus,
    marked: Vec<WorkbenchTabId>,
    completed: usize,
    deadline: Instant,
    step: Step,
    summary: SettledSummary,
}

/// The report of a set with nothing left in flight, which clears like any other bounded notice.
enum SettledSummary {
    Pending,
    Until(Instant),
    Hidden,
}

pub(crate) use crate::localization::CloseSetStatus;

impl CloseSetRun {
    pub(super) fn halted_target(&self) -> Option<&CloseTarget> {
        match &self.step {
            Step::Halted {
                awaiting: Some(key),
            } => Some(key.target()),
            _ => None,
        }
    }

    pub(super) fn resume_status(&self, target: &CloseTarget) -> Option<CloseSetStatus> {
        matches!(&self.step, Step::Halted { awaiting: Some(key) } if key.target() == target)
            .then_some(CloseSetStatus {
                completed: self.completed,
                total: self.capture.tabs.len(),
                halted: true,
                unconfirmed: true,
            })
    }

    fn active(&self) -> bool {
        !matches!(self.step, Step::Halted { .. } | Step::Complete)
    }
    fn halt(&mut self) {
        if self.active() {
            let awaiting = match &self.step {
                Step::Waiting(key) => Some(key.clone()),
                _ => None,
            };
            self.step = Step::Halted { awaiting };
        }
    }
}

impl CloseState {
    pub(in crate::workbench_state) fn set_active(&self) -> bool {
        self.set.as_ref().is_some_and(CloseSetRun::active)
    }

    /// Whether several Tabs are closing as one sequence, which any focus change stops.
    pub(in crate::workbench_state) fn sequence_active(&self) -> bool {
        self.set
            .as_ref()
            .is_some_and(|set| set.active() && set.capture.tabs.len() > 1)
    }

    pub(in crate::workbench_state) fn halt_set(&mut self) {
        if let Some(set) = &mut self.set {
            set.halt();
        }
    }

    // CloseState has already checked the primary/alias identity, generation and monotone facts.
    pub(super) fn observe_set_result(&mut self, operation: &CloseOperation, result: &CloseResult) {
        let Some(set) = &mut self.set else {
            return;
        };
        if let Step::Halted { awaiting } = &mut set.step {
            if awaiting
                .as_ref()
                .is_some_and(|key| key.target() == operation.key().target())
                && matches!(
                    result,
                    CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                        status: WorkbenchTabCloseStatus::Closed { .. },
                        ..
                    })
                )
            {
                set.completed += 1;
                *awaiting = None;
            }
            return;
        }
        let Step::Waiting(start) = &set.step else {
            return;
        };
        if start.target() != operation.key().target() {
            return;
        }
        match result {
            CloseResult::Accepted(_) if start == operation.key() => {}
            // The Server refused this request whole, so it can go again under a new identity; its
            // preflight then decides whether what moved the revision still lets it. One Tab has
            // no sequence to keep alive and reports the refusal instead.
            CloseResult::Precommit(WorkbenchTabClosePrecommit::Rejected {
                reason: WorkbenchRejectionReason::RevisionMismatch,
                ..
            }) if start == operation.key() && set.capture.tabs.len() > 1 => {
                set.step = Step::Ready;
            }
            CloseResult::Read(WorkbenchTabCloseReadOutcome::Available { status, .. }) => {
                match status {
                    WorkbenchTabCloseStatus::Closing { .. }
                    | WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                        reason:
                            WorkbenchTabClosePublicationReason::Preparing
                            | WorkbenchTabClosePublicationReason::TargetCleanupPending,
                        resumable: false,
                    } => {}
                    WorkbenchTabCloseStatus::Closed { revision } => {
                        set.completed += 1;
                        set.step = Step::Verifying {
                            key: start.clone(),
                            revision: *revision,
                        };
                    }
                    // This Close continuation requires the corresponding published receipt; other child states cannot prove final disposal.
                    _ => set.halt(),
                }
            }
            // This Close continuation requires the corresponding published receipt; other child states cannot prove final disposal.
            _ => set.halt(),
        }
    }

    /// Whether this refused Start is already on the status line, as the count of several Tabs that
    /// stopped or as a sequence that goes on. Any other refusal reports through a notice.
    pub(super) fn set_reports_refusal(&self, operation: &CloseOperation) -> bool {
        self.set.as_ref().is_some_and(|set| {
            set.capture.tabs.len() > 1
                && match &set.step {
                    Step::Ready => true,
                    Step::Halted {
                        awaiting: Some(key),
                    } => key == operation.key(),
                    _ => false,
                }
        })
    }

    /// Whether the halted member's submitted Close may still finish.
    fn set_unconfirmed(&self, set: &CloseSetRun) -> bool {
        set.halted_target()
            .is_some_and(|target| self.record_index(target).is_some())
    }

    /// Whether the set's report is final: it finished, or it stopped with nothing left in flight.
    fn set_settled(&self, set: &CloseSetRun) -> bool {
        match set.step {
            Step::Complete => true,
            Step::Halted { .. } => !self.set_unconfirmed(set),
            _ => false,
        }
    }
}

impl WorkbenchClientState {
    pub(super) fn close_set_focus(&self) -> LocalFocus {
        let presentation = self
            .active()
            .and_then(|active| {
                self.cache
                    .tabs()
                    .get(active.index())?
                    .panel(active.panel_index())
            })
            .map(|panel| self.selected_presentation(panel))
            .map(|presentation| {
                (
                    presentation.presentation_id().clone(),
                    presentation.binding().cloned(),
                )
            });
        LocalFocus {
            desired: self.desired.clone(),
            attached: self
                .focus_record()
                .map(|record| (record.pane_ref(), record.binding().clone())),
            presentation,
        }
    }

    fn close_set_marks(&self) -> Vec<WorkbenchTabId> {
        self.selected_tab_ids()
    }

    /// Freeze an explicit background target without navigating or impersonating active focus.
    pub(super) fn close_set_selection(
        &self,
        id: &WorkbenchTabId,
    ) -> Result<CloseSelection, CloseStateError> {
        let index = self
            .cache
            .tab_index(id)
            .ok_or(CloseStateError::UnknownTarget)?;
        let tab = &self.cache.tabs()[index];
        let active = self.active().filter(|active| active.index() == index);
        let binding = active
            .and_then(|active| tab.panel(active.panel_index()))
            .and_then(|panel| self.selected_presentation(panel).binding())
            .cloned();
        Ok(CloseSelection::new(
            CloseTarget::new(
                self.cache.session_id().clone(),
                self.cache
                    .terminal_context_id()
                    .ok_or(CloseStateError::UnknownTarget)?
                    .clone(),
                id.clone(),
            ),
            WorkbenchLabel::parse(&tab.display_label(index))
                .map_err(|_| CloseStateError::UnknownTarget)?,
            binding,
            active
                .and(self.desired.as_ref())
                .map(|key| key.anchor().clone()),
            self.cache.revision(),
        ))
    }

    pub(crate) fn open_close_set(&mut self, ids: &[WorkbenchTabId]) -> Result<(), CloseStateError> {
        if !self.setup_authority_available()
            || self.setup_blocks_structure()
            || ids.is_empty()
            || self.close.set_active()
            || self.close.confirmation.is_some()
        {
            return Err(CloseStateError::Busy);
        }
        let tabs: Vec<_> = self
            .cache
            .tabs()
            .iter()
            .filter(|tab| ids.contains(tab.tab_id()))
            .map(|tab| tab.tab_id().clone())
            .collect();
        if tabs.len() != ids.len() {
            return Err(CloseStateError::UnknownTarget);
        }
        let first = self.close_set_selection(&tabs[0])?;
        let labels = self
            .cache
            .tabs()
            .iter()
            .enumerate()
            .filter(|(_, tab)| ids.contains(tab.tab_id()))
            .map(|(index, tab)| (index + 1, tab.display_label(index).into_owned()))
            .collect();
        let capture = CloseSetCapture {
            first,
            source: Arc::new(self.cache.canonical_snapshot().clone()),
            tabs,
            labels,
            focus: self.close_set_focus(),
            marked: self.close_set_marks(),
        };
        let token = CloseConfirmationToken(self.close.token().ok_or(CloseStateError::Busy)?);
        self.close.confirmation = Some(CloseConfirmation {
            token,
            intent: CloseIntent::Set(Box::new(capture)),
            local_focus: None,
            presentation: crate::tui_presentation::PresentationPhase::Pending,
            shown: false,
        });
        self.close.set = None;
        Ok(())
    }

    pub(super) fn close_set_capture_current(&self, capture: &CloseSetCapture) -> bool {
        self.cache.canonical_snapshot() == capture.source.as_ref()
            && self.cache.session_id() == capture.first.target().session_id()
            && self.cache.terminal_context_id()
                == Some(capture.first.target().terminal_context_id())
            && self.close_set_focus() == capture.focus
            && self.close_set_marks() == capture.marked
            && self
                .close_set_selection(capture.first.target().tab_id())
                .as_ref()
                == Ok(&capture.first)
            && self.pending_refresh.is_none()
            && self.pending_membership.is_none()
            && !self.mutation_blocks_structure()
            && !self.rename_draft_blocks_structure()
    }

    pub(super) fn plan_first_close_set(
        &self,
        capture: &CloseSetCapture,
        request: WorkbenchRequestId,
        deadline: Instant,
    ) -> Result<CloseOperation, CloseStateError> {
        if !self.close_set_capture_current(capture) {
            return Err(CloseStateError::StaleConfirmation);
        }
        Ok(CloseOperation::new(
            request,
            capture.first.clone(),
            CloseOperationKind::Close,
            deadline,
        )
        .with_set_source(Arc::clone(&capture.source)))
    }

    pub(super) fn begin_first_close_set(
        &mut self,
        capture: CloseSetCapture,
        operation: &CloseOperation,
    ) {
        self.close.set = Some(CloseSetRun {
            source: Arc::clone(&capture.source),
            focus: capture.focus.clone(),
            marked: capture.marked.clone(),
            capture,
            completed: 0,
            deadline: operation.deadline(),
            step: Step::Waiting(operation.key().clone()),
            summary: SettledSummary::Pending,
        });
    }

    /// Check the old local premises before the membership owner changes them. Checking only
    /// after its commit would launder same-tick user navigation/marking into an owned transition.
    /// One Tab has no later Start for a local change to stop, such as the selection leaving it.
    pub(in crate::workbench_state) fn before_close_set_member_install(
        &mut self,
        target: &CloseTarget,
    ) {
        let Some(set) = &self.close.set else {
            return;
        };
        if !set.active() {
            return;
        }
        if !matches!(&set.step, Step::Verifying { key, .. } if key.target() == target)
            || (set.capture.tabs.len() > 1 && !self.close_set_premises_current(set))
        {
            self.close.halt_set();
        }
    }

    fn close_set_premises_current(&self, set: &CloseSetRun) -> bool {
        self.cache.canonical_snapshot() == set.source.as_ref()
            && self.cache.session_id() == set.capture.first.target().session_id()
            && self.cache.terminal_context_id()
                == Some(set.capture.first.target().terminal_context_id())
            && self.close_set_focus() == set.focus
            && self.close_set_marks() == set.marked
    }

    /// Only a correlated membership install can advance the frozen source or expected local focus.
    /// It adopts what it installed, so the Server's own following stays out of every later check.
    pub(in crate::workbench_state) fn installed_close_set_member(&mut self, target: &CloseTarget) {
        let Some(set) = &self.close.set else {
            return;
        };
        let Step::Verifying { key, revision } = &set.step else {
            return;
        };
        if key.target() != target {
            self.close.halt_set();
            return;
        }
        let mut expected = set.source.as_ref().clone();
        let Some(context) = expected
            .terminal_contexts
            .iter_mut()
            .find(|context| &context.terminal_context_id == key.target().terminal_context_id())
        else {
            self.close.halt_set();
            return;
        };
        let source_group = context
            .tabs
            .iter()
            .find(|tab| &tab.tab_id == key.target().tab_id())
            .and_then(|tab| tab.group_id.clone());
        context
            .tabs
            .retain(|tab| &tab.tab_id != key.target().tab_id());
        context.groups.retain(|group| {
            source_group.as_ref() != Some(&group.group_id)
                || context
                    .tabs
                    .iter()
                    .any(|tab| tab.group_id.as_ref() == Some(&group.group_id))
        });
        if self.cache.revision() < *revision
            || !close_set_premises_match(self.cache.canonical_snapshot(), &expected)
        {
            self.close.halt_set();
            return;
        }
        let installed = Arc::new(self.cache.canonical_snapshot().clone());
        let marked = self.close_set_marks();
        let focus = self.close_set_focus();
        let set = self
            .close
            .set
            .as_mut()
            .expect("the inspected coordinator remains owned here");
        set.source = installed;
        set.focus = focus;
        set.marked = marked;
        set.step = if set.completed == set.capture.tabs.len() {
            Step::Complete
        } else {
            Step::Ready
        };
    }

    pub(crate) fn settle_close_set(&mut self, now: Instant) -> bool {
        let settled = self
            .close
            .set
            .as_ref()
            .is_some_and(|set| self.close.set_settled(set));
        let Some(set) = &mut self.close.set else {
            return false;
        };
        if settled {
            return match set.summary {
                SettledSummary::Pending => {
                    set.summary = now
                        .checked_add(Duration::from_secs(5))
                        .map_or(SettledSummary::Hidden, SettledSummary::Until);
                    true
                }
                SettledSummary::Until(until) if now >= until => {
                    set.summary = SettledSummary::Hidden;
                    true
                }
                _ => false,
            };
        }
        if !set.active() {
            return false;
        }
        let set = self
            .close
            .set
            .as_ref()
            .expect("the coordinator remains owned here");
        // A matched close install updates these premises at its commit point. Every other
        // canonical or local focus/selection change stops the sequence before another Start;
        // one Tab has no other Start for that change to stop.
        if now >= set.deadline
            || (set.capture.tabs.len() > 1 && !self.close_set_premises_current(set))
            || matches!(self.phase, ClientPhase::Retired(_))
        {
            self.close.halt_set();
            return true;
        }
        false
    }

    pub(crate) fn close_set_deadline(&self) -> Option<Instant> {
        self.close
            .set
            .as_ref()
            .filter(|set| set.active())
            .map(|set| set.deadline)
    }

    pub(crate) fn close_set_summary_deadline(&self) -> Option<Instant> {
        match self.close.set.as_ref()?.summary {
            SettledSummary::Until(until) => Some(until),
            _ => None,
        }
    }

    /// Display retirement is independent of retained counts and CloseRecord recovery facts.
    pub(crate) fn dismiss_close_set_summary(&mut self) {
        let settled = self
            .close
            .set
            .as_ref()
            .is_some_and(|set| self.close.set_settled(set));
        if let Some(set) = &mut self.close.set
            && settled
        {
            set.summary = SettledSummary::Hidden;
        }
    }

    /// The count the status line shows. One Tab has no count to report: it speaks only when its
    /// retained close stopped short, and a refused close already left its own notice. Nothing is
    /// left to report once every Tab is confirmed closed, whatever stopped the sequence, and a
    /// final report clears with its summary. A submitted close that may still finish stays.
    pub(super) fn close_set_display_status(&self) -> Option<CloseSetStatus> {
        let set = self.close.set.as_ref()?;
        if matches!(set.summary, SettledSummary::Hidden) {
            return None;
        }
        let status = self.close_set_status()?;
        let visible = if status.total == 1 {
            status.unconfirmed
        } else {
            status.completed < status.total || !status.halted
        };
        visible.then_some(status)
    }

    pub(crate) fn close_set_ready(&self) -> bool {
        self.close
            .set
            .as_ref()
            .is_some_and(|set| matches!(set.step, Step::Ready))
            && self.close.records.is_empty()
            && self.close.pending.is_none()
            && self.close.completed_handoff.is_none()
    }

    pub(crate) fn plan_next_close_set(
        &self,
        request: WorkbenchRequestId,
        deadline: Instant,
    ) -> Result<CloseOperation, CloseStateError> {
        let set = self
            .close
            .set
            .as_ref()
            .filter(|set| matches!(set.step, Step::Ready))
            .ok_or(CloseStateError::Busy)?;
        if self.close.pending.is_some()
            || self.close.completed_handoff.is_some()
            || !self.close.records.is_empty()
            || self.pending_membership.is_some()
            || self.pending_refresh.is_some()
            || self.mutation_blocks_structure()
            || self.rename_draft_blocks_structure()
            || !self.close_set_premises_current(set)
            || deadline > set.deadline
            || !matches!(
                self.phase,
                ClientPhase::Ready(_)
                    | ClientPhase::SelectedWithoutAttachment(_)
                    | ClientPhase::RetryableAttachUnavailable(_)
            )
        {
            return Err(CloseStateError::Busy);
        }
        let id = set
            .capture
            .tabs
            .get(set.completed)
            .ok_or(CloseStateError::UnknownTarget)?;
        Ok(CloseOperation::new(
            request,
            self.close_set_selection(id)?,
            CloseOperationKind::Close,
            deadline,
        )
        .with_set_source(Arc::clone(&set.source)))
    }

    pub(crate) fn begin_next_close_set(
        &mut self,
        operation: &CloseOperation,
        now: Instant,
    ) -> Result<(), CloseStateError> {
        if self.settle_close_set(now) || now >= operation.deadline() {
            return Err(CloseStateError::StaleConfirmation);
        }
        if self.plan_next_close_set(operation.key().request_id().clone(), operation.deadline())?
            != *operation
        {
            return Err(CloseStateError::StaleConfirmation);
        }
        self.close
            .retain_submission(operation.selection().clone(), now)?;
        self.close.pending = Some(operation.clone());
        self.close.set.as_mut().ok_or(CloseStateError::Busy)?.step =
            Step::Waiting(operation.key().clone());
        Ok(())
    }

    pub(crate) fn close_set_status(&self) -> Option<CloseSetStatus> {
        self.close.set.as_ref().map(|set| CloseSetStatus {
            completed: set.completed,
            total: set.capture.tabs.len(),
            halted: matches!(set.step, Step::Halted { .. }),
            unconfirmed: self.close.set_unconfirmed(set),
        })
    }
}
