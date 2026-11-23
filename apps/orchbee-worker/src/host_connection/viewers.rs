//! Owns bounded viewer attachment state, retained event delivery, and exact feedback matching
//! for one Host connection. It routes validated records without decoding terminal cells,
//! rendering output, or authorizing product target exposure.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use orchbee_capability_node_protocol::{
    OperationId, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
    ViewerRejection,
};
use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, HistoryControl, HostEpoch, MessageRole,
    PayloadEncoding, ProjectionDemand, ProjectionDemandControl, SelectionChanged,
    SelectionChangedState, SelectionControl, SelectionExportControl, TerminalEffectEvent,
    TerminalFrameRoute, decode_history_control, decode_projection_demand_control,
    decode_selection_control, decode_selection_export_chunk_route, decode_selection_export_control,
    decode_terminal_effect_event, decode_transport_envelope, inspect_terminal_frame_route,
};

use super::validate_event;
use crate::diagnostic_events::Anomaly;

pub(super) const MAXIMUM_VIEWERS: usize = 24;
const MAXIMUM_VIEWER_EVENTS: usize = 4;
const MAXIMUM_RETAINED_VIEWER_RECORD_BYTES: usize = 128 * 1024 * 1024;

pub(crate) struct HostViewers {
    viewers: BTreeMap<[u8; 16], Viewer>,
    ready: BTreeSet<[u8; 16]>,
    retained_record_bytes: usize,
    retained_record_limit: usize,
}

impl Default for HostViewers {
    fn default() -> Self {
        Self {
            viewers: BTreeMap::new(),
            ready: BTreeSet::new(),
            retained_record_bytes: 0,
            retained_record_limit: MAXIMUM_RETAINED_VIEWER_RECORD_BYTES,
        }
    }
}

struct Viewer {
    terminal_id: orchbee_capability_node_protocol::HostTerminalId,
    /// The demand last accepted for this viewer, which a new controller grant starts from.
    demand: ProjectionDemand,
    record: Option<Vec<u8>>,
    lifecycle: Option<Vec<u8>>,
    events: VecDeque<QueuedEvent>,
    next_preference: PollSource,
    closed: bool,
    /// Closed because its terminal moved to this Host from another: its Client learns of it by Poll
    /// and attaches again, while its controller still relays input here, and no Host attachment of
    /// this connection is the viewer's.
    moved: bool,
    /// A hint went out and no Poll has answered it, which bounds hints to one per viewer.
    hinted: bool,
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

/// One retained event record, in arrival order.
struct QueuedEvent {
    record: Vec<u8>,
    /// A selection snapshot other than an eviction. Selection state is a level, so a newer
    /// snapshot for the same viewer makes this record obsolete.
    replaceable: bool,
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
                // Attaching activates the viewer before anything else can observe it.
                demand: ProjectionDemand::Active,
                record: None,
                lifecycle: None,
                events: VecDeque::new(),
                next_preference: PollSource::Record,
                closed: false,
                moved: false,
                hinted: false,
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

    /// The terminal of a viewer whose terminal moved here, which only its controller's input still
    /// reaches.
    pub(crate) fn moved_target(
        &self,
        id: OperationId,
    ) -> Option<orchbee_capability_node_protocol::HostTerminalId> {
        self.viewers
            .get(&id.as_bytes())
            .filter(|viewer| viewer.moved)
            .map(|viewer| viewer.terminal_id)
    }

    /// Whether `id` is a viewer whose terminal moved here.
    pub(crate) fn moved(&self, id: OperationId) -> bool {
        self.viewers
            .get(&id.as_bytes())
            .is_some_and(|viewer| viewer.moved)
    }

    /// Takes in the viewer `id`, whose terminal moved here as `terminal_id`, closed: its next Poll
    /// answers that it is gone, so its Client attaches again, and `demand` stays what its
    /// controller follows until then.
    #[cfg(any(windows, target_os = "macos"))]
    pub(crate) fn adopt_moved(
        &mut self,
        id: OperationId,
        terminal_id: orchbee_capability_node_protocol::HostTerminalId,
        demand: ProjectionDemand,
    ) -> Result<(), ViewerRejection> {
        let key = id.as_bytes();
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
                demand,
                record: None,
                lifecycle: None,
                events: VecDeque::new(),
                next_preference: PollSource::Record,
                closed: true,
                moved: true,
                hinted: false,
                flight: None,
                last_sequence: 0,
                last_bell_sequence: 0,
                last_clipboard_write_sequence: 0,
                needs_snapshot: true,
                blocked_revision: None,
            },
        );
        self.note_ready();
        Ok(())
    }

    pub(crate) fn demand(&self, id: OperationId) -> Option<ProjectionDemand> {
        self.viewers.get(&id.as_bytes()).map(|viewer| viewer.demand)
    }

    pub(crate) fn set_demand(&mut self, id: OperationId, demand: ProjectionDemand) {
        if let Some(viewer) = self.viewers.get_mut(&id.as_bytes()) {
            viewer.demand = demand;
            if demand == ProjectionDemand::Paused {
                // Host pause abandons its flight. Retire the matching relay state before
                // reactivation can deliver a new snapshot; event and lifecycle queues survive.
                self.retained_record_bytes -= viewer.record.take().map_or(0, |record| record.len());
                viewer.flight = None;
                viewer.needs_snapshot = true;
                viewer.blocked_revision = None;
            }
        }
    }

    pub(crate) fn remove(&mut self, id: OperationId) {
        if let Some(mut viewer) = self.viewers.remove(&id.as_bytes()) {
            self.retained_record_bytes -= viewer.close();
        }
    }

    /// Closes one viewer whose Host answer this Worker could not accept. Its Client learns of the
    /// close by Poll, and a later cleanup, after a request's own work, detaches it.
    pub(crate) fn close(&mut self, id: OperationId) {
        if let Some(viewer) = self.viewers.get_mut(&id.as_bytes()) {
            self.retained_record_bytes -= viewer.close();
        }
        self.note_ready();
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

    pub(crate) fn target_ids(
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
            .filter_map(|(id, viewer)| (viewer.closed && !viewer.moved).then_some(*id))
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

    /// Viewers the Server has not been told hold something to Poll. Each hint is only a prompt: a
    /// lost one costs the Client's fallback poll.
    pub(crate) fn take_ready(&mut self) -> Vec<OperationId> {
        std::mem::take(&mut self.ready)
            .into_iter()
            .filter(|id| self.viewers.contains_key(id))
            .filter_map(|id| OperationId::from_bytes(id).ok())
            .collect()
    }

    /// A closed viewer also owes its Client a Poll, which is how the Client learns of the close.
    fn note_ready(&mut self) {
        for (id, viewer) in &mut self.viewers {
            if !viewer.hinted
                && (viewer.closed
                    || viewer.record.is_some()
                    || viewer.lifecycle.is_some()
                    || !viewer.events.is_empty())
            {
                viewer.hinted = true;
                self.ready.insert(*id);
            }
        }
    }

    pub(crate) fn poll(&mut self, id: OperationId) -> TerminalViewerOutcome {
        let outcome = self.take_next(id);
        // The Poll answers the hint; anything it left behind owes another.
        if let Some(viewer) = self
            .viewers
            .get_mut(&id.as_bytes())
            .filter(|viewer| !viewer.closed)
        {
            viewer.hinted = false;
        }
        self.note_ready();
        outcome
    }

    fn take_next(&mut self, id: OperationId) -> TerminalViewerOutcome {
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
            self.retained_record_bytes -= event.record.len();
            viewer.next_preference = PollSource::Record;
            return TerminalViewerOutcome::Record(event.record);
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

    /// Routes one intact viewer event. An event this broker cannot use never ends the connection:
    /// one that breaks its envelope or payload contract is dropped, a flow or sequence break closes
    /// only the viewer it names, which a later cleanup detaches, and each is recorded.
    pub(crate) fn receive(&mut self, record: Vec<u8>, epoch: HostEpoch) {
        self.store(record, epoch);
        self.note_ready();
    }

    fn store(&mut self, record: Vec<u8>, epoch: HostEpoch) {
        let Ok(envelope) = decode_transport_envelope(&record) else {
            return anomaly(super::record_kind(&record), Anomaly::Contract);
        };
        if envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.role != MessageRole::Event
            || envelope.correlation_id.is_some()
            || envelope.host_epoch != Some(epoch)
            || envelope.optional_message
        {
            return anomaly(envelope.kind, Anomaly::Contract);
        }
        if matches!(envelope.kind, 41 | 42)
            && envelope.payload_encoding == PayloadEncoding::TerminalFrame
        {
            let Ok(route) = inspect_terminal_frame_route(envelope.payload) else {
                return anomaly(envelope.kind, Anomaly::Contract);
            };
            let Some(viewer) = self.viewers.get_mut(route.attachment_id.as_bytes()) else {
                unknown_attachment(&record);
                return;
            };
            if viewer.closed {
                return;
            }
            if viewer.record.is_some() || viewer.flight.is_some() {
                self.retained_record_bytes -= viewer.close();
                return;
            }
            if route.host_epoch != epoch || (envelope.kind == 41) != route.base_revision.is_none() {
                self.retained_record_bytes -= viewer.close();
                return anomaly(envelope.kind, Anomaly::Contract);
            }
            if route.sequence.get() <= viewer.last_sequence
                || (viewer.needs_snapshot && route.base_revision.is_some())
                || viewer.blocked_revision.is_some_and(|revision| {
                    route.base_revision.is_some() || route.revision <= revision
                })
            {
                self.retained_record_bytes -= viewer.close();
                return anomaly(envelope.kind, Anomaly::Flow);
            }
            let Some(total) = self
                .retained_record_bytes
                .checked_add(record.len())
                .filter(|total| *total <= self.retained_record_limit)
            else {
                self.retained_record_bytes -= viewer.close();
                return;
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
            return;
        }
        if envelope.kind == 65 && envelope.payload_encoding == PayloadEncoding::RawBytes {
            match decode_selection_export_chunk_route(
                envelope.kind,
                envelope.role,
                envelope.payload,
            ) {
                Ok(route) => self.queue_event(route.attachment_id, record, false),
                Err(_) => anomaly(envelope.kind, Anomaly::Contract),
            }
            return;
        }
        if envelope.payload_encoding != PayloadEncoding::ControlJson {
            return anomaly(envelope.kind, Anomaly::Contract);
        }
        if matches!(envelope.kind, 15 | 19) {
            let event = orchbee_terminal_protocol::decode_lifecycle_control(
                envelope.kind,
                envelope.role,
                envelope.payload,
            );
            let terminal = match event {
                Ok(orchbee_terminal_protocol::LifecycleControl::TerminalExited(event))
                    if validate_event(&record, epoch).is_ok() =>
                {
                    event.identity.terminal_id
                }
                Ok(orchbee_terminal_protocol::LifecycleControl::TerminalFailed(event))
                    if validate_event(&record, epoch).is_ok() =>
                {
                    event.identity.terminal_id
                }
                // Only an exit or a failure of a terminal of this epoch reaches its viewers.
                _ => return anomaly(envelope.kind, Anomaly::Contract),
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
            return;
        }
        if envelope.kind == 53 {
            match decode_history_control(envelope.kind, envelope.role, envelope.payload) {
                Ok(HistoryControl::HistoryChanged(event)) => {
                    self.queue_event(event.attachment_id, record, false);
                }
                _ => anomaly(envelope.kind, Anomaly::Contract),
            }
            return;
        }
        if envelope.kind == 49 {
            let Ok(ProjectionDemandControl::ProjectionBlocked(blocked)) =
                decode_projection_demand_control(envelope.kind, envelope.role, envelope.payload)
            else {
                return anomaly(envelope.kind, Anomaly::Contract);
            };
            let Some(viewer) = self.viewers.get_mut(blocked.attachment_id.as_bytes()) else {
                unknown_attachment(&record);
                return;
            };
            if viewer.closed {
                return;
            }
            if viewer.record.is_some() || viewer.flight.is_some() {
                self.retained_record_bytes -= viewer.close();
                return;
            }
            if viewer
                .blocked_revision
                .is_some_and(|revision| blocked.revision <= revision)
            {
                self.retained_record_bytes -= viewer.close();
                return anomaly(envelope.kind, Anomaly::Flow);
            }
            let Some(total) = self
                .retained_record_bytes
                .checked_add(record.len())
                .filter(|total| *total <= self.retained_record_limit)
            else {
                self.retained_record_bytes -= viewer.close();
                return;
            };
            self.retained_record_bytes = total;
            viewer.flight = None;
            viewer.record = Some(record);
            viewer.blocked_revision = Some(blocked.revision);
            viewer.needs_snapshot = true;
            return;
        }
        if envelope.kind == 62 {
            match decode_selection_control(envelope.kind, envelope.role, envelope.payload) {
                Ok(SelectionControl::ChangedEvent(changed)) => {
                    self.queue_selection_snapshot(changed, record);
                }
                _ => anomaly(envelope.kind, Anomaly::Contract),
            }
            return;
        }
        if envelope.kind == 67 {
            match decode_selection_export_control(envelope.kind, envelope.role, envelope.payload) {
                Ok(SelectionExportControl::FinishedEvent(finished)) => {
                    self.queue_event(finished.attachment_id, record, false);
                }
                _ => anomaly(envelope.kind, Anomaly::Contract),
            }
            return;
        }
        let kind = envelope.kind;
        match decode_terminal_effect_event(kind, envelope.role, envelope.payload) {
            Ok(TerminalEffectEvent::Bell(event)) => self.receive_effect(
                event.attachment_id,
                event.sequence.get(),
                EffectStream::Bell,
                record,
                kind,
            ),
            Ok(TerminalEffectEvent::ClipboardWriteRequest(event)) => self.receive_effect(
                event.attachment_id,
                event.sequence.get(),
                EffectStream::ClipboardWrite,
                record,
                kind,
            ),
            // ApplicationSignal records are intercepted in receive_event before this borrow; any
            // other record is a lifecycle or metadata notice this broker keeps no projection for.
            _ => {
                if validate_event(&record, epoch).is_err() {
                    anomaly(envelope.kind, Anomaly::Contract);
                }
            }
        }
    }

    fn receive_effect(
        &mut self,
        id: AttachmentId,
        sequence: u64,
        stream: EffectStream,
        record: Vec<u8>,
        kind: u16,
    ) {
        {
            let Some(viewer) = self.viewers.get_mut(id.as_bytes()) else {
                unknown_attachment(&record);
                return;
            };
            if viewer.closed {
                return;
            }
            let last_sequence = match stream {
                EffectStream::Bell => &mut viewer.last_bell_sequence,
                EffectStream::ClipboardWrite => &mut viewer.last_clipboard_write_sequence,
            };
            if sequence <= *last_sequence {
                self.retained_record_bytes -= viewer.close();
                return anomaly(kind, Anomaly::Flow);
            }
            *last_sequence = sequence;
        }
        self.queue_event(id, record, false);
    }

    /// The Host reports the selection again after every generation change, so a pane resized
    /// column by column emits a burst of snapshots that a busy Client cannot poll in step. Each
    /// snapshot replaces the queued snapshots of its viewer that it makes obsolete; an eviction
    /// stays queued because the Client must observe it. Only distinct events can saturate.
    fn queue_selection_snapshot(&mut self, changed: SelectionChanged, record: Vec<u8>) {
        let Some(viewer) = self.viewers.get_mut(changed.attachment_id.as_bytes()) else {
            unknown_attachment(&record);
            return;
        };
        let mut released = 0;
        viewer.events.retain(|event| {
            if event.replaceable {
                released += event.record.len();
            }
            !event.replaceable
        });
        self.retained_record_bytes -= released;
        let replaceable = changed.state != SelectionChangedState::Evicted;
        self.queue_event(changed.attachment_id, record, replaceable);
    }

    fn queue_event(&mut self, id: AttachmentId, record: Vec<u8>, replaceable: bool) {
        let Some(viewer) = self.viewers.get_mut(id.as_bytes()) else {
            unknown_attachment(&record);
            return;
        };
        if viewer.closed {
            return;
        }
        // The event FIFO keeps the original record bytes. Queue-depth and
        // aggregate-byte saturation each close only this viewer; neither
        // drops the oldest event nor retires a neighbor. Only a newer
        // selection snapshot removes an obsolete one before this check.
        if viewer.events.len() == MAXIMUM_VIEWER_EVENTS {
            self.retained_record_bytes -= viewer.close();
            return;
        }
        let Some(total) = self
            .retained_record_bytes
            .checked_add(record.len())
            .filter(|total| *total <= self.retained_record_limit)
        else {
            self.retained_record_bytes -= viewer.close();
            return;
        };
        self.retained_record_bytes = total;
        viewer.events.push_back(QueuedEvent {
            record,
            replaceable,
        });
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
            .map(|event| event.record.len())
            .sum::<usize>();
        frame + events + self.lifecycle.take().map_or(0, |record| record.len())
    }
}

/// An intact event for an attachment this Worker does not hold is dropped and recorded; events for
/// attachments it detached never reach here (the connection drops them first).
fn unknown_attachment(record: &[u8]) {
    anomaly(super::record_kind(record), Anomaly::UnknownAttachment);
}

fn anomaly(kind: u16, anomaly: Anomaly) {
    crate::diagnostic_events::host_unexpected(
        crate::diagnostic_events::Site::Viewer,
        kind,
        anomaly,
    );
}

#[cfg(test)]
mod tests;
