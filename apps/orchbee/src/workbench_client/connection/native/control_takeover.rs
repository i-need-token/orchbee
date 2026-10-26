use super::{ClientExecutor, Result, closed, event, submission};
use crate::{
    local_client::SubmissionError,
    workbench_client::{
        mailbox::{CommandLease, EventDeadline, MutationResult, RefreshMarker, WorkerEvent},
        state::{
            CarrierFailure, ControllerMode,
            control_takeover::{TakeoverEvidence, TakeoverOperation, TakeoverResult},
        },
    },
};
use orchbee_client_server_protocol::{
    ControlRejectedReason, ProjectionDemand, TerminalControlAction, TerminalControlOutcome,
    TerminalControlRequest, TerminalViewerAction, TerminalViewerOutcome,
};

impl ClientExecutor {
    pub(super) fn run_takeover(
        &mut self,
        lease: &CommandLease,
        marker: RefreshMarker,
        operation: TakeoverOperation,
    ) -> Result<()> {
        if self.pending_takeover.is_some()
            || self
                .mailbox
                .mark_refresh_in_flight(operation.generation)
                .map_err(closed)?
                != MutationResult::Applied
            || self.execution_desired(lease)?.as_ref() != Some(operation.key.pane())
        {
            return Err(CarrierFailure::Protocol);
        }
        self.retain_takeover_result(&operation, TakeoverResult::NotSubmitted)?;
        let cutoff = self.effective_lease_cutoff(lease, operation.deadlines.forward_cutoff())?;
        let viewer = self.viewer_id_for(&operation.key)?;
        self.ensure_before(cutoff)?;
        self.retain_takeover_result(&operation, TakeoverResult::Unknown)?;
        let outcome = match self.client.submit_terminal_control_before(
            TerminalControlRequest::new(
                operation.request_id.clone(),
                TerminalControlAction::Takeover {
                    attachment_id: viewer.clone(),
                },
            ),
            cutoff,
        ) {
            Ok(reply) => reply.into_outcome(),
            Err(error) => {
                if !matches!(error, SubmissionError::OutcomeUnknown { .. }) {
                    self.retain_takeover_result(&operation, TakeoverResult::NotSubmitted)?;
                }
                return Err(submission(error));
            }
        };
        match outcome {
            TerminalControlOutcome::Acquired { controller_id } => {
                // Receipt first: subsequent allocation, resize or UI wait may fail.
                self.retain_takeover_result(
                    &operation,
                    TakeoverResult::Granted(controller_id.clone()),
                )?;
                self.set_attached_mode(
                    &operation.key,
                    ControllerMode::Controlling(controller_id.clone()),
                )?;
                self.attached_mut(&operation.key)?.accepted_geometry = None;
                self.ensure_before(cutoff)?;
                let sequence = self.next_control_sequence(&operation.key)?;
                let resized = self.control_once(
                    TerminalControlAction::Resize {
                        attachment_id: viewer.clone(),
                        controller_id: controller_id.clone(),
                        sequence,
                        geometry: operation.geometry,
                    },
                    cutoff,
                )?;
                match resized {
                    TerminalControlOutcome::Resized { geometry }
                        if geometry == operation.geometry =>
                    {
                        self.set_accepted_geometry(&operation.key, geometry)?;
                        // A self-grant or same-size handoff need not change terminal content.
                        // Re-arm this Viewer so the UI's handoff gate always has a fresh full
                        // frame to draw and acknowledge, without replacing the attachment.
                        for demand in [ProjectionDemand::Paused, ProjectionDemand::Active] {
                            self.ensure_before(cutoff)?;
                            match self.viewer_once(
                                TerminalViewerAction::SetProjectionDemand {
                                    attachment_id: viewer.clone(),
                                    demand,
                                },
                                cutoff,
                            )? {
                                TerminalViewerOutcome::FeedbackAccepted {} => {}
                                TerminalViewerOutcome::Attached { .. }
                                | TerminalViewerOutcome::Empty { .. }
                                | TerminalViewerOutcome::Detached { .. }
                                | TerminalViewerOutcome::Rejected { .. }
                                | TerminalViewerOutcome::RowsRejected { .. }
                                | TerminalViewerOutcome::Unavailable { .. }
                                | TerminalViewerOutcome::Record(..) => {
                                    return Err(CarrierFailure::Transport);
                                }
                            }
                        }
                        self.retain_takeover_result(
                            &operation,
                            TakeoverResult::Ready(controller_id),
                        )?;
                    }
                    TerminalControlOutcome::Rejected {
                        reason: ControlRejectedReason::NotController,
                    } => {
                        self.set_attached_mode(&operation.key, ControllerMode::ReadOnly)?;
                        self.retain_takeover_result(
                            &operation,
                            TakeoverResult::Lost(controller_id),
                        )?;
                    }
                    TerminalControlOutcome::OutcomeUnknown {} => {
                        return Err(CarrierFailure::OutcomeUnknown);
                    }
                    TerminalControlOutcome::Acquired { .. }
                    | TerminalControlOutcome::InputAccepted {}
                    | TerminalControlOutcome::Resized { .. }
                    | TerminalControlOutcome::Pasted { .. }
                    | TerminalControlOutcome::HostColorsAccepted {}
                    | TerminalControlOutcome::Cleared { .. }
                    | TerminalControlOutcome::Rejected { .. }
                    | TerminalControlOutcome::Unavailable { .. } => {
                        return Err(CarrierFailure::Transport);
                    }
                }
            }
            TerminalControlOutcome::Rejected { .. }
            | TerminalControlOutcome::Unavailable { .. } => {
                self.retain_takeover_result(&operation, TakeoverResult::Refused)?
            }
            TerminalControlOutcome::OutcomeUnknown {} => {
                return Err(CarrierFailure::OutcomeUnknown);
            }
            TerminalControlOutcome::InputAccepted {}
            | TerminalControlOutcome::Resized { .. }
            | TerminalControlOutcome::Pasted { .. }
            | TerminalControlOutcome::HostColorsAccepted {}
            | TerminalControlOutcome::Cleared { .. } => return Err(CarrierFailure::Protocol),
        }
        let evidence = self
            .pending_takeover
            .clone()
            .ok_or(CarrierFailure::Protocol)?;
        let wait = self
            .mailbox
            .publish_event_before(
                WorkerEvent::ControlTakeoverResult { evidence },
                EventDeadline::PlacementOnly(cutoff),
                &self.clock,
            )
            .map_err(event)?;
        let result = self.finish_refresh(lease, &marker, &wait);
        if result.is_ok() {
            self.pending_takeover = None;
        }
        result
    }
    fn retain_takeover_result(
        &mut self,
        operation: &TakeoverOperation,
        result: TakeoverResult,
    ) -> Result<()> {
        if self
            .pending_takeover
            .as_ref()
            .is_some_and(|prior| prior.operation != *operation || !result.retains(&prior.result))
        {
            return Err(CarrierFailure::Protocol);
        }
        self.pending_takeover = Some(TakeoverEvidence {
            operation: operation.clone(),
            result: result.clone(),
        });
        if !self.mailbox.retain_takeover_result(operation, result) {
            return Err(CarrierFailure::Protocol);
        }
        Ok(())
    }
}
