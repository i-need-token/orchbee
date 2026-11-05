//! Executes one captured controller takeover and reconciles its exact receipt and projection
//! through the UI handoff. It does not decide confirmation presentation or repeat a possibly
//! delivered takeover.

use super::{ClientExecutor, Result, closed, event, submission};
use crate::local_client::SubmissionError;
use crate::workbench_client::mailbox::{
    CommandLease, EventDeadline, MutationResult, RefreshMarker, WorkerEvent,
};
use crate::workbench_state::control_takeover::{
    TakeoverEvidence, TakeoverOperation, TakeoverResult,
};
use crate::workbench_state::{CarrierFailure, ControllerMode};
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
        let observed_grant = if operation.automatic {
            let state = match self.control_once(
                TerminalControlAction::Activate {
                    attachment_id: viewer.clone(),
                    geometry: operation.geometry,
                },
                cutoff,
            ) {
                Ok(TerminalControlOutcome::Observed { state }) => state,
                Ok(
                    TerminalControlOutcome::Rejected { .. }
                    | TerminalControlOutcome::Unavailable { .. },
                ) => {
                    self.retain_takeover_result(&operation, TakeoverResult::Refused)?;
                    return self.finish_control_activation(lease, marker, cutoff);
                }
                Ok(TerminalControlOutcome::OutcomeUnknown {})
                | Err(CarrierFailure::OutcomeUnknown) => {
                    self.retain_takeover_result(&operation, TakeoverResult::Unknown)?;
                    return self.finish_control_activation(
                        lease,
                        marker,
                        operation.deadlines.total(),
                    );
                }
                Err(failure) => return Err(failure),
                _ => return Err(CarrierFailure::Protocol),
            };
            self.attached_mut(&operation.key)?.control_observation_due =
                Some(self.clock.now() + std::time::Duration::from_millis(250));
            if let Some(id) = &state.controller_id {
                let attached = self.attached_mut(&operation.key)?;
                attached.next_sequence = attached.next_sequence.max(state.sequence);
                if attached.mode == Some(ControllerMode::Controlling(id.clone()))
                    && attached.accepted_geometry == Some(operation.geometry)
                    && attached.control_frame.is_none()
                {
                    self.retain_takeover_result(&operation, TakeoverResult::Unchanged(id.clone()))?;
                    return self.finish_control_activation(lease, marker, cutoff);
                }
            }
            state.controller_id
        } else {
            None
        };
        self.retain_takeover_result(&operation, TakeoverResult::Unknown)?;
        let outcome = if let Some(controller_id) = observed_grant {
            TerminalControlOutcome::Acquired { controller_id }
        } else {
            match self.client.submit_terminal_control_before(
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
                    if matches!(error, SubmissionError::OutcomeUnknown { .. }) {
                        return self.finish_control_activation(
                            lease,
                            marker,
                            operation.deadlines.total(),
                        );
                    }
                    self.retain_takeover_result(&operation, TakeoverResult::NotSubmitted)?;
                    return Err(submission(error));
                }
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
                super::settle_ready_flow(self.sync_host_colors_for_lease(
                    lease,
                    &operation.key,
                    controller_id.clone(),
                    cutoff,
                ))?;
                self.attached_mut(&operation.key)?.control_frame =
                    Some(super::ControlFrame::Request);
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
                    TerminalControlOutcome::Observed { .. } => {
                        return Err(CarrierFailure::Protocol);
                    }
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
                        self.attached_mut(&operation.key)?.control_frame =
                            Some(super::ControlFrame::Waiting);
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
                        return self.finish_control_activation(lease, marker, cutoff);
                    }
                    TerminalControlOutcome::Acquired { .. }
                    | TerminalControlOutcome::InputAccepted {}
                    | TerminalControlOutcome::Resized { .. }
                    | TerminalControlOutcome::Pasted { .. }
                    | TerminalControlOutcome::HostColorsAccepted {}
                    | TerminalControlOutcome::Cleared { .. }
                    | TerminalControlOutcome::ResetAccepted { .. }
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
                return self.finish_control_activation(lease, marker, cutoff);
            }
            TerminalControlOutcome::Observed { .. }
            | TerminalControlOutcome::InputAccepted {}
            | TerminalControlOutcome::Resized { .. }
            | TerminalControlOutcome::Pasted { .. }
            | TerminalControlOutcome::HostColorsAccepted {}
            | TerminalControlOutcome::Cleared { .. }
            | TerminalControlOutcome::ResetAccepted { .. } => return Err(CarrierFailure::Protocol),
        }
        self.finish_control_activation(lease, marker, cutoff)
    }
    fn finish_control_activation(
        &mut self,
        lease: &CommandLease,
        marker: RefreshMarker,
        cutoff: std::time::Instant,
    ) -> Result<()> {
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
    pub(super) fn observe_controller(
        &mut self,
        lease: &CommandLease,
        key: &crate::workbench_state::PaneRef,
        deadline: std::time::Instant,
    ) -> super::ReadyResult<()> {
        if self
            .attached_ref(key)?
            .control_observation_due
            .is_none_or(|due| self.clock.now() < due)
        {
            return Ok(());
        }
        let cutoff = self.effective_lease_cutoff(lease, deadline)?;
        let outcome = self.control_once(
            TerminalControlAction::Observe {
                attachment_id: self.viewer_id_for(key)?,
            },
            cutoff,
        )?;
        let sampled = self.clock.now();
        let attached = self.attached_mut(key)?;
        attached.control_observation_due = Some(sampled + std::time::Duration::from_millis(250));
        let TerminalControlOutcome::Observed { state } = outcome else {
            return Ok(());
        };
        let (mode, geometry) = match (state.controller_id, state.geometry) {
            (Some(id), Some(geometry)) => (ControllerMode::Controlling(id), Some(geometry)),
            _ => (ControllerMode::ReadOnly, None),
        };
        attached.next_sequence = attached.next_sequence.max(state.sequence);
        if attached.mode.as_ref() == Some(&mode) && attached.accepted_geometry == geometry {
            return Ok(());
        }
        attached.control_frame =
            matches!(mode, ControllerMode::Controlling(_)).then_some(super::ControlFrame::Request);
        attached.mode = Some(mode.clone());
        attached.accepted_geometry = geometry;
        attached.reported_host_colors = None;
        // Keep the accepted picture while a newly learned grant waits for a compatible,
        // drawn frame. Ownership alone cannot open the input gate.
        self.mailbox
            .publish_notice_before(
                WorkerEvent::ControllerObserved {
                    key: key.clone(),
                    mode: ControllerMode::ReadOnly,
                    geometry: None,
                },
                cutoff,
                &self.clock,
            )
            .map_err(|error| self.ready_event_error(lease, key, deadline, error))?;
        Ok(())
    }
    pub(super) fn prepare_control_frame(
        &mut self,
        lease: &CommandLease,
        key: &crate::workbench_state::PaneRef,
        deadline: std::time::Instant,
    ) -> super::ReadyResult<()> {
        if self.attached_ref(key)?.control_frame != Some(super::ControlFrame::Request) {
            return Ok(());
        }
        for demand in [ProjectionDemand::Paused, ProjectionDemand::Active] {
            let Some((_, outcome)) = self.viewer_for_ready_lease(
                lease,
                key,
                TerminalViewerAction::SetProjectionDemand {
                    attachment_id: self.viewer_id_for(key)?,
                    demand,
                },
                deadline,
            )?
            else {
                return Ok(());
            };
            if !matches!(outcome, TerminalViewerOutcome::FeedbackAccepted {}) {
                return Err(CarrierFailure::Protocol.into());
            }
        }
        self.attached_mut(key)?.control_frame = Some(super::ControlFrame::Waiting);
        Ok(())
    }
    pub(super) fn finish_control_frame(
        &mut self,
        lease: &CommandLease,
        key: &crate::workbench_state::PaneRef,
        deadline: std::time::Instant,
    ) -> super::ReadyResult<()> {
        let attached = self.attached_mut(key)?;
        if attached.control_frame.take().is_none() {
            return Ok(());
        }
        let mode = attached.mode.clone().ok_or(CarrierFailure::Protocol)?;
        let geometry = attached.accepted_geometry;
        let cutoff = self.effective_lease_cutoff(lease, deadline)?;
        self.mailbox
            .publish_notice_before(
                WorkerEvent::ControllerObserved {
                    key: key.clone(),
                    mode,
                    geometry,
                },
                cutoff,
                &self.clock,
            )
            .map_err(|error| self.ready_event_error(lease, key, deadline, error))?;
        Ok(())
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
