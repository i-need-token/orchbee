//! Owns bounded viewer attachment state, retained event delivery, and exact feedback matching
//! for one Host connection. It routes validated records without decoding terminal cells,
//! rendering output, or authorizing product target exposure.

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
    PayloadEncoding, ProjectionDemandControl, SelectionControl, SelectionExportControl,
    TerminalEffectEvent, TerminalFrameRoute, decode_history_control,
    decode_projection_demand_control, decode_selection_control,
    decode_selection_export_chunk_route, decode_selection_export_control,
    decode_terminal_effect_event, decode_transport_envelope, inspect_terminal_frame_route,
};

use super::validate_event;

pub(super) const MAXIMUM_VIEWERS: usize = 24;
const MAXIMUM_VIEWER_EVENTS: usize = 4;
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
    lifecycle: Option<Vec<u8>>,
    events: VecDeque<Vec<u8>>,
    next_preference: PollSource,
    closed: bool,
    flight: Option<Flight>,
    last_sequence: u64,
    last_bell_sequence: u64,
    last_clipboard_write_sequence: u64,
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
    Event,
}

#[derive(Clone, Copy)]
enum EffectStream {
    Bell,
    ClipboardWrite,
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
                lifecycle: None,
                events: VecDeque::new(),
                next_preference: PollSource::Record,
                closed: false,
                flight: None,
                last_sequence: 0,
                last_bell_sequence: 0,
                last_clipboard_write_sequence: 0,
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
        if let Some(record) = viewer.lifecycle.take() {
            self.retained_record_bytes -= record.len();
            return TerminalViewerOutcome::Record(record);
        }
        // The preferred source wins when both are readable; a single readable
        // source is always served. Only a completed take flips the preference,
        // so neither frames nor queued events can starve the other. Serving
        // the event FIFO never touches the frame flight.
        let event_ready = !viewer.events.is_empty();
        let serve_event = match viewer.next_preference {
            PollSource::Event => event_ready,
            PollSource::Record => viewer.record.is_none() && event_ready,
        };
        if serve_event {
            let Some(event) = viewer.events.pop_front() else {
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
                viewer.next_preference = PollSource::Event;
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
            // The enclosing path selects a viewer feedback subset; other actions require their own exchanges.
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
        if envelope.kind == 65 && envelope.payload_encoding == PayloadEncoding::RawBytes {
            let route =
                decode_selection_export_chunk_route(envelope.kind, envelope.role, envelope.payload)
                    .map_err(invalid)?;
            return self.queue_event(route.attachment_id, record);
        }
        if envelope.payload_encoding != PayloadEncoding::ControlJson {
            return Err(invalid("unexpected Host event encoding"));
        }
        if matches!(envelope.kind, 15 | 19) {
            validate_event(&record, epoch)?;
            let event = orchbee_terminal_protocol::decode_lifecycle_control(
                envelope.kind,
                envelope.role,
                envelope.payload,
            )
            .map_err(invalid)?;
            let terminal = match event {
                orchbee_terminal_protocol::LifecycleControl::TerminalExited(event) => {
                    event.identity.terminal_id
                }
                orchbee_terminal_protocol::LifecycleControl::TerminalFailed(event) => {
                    event.identity.terminal_id
                }
                // This lifecycle exchange requires its exact target-bearing acknowledgment; other control records cannot confirm it.
                _ => return Err(invalid("unexpected lifecycle event")),
            };
            for viewer in self.viewers.values_mut().filter(|viewer| {
                !viewer.closed && viewer.terminal_id.as_bytes() == *terminal.as_bytes()
            }) {
                let previous = viewer.lifecycle.as_ref().map_or(0, Vec::len);
                let total = self.retained_record_bytes - previous;
                if record.len() > self.retained_record_limit.saturating_sub(total) {
                    self.retained_record_bytes -= viewer.close();
                    continue;
                }
                self.retained_record_bytes = total + record.len();
                viewer.lifecycle = Some(record.clone());
            }
            return Ok(());
        }
        if envelope.kind == 53 {
            let HistoryControl::HistoryChanged(event) =
                decode_history_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(invalid)?
            else {
                return Err(invalid("unexpected history event"));
            };
            return self.queue_event(event.attachment_id, record);
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
        if envelope.kind == 62 {
            let SelectionControl::ChangedEvent(changed) =
                decode_selection_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(invalid)?
            else {
                return Err(invalid("unexpected selection event"));
            };
            return self.queue_event(changed.attachment_id, record);
        }
        if envelope.kind == 67 {
            let SelectionExportControl::FinishedEvent(finished) =
                decode_selection_export_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(invalid)?
            else {
                return Err(invalid("unexpected selection-export event"));
            };
            return self.queue_event(finished.attachment_id, record);
        }
        match decode_terminal_effect_event(envelope.kind, envelope.role, envelope.payload) {
            Ok(TerminalEffectEvent::Bell(event)) => self.receive_effect(
                event.attachment_id,
                event.sequence.get(),
                EffectStream::Bell,
                record,
            ),
            Ok(TerminalEffectEvent::ClipboardWriteRequest(event)) => self.receive_effect(
                event.attachment_id,
                event.sequence.get(),
                EffectStream::ClipboardWrite,
                record,
            ),
            // ApplicationSignal records are intercepted in receive_event
            // before this borrow; any that reach here are unexpected.
            _ => validate_event(&record, epoch),
        }
    }

    fn viewer(&mut self, id: AttachmentId) -> io::Result<&mut Viewer> {
        self.viewers
            .get_mut(id.as_bytes())
            .ok_or_else(|| invalid("Host event targets an unknown attachment"))
    }

    fn receive_effect(
        &mut self,
        id: AttachmentId,
        sequence: u64,
        stream: EffectStream,
        record: Vec<u8>,
    ) -> io::Result<()> {
        {
            let viewer = self.viewer(id)?;
            if viewer.closed {
                return Ok(());
            }
            let last_sequence = match stream {
                EffectStream::Bell => &mut viewer.last_bell_sequence,
                EffectStream::ClipboardWrite => &mut viewer.last_clipboard_write_sequence,
            };
            if sequence <= *last_sequence {
                return Err(invalid("Host effect sequence did not advance"));
            }
            *last_sequence = sequence;
        }
        self.queue_event(id, record)
    }

    fn queue_event(&mut self, id: AttachmentId, record: Vec<u8>) -> io::Result<()> {
        let viewer = self
            .viewers
            .get_mut(id.as_bytes())
            .ok_or_else(|| invalid("Host event targets an unknown attachment"))?;
        if viewer.closed {
            return Ok(());
        }
        // The event FIFO keeps the original record bytes. Queue-depth and
        // aggregate-byte saturation each close only this viewer; neither
        // drops the oldest event nor retires a neighbor.
        if viewer.events.len() == MAXIMUM_VIEWER_EVENTS {
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
        viewer.events.push_back(record);
        Ok(())
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
            .events
            .drain(..)
            .map(|event| event.len())
            .sum::<usize>();
        frame + events + self.lifecycle.take().map_or(0, |record| record.len())
    }
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests;
