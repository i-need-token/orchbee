use std::{collections::BTreeMap, io};

use orchbee_capability_node_protocol::{
    OperationId, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
    ViewerRejection,
};
use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, HostEpoch, MessageRole, PayloadEncoding,
    ProjectionDemandControl, TerminalEffectEvent, TerminalFrameRoute,
    decode_projection_demand_control, decode_terminal_effect_event, decode_transport_envelope,
    inspect_terminal_frame_route,
};

use super::validate_event;

const MAXIMUM_VIEWERS: usize = 8;

#[derive(Default)]
pub(crate) struct HostViewers {
    viewers: BTreeMap<[u8; 16], Viewer>,
}

struct Viewer {
    terminal_id: orchbee_capability_node_protocol::HostTerminalId,
    record: Option<Vec<u8>>,
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

impl HostViewers {
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
        self.viewers.remove(&id.as_bytes());
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
        match viewer.record.take() {
            Some(record) => {
                if let Some(flight) = &mut viewer.flight {
                    flight.delivered = true;
                }
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
            let viewer = self.viewer(route.attachment_id)?;
            if viewer.closed {
                return Ok(());
            }
            if viewer.record.is_some() || viewer.flight.is_some() {
                viewer.close();
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
        if envelope.kind == 49 {
            let ProjectionDemandControl::ProjectionBlocked(blocked) =
                decode_projection_demand_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(invalid)?
            else {
                return Err(invalid("unexpected projection-demand event"));
            };
            let viewer = self.viewer(blocked.attachment_id)?;
            if viewer.closed {
                return Ok(());
            }
            if viewer.record.is_some() || viewer.flight.is_some() {
                viewer.close();
                return Ok(());
            }
            if viewer
                .blocked_revision
                .is_some_and(|revision| blocked.revision <= revision)
            {
                return Err(invalid("Host blocked revision did not advance"));
            }
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
            Ok(TerminalEffectEvent::ApplicationSignal(event))
                if event.identity.host_epoch == epoch => {}
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
    fn close(&mut self) {
        self.closed = true;
        self.record = None;
        self.flight = None;
    }
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
#[path = "viewers_tests.rs"]
mod tests;
