//! Owns one attachment's validated text projection, its retained Kitty graphics, lifecycle
//! observations, and exact generation or revision feedback decisions. It does not read transport
//! bytes, paint the frame, or treat projection state as Server authority.

use orchbee_client_server_protocol::{
    Geometry, TerminalViewerAction as Feedback, ViewerId, ViewerSnapshotReason as Reason,
};
use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, HistoryControl, HostEpoch, MessageRole,
    PayloadEncoding, ProjectionDemandControl, TerminalGraphics, TerminalTextFrame, TerminalTextRow,
    decode_history_control, decode_projection_demand_control, decode_terminal_text_frame,
    decode_transport_envelope, inspect_terminal_frame_route,
};
use std::{collections::HashSet, sync::Arc};

mod graphics;
mod rejection;
pub use rejection::ProjectionRejection;
use rejection::{ProjectionOperation as Operation, ProjectionStage as Stage};

const MAX_UNIQUE_TEXT_BYTES: usize = 36 * 1024 * 1024;

#[derive(Clone)]
pub struct TerminalProjection {
    end: Option<TerminalEnd>,
    attachment: AttachmentId,
    epoch: HostEpoch,
    host_terminal_id: Option<orchbee_client_server_protocol::HostTerminalId>,
    last_sequence: u64,
    last_revision: u64,
    blocked_revision: Option<u64>,
    frame: Option<Arc<TerminalTextFrame>>,
    row_raw_bytes: Vec<usize>,
    total_raw_bytes: usize,
    /// Taken out of each applied frame, so retained frame images never hold pixels.
    graphics: Option<Arc<TerminalGraphics>>,
}

pub(crate) use crate::localization::TerminalEnd;

/// What a refused record lets the Client do to resync its pane.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RejectedRecord {
    /// Nothing in the record is readable, so it may be a frame the Server still waits on.
    Unreadable,
    /// A program effect, which never touches the frame.
    Effect,
    /// A frame whose route names the Server's flight; this snapshot request answers it.
    Frame(Feedback),
    /// Any other record; the next frame brings the snapshot.
    Other,
}

impl TerminalProjection {
    pub const fn new(attachment: AttachmentId, epoch: HostEpoch) -> Self {
        Self {
            end: None,
            attachment,
            epoch,
            host_terminal_id: None,
            last_sequence: 0,
            last_revision: 0,
            blocked_revision: None,
            frame: None,
            row_raw_bytes: Vec::new(),
            total_raw_bytes: 0,
            graphics: None,
        }
    }
    pub(crate) const fn end(&self) -> Option<TerminalEnd> {
        self.end
    }

    /// Lifecycle records never invalidate the final frame or mint frame feedback.
    ///
    /// # Errors
    ///
    /// Returns the operation and stage that refused a malformed, misrouted, or contradictory
    /// lifecycle record; the projection is unchanged.
    pub fn receive_lifecycle(&mut self, record: &[u8]) -> Result<bool, ProjectionRejection> {
        use orchbee_terminal_protocol::{LifecycleControl, decode_lifecycle_control};
        let envelope = decode_transport_envelope(record).map_err(ProjectionRejection::decoding(
            Operation::Record,
            Stage::EnvelopeDecode,
        ))?;
        if !matches!(envelope.kind, 15 | 19) {
            return Ok(false);
        }
        if envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.role != MessageRole::Event
            || envelope.correlation_id.is_some()
            || envelope.host_epoch != Some(self.epoch)
            || envelope.optional_message
            || envelope.payload_encoding != PayloadEncoding::ControlJson
        {
            return Err(reject(Operation::Lifecycle, Stage::EnvelopeRoute));
        }
        let (identity, end) =
            match decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload).map_err(
                ProjectionRejection::decoding(Operation::Lifecycle, Stage::PayloadDecode),
            )? {
                LifecycleControl::TerminalExited(event) => {
                    (event.identity, TerminalEnd::Exited(event.exit))
                }
                LifecycleControl::TerminalFailed(event) => (event.identity, TerminalEnd::Failed),
                // This lifecycle exchange requires its exact target-bearing acknowledgment; other control records cannot confirm it.
                _ => return Err(reject(Operation::Lifecycle, Stage::UnexpectedMessage)),
            };
        if identity.host_epoch != self.epoch
            || self
                .host_terminal_id
                .as_ref()
                .is_none_or(|terminal| terminal.as_str() != identity.terminal_id.to_text())
        {
            return Err(reject(Operation::Lifecycle, Stage::Identity));
        }
        if self.end.is_some_and(|previous| previous != end) {
            return Err(reject(Operation::Lifecycle, Stage::ConflictingEnd));
        }
        self.end = Some(end);
        Ok(true)
    }
    pub fn frame(&self) -> Option<&TerminalTextFrame> {
        self.frame.as_deref()
    }
    /// The Host's complete current Kitty resource set and placements for the applied frame.
    pub(crate) fn graphics(&self) -> Option<&TerminalGraphics> {
        self.frame.as_ref().and(self.graphics.as_deref())
    }
    /// Share immutable pixels with a bounded presentation cache. This grants
    /// neither the attachment's feedback route nor permission to apply deltas.
    pub(crate) fn share_frame(&self) -> Option<Arc<TerminalTextFrame>> {
        self.frame.clone()
    }
    pub fn with_host_terminal(
        mut self,
        terminal: orchbee_client_server_protocol::HostTerminalId,
    ) -> Self {
        self.host_terminal_id = Some(terminal);
        self
    }
    pub(crate) fn host_terminal_id(&self) -> Option<&str> {
        self.host_terminal_id.as_ref().map(|id| id.as_str())
    }
    /// The attachment identity this projection is bound to — the route key
    /// history events and row windows validate against.
    pub const fn attachment(&self) -> &AttachmentId {
        &self.attachment
    }
    /// The host epoch this projection's attachment was minted under.
    pub const fn epoch(&self) -> &HostEpoch {
        &self.epoch
    }
    /// Hand the applied frame to a consumer that must outlive the
    /// projection — the CLI read path maps it after the session drops.
    pub fn take_frame(&mut self) -> Option<TerminalTextFrame> {
        self.row_raw_bytes.clear();
        self.total_raw_bytes = 0;
        self.frame.take().map(Arc::unwrap_or_clone)
    }

    pub(crate) fn clear_frame(&mut self) {
        self.frame = None;
        self.row_raw_bytes.clear();
        self.total_raw_bytes = 0;
        self.graphics = None;
    }
    #[cfg(test)]
    pub(crate) fn receive(
        &mut self,
        record: &[u8],
    ) -> Result<Option<Feedback>, ProjectionRejection> {
        self.receive_for_geometry(record, None)
    }

    /// Installs a frame directly — UI-level fixtures that exercise
    /// frame-dependent gates (history entry availability) without needing
    /// an OBRW encoder for the receive path.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn install_frame_for_test(&mut self, mut frame: TerminalTextFrame) {
        self.graphics = frame.graphics.take().map(Arc::new);
        let (row_raw_bytes, total_raw_bytes) = measure_row_bytes(&frame.rows);
        self.row_raw_bytes = row_raw_bytes;
        self.total_raw_bytes = total_raw_bytes;
        self.frame = Some(Arc::new(frame));
    }
    /// Applies one terminal record and mints its exact feedback.
    ///
    /// # Errors
    ///
    /// Returns the operation and stage that refused a malformed or misrouted record. A frame that
    /// decodes but cannot be applied is not an error: it clears the frame and requests a snapshot.
    pub fn receive_for_geometry(
        &mut self,
        record: &[u8],
        expected_geometry: Option<Geometry>,
    ) -> Result<Option<Feedback>, ProjectionRejection> {
        if self.receive_lifecycle(record)? {
            return Ok(None);
        }
        let envelope = decode_transport_envelope(record).map_err(ProjectionRejection::decoding(
            Operation::Record,
            Stage::EnvelopeDecode,
        ))?;
        if envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.role != MessageRole::Event
            || envelope.correlation_id.is_some()
            || envelope.host_epoch != Some(self.epoch)
            || envelope.optional_message
        {
            return Err(reject(Operation::Record, Stage::EnvelopeRoute));
        }
        if envelope.kind == 49 && envelope.payload_encoding == PayloadEncoding::ControlJson {
            let ProjectionDemandControl::ProjectionBlocked(blocked) =
                decode_projection_demand_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(ProjectionRejection::decoding(
                        Operation::ProjectionBlocked,
                        Stage::PayloadDecode,
                    ))?
            else {
                return Err(reject(
                    Operation::ProjectionBlocked,
                    Stage::UnexpectedMessage,
                ));
            };
            if blocked.attachment_id != self.attachment {
                return Err(reject(Operation::ProjectionBlocked, Stage::Identity));
            }
            if self
                .blocked_revision
                .is_some_and(|old| blocked.revision <= old)
            {
                return Err(reject(Operation::ProjectionBlocked, Stage::StaleRevision));
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
                decode_history_control(envelope.kind, envelope.role, envelope.payload).map_err(
                    ProjectionRejection::decoding(Operation::HistoryChanged, Stage::PayloadDecode),
                )?
            else {
                return Err(reject(Operation::HistoryChanged, Stage::UnexpectedMessage));
            };
            if event.attachment_id != self.attachment {
                return Err(reject(Operation::HistoryChanged, Stage::Identity));
            }
            return Ok(Some(Feedback::HistoryObserved {
                attachment_id: self.viewer_id(Operation::HistoryChanged)?,
                sequence: event.sequence,
                generation: event.generation,
                revision: event.revision,
            }));
        }
        if !matches!(envelope.kind, 41 | 42)
            || envelope.payload_encoding != PayloadEncoding::TerminalFrame
        {
            return Err(reject(Operation::Record, Stage::UnexpectedKind));
        }
        let route = inspect_terminal_frame_route(envelope.payload).map_err(
            ProjectionRejection::decoding(Operation::Frame, Stage::PayloadDecode),
        )?;
        if route.attachment_id != self.attachment || route.host_epoch != self.epoch {
            return Err(reject(Operation::Frame, Stage::Identity));
        }
        if (envelope.kind == 41) != route.base_revision.is_none()
            || route.sequence.get() <= self.last_sequence
        {
            return Err(reject(Operation::Frame, Stage::FrameRoute));
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
                    attachment_id: self.viewer_id(Operation::Frame)?,
                    sequence: route.sequence,
                    generation: route.generation,
                    revision: route.revision,
                }))
            }
            Err(reason) => {
                self.clear_frame();
                Ok(Some(Feedback::NeedSnapshot {
                    attachment_id: self.viewer_id(Operation::Frame)?,
                    sequence: route.sequence,
                    reason,
                }))
            }
        }
    }
    /// Classifies a record this projection or the pane's effect stream refused, and invalidates
    /// the frame unless the record is a program effect. Only a frame whose route is still
    /// readable can be answered, with a snapshot request naming that flight.
    pub(crate) fn resync_after_rejection(&mut self, record: &[u8]) -> RejectedRecord {
        let Ok(envelope) = decode_transport_envelope(record) else {
            return RejectedRecord::Unreadable;
        };
        if matches!(envelope.kind, 55 | 56) {
            return RejectedRecord::Effect;
        }
        self.clear_frame();
        if !matches!(envelope.kind, 41 | 42)
            || envelope.payload_encoding != PayloadEncoding::TerminalFrame
        {
            return RejectedRecord::Other;
        }
        let (Ok(route), Ok(attachment_id)) = (
            inspect_terminal_frame_route(envelope.payload),
            self.viewer_id(Operation::Frame),
        ) else {
            return RejectedRecord::Unreadable;
        };
        self.last_sequence = self.last_sequence.max(route.sequence.get());
        let reason = if route.attachment_id == self.attachment && route.host_epoch == self.epoch {
            Reason::DecoderRejected
        } else {
            Reason::IdentityMismatch
        };
        RejectedRecord::Frame(Feedback::NeedSnapshot {
            attachment_id,
            sequence: route.sequence,
            reason,
        })
    }
    fn viewer_id(&self, operation: Operation) -> Result<ViewerId, ProjectionRejection> {
        ViewerId::parse(&self.attachment.to_text())
            .map_err(|_| reject(operation, Stage::FeedbackIdentity))
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
            if self.row_raw_bytes.len() != previous.rows.len() {
                let (counts, total) = measure_row_bytes(&previous.rows);
                self.row_raw_bytes = counts;
                self.total_raw_bytes = total;
            }
            let row_count = previous.rows.len();
            let mut merged_refs: Vec<&TerminalTextRow> = previous.rows.iter().collect();
            let mut new_total_raw_bytes = self.total_raw_bytes;
            let mut delta_counts = Vec::with_capacity(next.rows.len());
            for row in &next.rows {
                let index = usize::from(row.index);
                let prev_slot = merged_refs.get_mut(index).ok_or(Reason::DecoderRejected)?;
                let old_bytes = if std::ptr::eq(*prev_slot, &previous.rows[index]) {
                    self.row_raw_bytes[index]
                } else {
                    row_text_bytes(prev_slot)
                };
                let new_bytes = row_text_bytes(row);
                new_total_raw_bytes = new_total_raw_bytes - old_bytes + new_bytes;
                *prev_slot = row;
                delta_counts.push((index, new_bytes));
            }
            if new_total_raw_bytes > MAX_UNIQUE_TEXT_BYTES {
                validate_unique_bytes(merged_refs.iter().copied())?;
            }
            if next.rows.len().saturating_mul(2) >= row_count {
                for pair in merged_refs.windows(2) {
                    let [first, second] = pair else {
                        return Err(Reason::DecoderRejected);
                    };
                    validate_row_pair(first, second)?;
                }
            } else {
                for &(index, _) in &delta_counts {
                    if let Some(above) = index.checked_sub(1) {
                        validate_row_pair(merged_refs[above], merged_refs[index])?;
                    }
                    if index + 1 < row_count {
                        validate_row_pair(merged_refs[index], merged_refs[index + 1])?;
                    }
                }
            }
            let resolved = graphics::resolve(self.graphics.as_deref(), next.graphics.take(), true)?;
            drop(merged_refs);
            for (index, new_bytes) in delta_counts {
                self.row_raw_bytes[index] = new_bytes;
            }
            self.total_raw_bytes = new_total_raw_bytes;
            let prev_arc = self.frame.take().expect("checked previous frame");
            let mut rows = match Arc::try_unwrap(prev_arc) {
                Ok(prev_frame) => {
                    next.graphics_present = next.graphics_present.or(prev_frame.graphics_present);
                    prev_frame.rows
                }
                Err(shared) => {
                    next.graphics_present = next.graphics_present.or(shared.graphics_present);
                    shared.rows.clone()
                }
            };
            for row in next.rows {
                let index = usize::from(row.index);
                rows[index] = row;
            }
            next.rows = rows;
            self.last_revision = next.route.revision;
            if let graphics::Resolved::Replaced(graphics) = resolved {
                self.graphics = graphics;
            }
            self.frame = Some(Arc::new(next));
            return Ok(());
        }
        let (row_raw_bytes, total_raw_bytes) = measure_row_bytes(&next.rows);
        if total_raw_bytes > MAX_UNIQUE_TEXT_BYTES {
            validate_unique_bytes(next.rows.iter())?;
        }
        for pair in next.rows.windows(2) {
            let [first, second] = pair else {
                return Err(Reason::DecoderRejected);
            };
            validate_row_pair(first, second)?;
        }
        let resolved = graphics::resolve(self.graphics.as_deref(), next.graphics.take(), false)?;
        self.row_raw_bytes = row_raw_bytes;
        self.total_raw_bytes = total_raw_bytes;
        self.last_revision = next.route.revision;
        if let graphics::Resolved::Replaced(graphics) = resolved {
            self.graphics = graphics;
        }
        self.frame = Some(Arc::new(next));
        Ok(())
    }
}
const fn reject(operation: Operation, stage: Stage) -> ProjectionRejection {
    ProjectionRejection::new(operation, stage)
}
fn row_text_bytes(row: &TerminalTextRow) -> usize {
    row.cells.iter().map(|cell| cell.text.len()).sum()
}
fn measure_row_bytes(rows: &[TerminalTextRow]) -> (Vec<usize>, usize) {
    let mut per_row = Vec::with_capacity(rows.len());
    let mut total = 0_usize;
    for row in rows {
        let bytes = row_text_bytes(row);
        total = total.saturating_add(bytes);
        per_row.push(bytes);
    }
    (per_row, total)
}
fn validate_unique_bytes<'a>(
    rows: impl Iterator<Item = &'a TerminalTextRow>,
) -> Result<(), Reason> {
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
    if text_bytes > MAX_UNIQUE_TEXT_BYTES {
        return Err(Reason::DecoderRejected);
    }
    Ok(())
}
fn validate_row_pair(first: &TerminalTextRow, second: &TerminalTextRow) -> Result<(), Reason> {
    if (first.flags & 1 != 0) != (second.flags & 2 != 0)
        || (first.cells.last().is_some_and(|c| c.width == 3)
            && !second.cells.first().is_some_and(|c| c.width == 1))
    {
        return Err(Reason::DecoderRejected);
    }
    Ok(())
}
#[cfg(test)]
mod tests;
