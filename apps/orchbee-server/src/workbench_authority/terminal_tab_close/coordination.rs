use std::{num::NonZeroU64, time::Instant};

use orchbee_capability_node_protocol::{
    TERMINAL_LIFECYCLE_CAPABILITY_VERSION, TerminalLifecycleRequest,
};
use orchbee_workbench_protocol::{WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration};

use super::super::{RetainedRequest, WorkbenchAuthority};
use super::continuation::{
    CloseContinuationNext, CloseStepObservation, CloseStepPrequeueFailure, CloseStepToken,
    CloseStepTokenMismatch,
};
use super::{WorkbenchTabCloseRuntimeTarget, WorkbenchTabCloseTarget};

impl WorkbenchAuthority {
    /// Product identities are never reused; retained tombstones also fence new target effects.
    #[must_use]
    pub fn has_terminal_tab_close(
        &self,
        binding: &orchbee_workbench_protocol::PresentationBinding,
    ) -> bool {
        self.terminal_tab_close_request_for_binding(binding)
            .is_some()
    }

    #[must_use]
    pub fn terminal_tab_close_request_for_binding(
        &self,
        binding: &orchbee_workbench_protocol::PresentationBinding,
    ) -> Option<&WorkbenchRequestId> {
        self.ledger
            .iter()
            .find_map(|(request_id, entry)| match entry {
                RetainedRequest::TabClose(record) if record.target.member.binding() == binding => {
                    Some(request_id)
                }
                _ => None,
            })
    }

    #[must_use]
    pub fn terminal_tab_close_status(
        &self,
        close_request_id: &WorkbenchRequestId,
    ) -> Option<orchbee_workbench_protocol::WorkbenchTabCloseStatus> {
        self.close_record(close_request_id)
            .map(|record| record.continuation.status())
    }

    pub fn terminal_tab_close_operations(&self) -> impl Iterator<Item = &WorkbenchRequestId> {
        self.ledger
            .iter()
            .filter_map(|(request_id, entry)| match entry {
                RetainedRequest::TabClose(record) if !record.is_closed() => Some(request_id),
                _ => None,
            })
    }

    #[must_use]
    pub fn terminal_tab_close_target(
        &self,
        close_request_id: &WorkbenchRequestId,
    ) -> Option<&WorkbenchTabCloseTarget> {
        self.close_record(close_request_id)
            .map(|record| &record.target)
    }

    #[must_use]
    pub fn terminal_tab_close_generation(
        &self,
        close_request_id: &WorkbenchRequestId,
    ) -> Option<WorkbenchTabCloseAttemptGeneration> {
        self.close_record(close_request_id)
            .map(|record| record.generation)
    }

    pub fn next_terminal_tab_close_step(
        &mut self,
        close_request_id: &WorkbenchRequestId,
        now: Instant,
    ) -> Result<CloseContinuationNext, CloseStepTokenMismatch> {
        Ok(self
            .close_record_mut(close_request_id)
            .ok_or(CloseStepTokenMismatch)?
            .continuation
            .next(now))
    }

    pub fn reserve_terminal_tab_close_step(
        &mut self,
        close_request_id: &WorkbenchRequestId,
        now: Instant,
        request: TerminalLifecycleRequest,
    ) -> Result<CloseStepToken, CloseStepTokenMismatch> {
        let record = self
            .close_record_mut(close_request_id)
            .ok_or(CloseStepTokenMismatch)?;
        let runtime = record.target.runtime;
        let canonical_id = u128::from_str_radix(record.command.request_id.as_str(), 16)
            .map_err(|_error| CloseStepTokenMismatch)?
            .to_be_bytes();
        if request.close_operation_id.as_bytes() != canonical_id
            || request.authorization.capability_version != TERMINAL_LIFECYCLE_CAPABILITY_VERSION
            || request.authorization.worker_id != runtime.worker_id
            || request.authorization.worker_generation != runtime.worker_generation
            || request.authorization.terminal_id != runtime.terminal_id
            || request.authorization.binding_fence != runtime.fence
            || request.target != runtime.host_identity
        {
            return Err(CloseStepTokenMismatch);
        }
        let generation =
            NonZeroU64::new(record.generation.value()).ok_or(CloseStepTokenMismatch)?;
        record.continuation.reserve_step(now, generation, request)
    }

    pub fn confirm_terminal_tab_close_step_admitted(
        &mut self,
        close_request_id: &WorkbenchRequestId,
        token: &CloseStepToken,
        admitted_at: Instant,
    ) -> Result<(), CloseStepTokenMismatch> {
        let record = self
            .close_record_mut(close_request_id)
            .ok_or(CloseStepTokenMismatch)?;
        let generation =
            NonZeroU64::new(record.generation.value()).ok_or(CloseStepTokenMismatch)?;
        record
            .continuation
            .confirm_admitted(token, admitted_at, generation)
    }

    pub fn reject_terminal_tab_close_step_prequeue(
        &mut self,
        close_request_id: &WorkbenchRequestId,
        token: CloseStepToken,
        now: Instant,
        failure: CloseStepPrequeueFailure,
    ) -> Result<(), CloseStepTokenMismatch> {
        self.close_record_mut(close_request_id)
            .ok_or(CloseStepTokenMismatch)?
            .continuation
            .reject_prequeue(token, now, failure)
    }

    pub fn settle_terminal_tab_close_step(
        &mut self,
        close_request_id: &WorkbenchRequestId,
        token: CloseStepToken,
        now: Instant,
        observation: CloseStepObservation<'_>,
    ) -> Result<(), CloseStepTokenMismatch> {
        let record = self
            .close_record_mut(close_request_id)
            .ok_or(CloseStepTokenMismatch)?;
        let generation =
            NonZeroU64::new(record.generation.value()).ok_or(CloseStepTokenMismatch)?;
        record
            .continuation
            .settle_step(token, now, generation, observation)
    }

    /// The Server controller owner supplies its completed exact-target local cleanup.
    pub fn complete_terminal_tab_close_target_cleanup(
        &mut self,
        close_request_id: &WorkbenchRequestId,
        runtime: WorkbenchTabCloseRuntimeTarget,
    ) -> Result<(), CloseStepTokenMismatch> {
        let record = self
            .close_record_mut(close_request_id)
            .ok_or(CloseStepTokenMismatch)?;
        if record.target.runtime != runtime || !record.continuation.target_cleanup_complete() {
            return Err(CloseStepTokenMismatch);
        }
        Ok(())
    }

    pub fn park_terminal_tab_close_target_cleanup(
        &mut self,
        close_request_id: &WorkbenchRequestId,
        runtime: WorkbenchTabCloseRuntimeTarget,
    ) -> Result<(), CloseStepTokenMismatch> {
        let record = self
            .close_record_mut(close_request_id)
            .ok_or(CloseStepTokenMismatch)?;
        if record.target.runtime != runtime || !record.continuation.target_cleanup_failed() {
            return Err(CloseStepTokenMismatch);
        }
        Ok(())
    }
}
