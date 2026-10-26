//! Tracks one viewer's in-flight projection route and admits only matching frame or snapshot
//! feedback and capturable revisions. It validates routing state without decoding terminal
//! cells or proving that a Client painted the frame.

use orchbee_client_server_protocol::TerminalViewerAction;
use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, HostEpoch, MessageRole, PayloadEncoding,
    ProjectionDemandControl, TerminalFrameRoute, decode_projection_demand_control,
    decode_transport_envelope, inspect_terminal_frame_route,
};

pub(super) struct ViewerProjection {
    pub(super) attachment_id: AttachmentId,
    pub(super) host_epoch: HostEpoch,
    flight: Option<TerminalFrameRoute>,
    last_sequence: u64,
    needs_snapshot: bool,
    blocked_revision: Option<u64>,
    content_version: Option<(std::num::NonZeroU64, u64)>,
}

impl ViewerProjection {
    pub(super) const fn new(attachment_id: AttachmentId, host_epoch: HostEpoch) -> Self {
        Self {
            attachment_id,
            host_epoch,
            flight: None,
            last_sequence: 0,
            needs_snapshot: true,
            blocked_revision: None,
            content_version: None,
        }
    }
    #[cfg(windows)]
    pub(super) fn capturable_version(&self, generation: u64, revision: u64) -> bool {
        !self.needs_snapshot
            && self
                .content_version
                .is_some_and(|version| version.0.get() == generation && version.1 == revision)
    }
    pub(super) fn accepts_feedback(&self, action: &TerminalViewerAction) -> bool {
        let Some(flight) = self.flight else {
            return false;
        };
        match action {
            TerminalViewerAction::FrameApplied {
                attachment_id,
                sequence,
                generation,
                revision,
            } => {
                attachment_id.as_str() == self.attachment_id.to_text()
                    && *sequence == flight.sequence
                    && *generation == flight.generation
                    && *revision == flight.revision
            }
            TerminalViewerAction::NeedSnapshot {
                attachment_id,
                sequence,
                ..
            } => {
                attachment_id.as_str() == self.attachment_id.to_text()
                    && *sequence == flight.sequence
            }
            // Only the selected viewer actions produce the specialized record or feedback checked here.
            _ => false,
        }
    }
    pub(super) fn complete_feedback(&mut self, action: &TerminalViewerAction) {
        self.flight = None;
        self.needs_snapshot = matches!(action, TerminalViewerAction::NeedSnapshot { .. });
        if self.needs_snapshot {
            self.content_version = None;
        }
    }
    pub(super) fn receive(&mut self, record: &[u8]) -> Result<(), ()> {
        let envelope = decode_transport_envelope(record).map_err(|_| ())?;
        if self.flight.is_some()
            || envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.role != MessageRole::Event
            || envelope.correlation_id.is_some()
            || envelope.host_epoch != Some(self.host_epoch)
            || envelope.optional_message
        {
            return Err(());
        }
        match (envelope.kind, envelope.payload_encoding) {
            (41 | 42, PayloadEncoding::TerminalFrame) => {
                let route = inspect_terminal_frame_route(envelope.payload).map_err(|_| ())?;
                if route.attachment_id != self.attachment_id
                    || route.host_epoch != self.host_epoch
                    || (envelope.kind == 41) != route.base_revision.is_none()
                    || route.sequence.get() <= self.last_sequence
                    || (self.needs_snapshot && route.base_revision.is_some())
                    || self.blocked_revision.is_some_and(|revision| {
                        route.base_revision.is_some() || route.revision <= revision
                    })
                {
                    return Err(());
                }
                self.last_sequence = route.sequence.get();
                self.needs_snapshot = false;
                self.blocked_revision = None;
                self.flight = Some(route);
                self.content_version = Some((route.generation, route.revision));
            }
            (49, PayloadEncoding::ControlJson) => {
                let ProjectionDemandControl::ProjectionBlocked(blocked) =
                    decode_projection_demand_control(
                        envelope.kind,
                        envelope.role,
                        envelope.payload,
                    )
                    .map_err(|_| ())?
                else {
                    return Err(());
                };
                if blocked.attachment_id != self.attachment_id
                    || self
                        .blocked_revision
                        .is_some_and(|revision| blocked.revision <= revision)
                {
                    return Err(());
                }
                self.blocked_revision = Some(blocked.revision);
                self.needs_snapshot = true;
                self.content_version = None;
            }
            // Projection routing accepts only admitted frame kinds with their required payload encodings.
            _ => return Err(()),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
