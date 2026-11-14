//! A confirmed set of several Tabs closes as one Server operation, `workbench.tab-set-close`: the
//! Client sends its Start once and follows it by Reads, one exchange per lease. Each member's
//! removal is an ordinary Tab Close publication. Every member leaves the strip as the Server accepts
//! the set; those it did not close come back with its report, and the member whose own Close it
//! left stopped or in flight is retained as an ordinary close record, so that Tab's own reads and
//! report follow it. A focus change stops nothing. One Tab is no set: it closes as the key does.
use super::{
    ClientTerminalBinding, CloseConfirmation, CloseConfirmationToken, CloseIntent, CloseOperation,
    CloseOperationKind, CloseResult, CloseSelection, CloseState, CloseStateError, CloseTarget,
    PaneKey, WorkbenchLabel, WorkbenchRequestId, WorkbenchRevision, WorkbenchTabCloseStatus,
    WorkbenchTabId,
};
use crate::workbench_state::{ClientNotice, ClientPhase, PaneRef, WorkbenchClientState};
use orchbee_workbench_protocol::{
    Snapshot, Tab, TabSetCloseAction, TabSetCloseCondition, TabSetCloseFailure, TabSetCloseMember,
    TabSetCloseOutcome, TabSetCloseRejection, TabSetCloseStatus, TerminalPresentationId,
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
    /// The members' Tabs in `snapshot`, in its order.
    fn member_tabs<'a>(&self, snapshot: &'a Snapshot) -> Option<Vec<&'a Tab>> {
        let context = snapshot.terminal_contexts.iter().find(|context| {
            &context.terminal_context_id == self.first.target().terminal_context_id()
        })?;
        Some(
            context
                .tabs
                .iter()
                .filter(|tab| self.tabs.contains(&tab.tab_id))
                .collect(),
        )
    }
}

/// How often a Start refused for a revision race goes again while its members stand as frozen.
const MAXIMUM_RESENDS: u8 = 4;
/// How often an accepted set is read while no publication wakes it sooner.
const READ_INTERVAL: Duration = Duration::from_millis(250);
/// How long the report of a settled set stays, as any bounded notice does.
const SUMMARY: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    /// The Start is on its way; nothing has left yet.
    Starting,
    /// A revision race refused the Start; it goes again once this Client holds `revision`, and
    /// gives up at `until`.
    Resend {
        revision: WorkbenchRevision,
        until: Instant,
    },
    /// The Server holds the set and closes its members.
    Pending,
    Closed,
    Failed(TabSetCloseFailure),
    /// The Client lost the set before it settled.
    Lost,
}

pub(super) struct TabSetFollow {
    capture: CloseSetCapture,
    condition: TabSetCloseCondition,
    /// The set's identity at the Server: the request identity of the Start that admitted it.
    operation: WorkbenchRequestId,
    /// When the current Start was admitted.
    asked_at: Instant,
    resends: u8,
    phase: Phase,
    /// The Server's latest report, from the first that accepted the set.
    members: Option<Vec<TabSetCloseMember>>,
    /// The member whose own Close the set left stopped or in flight, now an ordinary close record,
    /// and whether that close has since been seen closed.
    adopted: Option<(CloseTarget, bool)>,
    read_at: Option<Instant>,
    wake: bool,
    summary: SettledSummary,
}

/// The report of a set with nothing left in flight, which clears like any other bounded notice.
enum SettledSummary {
    Pending,
    Until(Instant),
    Hidden,
}

pub(crate) use crate::localization::CloseSetStatus;

impl TabSetFollow {
    fn new(
        capture: CloseSetCapture,
        condition: TabSetCloseCondition,
        operation: WorkbenchRequestId,
        asked_at: Instant,
    ) -> Self {
        Self {
            capture,
            condition,
            operation,
            asked_at,
            resends: 0,
            phase: Phase::Starting,
            members: None,
            adopted: None,
            read_at: None,
            wake: false,
            summary: SettledSummary::Pending,
        }
    }

    /// Whether the set still runs or may still start: no other set or Close may begin meanwhile.
    fn active(&self) -> bool {
        matches!(
            self.phase,
            Phase::Starting | Phase::Resend { .. } | Phase::Pending
        )
    }

    fn halted(&self) -> bool {
        matches!(self.phase, Phase::Failed(_) | Phase::Lost)
    }

    /// Whether this Tab is out of the Client's view with the set: every member while the Server
    /// closes them, and afterwards each one it closed until the publication removes it.
    fn leaving(&self, tab_id: &WorkbenchTabId) -> bool {
        self.members.as_ref().is_some_and(|members| {
            members.iter().any(|member| {
                &member.tab_id == tab_id && (self.phase == Phase::Pending || member.is_done())
            })
        })
    }

    /// The member whose own Close the stopped set left unfinished, while that close may still
    /// finish.
    pub(super) fn halted_target(&self) -> Option<&CloseTarget> {
        match &self.adopted {
            Some((target, false)) if self.halted() => Some(target),
            _ => None,
        }
    }

    fn read_due(&self, now: Instant) -> bool {
        self.phase == Phase::Pending
            && (self.wake
                || self
                    .read_at
                    .is_none_or(|at| now.saturating_duration_since(at) >= READ_INTERVAL))
    }

    /// Takes the Server's report of the accepted set. Its members must be the frozen ones, and a
    /// settled set never runs again.
    fn observe(
        &mut self,
        terminal_context_id: &orchbee_workbench_protocol::WorkbenchTerminalContextId,
        condition: TabSetCloseCondition,
        status: &TabSetCloseStatus,
        members: &[TabSetCloseMember],
    ) -> Result<(), CloseStateError> {
        if terminal_context_id != self.capture.first.target().terminal_context_id()
            || condition != self.condition
            || !members
                .iter()
                .map(|member| &member.tab_id)
                .eq(&self.capture.tabs)
        {
            return Err(CloseStateError::OperationMismatch);
        }
        if matches!(self.phase, Phase::Closed | Phase::Failed(_))
            && (matches!(status, TabSetCloseStatus::Pending {})
                || self.members.as_deref() != Some(members))
        {
            return Err(CloseStateError::FactRegression);
        }
        self.members = Some(members.to_vec());
        self.phase = match status {
            TabSetCloseStatus::Pending {} => Phase::Pending,
            TabSetCloseStatus::Closed {} => Phase::Closed,
            TabSetCloseStatus::Failed { reason } => Phase::Failed(*reason),
        };
        Ok(())
    }
}

impl CloseState {
    pub(in crate::workbench_state) fn set_active(&self) -> bool {
        self.set.as_ref().is_some_and(TabSetFollow::active)
    }

    /// Whether the Server decides an if-idle set: its armed confirmation stays hidden until then.
    pub(super) fn set_deciding(&self) -> bool {
        self.set.as_ref().is_some_and(|set| {
            set.condition == TabSetCloseCondition::IfIdle
                && matches!(set.phase, Phase::Starting | Phase::Resend { .. })
        })
    }

    pub(super) fn set_leaving(&self, tab_id: &WorkbenchTabId) -> bool {
        self.set.as_ref().is_some_and(|set| set.leaving(tab_id))
    }

    pub(super) fn wake_set_for_publication(&mut self) {
        if let Some(set) = &mut self.set {
            set.wake = set.phase == Phase::Pending;
        }
    }

    /// The Client's carrier is gone: an accepted set it can no longer read is lost, and a Start
    /// waiting out a revision race never goes. A Start in flight settles by its own result.
    pub(in crate::workbench_state) fn lose_set(&mut self) {
        let Some(set) = &mut self.set else {
            return;
        };
        match set.phase {
            Phase::Pending => {
                set.phase = Phase::Lost;
                self.adopt_set_child();
            }
            Phase::Resend { .. } => self.set = None,
            Phase::Starting | Phase::Closed | Phase::Failed(_) | Phase::Lost => {}
        }
    }

    /// Keeps the Close the set left stopped or in flight as an ordinary record, so its own reads and
    /// report follow it. Only the first unfinished member can have one.
    fn adopt_set_child(&mut self) {
        let Some(set) = &self.set else {
            return;
        };
        let Some(member) = set
            .members
            .as_ref()
            .and_then(|members| members.iter().find(|member| !member.is_done()))
        else {
            return;
        };
        let Some(child) = member.close.clone() else {
            return;
        };
        let target = CloseTarget::new(
            set.capture.first.target().session_id().clone(),
            set.capture.first.target().terminal_context_id().clone(),
            member.tab_id.clone(),
        );
        if self.record_index(&target).is_some() {
            return;
        }
        let selection = CloseSelection::new(
            target.clone(),
            member.label.clone(),
            None,
            None,
            set.capture.source.revision,
        );
        let asked_at = set.asked_at;
        if self.retain_submission(selection, asked_at).is_err() {
            return;
        }
        let Some(index) = self.record_index(&target) else {
            return;
        };
        let token = self.token().map(super::CloseStatusToken);
        let record = &mut self.records[index];
        record.operation = Some(child.request_id.clone());
        record.status_token = token;
        record.initial_read_pending = child.observation.is_none();
        if let Some(observation) = child.observation
            && self
                .observe_available(
                    &target,
                    child.request_id,
                    observation.attempt_generation,
                    observation.status,
                )
                .is_err()
        {
            if let Some(index) = self.record_index(&target) {
                self.records.remove(index);
            }
            return;
        }
        if let Some(set) = &mut self.set {
            set.adopted = Some((target, false));
        }
    }

    /// The adopted member's own close was seen closed: the report counts it.
    pub(super) fn set_member_closed(&mut self, target: &CloseTarget) {
        if let Some(set) = &mut self.set
            && let Some((adopted, closed)) = &mut set.adopted
            && adopted == target
        {
            *closed = true;
        }
    }

    /// The armed confirmation is this follow's own: an if-idle set whose Server answer decides it.
    fn set_confirmation_armed(&self) -> bool {
        let Some(set) = &self.set else {
            return false;
        };
        matches!(&self.confirmation, Some(CloseConfirmation {
            intent: CloseIntent::Set(capture),
            ..
        }) if capture.tabs == set.capture.tabs)
    }

    /// Ends a set the Server holds nothing of, withdrawing the confirmation its if-idle Start kept.
    fn drop_set(&mut self) {
        if self.set_confirmation_armed() {
            self.cancel_confirmation();
        }
        self.set = None;
    }

    /// Settles one set exchange: the pending one, or one a lost carrier's worker returned.
    pub(super) fn apply_set_result(
        &mut self,
        operation: &CloseOperation,
        result: &CloseResult,
    ) -> Result<(), CloseStateError> {
        let CloseOperationKind::TabSet(action) = operation.kind() else {
            return Err(CloseStateError::OperationMismatch);
        };
        let set = self.set.as_mut().ok_or(CloseStateError::UnknownTarget)?;
        if action.operation_id() != &set.operation {
            return Err(CloseStateError::OperationMismatch);
        }
        if let TabSetCloseAction::Read { .. } = action {
            // A carrier lost while this Read was out already stopped following the set.
            let lost = match set.phase {
                Phase::Pending => false,
                Phase::Lost => true,
                _ => return Err(CloseStateError::OperationMismatch),
            };
            match result {
                CloseResult::TabSet(TabSetCloseOutcome::Available {
                    terminal_context_id,
                    condition,
                    status,
                    members,
                }) => {
                    set.observe(terminal_context_id, *condition, status, members)?;
                    if lost && set.phase == Phase::Pending {
                        set.phase = Phase::Lost;
                    }
                }
                CloseResult::NotSubmitted
                | CloseResult::ObservationUnavailable
                | CloseResult::TabSet(_) => set.phase = Phase::Lost,
                CloseResult::Precommit(_)
                | CloseResult::Accepted(_)
                | CloseResult::Read(_)
                | CloseResult::Ambiguous => return Err(CloseStateError::OperationMismatch),
            }
            if set.halted() {
                self.adopt_set_child();
            }
            return Ok(());
        }
        if set.phase != Phase::Starting {
            return Err(CloseStateError::OperationMismatch);
        }
        match result {
            CloseResult::TabSet(TabSetCloseOutcome::Available {
                terminal_context_id,
                condition,
                status,
                members,
            }) => {
                set.observe(terminal_context_id, *condition, status, members)?;
                let halted = set.halted();
                // Accepted, an if-idle set closes unasked.
                if self.set_confirmation_armed() {
                    self.cancel_confirmation();
                }
                if halted {
                    self.adopt_set_child();
                }
            }
            // Nothing is retained, and the armed confirmation now presents.
            CloseResult::TabSet(TabSetCloseOutcome::ConfirmationRequired {}) => self.set = None,
            CloseResult::TabSet(TabSetCloseOutcome::Rejected {
                revision,
                reason: TabSetCloseRejection::RevisionMismatch,
            }) => {
                set.phase = Phase::Resend {
                    revision: *revision,
                    until: operation.deadline(),
                };
            }
            CloseResult::NotSubmitted | CloseResult::TabSet(_) => self.drop_set(),
            // The Start may have been admitted, and no Read follows it on this carrier.
            CloseResult::Ambiguous => {
                set.phase = Phase::Lost;
                if self.set_confirmation_armed() {
                    self.cancel_confirmation();
                }
            }
            CloseResult::Precommit(_)
            | CloseResult::Accepted(_)
            | CloseResult::Read(_)
            | CloseResult::ObservationUnavailable => {
                return Err(CloseStateError::OperationMismatch);
            }
        }
        Ok(())
    }

    /// Whether the adopted member's own close may still finish.
    fn set_unconfirmed(&self, set: &TabSetFollow) -> bool {
        set.adopted
            .as_ref()
            .is_some_and(|(target, closed)| !closed && self.record_index(target).is_some())
    }

    /// Whether the set's report is final: it finished, or it stopped with nothing left in flight.
    fn set_settled(&self, set: &TabSetFollow) -> bool {
        matches!(set.phase, Phase::Closed | Phase::Failed(_) | Phase::Lost)
            && !self.set_unconfirmed(set)
    }

    fn set_status(&self, set: &TabSetFollow) -> Option<CloseSetStatus> {
        let members = set.members.as_ref()?;
        Some(CloseSetStatus {
            completed: members.iter().filter(|member| member.is_done()).count()
                + usize::from(set.adopted.as_ref().is_some_and(|(_, closed)| *closed)),
            total: set.capture.count(),
            halted: set.halted(),
            unconfirmed: self.set_unconfirmed(set),
        })
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

    /// Freezes `ids` in their current order with everything the confirmation names.
    fn capture_close_set(
        &self,
        ids: &[WorkbenchTabId],
    ) -> Result<CloseSetCapture, CloseStateError> {
        let tabs: Vec<_> = self
            .cache
            .tabs()
            .iter()
            .filter(|tab| ids.contains(tab.tab_id()))
            .map(|tab| tab.tab_id().clone())
            .collect();
        if tabs.is_empty() || tabs.len() != ids.len() {
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
        Ok(CloseSetCapture {
            first,
            source: Arc::new(self.cache.canonical_snapshot().clone()),
            tabs,
            labels,
            focus: self.close_set_focus(),
            marked: self.close_set_marks(),
        })
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
        let capture = self.capture_close_set(ids)?;
        let token = CloseConfirmationToken(self.close.token().ok_or(CloseStateError::Busy)?);
        self.close.confirmation = Some(CloseConfirmation {
            token,
            intent: CloseIntent::Set(Box::new(capture)),
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

    /// Whether every member is still the Tab the set froze, in the same order.
    fn close_set_members_unchanged(&self, capture: &CloseSetCapture) -> bool {
        self.cache.session_id() == capture.first.target().session_id()
            && capture.member_tabs(&capture.source).is_some_and(|frozen| {
                frozen.len() == capture.tabs.len()
                    && capture.member_tabs(self.cache.canonical_snapshot()) == Some(frozen)
            })
    }

    /// The Start of a confirmed set of several Tabs, conditional or confirmed.
    pub(super) fn plan_tab_set_start(
        &self,
        capture: &CloseSetCapture,
        request: WorkbenchRequestId,
        deadline: Instant,
        condition: TabSetCloseCondition,
    ) -> Result<CloseOperation, CloseStateError> {
        if self.close.set_active() || !matches!(self.close.carrier, super::CloseCarrier::Client) {
            return Err(CloseStateError::Busy);
        }
        if !self.close_set_capture_current(capture) {
            return Err(CloseStateError::StaleConfirmation);
        }
        Ok(tab_set_start(
            capture.first.clone(),
            &capture.tabs,
            request,
            self.cache.revision(),
            condition,
            deadline,
        ))
    }

    /// Whether a Start refused for a revision race may go again now: this Client holds that
    /// revision and every member is the Tab the set froze.
    fn close_set_resend_ready(&self, set: &TabSetFollow, now: Instant) -> bool {
        matches!(set.phase, Phase::Resend { revision, until }
            if self.cache.revision() >= revision && now < until)
            && set.resends < MAXIMUM_RESENDS
            && self.pending_refresh.is_none()
            && self.pending_membership.is_none()
            && !self.mutation_blocks_structure()
            && matches!(self.close.carrier, super::CloseCarrier::Client)
            && self.close_set_members_unchanged(&set.capture)
    }

    pub(crate) fn plan_tab_set_resend(
        &self,
        request: WorkbenchRequestId,
        deadline: Instant,
        now: Instant,
    ) -> Result<CloseOperation, CloseStateError> {
        let set = self.close.set.as_ref().ok_or(CloseStateError::Busy)?;
        if self.close.pending.is_some() || !self.close_set_resend_ready(set, now) {
            return Err(CloseStateError::Busy);
        }
        Ok(tab_set_start(
            self.close_set_selection(set.capture.first.target().tab_id())?,
            &set.capture.tabs,
            request,
            self.cache.revision(),
            set.condition,
            deadline,
        ))
    }

    pub(crate) fn plan_tab_set_read(
        &self,
        request: WorkbenchRequestId,
        deadline: Instant,
        now: Instant,
    ) -> Result<CloseOperation, CloseStateError> {
        let set = self
            .close
            .set
            .as_ref()
            .filter(|set| set.read_due(now))
            .ok_or(CloseStateError::Busy)?;
        if self.close.pending.is_some()
            || !matches!(self.close.carrier, super::CloseCarrier::Client)
            || matches!(self.phase, ClientPhase::Retired(_))
        {
            return Err(CloseStateError::Busy);
        }
        Ok(CloseOperation::new(
            request,
            set.capture.first.clone(),
            CloseOperationKind::TabSet(TabSetCloseAction::Read {
                operation_id: set.operation.clone(),
            }),
            deadline,
        ))
    }

    /// Admits one set exchange the matching planner produced: a first Start from the confirmation,
    /// a resend after a revision race, or a Read.
    pub(super) fn begin_tab_set_exchange(
        &mut self,
        operation: &CloseOperation,
        admitted_at: Instant,
    ) -> Result<(), CloseStateError> {
        let CloseOperationKind::TabSet(action) = operation.kind() else {
            return Err(CloseStateError::OperationMismatch);
        };
        let request = operation.key().request_id().clone();
        let deadline = operation.deadline();
        let resending = self
            .close
            .set
            .as_ref()
            .is_some_and(|set| matches!(set.phase, Phase::Resend { .. }));
        match action {
            TabSetCloseAction::Read { .. } => {
                if self.plan_tab_set_read(request, deadline, admitted_at)? != *operation {
                    return Err(CloseStateError::Busy);
                }
                let set = self.close.set.as_mut().ok_or(CloseStateError::Busy)?;
                set.read_at = Some(admitted_at);
                set.wake = false;
            }
            TabSetCloseAction::Start { .. } if resending => {
                if self.plan_tab_set_resend(request.clone(), deadline, admitted_at)? != *operation {
                    return Err(CloseStateError::StaleConfirmation);
                }
                let tabs = self
                    .close
                    .set
                    .as_ref()
                    .map(|set| set.capture.tabs.clone())
                    .unwrap_or_default();
                // The unchanged members go again as they stand now, and so does the hidden
                // confirmation of an if-idle set, which the Server's answer still decides.
                let capture = self.capture_close_set(&tabs)?;
                if self.close.set_confirmation_armed()
                    && let Some(CloseConfirmation {
                        intent: CloseIntent::Set(armed),
                        ..
                    }) = &mut self.close.confirmation
                {
                    **armed = capture.clone();
                }
                let set = self.close.set.as_mut().ok_or(CloseStateError::Busy)?;
                set.capture = capture;
                set.operation = request;
                set.asked_at = admitted_at;
                set.resends += 1;
                set.phase = Phase::Starting;
            }
            TabSetCloseAction::Start { condition, .. } => {
                let planned = match condition {
                    TabSetCloseCondition::IfIdle => {
                        let token = self
                            .close
                            .confirmation
                            .as_ref()
                            .map(|prompt| prompt.token)
                            .ok_or(CloseStateError::StaleConfirmation)?;
                        self.plan_close_if_idle(token, request.clone(), deadline, admitted_at)?
                    }
                    TabSetCloseCondition::Confirmed => {
                        self.plan_close_confirmation(request.clone(), deadline)?
                    }
                };
                if planned != *operation {
                    return Err(CloseStateError::StaleConfirmation);
                }
                let Some(CloseConfirmation {
                    intent: CloseIntent::Set(capture),
                    ..
                }) = &self.close.confirmation
                else {
                    return Err(CloseStateError::StaleConfirmation);
                };
                let mut set =
                    TabSetFollow::new((**capture).clone(), *condition, request, admitted_at);
                set.read_at = Some(admitted_at);
                self.close.set = Some(set);
                // A confirmed set no longer asks; an if-idle one keeps its confirmation armed, and
                // hidden, until the Server answers.
                if *condition == TabSetCloseCondition::Confirmed {
                    self.close.cancel_confirmation();
                }
            }
        }
        Ok(())
    }

    /// Settles what a set decides on its own: a resend that can no longer go, and the expiry of a
    /// settled set's report.
    pub(crate) fn settle_close_set(&mut self, now: Instant) -> bool {
        if let Some(set) = &self.close.set
            && let Phase::Resend { revision, until } = set.phase
        {
            let caught_up = self.cache.revision() >= revision
                && self.pending_refresh.is_none()
                && self.pending_membership.is_none();
            if now >= until
                || (caught_up
                    && (set.resends >= MAXIMUM_RESENDS
                        || !self.close_set_members_unchanged(&set.capture)))
            {
                self.close.drop_set();
                self.set_notice(ClientNotice::CloseRequestNotApplied);
                return true;
            }
            return false;
        }
        let settled = self
            .close
            .set
            .as_ref()
            .is_some_and(|set| self.close.set_settled(set));
        let Some(set) = &mut self.close.set else {
            return false;
        };
        if !settled {
            return false;
        }
        match set.summary {
            SettledSummary::Pending => {
                set.summary = now
                    .checked_add(SUMMARY)
                    .map_or(SettledSummary::Hidden, SettledSummary::Until);
                true
            }
            SettledSummary::Until(until) if now >= until => {
                set.summary = SettledSummary::Hidden;
                true
            }
            _ => false,
        }
    }

    /// What a settled set exchange asks of the Client beyond its own facts: a required
    /// confirmation presents the set as it stands now, and a Start that lost a revision race asks
    /// for the revision it goes again at.
    pub(super) fn settle_tab_set_exchange(&mut self, result: &CloseResult) {
        if matches!(
            result,
            CloseResult::TabSet(TabSetCloseOutcome::ConfirmationRequired {})
        ) {
            self.present_close_set_confirmation();
        }
        let behind = self.close.set.as_ref().is_some_and(|set| {
            matches!(set.phase, Phase::Resend { revision, .. } if self.cache.revision() < revision)
        });
        if behind {
            self.note_publication_hint();
        }
    }

    /// The members are still the Tabs the Start named, while the focus, the marks or another Tab
    /// may have moved meanwhile: the confirmation names the set as it stands now. A changed member
    /// withdraws it.
    fn present_close_set_confirmation(&mut self) {
        let Some(CloseConfirmation {
            intent: CloseIntent::Set(armed),
            ..
        }) = &self.close.confirmation
        else {
            return;
        };
        if self.close_set_capture_current(armed) {
            return;
        }
        let fresh = self
            .close_set_members_unchanged(armed)
            .then(|| self.capture_close_set(armed.tabs()))
            .and_then(Result::ok);
        match (fresh, &mut self.close.confirmation) {
            (
                Some(fresh),
                Some(CloseConfirmation {
                    intent: CloseIntent::Set(armed),
                    ..
                }),
            ) => **armed = fresh,
            _ => {
                self.close.cancel_confirmation();
                self.set_notice(ClientNotice::ConfirmationCancelled);
            }
        }
    }

    pub(crate) fn close_set_resend_due(&self, now: Instant) -> bool {
        self.close.pending.is_none()
            && self
                .close
                .set
                .as_ref()
                .is_some_and(|set| self.close_set_resend_ready(set, now))
    }

    pub(crate) fn close_set_read_due(&self, now: Instant) -> bool {
        self.close.pending.is_none() && self.close.set.as_ref().is_some_and(|set| set.read_due(now))
    }

    /// When the set next needs a tick of its own: its next Read while it runs, or the end of a
    /// resend's wait. A Read already due waits for the event that frees its slot instead.
    pub(crate) fn close_set_wake(&self, now: Instant) -> Option<Instant> {
        let set = self.close.set.as_ref()?;
        let at = match set.phase {
            Phase::Pending => set.read_at?.checked_add(READ_INTERVAL)?,
            Phase::Resend { until, .. } => until,
            Phase::Starting | Phase::Closed | Phase::Failed(_) | Phase::Lost => return None,
        };
        (at > now).then_some(at)
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

    /// The count the status line shows once the Server accepts the set. Nothing is left to report
    /// once every Tab is confirmed closed, whatever stopped the set, and a final report clears
    /// with its summary. A member's close that may still finish keeps it.
    pub(super) fn close_set_display_status(&self) -> Option<CloseSetStatus> {
        let set = self.close.set.as_ref()?;
        if matches!(set.summary, SettledSummary::Hidden) {
            return None;
        }
        let status = self.close.set_status(set)?;
        (status.completed < status.total || !status.halted).then_some(status)
    }

    #[cfg(test)]
    pub(crate) fn close_set_status(&self) -> Option<CloseSetStatus> {
        self.close
            .set
            .as_ref()
            .and_then(|set| self.close.set_status(set))
    }
}

fn tab_set_start(
    first: CloseSelection,
    tabs: &[WorkbenchTabId],
    request: WorkbenchRequestId,
    revision: WorkbenchRevision,
    condition: TabSetCloseCondition,
    deadline: Instant,
) -> CloseOperation {
    let terminal_context_id = first.target().terminal_context_id().clone();
    CloseOperation::new(
        request.clone(),
        first,
        CloseOperationKind::TabSet(TabSetCloseAction::Start {
            operation_id: request,
            terminal_context_id,
            tab_ids: tabs.to_vec(),
            expected_revision: revision,
            condition,
        }),
        deadline,
    )
}

/// Whether this set Start was refused whole: nothing closed, the Client keeps every member, and
/// the user is told. A revision race goes again instead, and a required confirmation presents.
pub(super) fn tab_set_start_refused(operation: &CloseOperation, result: &CloseResult) -> bool {
    matches!(
        operation.kind(),
        CloseOperationKind::TabSet(TabSetCloseAction::Start { .. })
    ) && match result {
        CloseResult::NotSubmitted => true,
        CloseResult::TabSet(outcome) => !matches!(
            outcome,
            TabSetCloseOutcome::Available { .. }
                | TabSetCloseOutcome::ConfirmationRequired {}
                | TabSetCloseOutcome::Rejected {
                    reason: TabSetCloseRejection::RevisionMismatch,
                    ..
                }
        ),
        _ => false,
    }
}

/// Whether the adopted record's read saw its close closed.
pub(super) const fn closed(status: &WorkbenchTabCloseStatus) -> bool {
    matches!(status, WorkbenchTabCloseStatus::Closed { .. })
}
