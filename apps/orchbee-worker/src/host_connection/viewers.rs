use std::{
    collections::{BTreeMap, VecDeque},
    io,
};

use orchbee_capability_node_protocol::{
    OperationId, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
    ViewerRejection,
};
use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, HistoryControl, HostEpoch, MessageRole,
    PayloadEncoding, ProjectionDemandControl, TerminalEffectEvent, TerminalFrameRoute,
    decode_history_control, decode_projection_demand_control, decode_terminal_effect_event,
    decode_transport_envelope, inspect_terminal_frame_route,
};

use super::validate_event;

pub(super) const MAXIMUM_VIEWERS: usize = 24;
const MAXIMUM_VIEWER_HISTORY_EVENTS: usize = 4;
const MAXIMUM_RETAINED_VIEWER_RECORD_BYTES: usize = 128 * 1024 * 1024;

pub(crate) struct HostViewers {
    viewers: BTreeMap<[u8; 16], Viewer>,
    retained_record_bytes: usize,
    retained_record_limit: usize,
}

impl Default for HostViewers {
    fn default() -> Self {
        Self {
            viewers: BTreeMap::new(),
            retained_record_bytes: 0,
            retained_record_limit: MAXIMUM_RETAINED_VIEWER_RECORD_BYTES,
        }
    }
}

struct Viewer {
    terminal_id: orchbee_capability_node_protocol::HostTerminalId,
    record: Option<Vec<u8>>,
    history: VecDeque<Vec<u8>>,
    next_preference: PollSource,
    closed: bool,
    flight: Option<Flight>,
    last_sequence: u64,
    needs_snapshot: bool,
    blocked_revision: Option<u64>,
}

struct Flight {
    route: TerminalFrameRoute,
    delivered: bool,
}

/// Which poll source the next completed poll prefers. A completed take flips
/// the preference, so a continuously ready source overtakes the other at most
/// once; the frame/projection-blocked record slot is preferred initially.
#[derive(Clone, Copy, Eq, PartialEq)]
enum PollSource {
    Record,
    History,
}

impl HostViewers {
    #[cfg(windows)]
    pub(crate) fn is_empty(&self) -> bool {
        self.viewers.is_empty()
    }
    pub(crate) fn admit(&mut self, request: &TerminalViewerRequest) -> Result<(), ViewerRejection> {
        let TerminalViewerAction::Attach { terminal_id, .. } = request.action else {
            return Err(ViewerRejection::AttachmentUnavailable);
        };
        let key = request.attachment_id.as_bytes();
        if self.viewers.contains_key(&key) {
            return Err(ViewerRejection::AttachmentAlreadyExists);
        }
        if self.viewers.len() == MAXIMUM_VIEWERS {
            return Err(ViewerRejection::CapacityExceeded);
        }
        self.viewers.insert(
            key,
            Viewer {
                terminal_id,
                record: None,
                history: VecDeque::new(),
                next_preference: PollSource::Record,
                closed: false,
                flight: None,
                last_sequence: 0,
                needs_snapshot: true,
                blocked_revision: None,
            },
        );
        Ok(())
    }

    pub(crate) fn target(
        &self,
        id: OperationId,
    ) -> Option<orchbee_capability_node_protocol::HostTerminalId> {
        self.viewers
            .get(&id.as_bytes())
            .filter(|viewer| !viewer.closed)
            .map(|viewer| viewer.terminal_id)
    }

    pub(crate) fn remove(&mut self, id: OperationId) {
        if let Some(mut viewer) = self.viewers.remove(&id.as_bytes()) {
            self.retained_record_bytes -= viewer.close();
        }
    }

    #[cfg(test)]
    fn with_retained_record_limit(limit: usize) -> Self {
        Self {
            retained_record_limit: limit,
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(super) fn retained_record_bytes(&self) -> usize {
        self.retained_record_bytes
    }

    pub(super) fn target_ids(
        &self,
        terminal: orchbee_capability_node_protocol::HostTerminalId,
    ) -> Result<
        [Option<OperationId>; MAXIMUM_VIEWERS],
        orchbee_capability_node_protocol::ProtocolError,
    > {
        let mut ids = [None; MAXIMUM_VIEWERS];
        for (destination, (id, _)) in ids.iter_mut().zip(
            self.viewers
                .iter()
                .filter(|(_, viewer)| viewer.terminal_id == terminal),
        ) {
            *destination = Some(OperationId::from_bytes(*id)?);
        }
        Ok(ids)
    }

    pub(crate) fn closed_ids(&self) -> Vec<[u8; 16]> {
        self.viewers
            .iter()
            .filter_map(|(id, viewer)| viewer.closed.then_some(*id))
            .collect()
    }

    pub(crate) fn contains(&self, id: OperationId) -> bool {
        self.viewers.contains_key(&id.as_bytes())
    }

    /// An admitted viewer that has not been closed is open for buffer reads.
    pub(crate) fn open(&self, id: OperationId) -> bool {
        self.viewers
            .get(&id.as_bytes())
            .is_some_and(|viewer| !viewer.closed)
    }

    pub(crate) fn poll(&mut self, id: OperationId) -> TerminalViewerOutcome {
        let Some(viewer) = self.viewers.get_mut(&id.as_bytes()) else {
            return TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable);
        };
        if viewer.closed {
            return TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable);
        }
        // The preferred source wins when both are readable; a single readable
        // source is always served. Only a completed take flips the preference,
        // so neither frames nor queued events can starve the other. Serving
        // the event FIFO never touches the frame flight.
        let history_ready = !viewer.history.is_empty();
        let serve_history = match viewer.next_preference {
            PollSource::History => history_ready,
            PollSource::Record => viewer.record.is_none() && history_ready,
        };
        if serve_history {
            let Some(event) = viewer.history.pop_front() else {
                return TerminalViewerOutcome::Empty;
            };
            self.retained_record_bytes -= event.len();
            viewer.next_preference = PollSource::Record;
            return TerminalViewerOutcome::Record(event);
        }
        match viewer.record.take() {
            Some(record) => {
                self.retained_record_bytes -= record.len();
                if let Some(flight) = &mut viewer.flight {
                    flight.delivered = true;
                }
                viewer.next_preference = PollSource::History;
                TerminalViewerOutcome::Record(record)
            }
            None => TerminalViewerOutcome::Empty,
        }
    }

    pub(crate) fn feedback(
        &mut self,
        request: &TerminalViewerRequest,
    ) -> Result<(), ViewerRejection> {
        let viewer = self
            .viewers
            .get_mut(&request.attachment_id.as_bytes())
            .ok_or(ViewerRejection::AttachmentUnavailable)?;
        if viewer.closed {
            return Err(ViewerRejection::AttachmentUnavailable);
        }
        let flight = viewer
            .flight
            .as_ref()
            .filter(|flight| flight.delivered)
            .ok_or(ViewerRejection::FeedbackMismatch)?;
        let needs_snapshot = match request.action {
            TerminalViewerAction::FrameApplied {
                sequence,
                generation,
                revision,
            } if sequence == flight.route.sequence
                && generation == flight.route.generation
                && revision == flight.route.revision =>
            {
                false
            }
            TerminalViewerAction::NeedSnapshot { sequence, .. }
                if sequence == flight.route.sequence =>
            {
                true
            }
            _ => return Err(ViewerRejection::FeedbackMismatch),
        };
        // Release before the Host exchange: its next event can precede the control reply.
        // Any failed or contradictory exchange retires the whole broker without replay.
        viewer.flight = None;
        viewer.needs_snapshot = needs_snapshot;
        Ok(())
    }

    pub(crate) fn receive(&mut self, record: Vec<u8>, epoch: HostEpoch) -> io::Result<()> {
        let envelope = decode_transport_envelope(&record).map_err(invalid)?;
        if envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.role != MessageRole::Event
            || envelope.correlation_id.is_some()
            || envelope.host_epoch != Some(epoch)
            || envelope.optional_message
        {
            return Err(invalid("unexpected Host event envelope"));
        }
        if matches!(envelope.kind, 41 | 42)
            && envelope.payload_encoding == PayloadEncoding::TerminalFrame
        {
            let route = inspect_terminal_frame_route(envelope.payload).map_err(invalid)?;
            if route.host_epoch != epoch || (envelope.kind == 41) != route.base_revision.is_none() {
                return Err(invalid("Host frame envelope and route disagree"));
            }
            let viewer = self
                .viewers
                .get_mut(route.attachment_id.as_bytes())
                .ok_or_else(|| invalid("Host event targets an unknown attachment"))?;
            if viewer.closed {
                return Ok(());
            }
            if viewer.record.is_some() || viewer.flight.is_some() {
                self.retained_record_bytes -= viewer.close();
                return Ok(());
            }
            if route.sequence.get() <= viewer.last_sequence
                || (viewer.needs_snapshot && route.base_revision.is_some())
                || viewer.blocked_revision.is_some_and(|revision| {
                    route.base_revision.is_some() || route.revision <= revision
                })
            {
                return Err(invalid("Host frame violates attachment projection flow"));
            }
            let Some(total) = self
                .retained_record_bytes
                .checked_add(record.len())
                .filter(|total| *total <= self.retained_record_limit)
            else {
                self.retained_record_bytes -= viewer.close();
                return Ok(());
            };
            self.retained_record_bytes = total;
            viewer.record = Some(record);
            viewer.last_sequence = route.sequence.get();
            viewer.blocked_revision = None;
            viewer.needs_snapshot = false;
            viewer.flight = Some(Flight {
                route,
                delivered: false,
            });
            return Ok(());
        }
        if envelope.payload_encoding != PayloadEncoding::ControlJson {
            return Err(invalid("unexpected Host event encoding"));
        }
        if envelope.kind == 53 {
            let HistoryControl::HistoryChanged(event) =
                decode_history_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(invalid)?
            else {
                return Err(invalid("unexpected history event"));
            };
            let viewer = self
                .viewers
                .get_mut(event.attachment_id.as_bytes())
                .ok_or_else(|| invalid("Host event targets an unknown attachment"))?;
            if viewer.closed {
                return Ok(());
            }
            // The event FIFO keeps the original record bytes. Queue-depth and
            // aggregate-byte saturation each close only this viewer; neither
            // drops the oldest event nor retires a neighbor.
            if viewer.history.len() == MAXIMUM_VIEWER_HISTORY_EVENTS {
                self.retained_record_bytes -= viewer.close();
                return Ok(());
            }
            let Some(total) = self
                .retained_record_bytes
                .checked_add(record.len())
                .filter(|total| *total <= self.retained_record_limit)
            else {
                self.retained_record_bytes -= viewer.close();
                return Ok(());
            };
            self.retained_record_bytes = total;
            viewer.history.push_back(record);
            return Ok(());
        }
        if envelope.kind == 49 {
            let ProjectionDemandControl::ProjectionBlocked(blocked) =
                decode_projection_demand_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(invalid)?
            else {
                return Err(invalid("unexpected projection-demand event"));
            };
            let viewer = self
                .viewers
                .get_mut(blocked.attachment_id.as_bytes())
                .ok_or_else(|| invalid("Host event targets an unknown attachment"))?;
            if viewer.closed {
                return Ok(());
            }
            if viewer.record.is_some() || viewer.flight.is_some() {
                self.retained_record_bytes -= viewer.close();
                return Ok(());
            }
            if viewer
                .blocked_revision
                .is_some_and(|revision| blocked.revision <= revision)
            {
                return Err(invalid("Host blocked revision did not advance"));
            }
            let Some(total) = self
                .retained_record_bytes
                .checked_add(record.len())
                .filter(|total| *total <= self.retained_record_limit)
            else {
                self.retained_record_bytes -= viewer.close();
                return Ok(());
            };
            self.retained_record_bytes = total;
            viewer.flight = None;
            viewer.record = Some(record);
            viewer.blocked_revision = Some(blocked.revision);
            viewer.needs_snapshot = true;
            return Ok(());
        }
        match decode_terminal_effect_event(envelope.kind, envelope.role, envelope.payload) {
            Ok(TerminalEffectEvent::Bell(event)) => {
                self.viewer(event.attachment_id)?;
            }
            Ok(TerminalEffectEvent::ClipboardWriteRequest(event)) => {
                self.viewer(event.attachment_id)?;
            }
            // ApplicationSignal records are intercepted in receive_event
            // before this borrow; any that reach here are unexpected.
            _ => return validate_event(&record, epoch),
        }
        Ok(())
    }

    fn viewer(&mut self, id: AttachmentId) -> io::Result<&mut Viewer> {
        self.viewers
            .get_mut(id.as_bytes())
            .ok_or_else(|| invalid("Host event targets an unknown attachment"))
    }
}

impl Viewer {
    /// Retires this viewer and returns every retained byte it still holds —
    /// the frame record slot plus all queued events — so the caller releases
    /// each byte exactly once.
    fn close(&mut self) -> usize {
        self.closed = true;
        self.flight = None;
        let frame = self.record.take().map_or(0, |record| record.len());
        let events = self
            .history
            .drain(..)
            .map(|event| event.len())
            .sum::<usize>();
        frame + events
    }
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
#[path = "viewers_tests.rs"]
mod tests;
