//! One confirmed, frozen Tab set advances through existing single-Tab Close and membership owners.
//! A halt stops new submissions, never cancels or discards a submitted Close's recovery facts.
use super::{
    ClientTerminalBinding, CloseConfirmation, CloseConfirmationToken, CloseFlightKey, CloseIntent,
    CloseOperation, CloseOperationKind, CloseResult, CloseSelection, CloseState, CloseStateError,
    CloseTarget, PaneKey, WorkbenchLabel, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseStatus, WorkbenchTabId,
};
use crate::workbench_client::state::{ClientPhase, PaneRef, WorkbenchClientState};
use orchbee_workbench_protocol::{Snapshot, TerminalPresentationId, WorkbenchTabCloseReadOutcome};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum CloseSetKind {
    Selected,
    Other,
    Trailing,
    Group,
}

/// Resolve the GUI menu's exact set; only relative operations protect pinned Tabs.
pub(in crate::workbench_client) fn close_set_targets(
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LocalFocus {
    desired: Option<PaneKey>,
    attached: Option<(PaneRef, ClientTerminalBinding)>,
    presentation: Option<(TerminalPresentationId, Option<ClientTerminalBinding>)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct CloseSetCapture {
    first: CloseSelection,
    source: Arc<Snapshot>,
    tabs: Vec<WorkbenchTabId>,
    labels: Vec<(usize, String)>,
    context_label: String,
    focus: LocalFocus,
    marked: Vec<WorkbenchTabId>,
}

impl CloseSetCapture {
    pub(super) fn first(&self) -> &CloseSelection {
        &self.first
    }
    pub(in crate::workbench_client) fn count(&self) -> usize {
        self.tabs.len()
    }
    pub(in crate::workbench_client) fn labels(&self) -> &[(usize, String)] {
        &self.labels
    }
    pub(in crate::workbench_client) fn context_label(&self) -> &str {
        &self.context_label
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
    summary: CompletedSummary,
}

enum CompletedSummary {
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
    pub(in crate::workbench_client::state) fn set_active(&self) -> bool {
        self.set.as_ref().is_some_and(CloseSetRun::active)
    }

    pub(in crate::workbench_client::state) fn halt_set(&mut self) {
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

    pub(in crate::workbench_client) fn open_close_set(
        &mut self,
        ids: &[WorkbenchTabId],
    ) -> Result<(), CloseStateError> {
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
        let context_label = self
            .cache
            .navigation()
            .iter()
            .find(|entry| &entry.location == self.cache.location())
            .and_then(|entry| entry.workspace_label.as_ref())
            .map(|label| label.as_str().to_owned())
            .unwrap_or_default();
        let capture = CloseSetCapture {
            first,
            source: Arc::new(self.cache.canonical_snapshot().clone()),
            tabs,
            labels,
            context_label,
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
            summary: CompletedSummary::Pending,
        });
    }

    /// Check the old local premises before the membership owner changes them. Checking only
    /// after its commit would launder same-tick user navigation/marking into an owned transition.
    pub(in crate::workbench_client::state) fn before_close_set_member_install(
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
            || !self.close_set_premises_current(set)
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
    pub(in crate::workbench_client::state) fn installed_close_set_member(
        &mut self,
        target: &CloseTarget,
    ) {
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
        let sequential = expected.revision.value().checked_add(1) == Some(revision.value());
        expected.revision = *revision;
        if !sequential || self.cache.canonical_snapshot() != &expected {
            self.close.halt_set();
            return;
        }
        let marked = self.close_set_marks();
        let focus = self.close_set_focus();
        let set = self
            .close
            .set
            .as_mut()
            .expect("the inspected coordinator remains owned here");
        set.source = Arc::new(expected);
        set.focus = focus;
        set.marked = marked;
        set.step = if set.completed == set.capture.tabs.len() {
            Step::Complete
        } else {
            Step::Ready
        };
    }

    pub(in crate::workbench_client) fn settle_close_set(&mut self, now: Instant) -> bool {
        let Some(set) = &mut self.close.set else {
            return false;
        };
        if matches!(set.step, Step::Complete) {
            return match set.summary {
                CompletedSummary::Pending => {
                    set.summary = now
                        .checked_add(Duration::from_secs(5))
                        .map_or(CompletedSummary::Hidden, CompletedSummary::Until);
                    true
                }
                CompletedSummary::Until(until) if now >= until => {
                    set.summary = CompletedSummary::Hidden;
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
        // canonical or local focus/selection change stops the sequence before another Start.
        if now >= set.deadline
            || !self.close_set_premises_current(set)
            || matches!(self.phase, ClientPhase::Retired(_))
        {
            self.close.halt_set();
            return true;
        }
        false
    }

    pub(in crate::workbench_client) fn close_set_deadline(&self) -> Option<Instant> {
        self.close
            .set
            .as_ref()
            .filter(|set| set.active())
            .map(|set| set.deadline)
    }

    pub(in crate::workbench_client) fn close_set_summary_deadline(&self) -> Option<Instant> {
        let set = self.close.set.as_ref()?;
        match (&set.step, &set.summary) {
            (Step::Complete, CompletedSummary::Until(until)) => Some(*until),
            _ => None,
        }
    }

    /// Display retirement is independent of retained counts and CloseRecord recovery facts.
    pub(in crate::workbench_client) fn dismiss_close_set_summary(&mut self) {
        if let Some(set) = &mut self.close.set
            && matches!(set.step, Step::Complete)
        {
            set.summary = CompletedSummary::Hidden;
        }
    }

    pub(super) fn close_set_display_status(&self) -> Option<CloseSetStatus> {
        let set = self.close.set.as_ref()?;
        if matches!(
            (&set.step, &set.summary),
            (Step::Complete, CompletedSummary::Hidden)
        ) {
            return None;
        }
        self.close_set_status()
    }

    pub(in crate::workbench_client) fn close_set_ready(&self) -> bool {
        self.close
            .set
            .as_ref()
            .is_some_and(|set| matches!(set.step, Step::Ready))
            && self.close.records.is_empty()
            && self.close.pending.is_none()
            && self.close.completed_handoff.is_none()
    }

    pub(in crate::workbench_client) fn plan_next_close_set(
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

    pub(in crate::workbench_client) fn begin_next_close_set(
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
            .retain_submission(operation.selection().clone())?;
        self.close.pending = Some(operation.clone());
        self.close.set.as_mut().ok_or(CloseStateError::Busy)?.step =
            Step::Waiting(operation.key().clone());
        Ok(())
    }

    pub(in crate::workbench_client) fn close_set_status(&self) -> Option<CloseSetStatus> {
        self.close.set.as_ref().map(|set| CloseSetStatus {
            completed: set.completed,
            total: set.capture.tabs.len(),
            halted: matches!(set.step, Step::Halted { .. }),
        })
    }
}
