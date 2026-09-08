use super::*;

#[derive(Clone, Copy)]
pub(in crate::terminal_session) struct SwitchAdmissionFacts {
    abandoned_causal: bool,
    authorized_cleanup: bool,
}

impl SwitchAdmissionFacts {
    pub(super) const fn new(abandoned_causal: bool, authorized_cleanup: bool) -> Self {
        Self {
            abandoned_causal,
            authorized_cleanup,
        }
    }
    pub(in crate::terminal_session) const fn abandoned_causal(self) -> bool {
        self.abandoned_causal
    }
    pub(in crate::terminal_session) const fn authorized_cleanup(self) -> bool {
        self.authorized_cleanup
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SwitchCall {
    Snapshot {
        marker: AttachmentKey,
        request: WorkbenchRequestId,
        cutoff: Instant,
    },
    Cleanup {
        cleanup: MembershipCleanup,
        request: RequestId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallStage {
    Calling,
    SnapshotAnswered,
    Detached,
    Skipped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SwitchHandoffRecord {
    lease: u64,
    call: SwitchCall,
    stage: CallStage,
}

impl SwitchHandoffRecord {
    pub(super) const fn is_cleanup(&self) -> bool {
        matches!(self.call, SwitchCall::Cleanup { .. })
    }
}

/// Non-clone permits identify exactly one invocation authorized before releasing the mutex.
pub(in crate::terminal_session) struct SwitchSnapshotPermit(SwitchHandoffRecord);
pub(in crate::terminal_session) struct SwitchCleanupPermit(SwitchHandoffRecord);

/// Local expiry has no wire request; this seals its original lease and full admitted target.
pub(in crate::terminal_session) struct SwitchExpiryPermit {
    lease: u64,
    target: AttachTarget,
}

impl SwitchExpiryPermit {
    pub(in crate::terminal_session) const fn target(&self) -> &AttachTarget {
        &self.target
    }
    pub(in crate::terminal_session) fn into_target(self) -> AttachTarget {
        self.target
    }
}

impl SwitchSnapshotPermit {
    pub(in crate::terminal_session) fn request_id(&self) -> &WorkbenchRequestId {
        let SwitchCall::Snapshot { request, .. } = &self.0.call else {
            unreachable!()
        };
        request
    }
}
impl SwitchCleanupPermit {
    pub(in crate::terminal_session) fn request_id(&self) -> &RequestId {
        let SwitchCall::Cleanup { request, .. } = &self.0.call else {
            unreachable!()
        };
        request
    }
}

impl CommandMailbox {
    pub(in crate::terminal_session) fn authorize_switch_expiry(
        &self,
        lease: &CommandLease,
        target: &AttachTarget,
        clock: &LocalOperationClock,
    ) -> Result<Option<SwitchExpiryPermit>, CarrierFailure> {
        let (state, now) = self.lock_with_fresh_sample(clock);
        if !matches!(lease.record(), QueuedCommand::Switch(original) if original.target() == target)
        {
            return Err(CarrierFailure::Protocol);
        }
        if !Self::switch_call_current(&state, lease, target.selected().key())? {
            return Ok(None);
        }
        if state.switch_handoff.is_some() || now < target.deadlines().forward_cutoff() {
            return Err(CarrierFailure::Protocol);
        }
        if now >= target.deadlines().total() {
            return Err(CarrierFailure::Deadline);
        }
        Ok(Some(SwitchExpiryPermit {
            lease: lease.serial,
            target: target.clone(),
        }))
    }
    fn switch_call_current(
        state: &MailboxState,
        lease: &CommandLease,
        marker: &AttachmentKey,
    ) -> Result<bool, CarrierFailure> {
        if state.closed.is_some()
            || !state
                .active_lease
                .is_some_and(|active| active.serial == lease.serial)
            || !matches!(lease.record(), QueuedCommand::Switch(original) if original.target().selected().key() == marker)
        {
            return Err(CarrierFailure::Protocol);
        }
        Ok(state.desired.key() == Some(marker))
    }

    pub(in crate::terminal_session) fn authorize_switch_snapshot(
        &self,
        lease: &CommandLease,
        target: &AttachTarget,
        clock: &LocalOperationClock,
    ) -> Result<Option<SwitchSnapshotPermit>, CarrierFailure> {
        let (mut state, now) = self.lock_with_fresh_sample(clock);
        let marker = target.selected().key();
        if !Self::switch_call_current(&state, lease, marker)? {
            return Ok(None);
        }
        if !matches!(lease.record(), QueuedCommand::Switch(original) if original.target() == target)
        {
            return Err(CarrierFailure::Protocol);
        }
        if state.switch_handoff.is_some() {
            return Err(CarrierFailure::Protocol);
        }
        if now >= target.deadlines().forward_cutoff() {
            return Err(CarrierFailure::Deadline);
        }
        let (cursor, request) = workbench_identity_candidate(state.identity_cursor)
            .ok_or(CarrierFailure::IdentityExhausted)?;
        let record = SwitchHandoffRecord {
            lease: lease.serial,
            call: SwitchCall::Snapshot {
                marker: marker.clone(),
                request,
                cutoff: target.deadlines().forward_cutoff(),
            },
            stage: CallStage::Calling,
        };
        state.identity_cursor = cursor;
        state.switch_handoff = Some(record.clone());
        Ok(Some(SwitchSnapshotPermit(record)))
    }

    pub(in crate::terminal_session) fn answer_switch_snapshot(
        &self,
        permit: SwitchSnapshotPermit,
    ) -> Result<(), CarrierFailure> {
        let mut state = self.lock();
        let record = state
            .switch_handoff
            .as_mut()
            .ok_or(CarrierFailure::Protocol)?;
        if *record != permit.0 {
            return Err(CarrierFailure::Protocol);
        }
        record.stage = CallStage::SnapshotAnswered;
        Ok(())
    }

    pub(in crate::terminal_session) fn authorize_switch_cleanup(
        &self,
        lease: &CommandLease,
        cleanup: &MembershipCleanup,
        attached: &AttachmentKey,
        clock: &LocalOperationClock,
    ) -> Result<Option<SwitchCleanupPermit>, CarrierFailure> {
        let (mut state, now) = self.lock_with_fresh_sample(clock);
        let MembershipResultKey::Switch(marker) = cleanup.result() else {
            return Err(CarrierFailure::Protocol);
        };
        if !Self::switch_call_current(&state, lease, marker)? {
            return Ok(None);
        }
        if state.switch_handoff.is_some()
            || attached != cleanup.old()
            || !matches!(lease.record(), QueuedCommand::Switch(original) if original.target().deadlines() == cleanup.deadlines())
        {
            return Err(CarrierFailure::Protocol);
        }
        if now >= cleanup.deadlines().total() {
            return Err(CarrierFailure::Deadline);
        }
        let (cursor, request) = obcs_identity_candidate(state.identity_cursor)
            .ok_or(CarrierFailure::IdentityExhausted)?;
        let record = SwitchHandoffRecord {
            lease: lease.serial,
            call: SwitchCall::Cleanup {
                cleanup: cleanup.clone(),
                request,
            },
            stage: CallStage::Calling,
        };
        state.identity_cursor = cursor;
        state.switch_handoff = Some(record.clone());
        Ok(Some(SwitchCleanupPermit(record)))
    }

    /// A false result is a current no-effect answer, released atomically with that classification.
    /// True retains an answered receipt until exact UI settlement (Detached or superseded no-effect).
    pub(in crate::terminal_session) fn answer_switch_cleanup(
        &self,
        permit: SwitchCleanupPermit,
        detached: bool,
    ) -> Result<bool, CarrierFailure> {
        let mut state = self.lock();
        if state.switch_handoff.as_ref() != Some(&permit.0) {
            return Err(CarrierFailure::Protocol);
        }
        let SwitchCall::Cleanup { cleanup, .. } = &permit.0.call else {
            return Err(CarrierFailure::Protocol);
        };
        let MembershipResultKey::Switch(marker) = cleanup.result() else {
            return Err(CarrierFailure::Protocol);
        };
        if detached || state.desired.key() != Some(marker) {
            if let Some(record) = &mut state.switch_handoff {
                record.stage = if detached {
                    CallStage::Detached
                } else {
                    CallStage::Skipped
                };
            }
            Ok(true)
        } else {
            state.switch_handoff = None;
            Ok(false)
        }
    }

    pub(super) fn switch_event_matches_receipt(
        state: &MailboxState,
        event: &WorkerEvent,
        deadline: EventDeadline,
        lease: Option<u64>,
    ) -> bool {
        if let WorkerEvent::SwitchForwardExpired { permit } = event {
            return state.switch_handoff.is_none()
                && lease == Some(permit.lease)
                && state
                    .active_lease
                    .is_some_and(|active| active.serial == permit.lease)
                && deadline == EventDeadline::TotalCleanup(permit.target.deadlines().total());
        }
        let expected = match event {
            WorkerEvent::SwitchSnapshotResult { marker, .. } => {
                Some((CallStage::SnapshotAnswered, Some(marker), None))
            }
            WorkerEvent::MembershipDetached { cleanup, detached }
                if matches!(cleanup.result(), MembershipResultKey::Switch(_)) =>
            {
                if cleanup.old() != detached {
                    return false;
                }
                Some((CallStage::Detached, None, Some(cleanup)))
            }
            WorkerEvent::MembershipCleanupSkipped { cleanup } => {
                Some((CallStage::Skipped, None, Some(cleanup)))
            }
            _ => None,
        };
        let Some((stage, marker, cleanup)) = expected else {
            return true;
        };
        let Some(record) = &state.switch_handoff else {
            return false;
        };
        if Some(record.lease) != lease || record.stage != stage {
            return false;
        }
        match &record.call {
            SwitchCall::Snapshot {
                marker: original,
                cutoff,
                ..
            } => marker == Some(original) && deadline == EventDeadline::PlacementOnly(*cutoff),
            SwitchCall::Cleanup {
                cleanup: original, ..
            } => {
                cleanup == Some(original)
                    && deadline == EventDeadline::TotalCleanup(original.deadlines().total())
            }
        }
    }

    pub(super) fn switch_ui_matches_receipt(state: &MailboxState, wait: &UiWaitKey) -> bool {
        let Some(record) = &state.switch_handoff else {
            return false;
        };
        if state.event_anchor_serial != Some(record.lease) {
            return false;
        }
        match (&record.call, &wait.correlation) {
            (SwitchCall::Snapshot { marker, .. }, UiCorrelation::Switch(expected)) => {
                record.stage == CallStage::SnapshotAnswered && marker == expected
            }
            (SwitchCall::Cleanup { cleanup, .. }, UiCorrelation::Membership(result, old)) => {
                matches!(record.stage, CallStage::Detached | CallStage::Skipped)
                    && cleanup.result() == result
                    && cleanup.old() == old
            }
            _ => false,
        }
    }
}
