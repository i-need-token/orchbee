//! Extends the authorization kernel with exact-binding lifecycle admission, mutation slots,
//! and correlated retained receipts. It separates inspection from mutation authority without
//! querying a Host or performing termination and disposal.

use orchbee_capability_node_protocol::{
    HostEpoch, HostTerminalIdentity, LifecycleRejection, TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
    TerminalLifecycleAction, TerminalLifecycleOutcome, TerminalLifecycleRequest,
    TerminalLifecycleResult,
};

use super::{
    AuthorizationKernelError, AuthorizedHostOperation, AuthorizedHostOperationKernel,
    AuthorizedHostOperationResult, HostHandoffAdmission, HostOperationSlot,
    TerminalAuthorizationBinding,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LifecycleDispatch {
    pub(crate) request: TerminalLifecycleRequest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LifecycleAdmission {
    Dispatch(LifecycleDispatch),
    ResultReady,
    SlotOccupied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LifecycleSlot {
    Awaiting(TerminalLifecycleRequest),
    HandedOff(TerminalLifecycleRequest),
    Ready(TerminalLifecycleResult),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LifecycleReceipts {
    owner: orchbee_capability_node_protocol::OperationId,
    high_attempt: std::num::NonZeroU64,
    high_ordinal: u8,
    last_step: Option<TerminalLifecycleResult>,
    last_mutation: Option<TerminalLifecycleResult>,
    mutation_unknown: bool,
    known_disposed: bool,
}

impl LifecycleReceipts {
    fn replay(self, request: TerminalLifecycleRequest) -> Option<TerminalLifecycleResult> {
        [self.last_step, self.last_mutation]
            .into_iter()
            .flatten()
            .find(|result| result.request() == request)
    }

    fn conflicts(self, request: TerminalLifecycleRequest) -> bool {
        let retained_id = [self.last_step, self.last_mutation]
            .into_iter()
            .flatten()
            .any(|result| result.request().request_id == request.request_id);
        let ordered = (request.attempt_generation == self.high_attempt
            && request.step_ordinal.get() > self.high_ordinal)
            || (request.attempt_generation > self.high_attempt && request.step_ordinal.get() == 1);
        request.close_operation_id != self.owner
            || retained_id
            || !ordered
            || (is_mutation(request) && (self.mutation_unknown || self.known_disposed))
    }
}

impl TerminalAuthorizationBinding {
    fn lifecycle_identity(&self) -> Option<HostTerminalIdentity> {
        match &self.operation {
            AuthorizedHostOperation::Start(start) => Some(start.identity()),
            AuthorizedHostOperation::Launch(_) => match self.cached_result.as_ref()? {
                AuthorizedHostOperationResult::Launch(result) => match result.outcome() {
                    orchbee_capability_node_protocol::TerminalLaunchOutcome::HostAccepted(
                        accepted,
                    ) => Some(accepted.identity()),
                    orchbee_capability_node_protocol::TerminalLaunchOutcome::Rejected(..)
                    | orchbee_capability_node_protocol::TerminalLaunchOutcome::Failed(..)
                    | orchbee_capability_node_protocol::TerminalLaunchOutcome::OutcomeUnknown {
                        ..
                    } => None,
                },
                AuthorizedHostOperationResult::Start(_) => None,
            },
        }
    }
}

impl AuthorizedHostOperationKernel {
    pub(crate) fn admit_lifecycle(
        &mut self,
        request: TerminalLifecycleRequest,
        now: u64,
        epoch: HostEpoch,
    ) -> Result<LifecycleAdmission, AuthorizationKernelError> {
        let now = self.observe_time(now);
        if self.slot.is_some() {
            return Ok(LifecycleAdmission::SlotOccupied);
        }
        if let Some(reason) = self.lifecycle_association_rejection(request, epoch) {
            self.queue_lifecycle_rejection(request, reason)?;
            return Ok(LifecycleAdmission::ResultReady);
        }
        if matches!(
            request.action,
            TerminalLifecycleAction::InspectInventoryPage { .. }
        ) {
            if expired(request, now) {
                self.queue_lifecycle_rejection(request, LifecycleRejection::LeaseExpired)?;
                return Ok(LifecycleAdmission::ResultReady);
            }
            self.slot = Some(HostOperationSlot::Lifecycle(LifecycleSlot::Awaiting(
                request,
            )));
            return Ok(LifecycleAdmission::Dispatch(LifecycleDispatch { request }));
        }
        let binding = self
            .bindings
            .get(&request.authorization.terminal_id)
            .ok_or(AuthorizationKernelError::InconsistentBinding)?;
        if let Some(receipts) = binding.lifecycle {
            if let Some(result) = receipts.replay(request) {
                self.slot = Some(HostOperationSlot::Lifecycle(LifecycleSlot::Ready(result)));
                return Ok(LifecycleAdmission::ResultReady);
            }
            if receipts.conflicts(request) {
                self.queue_lifecycle_rejection(request, LifecycleRejection::LifecycleConflict)?;
                return Ok(LifecycleAdmission::ResultReady);
            }
        } else if request.attempt_generation.get() != 1 || request.step_ordinal.get() != 1 {
            self.queue_lifecycle_rejection(request, LifecycleRejection::LifecycleConflict)?;
            return Ok(LifecycleAdmission::ResultReady);
        }
        if expired(request, now) {
            self.queue_lifecycle_rejection(request, LifecycleRejection::LeaseExpired)?;
            return Ok(LifecycleAdmission::ResultReady);
        }
        let binding = self
            .bindings
            .get_mut(&request.authorization.terminal_id)
            .ok_or(AuthorizationKernelError::InconsistentBinding)?;
        let receipts = binding.lifecycle.get_or_insert(LifecycleReceipts {
            owner: request.close_operation_id,
            high_attempt: request.attempt_generation,
            high_ordinal: request.step_ordinal.get(),
            last_step: None,
            last_mutation: None,
            mutation_unknown: false,
            known_disposed: false,
        });
        receipts.high_attempt = request.attempt_generation;
        receipts.high_ordinal = request.step_ordinal.get();
        self.slot = Some(HostOperationSlot::Lifecycle(LifecycleSlot::Awaiting(
            request,
        )));
        Ok(LifecycleAdmission::Dispatch(LifecycleDispatch { request }))
    }

    fn lifecycle_association_rejection(
        &self,
        request: TerminalLifecycleRequest,
        epoch: HostEpoch,
    ) -> Option<LifecycleRejection> {
        let authorization = request.authorization;
        if authorization.worker_id != self.current_worker_id {
            return Some(LifecycleRejection::WrongWorker);
        }
        if authorization.worker_generation != self.current_worker_generation {
            return Some(LifecycleRejection::StaleWorkerGeneration);
        }
        if authorization.capability_version != TERMINAL_LIFECYCLE_CAPABILITY_VERSION {
            return Some(LifecycleRejection::UnsupportedCapability);
        }
        let Some(binding) = self.bindings.get(&authorization.terminal_id) else {
            return Some(LifecycleRejection::BindingUnavailable);
        };
        if authorization.binding_fence != binding.operation.authorization().fence() {
            return Some(LifecycleRejection::BindingFenceMismatch);
        }
        let Some(identity) = binding.lifecycle_identity() else {
            return Some(LifecycleRejection::BindingUnavailable);
        };
        if identity != request.target || request.target.host_epoch() != epoch {
            return Some(LifecycleRejection::HostIdentityMismatch);
        }
        None
    }

    fn queue_lifecycle_rejection(
        &mut self,
        request: TerminalLifecycleRequest,
        reason: LifecycleRejection,
    ) -> Result<(), AuthorizationKernelError> {
        let result =
            TerminalLifecycleResult::new(request, TerminalLifecycleOutcome::Rejected(reason))?;
        self.slot = Some(HostOperationSlot::Lifecycle(LifecycleSlot::Ready(result)));
        Ok(())
    }

    pub(crate) fn begin_lifecycle_handoff(
        &mut self,
        dispatch: LifecycleDispatch,
        now: u64,
        epoch: HostEpoch,
    ) -> Result<HostHandoffAdmission, AuthorizationKernelError> {
        self.require_lifecycle_dispatch(dispatch, false)?;
        let now = self.observe_time(now);
        let rejection = self
            .lifecycle_association_rejection(dispatch.request, epoch)
            .or_else(|| expired(dispatch.request, now).then_some(LifecycleRejection::LeaseExpired));
        if let Some(reason) = rejection {
            self.complete_lifecycle(dispatch.request, TerminalLifecycleOutcome::Rejected(reason))?;
            return Ok(HostHandoffAdmission::ResultReady);
        }
        self.slot = Some(HostOperationSlot::Lifecycle(LifecycleSlot::HandedOff(
            dispatch.request,
        )));
        Ok(HostHandoffAdmission::HandedOff)
    }

    pub(crate) fn lifecycle_pre_handoff_failure(
        &mut self,
        dispatch: LifecycleDispatch,
        now: u64,
    ) -> Result<(), AuthorizationKernelError> {
        self.require_lifecycle_dispatch(dispatch, false)?;
        let now = self.observe_time(now);
        let outcome = if expired(dispatch.request, now) {
            TerminalLifecycleOutcome::Rejected(LifecycleRejection::LeaseExpired)
        } else {
            TerminalLifecycleOutcome::HostUnavailableBeforeHandoff
        };
        self.complete_lifecycle(dispatch.request, outcome)
    }

    pub(crate) fn record_lifecycle_completion(
        &mut self,
        dispatch: LifecycleDispatch,
        outcome: TerminalLifecycleOutcome,
    ) -> Result<(), AuthorizationKernelError> {
        self.require_lifecycle_dispatch(dispatch, true)?;
        if matches!(
            outcome,
            TerminalLifecycleOutcome::HostUnavailableBeforeHandoff
                | TerminalLifecycleOutcome::Rejected(_)
        ) {
            return Err(AuthorizationKernelError::InvalidDispatchPhase);
        }
        self.complete_lifecycle(dispatch.request, outcome)
    }

    fn require_lifecycle_dispatch(
        &self,
        dispatch: LifecycleDispatch,
        handed_off: bool,
    ) -> Result<(), AuthorizationKernelError> {
        let expected = if handed_off {
            LifecycleSlot::HandedOff(dispatch.request)
        } else {
            LifecycleSlot::Awaiting(dispatch.request)
        };
        if self.slot == Some(HostOperationSlot::Lifecycle(expected)) {
            Ok(())
        } else {
            Err(AuthorizationKernelError::DispatchMismatch)
        }
    }

    fn complete_lifecycle(
        &mut self,
        request: TerminalLifecycleRequest,
        outcome: TerminalLifecycleOutcome,
    ) -> Result<(), AuthorizationKernelError> {
        let result = TerminalLifecycleResult::new(request, outcome)?;
        if matches!(
            request.action,
            TerminalLifecycleAction::InspectInventoryPage { .. }
        ) {
            self.slot = Some(HostOperationSlot::Lifecycle(LifecycleSlot::Ready(result)));
            return Ok(());
        }
        let receipts = self
            .bindings
            .get_mut(&request.authorization.terminal_id)
            .and_then(|binding| binding.lifecycle.as_mut())
            .ok_or(AuthorizationKernelError::InconsistentBinding)?;
        receipts.last_step = Some(result);
        if is_mutation(request) {
            receipts.last_mutation = Some(result);
            receipts.mutation_unknown |=
                matches!(outcome, TerminalLifecycleOutcome::OutcomeUnknown(_));
        }
        receipts.known_disposed |= outcome == TerminalLifecycleOutcome::Disposed;
        self.slot = Some(HostOperationSlot::Lifecycle(LifecycleSlot::Ready(result)));
        Ok(())
    }

    pub(crate) fn take_lifecycle_result(&mut self) -> Option<TerminalLifecycleResult> {
        match self.slot.take() {
            Some(HostOperationSlot::Lifecycle(LifecycleSlot::Ready(result))) => Some(result),
            slot => {
                self.slot = slot;
                None
            }
        }
    }
}

fn is_mutation(request: TerminalLifecycleRequest) -> bool {
    !matches!(
        request.action,
        TerminalLifecycleAction::InventoryPage { .. }
            | TerminalLifecycleAction::InspectInventoryPage { .. }
    )
}

fn expired(request: TerminalLifecycleRequest, now: u64) -> bool {
    now >= request.authorization.lease.expires_at().get()
}

#[cfg(test)]
mod tests;
