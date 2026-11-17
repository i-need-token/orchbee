//! Contains a projection record the Client refused to its pane. The record is dropped. Unless it is
//! a program effect, the pane's frame is invalidated: a frame whose route is still readable is
//! answered with a snapshot request naming that flight, and any other record leaves the next frame
//! to bring the snapshot. Resyncs without an applied frame are bounded. Past the bound, or on a
//! record the Client cannot read (it may be a frame the Server still waits on), only this pane
//! stops, with the refused record's class in the log. The Session and connection stay; the
//! protocol and its bound are modeled in `formal/tla/ProjectionRecordResync.tla`.
use super::{CausalSettlement, ClientProjectionStatus, PendingDraw, WorkbenchClientUi};
use crate::terminal_projection::{ProjectionRejection, RejectedRecord};
use crate::workbench_client::diagnostic_events::{self, RecordContainment};
use crate::workbench_client::mailbox::PollKey;
use crate::workbench_state::CarrierFailure;
use orchbee_terminal_protocol::decode_transport_envelope;

/// Consecutive resyncs a pane may take without an applied frame before it stops.
const RESYNC_BOUND: u8 = 3;

/// A refused record's class as closed log tokens: its family and the stage that refused it.
#[derive(Clone, Copy)]
pub(super) struct RecordClass {
    operation: &'static str,
    stage: &'static str,
}

impl RecordClass {
    pub(super) const fn new(operation: &'static str, stage: &'static str) -> Self {
        Self { operation, stage }
    }

    pub(super) const fn of(rejection: ProjectionRejection) -> Self {
        Self::new(rejection.operation().as_str(), rejection.stage().as_str())
    }

    /// The class of a record the pane's effect stream refused: an envelope it could not decode, or
    /// an effect event it refused.
    pub(super) fn effect(record: &[u8]) -> Self {
        if decode_transport_envelope(record).is_err() {
            Self::new("record", "envelope-decode")
        } else {
            Self::new("effect", "invalid")
        }
    }
}

impl WorkbenchClientUi {
    /// Contains one refused record at its pane and owes the poll's causal settlement.
    pub(super) fn contain_rejected_record(
        &mut self,
        index: usize,
        poll_key: PollKey,
        record: &[u8],
        class: RecordClass,
        ordinary: bool,
    ) -> bool {
        let projection = &mut self.projections[index];
        let rejected = projection.value.resync_after_rejection(record);
        let settlement = match rejected {
            RejectedRecord::Effect => {
                diagnostic_events::record_contained(
                    class.operation,
                    class.stage,
                    RecordContainment::Dropped,
                );
                CausalSettlement::NoFeedback
            }
            RejectedRecord::Unreadable => {
                self.stop_pane(index, class);
                CausalSettlement::NoFeedback
            }
            RejectedRecord::Frame(_) | RejectedRecord::Other
                if projection.resyncs >= RESYNC_BOUND =>
            {
                self.stop_pane(index, class);
                CausalSettlement::NoFeedback
            }
            rejected @ (RejectedRecord::Frame(_) | RejectedRecord::Other) => {
                projection.resyncs += 1;
                projection.status = ClientProjectionStatus::WaitingForFullFrame;
                if self
                    .state
                    .as_mut()
                    .is_none_or(|state| !state.invalidate_projection_frame(poll_key.pane_ref()))
                {
                    self.retire(CarrierFailure::Protocol);
                    return true;
                }
                diagnostic_events::record_contained(
                    class.operation,
                    class.stage,
                    RecordContainment::Resynced,
                );
                match rejected {
                    RejectedRecord::Frame(feedback) => CausalSettlement::Feedback(feedback),
                    _ => CausalSettlement::NoFeedback,
                }
            }
        };
        self.pending_draws.push(PendingDraw {
            poll_key,
            settlement,
            frame: None,
            ordinary,
        });
        true
    }

    /// Stops one pane alone: its frame and retained pixels go, it is no longer polled, and a late
    /// result for it is dropped. A new attachment of the pane starts afresh.
    fn stop_pane(&mut self, index: usize, class: RecordClass) {
        let projection = &mut self.projections[index];
        projection.value.clear_frame();
        projection.status = ClientProjectionStatus::Stopped;
        let key = projection.key.clone();
        if let Some(state) = self.state.as_mut() {
            // A pane the state no longer projects has no frame proof left to void.
            state.invalidate_projection_frame(&key);
            self.retained_frames.invalidate(state, key.pane().panel());
        }
        diagnostic_events::record_contained(
            class.operation,
            class.stage,
            RecordContainment::Stopped,
        );
    }

    /// Whether this pane stopped on refused records and takes no further polls.
    pub(super) fn pane_stopped(&self, key: &crate::workbench_state::PaneRef) -> bool {
        self.projections.iter().any(|projection| {
            projection.key == *key && projection.status == ClientProjectionStatus::Stopped
        })
    }
}
