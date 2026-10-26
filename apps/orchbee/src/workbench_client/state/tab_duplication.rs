//! Bounded selected duplication. The ordinary Create owner retains every member's effects.
//! Only an exact membership install may replace this sequence's old concrete pane fence.
use super::{
    ClientContextTabs, ClientPhase, ClientTerminalBinding, CommittedTabCreate, CreateOperation,
    CreateOperationKey, CreateTerminalProvenance, MembershipInstallDecision, OperationDeadlines,
    PaneRef, WorkbenchClientState, WorkbenchTabId,
};
use orchbee_workbench_protocol::TerminalPresentationId;
use std::time::{Duration, Instant};

const TOTAL: Duration = Duration::from_secs(120);
const NOTICE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DuplicateStop {
    Changed,
    Deadline,
    NotCreated,
    TerminalNotStarted,
    Recovery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DuplicateProgress {
    pub(crate) total: usize,
    pub(crate) submitted: usize,
    pub(crate) committed: usize,
    pub(crate) started: usize,
    pub(crate) stop: Option<DuplicateStop>,
    pub(crate) complete: bool,
}

struct SelectionFence {
    marks: Vec<WorkbenchTabId>,
    pane: PaneRef,
    presentation: TerminalPresentationId,
    binding: ClientTerminalBinding,
}

impl SelectionFence {
    fn capture(state: &WorkbenchClientState) -> Option<Self> {
        let record = state.focus_record()?;
        if !record.attached || !record.projected || record.departing || record.fault.is_some() {
            return None;
        }
        let selected = state.active()?;
        let panel = state
            .cache
            .tabs()
            .get(selected.index())?
            .panel(selected.panel_index())?;
        let member = state.selected_presentation(panel);
        if member.binding() != Some(&record.binding) {
            return None;
        }
        Some(Self {
            marks: state.selected_tab_ids(),
            pane: record.pane_ref(),
            presentation: member.presentation_id().clone(),
            binding: record.binding.clone(),
        })
    }

    fn matches(&self, state: &WorkbenchClientState) -> bool {
        let Some(selected) = state.active() else {
            return false;
        };
        let Some(panel) = state
            .cache
            .tabs()
            .get(selected.index())
            .and_then(|tab| tab.panel(selected.panel_index()))
        else {
            return false;
        };
        let member = state.selected_presentation(panel);
        state.selected_tab_ids() == self.marks
            && state.focus_pane_ref().as_ref() == Some(&self.pane)
            && state
                .focus_record()
                .is_some_and(|record| record.binding == self.binding && record.fault.is_none())
            && member.presentation_id() == &self.presentation
            && member.binding() == Some(&self.binding)
    }
}

/// Facts survive notice expiry and a stopped tail. No later receipt restarts the sequence.
struct MemberFact {
    operation: CreateOperation,
    committed: Option<CommittedTabCreate>,
}

enum Flight {
    Creating(CreateOperationKey),
    Reattaching(CreateOperationKey),
}

pub(super) struct DuplicateSequence {
    sources: Vec<WorkbenchTabId>,
    accepted: ClientContextTabs,
    fence: SelectionFence,
    deadline: Instant,
    next: usize,
    flight: Option<Flight>,
    facts: Vec<MemberFact>,
    stop: Option<DuplicateStop>,
    notice_until: Option<Instant>,
    notice_hidden: bool,
}

/// Not Clone: derived under the install's exclusive state borrow, consumed once after pane minting.
pub(super) struct DuplicateInstallWitness {
    operation: CreateOperationKey,
    candidate: ClientContextTabs,
    pane: PaneRef,
}

impl DuplicateSequence {
    fn complete(&self) -> bool {
        self.next == self.sources.len()
    }
    fn live(&self) -> bool {
        self.stop.is_none() && !self.complete()
    }
    fn halt(&mut self, reason: DuplicateStop) {
        if self.live() {
            self.stop = Some(reason);
        }
    }
    fn matches(&self, state: &WorkbenchClientState) -> bool {
        self.accepted.location() == state.cache.location()
            && self.accepted.canonical_snapshot() == state.cache.canonical_snapshot()
            && self.fence.matches(state)
    }
    fn progress(&self) -> DuplicateProgress {
        DuplicateProgress {
            total: self.sources.len(),
            submitted: self.facts.len(),
            committed: self
                .facts
                .iter()
                .filter(|fact| fact.committed.is_some())
                .count(),
            started: self
                .facts
                .iter()
                .filter(|fact| {
                    fact.committed.as_ref().is_some_and(|commit| {
                        commit.provenance == CreateTerminalProvenance::Started
                    })
                })
                .count(),
            stop: self.stop,
            complete: self.complete(),
        }
    }
}

impl WorkbenchClientState {
    pub(in crate::workbench_client) fn begin_duplicate_sequence(
        &mut self,
        sources: Vec<WorkbenchTabId>,
        now: Instant,
    ) -> bool {
        if self
            .duplicate_sequence
            .as_ref()
            .is_some_and(DuplicateSequence::live)
            || !matches!(self.phase, ClientPhase::Ready(_))
            || sources.len() < 2
            || sources != self.selected_tab_ids()
        {
            return false;
        }
        let Some(fence) = SelectionFence::capture(self) else {
            return false;
        };
        let Some(deadline) = now.checked_add(TOTAL) else {
            return false;
        };
        self.duplicate_sequence = Some(DuplicateSequence {
            sources,
            accepted: self.cache.clone(),
            fence,
            deadline,
            next: 0,
            flight: None,
            facts: Vec::new(),
            stop: None,
            notice_until: None,
            notice_hidden: false,
        });
        true
    }

    /// Call on intent, not merely unequal final values: away/back navigation or marking is final.
    pub(in crate::workbench_client) fn cancel_duplicate_sequence(&mut self) {
        if let Some(sequence) = &mut self.duplicate_sequence {
            sequence.halt(DuplicateStop::Changed);
        }
    }

    pub(in crate::workbench_client) fn stop_duplicate_sequence(&mut self, reason: DuplicateStop) {
        if let Some(sequence) = &mut self.duplicate_sequence {
            sequence.halt(reason);
        }
    }

    pub(in crate::workbench_client) fn duplicate_progress(&self) -> Option<DuplicateProgress> {
        self.duplicate_sequence
            .as_ref()
            .filter(|sequence| !sequence.notice_hidden)
            .map(DuplicateSequence::progress)
    }

    pub(in crate::workbench_client) fn duplicate_waiting(&self) -> bool {
        self.duplicate_sequence
            .as_ref()
            .is_some_and(|sequence| sequence.live() && sequence.flight.is_none())
    }

    pub(in crate::workbench_client) fn duplicate_sequence_active(&self) -> bool {
        self.duplicate_sequence
            .as_ref()
            .is_some_and(DuplicateSequence::live)
    }

    pub(in crate::workbench_client) fn duplicate_owns_operation(
        &self,
        key: &CreateOperationKey,
    ) -> bool {
        self.duplicate_sequence
            .as_ref()
            .is_some_and(|sequence| sequence.facts.iter().any(|fact| &fact.operation.key == key))
    }

    #[cfg(test)]
    pub(in crate::workbench_client) fn duplicate_facts_for_test(
        &self,
    ) -> Option<DuplicateProgress> {
        self.duplicate_sequence
            .as_ref()
            .map(DuplicateSequence::progress)
    }

    pub(in crate::workbench_client) fn observe_duplicate_sequence(&mut self, now: Instant) -> bool {
        let Some(mut sequence) = self.duplicate_sequence.take() else {
            return false;
        };
        let before = (sequence.progress(), sequence.notice_hidden);
        if sequence.live() {
            if now >= sequence.deadline - super::CLEANUP_RESERVE
                || sequence.facts.last().is_some_and(|fact| {
                    sequence.flight.is_some() && now >= fact.operation.deadlines.forward_cutoff()
                })
            {
                sequence.halt(DuplicateStop::Deadline);
            } else if !sequence.matches(self) {
                sequence.halt(DuplicateStop::Changed);
            } else if matches!(
                self.phase,
                ClientPhase::ProjectionUnavailableBeforeCleanup(_)
                    | ClientPhase::Retired(_)
                    | ClientPhase::RetryableAttachUnavailable(_)
                    | ClientPhase::SelectedWithoutAttachment(_)
            ) {
                sequence.halt(DuplicateStop::Recovery);
            } else if let Some(Flight::Reattaching(key)) = &sequence.flight
                && self
                    .settled_create
                    .as_ref()
                    .is_some_and(|commit| &commit.operation.key == key)
                && matches!(self.phase, ClientPhase::Ready(_))
                && self
                    .focus_record()
                    .is_some_and(|record| record.attached && record.projected)
            {
                sequence.next += 1;
                sequence.flight = None;
            }
        }
        if !sequence.live() {
            let until = *sequence.notice_until.get_or_insert(now + NOTICE);
            if now >= until {
                sequence.notice_hidden = true;
            }
        }
        let changed = before != (sequence.progress(), sequence.notice_hidden);
        self.duplicate_sequence = Some(sequence);
        changed
    }

    /// Rechecked inside mailbox admission, immediately before build_create can change state.
    pub(in crate::workbench_client) fn next_duplicate(
        &mut self,
        now: Instant,
    ) -> Option<(WorkbenchTabId, OperationDeadlines)> {
        self.observe_duplicate_sequence(now);
        let sequence = self.duplicate_sequence.as_ref()?;
        if !sequence.live()
            || sequence.flight.is_some()
            || !matches!(self.phase, ClientPhase::Ready(_))
        {
            return None;
        }
        let total = now
            .checked_add(super::OPERATION_TOTAL)?
            .min(sequence.deadline);
        let deadlines = OperationDeadlines {
            total,
            forward_cutoff: total.checked_sub(super::CLEANUP_RESERVE)?,
        };
        (now < deadlines.forward_cutoff)
            .then(|| (sequence.sources[sequence.next].clone(), deadlines))
    }

    pub(in crate::workbench_client) fn duplicate_submitted(&mut self, operation: &CreateOperation) {
        let Some(sequence) = &mut self.duplicate_sequence else {
            return;
        };
        debug_assert!(sequence.live() && sequence.flight.is_none());
        sequence.flight = Some(Flight::Creating(operation.key.clone()));
        sequence.facts.push(MemberFact {
            operation: operation.clone(),
            committed: None,
        });
    }

    pub(super) fn duplicate_commit_fact(&mut self, committed: &CommittedTabCreate) {
        let Some(sequence) = &mut self.duplicate_sequence else {
            return;
        };
        let Some(fact) = sequence
            .facts
            .iter_mut()
            .find(|fact| fact.operation == committed.operation)
        else {
            return;
        };
        fact.committed = Some(committed.clone());
        if committed.provenance != CreateTerminalProvenance::Started {
            sequence.halt(DuplicateStop::TerminalNotStarted);
        }
    }

    pub(super) fn duplicate_precommit_failed(&mut self, operation: &CreateOperationKey) {
        if self.duplicate_sequence.as_ref().is_some_and(|sequence| {
            sequence
                .facts
                .iter()
                .any(|fact| &fact.operation.key == operation)
        }) {
            self.stop_duplicate_sequence(DuplicateStop::NotCreated);
        }
    }

    /// Critical: called before cache/desired/old records change, including after detach preparation.
    pub(super) fn prepare_duplicate_install(
        &mut self,
        committed: Option<&CommittedTabCreate>,
        candidate: &ClientContextTabs,
        decision: &MembershipInstallDecision,
    ) -> Option<DuplicateInstallWitness> {
        let sequence = self.duplicate_sequence.as_ref()?;
        if !sequence.live() {
            return None;
        }
        if committed.is_none() && candidate == &sequence.accepted && sequence.matches(self) {
            return None;
        }
        let valid = committed.filter(|commit| {
            matches!(&sequence.flight, Some(Flight::Creating(key)) if key == &commit.operation.key)
                && sequence.matches(self)
                && commit.operation.expected_revision == sequence.accepted.revision()
                && Some(candidate.revision().value())
                    == sequence.accepted.revision().value().checked_add(1)
                && super::validate_created_tab(
                    &sequence.accepted,
                    &commit.expectation,
                    candidate.clone(),
                )
                .is_ok()
        });
        let target = match decision {
            MembershipInstallDecision::Attach(target) => Some(target),
            _ => None,
        };
        let witness = valid
            .zip(target)
            .filter(|(_, target)| {
                target.selected.key.anchor().tab_id()
                    == sequence.fence.pane.pane().anchor().tab_id()
                    && target.selected.key.panel() == sequence.fence.pane.pane().panel()
                    && *target.presentation_id == sequence.fence.presentation
                    && target.binding.as_ref() == Some(&sequence.fence.binding)
            })
            .map(|(commit, target)| DuplicateInstallWitness {
                operation: commit.operation.key.clone(),
                candidate: candidate.clone(),
                pane: target.pane_ref(),
            });
        if witness.is_none() {
            self.cancel_duplicate_sequence();
        }
        witness
    }

    pub(super) fn commit_duplicate_install(&mut self, witness: DuplicateInstallWitness) {
        let Some(sequence) = &mut self.duplicate_sequence else {
            return;
        };
        // The exclusively borrowed install owns both old-record disposal and new-record minting.
        debug_assert!(
            matches!(&sequence.flight, Some(Flight::Creating(key)) if key == &witness.operation)
        );
        sequence.accepted = witness.candidate;
        sequence.fence.pane = witness.pane;
        sequence.flight = Some(Flight::Reattaching(witness.operation));
    }
}

impl super::WorkbenchClientView<'_> {
    pub(in crate::workbench_client) fn duplicate_progress(&self) -> Option<DuplicateProgress> {
        self.state.duplicate_progress()
    }
}
