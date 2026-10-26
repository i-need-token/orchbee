//! Explicit control transfer; Server grants and frame admission remain separate authorities.
use super::{
    ClientPhase, ControllerMode, OperationDeadlines, PaneRef, PendingRefreshFlight,
    WorkbenchClientState,
};
use orchbee_client_server_protocol::{ControllerId, Geometry, RequestId};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct TakeoverOperation {
    pub(in crate::workbench_client) key: PaneRef,
    pub(in crate::workbench_client) request_id: RequestId,
    pub(in crate::workbench_client) generation: u64,
    pub(in crate::workbench_client) geometry: Geometry,
    pub(in crate::workbench_client) deadlines: OperationDeadlines,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum TakeoverResult {
    NotSubmitted,
    Refused,
    Unknown,
    Granted(ControllerId),
    Ready(ControllerId),
    Lost(ControllerId),
}
impl TakeoverResult {
    fn grant(&self) -> Option<&ControllerId> {
        match self {
            Self::Granted(id) | Self::Ready(id) | Self::Lost(id) => Some(id),
            _ => None,
        }
    }
    pub(in crate::workbench_client) fn retains(&self, prior: &Self) -> bool {
        prior.grant().is_none_or(|id| self.grant() == Some(id))
            && (!matches!(prior, Self::Ready(_) | Self::Lost(_)) || self == prior)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct TakeoverEvidence {
    pub(in crate::workbench_client) operation: TakeoverOperation,
    pub(in crate::workbench_client) result: TakeoverResult,
}
#[derive(Default)]
pub(super) struct TakeoverState {
    pending: Option<TakeoverOperation>,
    awaiting_frame: Option<PaneRef>,
    pub(super) evidence: Option<TakeoverEvidence>,
    show_pending: bool,
    status_until: Option<Instant>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TakeoverStatus {
    Pending,
    Refused,
    Unknown,
    Granted,
    WaitingForFrame,
    Ready,
    Lost,
}
impl WorkbenchClientState {
    pub(in crate::workbench_client) fn takeover_pending(&self) -> bool {
        self.takeover.pending.is_some()
    }
    pub(in crate::workbench_client) fn takeover_input_ready(&self) -> bool {
        if self.takeover_pending() {
            return false;
        }
        self.takeover
            .awaiting_frame
            .as_ref()
            .is_none_or(|key| self.focus_pane_ref().as_ref() != Some(key))
    }
    pub(super) fn accept_takeover_frame(&mut self, key: &PaneRef) {
        if self.takeover.awaiting_frame.as_ref() == Some(key)
            && self.live_pane_record(key).is_some_and(|record| {
                record.accepted_frame.is_some()
                    && record.last_accepted_geometry == Some(record.observed_geometry)
            })
        {
            // A handoff gate, not a gate around every subsequent ordinary frame update.
            self.takeover.awaiting_frame = None;
        }
    }
    pub(in crate::workbench_client) fn plan_takeover(
        &self,
        request_id: RequestId,
        generation: u64,
        deadlines: OperationDeadlines,
    ) -> Option<TakeoverOperation> {
        if generation == 0
            || self.pending_refresh.is_some()
            || self.pending_membership.is_some()
            || self.mutation_blocks_structure()
            || self.rename_draft_blocks_structure()
            || self.panel_close.confirmation().is_some()
            || !self.close.permits_retired_membership()
            || !matches!(self.phase, ClientPhase::Ready(_))
            || self.takeover.evidence.as_ref().is_some_and(|evidence| {
                matches!(
                    evidence.result,
                    TakeoverResult::Unknown | TakeoverResult::Granted(_)
                )
            })
        {
            return None;
        }
        let key = self.focus_pane_ref()?;
        let record = self.live_pane_record(&key)?;
        if !record.attached || !record.projected || record.departing {
            return None;
        }
        Some(TakeoverOperation {
            key,
            request_id,
            generation,
            geometry: record.observed_geometry,
            deadlines,
        })
    }
    pub(in crate::workbench_client) fn begin_takeover(
        &mut self,
        operation: TakeoverOperation,
    ) -> bool {
        if self
            .plan_takeover(
                operation.request_id.clone(),
                operation.generation,
                operation.deadlines,
            )
            .as_ref()
            != Some(&operation)
        {
            return false;
        }
        self.pending_refresh = Some(PendingRefreshFlight::recovery(operation.generation));
        self.takeover.pending = Some(operation);
        self.takeover.evidence = None;
        self.takeover.show_pending = false;
        self.takeover.status_until = None;
        true
    }
    pub(in crate::workbench_client) fn retain_takeover(
        &mut self,
        evidence: &TakeoverEvidence,
    ) -> bool {
        // A settled lease can return after a new explicit gesture has entered the lane.
        if self
            .takeover
            .pending
            .as_ref()
            .is_some_and(|next| next.generation > evidence.operation.generation)
            && matches!(
                evidence.result,
                TakeoverResult::Ready(_)
                    | TakeoverResult::Lost(_)
                    | TakeoverResult::Refused
                    | TakeoverResult::NotSubmitted
            )
        {
            return true;
        }
        if let Some(prior) = &self.takeover.evidence {
            if prior.operation != evidence.operation {
                return false;
            }
            if let (Some(before), Some(after)) = (prior.result.grant(), evidence.result.grant())
                && before != after
            {
                return false;
            }
            if !evidence.result.retains(&prior.result) {
                return true;
            }
        } else if self.takeover.pending.as_ref() != Some(&evidence.operation) {
            return false;
        }
        self.takeover.evidence = Some(evidence.clone());
        true
    }
    pub(in crate::workbench_client) fn finish_takeover(
        &mut self,
        evidence: &TakeoverEvidence,
        sampled: Instant,
    ) -> bool {
        if !self.retain_takeover(evidence) {
            return false;
        }
        if self.takeover.pending.as_ref() != Some(&evidence.operation) {
            return true;
        }
        if !matches!(self.phase, ClientPhase::Retired(_)) {
            if self.focus_pane_ref().as_ref() != Some(&evidence.operation.key) {
                return false;
            }
            let mode = match &evidence.result {
                TakeoverResult::Ready(id) => Some(ControllerMode::Controlling(id.clone())),
                TakeoverResult::Lost(_) => Some(ControllerMode::ReadOnly),
                _ => None,
            };
            if let Some(mode) = mode {
                self.takeover.awaiting_frame = matches!(evidence.result, TakeoverResult::Ready(_))
                    .then(|| evidence.operation.key.clone());
                let ClientPhase::Ready(ready) = &mut self.phase else {
                    return false;
                };
                ready.mode = mode.clone();
                let Some(record) = self.live_pane_record_mut(&evidence.operation.key) else {
                    return false;
                };
                record.mode = Some(mode);
                record.fault = None;
                record.last_accepted_geometry = matches!(evidence.result, TakeoverResult::Ready(_))
                    .then_some(evidence.operation.geometry);
                record.last_admitted_geometry = record.last_accepted_geometry;
                record.staged_frame = None;
                record.accepted_frame = None;
            }
            self.pending_refresh = None;
        }
        self.takeover.pending = None;
        self.takeover.status_until = sampled.checked_add(Duration::from_secs(5));
        true
    }
    pub(in crate::workbench_client) fn tick_takeover(&mut self, sampled: Instant) -> bool {
        if self
            .takeover
            .status_until
            .is_some_and(|until| sampled >= until)
        {
            self.takeover.status_until = None;
            return true;
        }
        if !self.takeover.show_pending
            && self.takeover.pending.as_ref().is_some_and(|operation| {
                sampled
                    >= operation.deadlines.total() - super::OPERATION_TOTAL
                        + Duration::from_millis(200)
            })
        {
            self.takeover.show_pending = true;
            return true;
        }
        false
    }
    pub(in crate::workbench_client) fn takeover_status(
        &self,
    ) -> Option<(TakeoverStatus, &RequestId)> {
        if let Some(pending) = &self.takeover.pending {
            return self
                .takeover
                .show_pending
                .then_some((TakeoverStatus::Pending, &pending.request_id));
        }
        let evidence = self.takeover.evidence.as_ref()?;
        let retained = matches!(
            evidence.result,
            TakeoverResult::Unknown | TakeoverResult::Granted(_)
        );
        let awaiting_frame = self.takeover.awaiting_frame.as_ref() == Some(&evidence.operation.key);
        if !retained
            && (self.focus_pane_ref().as_ref() != Some(&evidence.operation.key)
                || (!awaiting_frame && self.takeover.status_until.is_none()))
        {
            return None;
        }
        let status = match &evidence.result {
            TakeoverResult::NotSubmitted | TakeoverResult::Refused => TakeoverStatus::Refused,
            TakeoverResult::Unknown => TakeoverStatus::Unknown,
            TakeoverResult::Granted(_) => TakeoverStatus::Granted,
            TakeoverResult::Ready(id) => {
                if self
                    .ready()
                    .is_none_or(|ready| ready.mode != ControllerMode::Controlling(id.clone()))
                {
                    return None;
                }
                if awaiting_frame {
                    TakeoverStatus::WaitingForFrame
                } else {
                    TakeoverStatus::Ready
                }
            }
            TakeoverResult::Lost(_) => TakeoverStatus::Lost,
        };
        Some((status, &evidence.operation.request_id))
    }
    pub(super) fn takeover_invariant_holds(&self) -> bool {
        self.takeover.pending.as_ref().is_none_or(|operation| {
            matches!(self.phase, ClientPhase::Retired(_))
                || (self.focus_pane_ref().as_ref() == Some(&operation.key)
                    && self.pending_refresh_generation() == Some(operation.generation))
        })
    }
}
impl super::WorkbenchClientView<'_> {
    pub(in crate::workbench_client) fn takeover_status(
        &self,
    ) -> Option<(TakeoverStatus, &RequestId)> {
        self.state.takeover_status()
    }
}
