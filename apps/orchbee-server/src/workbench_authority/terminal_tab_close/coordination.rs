//! Associates retained close records with exact physical lifecycle steps and settles matching
//! admission, completion, and local cleanup reports. It updates canonical close state without
//! performing Worker requests or directly publishing topology removal.

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
use super::{
    CloseContinuation, CloseRecord, WorkbenchTabCloseRuntimeTarget, WorkbenchTabCloseStepTarget,
    WorkbenchTabCloseTarget,
};

impl WorkbenchAuthority {
    pub(in crate::workbench_authority) fn terminal_close_captured_members(
        &self,
        close: &WorkbenchRequestId,
    ) -> Option<Vec<super::super::WorkbenchTerminalTab>> {
        self.close_record(close)
            .map(|record| record.members().cloned().collect())
    }
    pub(in crate::workbench_authority) fn terminal_close_matches_workspace(
        &self,
        close: &WorkbenchRequestId,
        workspace: &orchbee_workbench_protocol::WorkbenchWorkspaceId,
    ) -> bool {
        self.close_record(close)
            .is_some_and(|record| !record.is_finished() && record.matches_context_scope(workspace))
    }
    pub(in crate::workbench_authority) fn terminal_close_publication_ready(
        &self,
        close: &WorkbenchRequestId,
    ) -> bool {
        self.close_record(close)
            .is_some_and(|record| !record.is_finished() && record.publication_ready())
    }
    /// Called once no Worktree operation holds this Close any longer.
    pub(in crate::workbench_authority) fn renew_released_close_publication(
        &mut self,
        close: &WorkbenchRequestId,
        now: Instant,
    ) {
        if let Some(record) = self.close_record_mut(close) {
            record.renew_held_publication(now);
        }
    }
    pub(in crate::workbench_authority) fn retain_quiescent_workspace(
        &mut self,
        close: &WorkbenchRequestId,
    ) -> bool {
        let Some(record) = self.close_record_mut(close) else {
            return false;
        };
        if record.is_finished()
            || !record.publication_ready()
            || !matches!(record.scope, super::CloseScope::Context { .. })
        {
            return false;
        }
        record.publication_failed(
            orchbee_workbench_protocol::WorkbenchTabClosePublicationReason::WorkspaceRetained,
        );
        record.workspace_retained = true;
        true
    }

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
                RetainedRequest::TabClose(record)
                    if record.target_index_for_binding(binding).is_some() =>
                {
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
        self.close_record(close_request_id).map(CloseRecord::status)
    }

    pub fn terminal_tab_close_operations(&self) -> impl Iterator<Item = &WorkbenchRequestId> {
        self.ledger
            .iter()
            .filter_map(|(request_id, entry)| match entry {
                RetainedRequest::TabClose(record) if !record.is_finished() => Some(request_id),
                _ => None,
            })
    }

    #[must_use]
    pub fn terminal_tab_close_target(
        &self,
        close_request_id: &WorkbenchRequestId,
    ) -> Option<&WorkbenchTabCloseTarget> {
        self.close_record(close_request_id)
            .and_then(|record| record.targets().next())
    }

    #[must_use]
    pub fn terminal_tab_close_target_for_binding(
        &self,
        close_request_id: &WorkbenchRequestId,
        binding: &orchbee_workbench_protocol::PresentationBinding,
    ) -> Option<&WorkbenchTabCloseTarget> {
        self.close_record(close_request_id)
            .and_then(|record| record.target_for_binding(binding))
    }

    #[must_use]
    pub fn terminal_tab_close_target_status(
        &self,
        close_request_id: &WorkbenchRequestId,
        runtime: WorkbenchTabCloseRuntimeTarget,
    ) -> Option<orchbee_workbench_protocol::WorkbenchTabCloseStatus> {
        let record = self.close_record(close_request_id)?;
        let index = record.target_index_for_runtime(runtime)?;
        record.continuation(index).map(CloseContinuation::status)
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
        self.next_terminal_tab_close_step_target(close_request_id, now)
            .map(|step| step.next())
    }

    pub fn next_terminal_tab_close_step_target(
        &mut self,
        close_request_id: &WorkbenchRequestId,
        now: Instant,
    ) -> Result<WorkbenchTabCloseStepTarget, CloseStepTokenMismatch> {
        let held = self.worktree_holds_close(close_request_id);
        let record = self
            .close_record_mut(close_request_id)
            .ok_or(CloseStepTokenMismatch)?;
        Ok(if held {
            record.held_step(now)
        } else {
            record.next_step(now)
        })
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
        let index = record
            .target_index_for_request(&request)
            .ok_or(CloseStepTokenMismatch)?;
        let runtime = record
            .target(index)
            .ok_or(CloseStepTokenMismatch)?
            .runtime()
            .ok_or(CloseStepTokenMismatch)?;
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
        record
            .continuation_mut(index)
            .ok_or(CloseStepTokenMismatch)?
            .reserve_step(now, generation, request)
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
        let index = record
            .target_index_for_request(&token.request())
            .ok_or(CloseStepTokenMismatch)?;
        let generation =
            NonZeroU64::new(record.generation.value()).ok_or(CloseStepTokenMismatch)?;
        record
            .continuation_mut(index)
            .ok_or(CloseStepTokenMismatch)?
            .confirm_admitted(token, admitted_at, generation)
    }

    pub fn reject_terminal_tab_close_step_prequeue(
        &mut self,
        close_request_id: &WorkbenchRequestId,
        token: CloseStepToken,
        now: Instant,
        failure: CloseStepPrequeueFailure,
    ) -> Result<(), CloseStepTokenMismatch> {
        let record = self
            .close_record_mut(close_request_id)
            .ok_or(CloseStepTokenMismatch)?;
        let index = record
            .target_index_for_request(&token.request())
            .ok_or(CloseStepTokenMismatch)?;
        record
            .continuation_mut(index)
            .ok_or(CloseStepTokenMismatch)?
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
        let index = record
            .target_index_for_request(&token.request())
            .ok_or(CloseStepTokenMismatch)?;
        let generation =
            NonZeroU64::new(record.generation.value()).ok_or(CloseStepTokenMismatch)?;
        record
            .continuation_mut(index)
            .ok_or(CloseStepTokenMismatch)?
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
        let index = record
            .target_index_for_runtime(runtime)
            .ok_or(CloseStepTokenMismatch)?;
        if !record
            .continuation_mut(index)
            .ok_or(CloseStepTokenMismatch)?
            .target_cleanup_complete()
        {
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
        let index = record
            .target_index_for_runtime(runtime)
            .ok_or(CloseStepTokenMismatch)?;
        if !record
            .continuation_mut(index)
            .ok_or(CloseStepTokenMismatch)?
            .target_cleanup_failed()
        {
            return Err(CloseStepTokenMismatch);
        }
        Ok(())
    }
}
