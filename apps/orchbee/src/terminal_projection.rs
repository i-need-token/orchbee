use orchbee_client_server_protocol::{
    Geometry, TerminalViewerAction as Feedback, ViewerId, ViewerSnapshotReason as Reason,
};
use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, HistoryControl, HostEpoch, MessageRole,
    PayloadEncoding, ProjectionDemandControl, TerminalTextFrame, TerminalTextRow,
    decode_history_control, decode_projection_demand_control, decode_terminal_text_frame,
    decode_transport_envelope, inspect_terminal_frame_route,
};
use std::{collections::HashSet, sync::Arc};

pub(crate) struct TerminalProjection {
    attachment: AttachmentId,
    epoch: HostEpoch,
    last_sequence: u64,
    last_revision: u64,
    blocked_revision: Option<u64>,
    frame: Option<TerminalTextFrame>,
}
impl TerminalProjection {
    pub(crate) const fn new(attachment: AttachmentId, epoch: HostEpoch) -> Self {
        Self {
            attachment,
            epoch,
            last_sequence: 0,
            last_revision: 0,
            blocked_revision: None,
            frame: None,
        }
    }
    pub(crate) const fn frame(&self) -> Option<&TerminalTextFrame> {
        self.frame.as_ref()
    }
    /// The attachment identity this projection is bound to — the route key
    /// history events and row windows validate against.
    pub(crate) const fn attachment(&self) -> &AttachmentId {
        &self.attachment
    }
    /// The host epoch this projection's attachment was minted under.
    pub(crate) const fn epoch(&self) -> &HostEpoch {
        &self.epoch
    }
    /// Hand the applied frame to a consumer that must outlive the
    /// projection — the CLI read path maps it after the session drops.
    pub(crate) fn take_frame(&mut self) -> Option<TerminalTextFrame> {
        self.frame.take()
    }
    #[cfg(test)]
    pub(crate) const fn blocked(&self) -> bool {
        self.frame.is_none()
    }
    pub(crate) fn clear_frame(&mut self) {
        self.frame = None;
    }
    #[cfg(test)]
    pub(crate) fn receive(&mut self, record: &[u8]) -> Result<Option<Feedback>, ()> {
        self.receive_for_geometry(record, None)
    }

    /// Installs a frame directly — UI-level fixtures that exercise
    /// frame-dependent gates (history entry availability) without needing
    /// an OBRW encoder for the receive path.
    #[cfg(test)]
    pub(crate) fn install_frame_for_test(&mut self, frame: TerminalTextFrame) {
        self.frame = Some(frame);
    }
    pub(crate) fn receive_for_geometry(
        &mut self,
        record: &[u8],
        expected_geometry: Option<Geometry>,
    ) -> Result<Option<Feedback>, ()> {
        let envelope = decode_transport_envelope(record).map_err(|_| ())?;
        if envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.role != MessageRole::Event
            || envelope.correlation_id.is_some()
            || envelope.host_epoch != Some(self.epoch)
            || envelope.optional_message
        {
            return Err(());
        }
        if envelope.kind == 49 && envelope.payload_encoding == PayloadEncoding::ControlJson {
            let ProjectionDemandControl::ProjectionBlocked(blocked) =
                decode_projection_demand_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(|_| ())?
            else {
                return Err(());
            };
            if blocked.attachment_id != self.attachment
                || self
                    .blocked_revision
                    .is_some_and(|old| blocked.revision <= old)
            {
                return Err(());
            }
            self.blocked_revision = Some(blocked.revision);
            self.clear_frame();
            return Ok(None);
        }
        // A HistoryChanged event is not a frame: it validates its own envelope
        // and exact attachment identity, leaves every projection field alone,
        // and answers with the event's own acknowledgement.
        if envelope.kind == 53 && envelope.payload_encoding == PayloadEncoding::ControlJson {
            let HistoryControl::HistoryChanged(event) =
                decode_history_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(|_| ())?
            else {
                return Err(());
            };
            if event.attachment_id != self.attachment {
                return Err(());
            }
            return Ok(Some(Feedback::HistoryObserved {
                attachment_id: ViewerId::parse(&self.attachment.to_text()).map_err(|_| ())?,
                sequence: event.sequence,
                generation: event.generation,
                revision: event.revision,
            }));
        }
        if !matches!(envelope.kind, 41 | 42)
            || envelope.payload_encoding != PayloadEncoding::TerminalFrame
        {
            return Err(());
        }
        let route = inspect_terminal_frame_route(envelope.payload).map_err(|_| ())?;
        if route.attachment_id != self.attachment
            || route.host_epoch != self.epoch
            || (envelope.kind == 41) != route.base_revision.is_none()
            || route.sequence.get() <= self.last_sequence
        {
            return Err(());
        }
        let previous_sequence = self.last_sequence;
        self.last_sequence = route.sequence.get();
        let result = decode_terminal_text_frame(envelope.payload)
            .map_err(|_| Reason::DecoderRejected)
            .and_then(|frame| {
                if self.blocked_revision.is_some_and(|revision| {
                    frame.route.revision <= revision || frame.route.base_revision.is_some()
                }) {
                    return Err(Reason::BaseMismatch);
                }
                if frame.route.base_revision.is_some()
                    && frame.route.sequence.get() != previous_sequence.saturating_add(1)
                {
                    return Err(Reason::SequenceGap);
                }
                if frame.route.base_revision.is_none()
                    && expected_geometry.is_some_and(|expected| {
                        frame.geometry.columns != expected.columns()
                            || frame.geometry.rows != expected.rows()
                            || frame.geometry.cell_pixel_width != expected.cell_pixel_width()
                            || frame.geometry.cell_pixel_height != expected.cell_pixel_height()
                    })
                {
                    return Err(Reason::DimensionsMismatch);
                }
                self.apply(frame)
            });
        match result {
            Ok(()) => {
                self.blocked_revision = None;
                Ok(Some(Feedback::FrameApplied {
                    attachment_id: ViewerId::parse(&self.attachment.to_text()).map_err(|_| ())?,
                    sequence: route.sequence,
                    generation: route.generation,
                    revision: route.revision,
                }))
            }
            Err(reason) => {
                self.clear_frame();
                Ok(Some(Feedback::NeedSnapshot {
                    attachment_id: ViewerId::parse(&self.attachment.to_text()).map_err(|_| ())?,
                    sequence: route.sequence,
                    reason,
                }))
            }
        }
    }
    fn apply(&mut self, mut next: TerminalTextFrame) -> Result<(), Reason> {
        if next.route.revision < self.last_revision {
            return Err(Reason::BaseMismatch);
        }
        if let Some(base) = next.route.base_revision {
            let previous = self.frame.as_ref().ok_or(Reason::StoreReset)?;
            if next.route.generation != previous.route.generation {
                return Err(Reason::GenerationMismatch);
            }
            if base != previous.route.revision
                || next.theme_revision != previous.theme_revision
                || next.alternate_screen != previous.alternate_screen
            {
                return Err(Reason::BaseMismatch);
            }
            if next.geometry != previous.geometry {
                return Err(Reason::DimensionsMismatch);
            }
            let mut rows = previous.rows.clone();
            for row in next.rows {
                let index = usize::from(row.index);
                *rows.get_mut(index).ok_or(Reason::DecoderRejected)? = row;
            }
            next.rows = rows;
        }
        validate_rows(&next.rows)?;
        self.last_revision = next.route.revision;
        self.frame = Some(next);
        Ok(())
    }
}
fn validate_rows(rows: &[TerminalTextRow]) -> Result<(), Reason> {
    let mut strings = HashSet::new();
    let mut text_bytes = 0_usize;
    for row in rows {
        for cell in &row.cells {
            // Count allocations, including equal text from distinct delta tables.
            if strings.insert(Arc::as_ptr(&cell.text)) {
                text_bytes = text_bytes.saturating_add(cell.text.len());
            }
        }
    }
    if text_bytes > 36 * 1024 * 1024 {
        return Err(Reason::DecoderRejected);
    }
    for pair in rows.windows(2) {
        let [first, second] = pair else {
            return Err(Reason::DecoderRejected);
        };
        if (first.flags & 1 != 0) != (second.flags & 2 != 0)
            || (first.cells.last().is_some_and(|c| c.width == 3)
                && !second.cells.first().is_some_and(|c| c.width == 1))
        {
            return Err(Reason::DecoderRejected);
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests;
