//! Several Tabs duplicate as one retained Server operation: this Client sends its Start once and
//! follows the Server's published progress by operation ID. One Tab stays an ordinary duplicate
//! Create. The Client never copies, cancels or replays a member itself.
use super::{ClientActionDisposition, WorkbenchClientUi};
use crate::{
    local_client::{LocalClientOrigin, LocalOperationClock, subscription::Request},
    localization::{DuplicateProgress, DuplicateStop},
    workbench_client::mailbox::CommandMailbox,
    workbench_state::WorkbenchClientState,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, TabCreationPlacement, TabDuplicateAction, TabDuplicateFailure,
    TabDuplicateMember, TabDuplicateOutcome, TabDuplicateRejection, TabDuplicateStatus,
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision,
    WorkbenchTabDuplicateRequest, WorkbenchTabId, WorkbenchTerminalGeometry,
    WorkbenchTerminalOutcome,
};
use std::time::{Duration, Instant};

/// How long this Client follows one operation: the Server's 120 seconds and room for a last read.
const FOLLOW: Duration = Duration::from_secs(130);
/// How long an ended operation's counts stay on the status line.
const NOTICE: Duration = Duration::from_secs(5);
/// How often a pending operation is read, and how soon a lost exchange goes again.
const READ_INTERVAL: Duration = Duration::from_millis(100);
/// How long one exchange waits for its reply.
const REPLY: Duration = Duration::from_secs(2);
/// How often a Start refused for a revision race goes again.
const MAXIMUM_RESENDS: u8 = 4;

#[derive(Default)]
pub(super) struct Duplication {
    origin: Option<LocalClientOrigin>,
    follow: Option<Follow>,
}

struct Follow {
    /// The Start, sent unchanged after a lost reply so the Server observes the same operation.
    start: WorkbenchTabDuplicateRequest,
    /// The Server retained the operation; it is observed by Read from now on.
    admitted: bool,
    flight: Option<(WorkbenchRequestId, Request)>,
    /// A Start refused for this revision goes again once this Client holds it.
    stale: Option<WorkbenchRevision>,
    resends: u8,
    next_at: Instant,
    deadline: Instant,
    progress: DuplicateProgress,
    ended_at: Option<Instant>,
}

impl Duplication {
    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.origin = Some(origin);
    }

    fn live(&self) -> bool {
        self.follow
            .as_ref()
            .is_some_and(|follow| follow.ended_at.is_none())
    }

    pub(super) fn progress(&self) -> Option<DuplicateProgress> {
        self.follow.as_ref().map(|follow| follow.progress)
    }

    /// When the follow next needs a turn: its next exchange or the end of its status.
    pub(super) fn wake(&self) -> Option<Instant> {
        let follow = self.follow.as_ref()?;
        match follow.ended_at {
            Some(ended) => ended.checked_add(NOTICE),
            None if follow.flight.is_none() => Some(follow.next_at),
            None => None,
        }
    }

    /// Ends a live follow because this Client lost its connection; the operation itself goes on.
    pub(super) fn interrupt(&mut self, now: Instant) {
        if let Some(follow) = self.follow.as_mut() {
            follow.end(Some(DuplicateStop::Recovery), now);
        }
    }

    /// Hides an ended operation's counts once they have shown long enough.
    pub(super) fn expire(&mut self, now: Instant) -> bool {
        let expired = self.follow.as_ref().is_some_and(|follow| {
            follow
                .ended_at
                .and_then(|ended| ended.checked_add(NOTICE))
                .is_some_and(|until| now >= until)
        });
        if expired {
            self.follow = None;
        }
        expired
    }
}

impl Follow {
    fn end(&mut self, stop: Option<DuplicateStop>, now: Instant) {
        if self.ended_at.is_some() {
            return;
        }
        self.flight = None;
        if stop.is_some() {
            self.progress.stop = stop;
        }
        self.ended_at = Some(now);
    }

    fn operation_id(&self) -> &WorkbenchRequestId {
        self.start.action.operation_id()
    }

    /// Applies one reply. A transport loss goes again until the follow's own deadline.
    fn settle(
        &mut self,
        reply: Result<WorkbenchResponse, bool>,
        expected: &WorkbenchRequestId,
        now: Instant,
    ) {
        let outcome = match reply {
            Ok(WorkbenchResponse::TabDuplicate(result))
                if &result.request_id == expected
                    && &result.operation_id == self.operation_id() =>
            {
                result.outcome
            }
            Ok(_) | Err(_) => {
                self.next_at = now + READ_INTERVAL;
                return;
            }
        };
        match outcome {
            TabDuplicateOutcome::Available {
                status, members, ..
            } => {
                self.admitted = true;
                self.observe(&status, &members, now);
            }
            TabDuplicateOutcome::Rejected {
                revision,
                reason: TabDuplicateRejection::RevisionMismatch,
            } if !self.admitted && self.resends < MAXIMUM_RESENDS => {
                self.stale = Some(revision);
            }
            TabDuplicateOutcome::Rejected {
                reason: TabDuplicateRejection::ResourceLimit,
                ..
            }
            | TabDuplicateOutcome::Unavailable { .. }
            | TabDuplicateOutcome::Denied {}
                if !self.admitted =>
            {
                self.end(Some(DuplicateStop::NotCreated), now);
            }
            TabDuplicateOutcome::Rejected { .. } if !self.admitted => {
                self.end(Some(DuplicateStop::Changed), now);
            }
            _ => self.end(Some(DuplicateStop::Recovery), now),
        }
    }

    fn observe(
        &mut self,
        status: &TabDuplicateStatus,
        members: &[TabDuplicateMember],
        now: Instant,
    ) {
        let copies = members
            .iter()
            .filter(|member| member.copy.is_some())
            .count();
        self.progress.submitted = copies;
        self.progress.committed = copies;
        self.progress.started = members
            .iter()
            .filter(|member| {
                matches!(
                    member.copy.as_ref().and_then(|copy| copy.terminal.as_ref()),
                    Some(WorkbenchTerminalOutcome::Started { .. })
                )
            })
            .count();
        match status {
            TabDuplicateStatus::Pending {} => self.next_at = now + READ_INTERVAL,
            TabDuplicateStatus::Duplicated {} => {
                self.progress.complete = true;
                self.end(None, now);
            }
            TabDuplicateStatus::Failed { reason } => self.end(Some(stop(*reason)), now),
        }
    }

    /// Whether a Start refused for a revision race may go again: this Client holds that revision
    /// and still shows every source in the same order. Otherwise the selection changed.
    fn resend_ready(&mut self, state: &WorkbenchClientState, now: Instant) -> bool {
        let Some(revision) = self.stale else {
            return true;
        };
        if state.cache().revision() < revision {
            return false;
        }
        let TabDuplicateAction::Start {
            tab_ids,
            expected_revision,
            ..
        } = &mut self.start.action
        else {
            return false;
        };
        if sources(state, tab_ids).as_ref() != Some(tab_ids) {
            self.end(Some(DuplicateStop::Changed), now);
            return false;
        }
        *expected_revision = state.cache().revision();
        self.stale = None;
        self.resends += 1;
        true
    }
}

/// How a failed operation reads on the status line.
const fn stop(reason: TabDuplicateFailure) -> DuplicateStop {
    match reason {
        TabDuplicateFailure::SourceUnavailable => DuplicateStop::Changed,
        TabDuplicateFailure::CopyRefused => DuplicateStop::NotCreated,
        TabDuplicateFailure::TerminalNotStarted | TabDuplicateFailure::OutcomeUnknown => {
            DuplicateStop::TerminalNotStarted
        }
        TabDuplicateFailure::Deadline => DuplicateStop::Deadline,
        TabDuplicateFailure::SessionStopping | TabDuplicateFailure::Internal => {
            DuplicateStop::Recovery
        }
    }
}

/// The named Tabs in this Client's canonical order, or nothing when any is gone.
fn sources(state: &WorkbenchClientState, named: &[WorkbenchTabId]) -> Option<Vec<WorkbenchTabId>> {
    let ordered = state
        .cache()
        .tabs()
        .iter()
        .map(|tab| tab.tab_id())
        .filter(|tab_id| named.contains(tab_id))
        .cloned()
        .collect::<Vec<_>>();
    (ordered.len() == named.len()).then_some(ordered)
}

impl WorkbenchClientUi {
    pub(super) fn duplicate_tabs(&mut self, target: &WorkbenchTabId) -> ClientActionDisposition {
        let Some(state) = &self.state else {
            return ClientActionDisposition::KeepNavigator;
        };
        let named = super::tab_groups::configuration_tab_ids(state, target);
        if named.len() == 1 {
            return self.create_tab_placed(Some(TabCreationPlacement::Duplicate {
                tab_id: target.clone(),
            }));
        }
        if self.duplication.live() || self.duplication.origin.is_none() {
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(start) = self.duplicate_start(&named) else {
            return ClientActionDisposition::KeepNavigator;
        };
        let now = self.clock.now();
        let total = named.len();
        self.duplication.follow = Some(Follow {
            start,
            admitted: false,
            flight: None,
            stale: None,
            resends: 0,
            next_at: now,
            deadline: now + FOLLOW,
            progress: DuplicateProgress {
                total,
                submitted: 0,
                committed: 0,
                started: 0,
                stop: None,
                complete: false,
            },
            ended_at: None,
        });
        if let Some(state) = self.state.as_mut() {
            state.clear_notice();
        }
        self.drive_duplication();
        ClientActionDisposition::DismissNavigator
    }

    fn duplicate_start(&self, named: &[WorkbenchTabId]) -> Option<WorkbenchTabDuplicateRequest> {
        let state = self.state.as_ref()?;
        let operation_id = self
            .mailbox
            .as_ref()?
            .allocate_workbench_request_id()
            .ok()?;
        let geometry = state.latest_geometry();
        Some(WorkbenchTabDuplicateRequest {
            protocol_version: ProtocolVersion,
            request_id: operation_id.clone(),
            session_id: state.cache().session_id().clone(),
            action: TabDuplicateAction::Start {
                operation_id,
                terminal_context_id: state.cache().terminal_context_id()?.clone(),
                tab_ids: sources(state, named)?,
                expected_revision: state.cache().revision(),
                geometry: WorkbenchTerminalGeometry::new(
                    geometry.columns(),
                    geometry.rows(),
                    geometry.cell_pixel_width(),
                    geometry.cell_pixel_height(),
                )
                .ok()?,
            },
        })
    }

    /// Settles the follow's reply and sends its next exchange when one is due.
    pub(super) fn drive_duplication(&mut self) -> bool {
        let now = self.clock.now();
        let Some(follow) = self.duplication.follow.as_mut() else {
            return false;
        };
        if follow.ended_at.is_some() {
            return false;
        }
        let before = follow.progress;
        if let Some((expected, request)) = &follow.flight {
            let Some(reply) = request.poll() else {
                return false;
            };
            let expected = expected.clone();
            follow.flight = None;
            follow.settle(reply, &expected, now);
        }
        if self
            .state
            .as_ref()
            .is_some_and(|state| state.cache().session_id() != &follow.start.session_id)
        {
            follow.end(Some(DuplicateStop::Recovery), now);
        } else if follow.ended_at.is_none() && now >= follow.deadline {
            follow.end(Some(DuplicateStop::Deadline), now);
        } else if follow.ended_at.is_none() && now >= follow.next_at {
            // A turn that sends nothing, such as a resend waiting for its revision, waits a read
            // interval instead of asking for an immediate turn again.
            follow.next_at = now + READ_INTERVAL;
            let ready = self
                .state
                .as_ref()
                .is_some_and(|state| follow.admitted || follow.resend_ready(state, now));
            if ready && follow.ended_at.is_none() {
                send(
                    follow,
                    self.duplication.origin.as_ref(),
                    self.mailbox.as_deref(),
                    &self.clock,
                    now,
                );
            }
        }
        before != follow.progress
    }
}

fn send(
    follow: &mut Follow,
    origin: Option<&LocalClientOrigin>,
    mailbox: Option<&CommandMailbox>,
    clock: &LocalOperationClock,
    now: Instant,
) {
    let (Some(origin), Some(Ok(request_id))) = (
        origin,
        mailbox.map(CommandMailbox::allocate_workbench_request_id),
    ) else {
        follow.end(Some(DuplicateStop::Recovery), now);
        return;
    };
    let action = if follow.admitted {
        TabDuplicateAction::Read {
            operation_id: follow.operation_id().clone(),
        }
    } else {
        follow.start.action.clone()
    };
    let request = WorkbenchRequest::TabDuplicate(WorkbenchTabDuplicateRequest {
        request_id: request_id.clone(),
        action,
        ..follow.start.clone()
    });
    match origin.request(&request, now + REPLY, clock.clone()) {
        Ok(pending) => follow.flight = Some((request_id, pending)),
        Err(_) => follow.end(Some(DuplicateStop::Recovery), now),
    }
}

impl super::WorkbenchClientView<'_> {
    pub(crate) fn duplicate_progress(&self) -> Option<DuplicateProgress> {
        self.duplication
    }
}
