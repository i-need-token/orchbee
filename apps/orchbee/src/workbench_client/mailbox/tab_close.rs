use super::{
    BarrierFence, BarrierKind, COMMAND_CAPACITY, CloseFlightKey, CloseOperation, CloseResult,
    CommandMailbox, EventDeadline, EventError, Geometry, Instant, LocalOperationClock,
    MailboxClock, MailboxClosed, MailboxMembership, MailboxState, MutationResult,
    PendingRefreshStage, QueuedCommand, QueuedRecord, UiCorrelation, UiWaitKey, WorkbenchRequestId,
    WorkerEvent, workbench_identity_candidate,
};

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct RetiredMailboxSeed {
    identity_cursor: u128,
    attachment_generation: u64,
    refresh_generation: u64,
    next_command_serial: u64,
    next_ui_wait_serial: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct CloseMarker {
    operation: CloseOperation,
}

impl CloseMarker {
    pub(in crate::workbench_client) const fn operation(&self) -> &CloseOperation {
        &self.operation
    }
    pub(super) fn barrier(&self) -> BarrierFence {
        BarrierFence {
            kind: BarrierKind::Close,
            forward_cutoff: self.operation.deadline(),
            total: self.operation.deadline(),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum CloseAdmissionError<BuildError> {
    Closed(MailboxClosed),
    Deadline,
    Busy,
    Full,
    IdentityExhausted,
    InvalidOperation,
    Build(BuildError),
}

pub(super) struct CloseFlight {
    serial: u64,
    operation: CloseOperation,
    stage: PendingRefreshStage,
}

impl MailboxState {
    pub(super) fn place_close_result(
        &mut self,
        operation: &CloseOperation,
        deadline: EventDeadline,
        anchor: Option<u64>,
    ) -> bool {
        let Some(flight) = self.close_flight.as_mut() else {
            return false;
        };
        if !matches!(deadline, EventDeadline::PlacementOnly(cutoff) if cutoff <= operation.deadline())
            || flight.operation != *operation
            || flight.stage != PendingRefreshStage::InFlight
            || anchor != Some(flight.serial)
        {
            return false;
        }
        flight.stage = PendingRefreshStage::AwaitingUi;
        true
    }

    pub(super) fn settle_close_result(
        &mut self,
        correlation: &UiCorrelation,
        key: &CloseFlightKey,
    ) -> bool {
        if !matches!(correlation, UiCorrelation::Close(current) if current == key)
            || !self.close_flight.as_ref().is_some_and(|flight| {
                flight.operation.key() == key && flight.stage == PendingRefreshStage::AwaitingUi
            })
        {
            return false;
        }
        self.close_flight = None;
        self.close_deposited_result = None;
        true
    }
}

impl CommandMailbox {
    pub(in crate::workbench_client) fn close_worker_loss(
        &self,
    ) -> crate::workbench_client::state::CloseWorkerReturn {
        let state = self.lock();
        crate::workbench_client::state::CloseWorkerReturn::Missing {
            queued: state.close_queued_key.clone(),
            deposited: state.close_deposited_result.clone(),
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(in crate::workbench_client) fn retired_seed(&self) -> RetiredMailboxSeed {
        let state = self.lock();
        RetiredMailboxSeed {
            identity_cursor: state.identity_cursor,
            attachment_generation: state.attachment_generation,
            refresh_generation: state.refresh_generation,
            next_command_serial: state.next_command_serial,
            next_ui_wait_serial: state.next_ui_wait_serial,
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(in crate::workbench_client) fn for_status_recovery(
        seed: RetiredMailboxSeed,
        geometry: Geometry,
    ) -> Self {
        let mailbox = Self::new(seed.identity_cursor, geometry);
        {
            let mut state = mailbox.lock();
            state.desired = MailboxMembership::Live {
                desired: None,
                panes: Vec::new(),
            };
            state.attachment_generation = seed.attachment_generation;
            state.refresh_generation = seed.refresh_generation;
            state.next_command_serial = seed.next_command_serial;
            state.next_ui_wait_serial = seed.next_ui_wait_serial;
        }
        mailbox
    }

    pub(in crate::workbench_client) fn try_enqueue_close_for<Canonical, BuildError>(
        &self,
        canonical: &mut Canonical,
        deadline: Instant,
        clock: &LocalOperationClock,
        build: impl FnOnce(
            &Canonical,
            WorkbenchRequestId,
            Instant,
        ) -> Result<CloseOperation, BuildError>,
        commit: impl FnOnce(&mut Canonical, &CloseOperation, Instant) -> Result<(), BuildError>,
    ) -> Result<CloseOperation, CloseAdmissionError<BuildError>> {
        self.try_enqueue_close_for_using(canonical, deadline, clock, build, commit)
    }

    pub(super) fn try_enqueue_close_for_using<Canonical, BuildError>(
        &self,
        canonical: &mut Canonical,
        deadline: Instant,
        clock: &dyn MailboxClock,
        build: impl FnOnce(
            &Canonical,
            WorkbenchRequestId,
            Instant,
        ) -> Result<CloseOperation, BuildError>,
        commit: impl FnOnce(&mut Canonical, &CloseOperation, Instant) -> Result<(), BuildError>,
    ) -> Result<CloseOperation, CloseAdmissionError<BuildError>> {
        let (mut state, admitted_at) = self
            .lock_before(deadline, clock)
            .ok_or(CloseAdmissionError::Deadline)?;
        Self::ensure_open(&state).map_err(CloseAdmissionError::Closed)?;
        if state.desired.awaiting_bootstrap()
            || (state.close_flight.is_some() || state.close_reconciliation.is_some())
            || state.structural_mutation_pending.is_some()
            || state.refresh_flight.is_some()
            || state
                .queue
                .iter()
                .any(|record| record.command.barrier().is_some())
        {
            return Err(CloseAdmissionError::Busy);
        }
        if state.queue.len() == COMMAND_CAPACITY {
            return Err(CloseAdmissionError::Full);
        }
        let (cursor, request_id) = workbench_identity_candidate(state.identity_cursor)
            .ok_or(CloseAdmissionError::IdentityExhausted)?;
        let serial = state
            .next_command_serial
            .checked_add(1)
            .ok_or(CloseAdmissionError::IdentityExhausted)?;
        let operation = build(canonical, request_id.clone(), admitted_at)
            .map_err(CloseAdmissionError::Build)?;
        if operation.key().request_id() != &request_id || operation.deadline() != deadline {
            return Err(CloseAdmissionError::InvalidOperation);
        }
        commit(canonical, &operation, admitted_at).map_err(CloseAdmissionError::Build)?;
        state.identity_cursor = cursor;
        state.next_command_serial = serial;
        state.close_flight = Some(CloseFlight {
            serial,
            operation: operation.clone(),
            stage: PendingRefreshStage::Queued,
        });
        state.close_queued_key = Some(operation.key().clone());
        state.queue.push_back(QueuedRecord {
            serial,
            command: QueuedCommand::Close(CloseMarker {
                operation: operation.clone(),
            }),
            retry_at: None,
        });
        self.changed.notify_all();
        Ok(operation)
    }

    pub(in crate::workbench_client) fn mark_close_in_flight(
        &self,
        operation: &CloseOperation,
    ) -> Result<MutationResult, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        let active_serial = state.active_lease.map(|lease| lease.serial);
        match state.close_flight.as_mut() {
            Some(flight)
                if flight.operation == *operation
                    && flight.stage == PendingRefreshStage::Queued
                    && active_serial == Some(flight.serial) =>
            {
                flight.stage = PendingRefreshStage::InFlight;
                state.close_queued_key = None;
                self.changed.notify_all();
                Ok(MutationResult::Applied)
            }
            _ => Ok(MutationResult::Stale),
        }
    }

    pub(in crate::workbench_client) fn publish_close_result_before(
        &self,
        operation: CloseOperation,
        outcome: CloseResult,
        deadline: Instant,
        clock: &LocalOperationClock,
    ) -> Result<UiWaitKey, EventError> {
        self.publish_event_before(
            WorkerEvent::CloseResult { operation, outcome },
            EventDeadline::PlacementOnly(deadline),
            clock,
        )
    }
}
