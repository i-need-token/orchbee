//! Client acknowledgement follows a single Server commit and the shared projection.
use super::{
    ClientLayoutAdjustment, ClientViewCompletionOutcome, ClientViewObservation, ControlStep,
    Reason, WorkbenchClientUi,
};
use crate::local_client::workbench::split_ratio::SplitRatioFailure;
use orchbee_workbench_protocol::{
    ClientLayoutChange, ClientSplitIdentity, SessionId, WorkbenchAction, WorkbenchCommand,
    WorkbenchRevision,
};

pub(super) struct SharedAdjustment {
    adjustment: ClientLayoutAdjustment,
    before: ClientViewObservation,
    submission: Option<Result<WorkbenchRevision, SplitRatioFailure>>,
    refresh_requested: bool,
}
impl SharedAdjustment {
    pub(super) fn prepare(
        session: SessionId,
        operation_id: orchbee_workbench_protocol::WorkbenchRequestId,
        adjustment: ClientLayoutAdjustment,
        before: ClientViewObservation,
    ) -> Result<(Self, WorkbenchCommand), Reason> {
        let ClientLayoutChange::Split { split } = &adjustment.change else {
            return Err(Reason::Unsupported);
        };
        let mutation = WorkbenchCommand {
            protocol_version: orchbee_workbench_protocol::ProtocolVersion,
            request_id: operation_id,
            session_id: session,
            expected_revision: adjustment.workbench_revision,
            action: WorkbenchAction::SplitRatioSet {
                terminal_context_id: adjustment.terminal_context_id.clone(),
                tab_id: adjustment.tab_id.clone(),
                split: ClientSplitIdentity {
                    split_id: split.split_id.clone(),
                    first_slot_id: split.first_slot_id.clone(),
                    second_slot_id: split.second_slot_id.clone(),
                },
                previous_ratio: split.previous_ratio,
                ratio: split.ratio,
            },
        };
        Ok((
            Self {
                adjustment,
                before,
                submission: None,
                refresh_requested: false,
            },
            mutation,
        ))
    }
    pub(super) fn settle(&mut self, reply: Result<WorkbenchRevision, SplitRatioFailure>) {
        if self.submission.is_none() {
            if let Ok(revision) = &reply {
                self.adjustment.workbench_revision = *revision;
            }
            self.submission = Some(reply);
        }
    }
    /// A refusal for an older revision alone: the Server proved the CAS had no effect.
    pub(super) fn refused_for_revision(&self) -> bool {
        matches!(
            self.submission,
            Some(Err(SplitRatioFailure {
                may_have_committed: false,
                reason: Reason::StaleView,
            }))
        )
    }
    /// Prepares the same split move against the newest installed revision, or `None` while that
    /// revision is still being read.
    pub(super) fn rebase(
        &mut self,
        native: &mut WorkbenchClientUi,
    ) -> Result<Option<(Self, WorkbenchCommand)>, Reason> {
        let Some((session, adjustment, before)) = native.rebase_layout_adjustment(
            &self.adjustment,
            &self.before,
            &mut self.refresh_requested,
        )?
        else {
            return Ok(None);
        };
        let request_id = native.local_adjustment_request_id()?;
        Self::prepare(session, request_id, adjustment, before).map(Some)
    }
    pub(super) fn had_effect(&self) -> bool {
        !matches!(
            self.submission,
            Some(Err(SplitRatioFailure {
                may_have_committed: false,
                ..
            }))
        )
    }
    pub(super) fn step(&mut self, native: &mut WorkbenchClientUi) -> ControlStep {
        match self.submission {
            None => ControlStep::Waiting,
            Some(Err(failure)) => ControlStep::Failed(failure.reason),
            Some(Ok(_)) => {
                let Some((_, view)) = native.control_observation(false) else {
                    return ControlStep::Failed(Reason::ProjectionUnavailable);
                };
                if view.selection != self.before.selection {
                    return ControlStep::Failed(Reason::Interrupted);
                }
                if view.phase != self.before.phase {
                    return ControlStep::Waiting;
                }
                if !self.adjustment.selected {
                    return ControlStep::Completed(ClientViewCompletionOutcome::LayoutAdjusted {
                        adjustment: self.adjustment.clone(),
                    });
                }
                if view.workbench_revision < self.adjustment.workbench_revision {
                    return native.refresh_layout_adjustment(&mut self.refresh_requested);
                }
                if native.adjustment_observed(&self.adjustment, &self.before) {
                    ControlStep::Observed
                } else {
                    ControlStep::Failed(Reason::StaleView)
                }
            }
        }
    }
    pub(super) fn outcome(
        &self,
        native: &WorkbenchClientUi,
    ) -> Option<ClientViewCompletionOutcome> {
        self.submission.as_ref()?.as_ref().ok()?;
        native
            .adjustment_observed(&self.adjustment, &self.before)
            .then(|| {
                let mut adjustment = self.adjustment.clone();
                if let Some((_, view)) = native.control_observation(false) {
                    adjustment.workbench_revision = view.workbench_revision;
                }
                ClientViewCompletionOutcome::LayoutAdjusted { adjustment }
            })
    }
}
