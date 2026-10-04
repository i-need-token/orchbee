//! One exact, bounded read-only viewer for a painted link source.

use std::{num::NonZeroU64, ops::Range, time::Instant};

use orchbee_client_server_protocol::{
    RequestId, SiteId, TerminalId, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, ViewerId, ViewerRowDetail, WorkerId,
};
use orchbee_terminal_protocol::{
    AttachmentId, HostEpoch, MessageRole, PayloadEncoding, TerminalTextFrame,
    TerminalTextRowWindow, TerminalTextWindowRow,
};
use orchbee_workbench_protocol::{PluginInvocationContext, Snapshot};

use crate::{
    local_client::LocalClient,
    terminal_buffer_text::TerminalTextPoint,
    terminal_link_activation::{Error, Guard, PaintedLinkSource, SnapshotMember, context},
    terminal_links::{self, LinkRead},
    terminal_projection::TerminalProjection,
};

const MAXIMUM_ROWS: usize = 65_536;
const MAXIMUM_GLYPHS: usize = 1_048_576;
const MAXIMUM_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Resolved {
    pub(crate) url: Option<String>,
    pub(crate) explicit: bool,
}

/// One temporary read-only attachment with cumulative row, glyph, and byte bounds.
pub(crate) struct ReadSession<'a, G: Guard> {
    client: &'a mut LocalClient,
    guard: &'a mut G,
    viewer: ViewerId,
    host_terminal: orchbee_terminal_protocol::TerminalId,
    frame: TerminalTextFrame,
    total: u64,
    deadline: Instant,
    detach_request: RequestId,
    acknowledged: bool,
    sequence: u64,
    remaining_bytes: usize,
    remaining_glyphs: usize,
    detach_result: Option<bool>,
}

impl<'a, G: Guard> ReadSession<'a, G> {
    /// Attach to the exact painted binding and reproduce its terminal content fence.
    ///
    /// # Errors
    ///
    /// Returns the typed gate, transport, protocol, identity, or source-drift failure without
    /// retaining an attachment for later use.
    pub(crate) fn open(
        client: &'a mut LocalClient,
        snapshot: &orchbee_workbench_protocol::Snapshot,
        source: &PaintedLinkSource,
        deadline: Instant,
        guard: &'a mut G,
    ) -> Result<Self, Error<G::Error>> {
        source
            .snapshot_member(snapshot)
            .ok_or(Error::SourceChanged)?;
        guard.check(deadline).map_err(Error::Gate)?;
        // Allocate both identities before Attach so every accepted viewer has
        // a deterministic best-effort Detach path even if entropy later fails.
        let attach_request = request_id()?;
        let detach_request = request_id()?;
        let request = TerminalViewerRequest::new(
            attach_request,
            TerminalViewerAction::AttachBinding {
                site_id: SiteId::parse(source.member.site.as_str())
                    .map_err(|_| Error::ProtocolInvalid)?,
                worker_id: WorkerId::parse(source.member.worker.as_str())
                    .map_err(|_| Error::ProtocolInvalid)?,
                terminal_id: TerminalId::parse(source.member.terminal.as_str())
                    .map_err(|_| Error::ProtocolInvalid)?,
            },
        );
        let response = client
            .submit_terminal_viewer_cancellable(request, Some(deadline), guard.wake())
            .map_err(|_| Error::TransportUnavailable)?;
        let (viewer, epoch_text, terminal_text) = match response.outcome() {
            TerminalViewerOutcome::Attached {
                attachment_id,
                host_epoch,
                host_terminal_id,
            } => (
                attachment_id.clone(),
                host_epoch.as_str().to_owned(),
                host_terminal_id.as_str().to_owned(),
            ),
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. } => {
                return Err(Error::TransportUnavailable);
            }
            _ => return Err(Error::ProtocolInvalid),
        };
        if let Err(error) = guard.check(deadline) {
            detach_viewer(client, guard, &viewer, &detach_request, deadline);
            return Err(Error::Gate(error));
        }
        let attachment = match AttachmentId::parse(viewer.as_str()) {
            Ok(attachment) => attachment,
            Err(_) => {
                detach_viewer(client, guard, &viewer, &detach_request, deadline);
                return Err(Error::ProtocolInvalid);
            }
        };
        let epoch = match HostEpoch::parse(&epoch_text) {
            Ok(epoch) => epoch,
            Err(_) => {
                detach_viewer(client, guard, &viewer, &detach_request, deadline);
                return Err(Error::ProtocolInvalid);
            }
        };
        let host_terminal = match orchbee_terminal_protocol::TerminalId::parse(&terminal_text) {
            Ok(terminal) => terminal,
            Err(_) => {
                detach_viewer(client, guard, &viewer, &detach_request, deadline);
                return Err(Error::ProtocolInvalid);
            }
        };
        let frame = match poll_frame(client, guard, &viewer, attachment, epoch, deadline) {
            Ok(frame) => frame,
            Err(error) => {
                detach_viewer(client, guard, &viewer, &detach_request, deadline);
                return Err(error);
            }
        };
        let Some(total) = frame
            .scrollback_rows
            .checked_add(u64::from(frame.geometry.rows))
        else {
            detach_viewer(client, guard, &viewer, &detach_request, deadline);
            return Err(Error::ProtocolInvalid);
        };
        if frame.geometry.columns == 0
            || frame.route.host_epoch != source.rows.host_epoch
            || frame.route.generation != source.rows.generation
            || frame.route.revision != source.rows.revision
            || frame.geometry.columns != source.rows.columns
            || total != source.rows.total_rows
        {
            detach_viewer(client, guard, &viewer, &detach_request, deadline);
            return Err(Error::SourceChanged);
        }
        Ok(Self {
            client,
            guard,
            viewer,
            host_terminal,
            frame,
            total,
            deadline,
            detach_request,
            acknowledged: false,
            sequence: 0,
            remaining_bytes: MAXIMUM_BYTES,
            remaining_glyphs: MAXIMUM_GLYPHS,
            detach_result: None,
        })
    }

    /// Resolve OSC 8 metadata first, then a bounded soft-wrapped plain URL at the exact cell.
    ///
    /// # Errors
    ///
    /// Returns a typed bound, gate, protocol, transport, or content-drift failure. No external
    /// effect occurs in this reader.
    pub(crate) fn resolve(
        &mut self,
        column: u16,
        row: u64,
        viewport: Range<u64>,
    ) -> Result<Resolved, Error<G::Error>> {
        if row >= self.total || column >= self.frame.geometry.columns {
            return Err(Error::LimitExceeded);
        }
        if let Some(url) = self.hyperlink(row, column)? {
            return Ok(Resolved {
                url: Some(url),
                explicit: true,
            });
        }
        let cursor = TerminalTextPoint {
            row: u32::try_from(row).map_err(|_| Error::LimitExceeded)?,
            col: column,
        };
        let mut start = row;
        let mut end = row.saturating_add(2).min(self.total);
        let mut rows = self.rows(start..end)?;
        loop {
            match terminal_links::resolve(
                &rows,
                self.frame.geometry.columns,
                cursor,
                self.total,
                viewport.clone(),
            ) {
                LinkRead::Complete(link) => {
                    return Ok(Resolved {
                        url: link.url,
                        explicit: false,
                    });
                }
                LinkRead::LimitExceeded => return Err(Error::LimitExceeded),
                LinkRead::NeedContext { before, after } => {
                    let remaining = MAXIMUM_ROWS.saturating_sub(rows.len());
                    if remaining == 0 {
                        return Err(Error::LimitExceeded);
                    }
                    let extension = rows.len().max(64).min(remaining) as u64;
                    if before {
                        let next = start.saturating_sub(extension);
                        let mut incoming = self.rows(next..start)?;
                        validate_join(&incoming, &rows)?;
                        incoming.extend(rows);
                        rows = incoming;
                        start = next;
                    }
                    if after {
                        let remaining = MAXIMUM_ROWS.saturating_sub(rows.len()) as u64;
                        if remaining == 0 {
                            return Err(Error::LimitExceeded);
                        }
                        let next = end.saturating_add(extension.min(remaining)).min(self.total);
                        let incoming = self.rows(end..next)?;
                        validate_join(&rows, &incoming)?;
                        rows.extend(incoming);
                        end = next;
                    }
                }
            }
        }
    }

    /// Observe the invocation context while this exact viewer remains
    /// attached. The caller must perform its final link read before detach so
    /// both the context and URL share the same attachment/content fence.
    pub(crate) fn observe_context(
        &mut self,
        snapshot: &Snapshot,
        member: &SnapshotMember,
        provided: PluginInvocationContext,
    ) -> Result<PluginInvocationContext, Error<G::Error>> {
        let pane_directory = self.pane_directory()?;
        context::observe(
            self.client,
            snapshot,
            member,
            pane_directory,
            provided,
            self.deadline,
            self.guard,
        )
    }

    /// Submit the one pre-allocated detach and report whether the peer
    /// acknowledged disposal. An uncertain detach is never retried; the
    /// owning lane drops the connection and refuses any later effect.
    pub(crate) fn detach(&mut self) -> bool {
        if let Some(detached) = self.detach_result {
            return detached;
        }
        let detached = detach_viewer(
            self.client,
            self.guard,
            &self.viewer,
            &self.detach_request,
            self.deadline,
        );
        self.detach_result = Some(detached);
        detached
    }

    fn pane_directory(&mut self) -> Result<Option<String>, Error<G::Error>> {
        if let Some(directory) = self
            .frame
            .current_directory
            .as_deref()
            .and_then(orchbee_local_site_runtime::windows_reported_directory)
        {
            return Ok(Some(directory));
        }
        self.guard.check(self.deadline).map_err(Error::Gate)?;
        self.acknowledge_frame()?;
        let response = self
            .client
            .submit_terminal_viewer_cancellable(
                TerminalViewerRequest::new(
                    request_id()?,
                    TerminalViewerAction::ReadProcessInfo {
                        attachment_id: self.viewer.clone(),
                    },
                ),
                Some(self.deadline),
                self.guard.wake(),
            )
            .map_err(|_| Error::TransportUnavailable)?;
        self.guard.check(self.deadline).map_err(Error::Gate)?;
        match response.outcome() {
            TerminalViewerOutcome::Record(record) => context::process_directory(
                record,
                self.frame.route.attachment_id,
                self.frame.route.host_epoch,
                &self.host_terminal,
            )
            .map_err(|()| Error::ProtocolInvalid),
            TerminalViewerOutcome::Rejected { .. } => Err(Error::SourceChanged),
            TerminalViewerOutcome::Unavailable {
                reason:
                    orchbee_client_server_protocol::ViewerUnavailableReason::TerminalRuntimeUnavailable,
            } => Ok(None),
            TerminalViewerOutcome::Unavailable { .. } => Err(Error::TransportUnavailable),
            _ => Err(Error::ProtocolInvalid),
        }
    }

    fn hyperlink(&mut self, row: u64, column: u16) -> Result<Option<String>, Error<G::Error>> {
        let record = self.window_record(row, 1, ViewerRowDetail::Render)?;
        let envelope = orchbee_terminal_protocol::decode_transport_envelope(&record)
            .map_err(|_| Error::ProtocolInvalid)?;
        if envelope.protocol != orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION
            || envelope.role != MessageRole::Response
            || envelope.optional_message
            || envelope.payload_encoding != PayloadEncoding::RowWindow
            || envelope.host_epoch != Some(self.frame.route.host_epoch)
        {
            return Err(Error::ProtocolInvalid);
        }
        let window = orchbee_terminal_protocol::decode_terminal_render_row_window(envelope.payload)
            .map_err(|_| Error::ProtocolInvalid)?;
        let route = window.route();
        if route.attachment_id != self.frame.route.attachment_id
            || route.host_epoch != self.frame.route.host_epoch
            || route.sequence.get() != self.sequence
            || route.revision != self.frame.route.revision
            || route.generation != self.frame.route.generation
            || route.total_rows != self.total
            || route.start_row != row
            || route.row_count != 1
            || route.columns != self.frame.geometry.columns
        {
            return Err(Error::SourceChanged);
        }
        let row = window
            .projection()
            .rows()
            .next()
            .ok_or(Error::ProtocolInvalid)?;
        terminal_links::explicit_url(row, column).map_err(|()| Error::ProtocolInvalid)
    }

    fn rows(&mut self, range: Range<u64>) -> Result<Vec<TerminalTextWindowRow>, Error<G::Error>> {
        if range.start >= range.end
            || range.end > self.total
            || range.end - range.start > MAXIMUM_ROWS as u64
            || range.end > u64::from(u32::MAX) + 1
        {
            return Err(Error::LimitExceeded);
        }
        let chunk = (orchbee_terminal_protocol::MAXIMUM_FRAME_CELLS
            / usize::from(self.frame.geometry.columns))
        .min(4096) as u64;
        let mut rows: Vec<TerminalTextWindowRow> = Vec::new();
        let mut at = range.start;
        while at < range.end {
            let count = (range.end - at).min(chunk) as u16;
            let window = self.text_window(at, count)?;
            validate_join(&rows, &window.rows)?;
            for row in &window.rows {
                let glyphs = if row.identity_ascii {
                    row.text.len()
                } else {
                    row.mappings.len()
                };
                if glyphs > self.remaining_glyphs {
                    return Err(Error::LimitExceeded);
                }
                self.remaining_glyphs -= glyphs;
            }
            rows.extend(window.rows);
            at += u64::from(count);
        }
        Ok(rows)
    }

    fn text_window(
        &mut self,
        start: u64,
        count: u16,
    ) -> Result<TerminalTextRowWindow, Error<G::Error>> {
        let record = self.window_record(start, count, ViewerRowDetail::Text)?;
        let envelope = orchbee_terminal_protocol::decode_transport_envelope(&record)
            .map_err(|_| Error::ProtocolInvalid)?;
        if envelope.protocol != orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION
            || envelope.role != MessageRole::Response
            || envelope.optional_message
            || envelope.payload_encoding != PayloadEncoding::RowWindow
            || envelope.host_epoch != Some(self.frame.route.host_epoch)
        {
            return Err(Error::ProtocolInvalid);
        }
        let window = orchbee_terminal_protocol::decode_terminal_text_row_window(envelope.payload)
            .map_err(|_| Error::ProtocolInvalid)?;
        if window.attachment_id != self.frame.route.attachment_id
            || window.host_epoch != self.frame.route.host_epoch
            || window.sequence.get() != self.sequence
            || window.revision != self.frame.route.revision
            || window.generation != self.frame.route.generation
            || window.total_rows != self.total
            || window.start_row != start
            || window.rows.len() != usize::from(count)
            || window.columns != self.frame.geometry.columns
        {
            return Err(Error::SourceChanged);
        }
        Ok(window)
    }

    fn window_record(
        &mut self,
        start: u64,
        count: u16,
        detail: ViewerRowDetail,
    ) -> Result<Vec<u8>, Error<G::Error>> {
        self.guard.check(self.deadline).map_err(Error::Gate)?;
        if count == 0
            || count > 4096
            || start >= self.total
            || u64::from(count) > self.total - start
        {
            return Err(Error::LimitExceeded);
        }
        self.acknowledge_frame()?;
        self.sequence = self.sequence.checked_add(1).ok_or(Error::ProtocolInvalid)?;
        let sequence = NonZeroU64::new(self.sequence).ok_or(Error::ProtocolInvalid)?;
        let response = self
            .client
            .submit_terminal_viewer_cancellable(
                TerminalViewerRequest::new(
                    request_id()?,
                    TerminalViewerAction::ReadRows {
                        detail,
                        attachment_id: self.viewer.clone(),
                        start_row: start,
                        row_count: count,
                        revision: self.frame.route.revision,
                        generation: self.frame.route.generation,
                        request_sequence: sequence,
                        maximum_bytes: orchbee_client_server_protocol::MAXIMUM_READ_ROWS_BYTES,
                    },
                ),
                Some(self.deadline),
                self.guard.wake(),
            )
            .map_err(|_| Error::TransportUnavailable)?;
        self.guard.check(self.deadline).map_err(Error::Gate)?;
        match response.outcome() {
            TerminalViewerOutcome::Record(record) => {
                let payload = orchbee_terminal_protocol::decode_transport_envelope(record)
                    .map_err(|_| Error::ProtocolInvalid)?
                    .payload;
                if payload.len() > self.remaining_bytes {
                    return Err(Error::LimitExceeded);
                }
                self.remaining_bytes -= payload.len();
                Ok(record.clone())
            }
            TerminalViewerOutcome::RowsRejected { .. }
            | TerminalViewerOutcome::Rejected { .. }
            | TerminalViewerOutcome::Unavailable { .. } => Err(Error::SourceChanged),
            _ => Err(Error::ProtocolInvalid),
        }
    }

    fn acknowledge_frame(&mut self) -> Result<(), Error<G::Error>> {
        if self.acknowledged {
            return Ok(());
        }
        let response = self
            .client
            .submit_terminal_viewer_cancellable(
                TerminalViewerRequest::new(
                    request_id()?,
                    TerminalViewerAction::FrameApplied {
                        attachment_id: self.viewer.clone(),
                        sequence: self.frame.route.sequence,
                        generation: self.frame.route.generation,
                        revision: self.frame.route.revision,
                    },
                ),
                Some(self.deadline),
                self.guard.wake(),
            )
            .map_err(|_| Error::TransportUnavailable)?;
        self.guard.check(self.deadline).map_err(Error::Gate)?;
        match response.outcome() {
            TerminalViewerOutcome::FeedbackAccepted {} => {
                self.acknowledged = true;
                Ok(())
            }
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. } => {
                Err(Error::SourceChanged)
            }
            _ => Err(Error::ProtocolInvalid),
        }
    }
}

impl<G: Guard> Drop for ReadSession<'_, G> {
    fn drop(&mut self) {
        let _detached = self.detach();
    }
}

fn poll_frame<G: Guard>(
    client: &mut LocalClient,
    guard: &mut G,
    viewer: &ViewerId,
    attachment: AttachmentId,
    epoch: HostEpoch,
    deadline: Instant,
) -> Result<TerminalTextFrame, Error<G::Error>> {
    let mut projection = TerminalProjection::new(attachment, epoch);
    loop {
        guard.check(deadline).map_err(Error::Gate)?;
        let response = client
            .submit_terminal_viewer_cancellable(
                TerminalViewerRequest::new(
                    request_id()?,
                    TerminalViewerAction::Poll {
                        attachment_id: viewer.clone(),
                    },
                ),
                Some(deadline),
                guard.wake(),
            )
            .map_err(|_| Error::TransportUnavailable)?;
        guard.check(deadline).map_err(Error::Gate)?;
        match response.outcome() {
            TerminalViewerOutcome::Empty {} => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            TerminalViewerOutcome::Record(record) => {
                match projection.receive_for_geometry(record, None) {
                    Ok(Some(feedback)) => {
                        if matches!(&feedback, TerminalViewerAction::FrameApplied { .. }) {
                            return projection.take_frame().ok_or(Error::ProtocolInvalid);
                        }
                        let response = client
                            .submit_terminal_viewer_cancellable(
                                TerminalViewerRequest::new(request_id()?, feedback),
                                Some(deadline),
                                guard.wake(),
                            )
                            .map_err(|_| Error::TransportUnavailable)?;
                        guard.check(deadline).map_err(Error::Gate)?;
                        if !matches!(
                            response.outcome(),
                            TerminalViewerOutcome::FeedbackAccepted {}
                        ) {
                            return Err(Error::ProtocolInvalid);
                        }
                    }
                    Ok(None) => {}
                    Err(()) => return Err(Error::ProtocolInvalid),
                }
            }
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. } => {
                return Err(Error::TransportUnavailable);
            }
            _ => return Err(Error::ProtocolInvalid),
        }
    }
}

fn detach_viewer<G: Guard>(
    client: &mut LocalClient,
    guard: &G,
    viewer: &ViewerId,
    request: &RequestId,
    deadline: Instant,
) -> bool {
    client
        .submit_terminal_viewer_cancellable(
            TerminalViewerRequest::new(
                request.clone(),
                TerminalViewerAction::Detach {
                    attachment_id: viewer.clone(),
                },
            ),
            Some(deadline),
            guard.wake(),
        )
        .is_ok_and(|response| matches!(response.outcome(), TerminalViewerOutcome::Detached {}))
}

fn validate_join<E>(
    before: &[TerminalTextWindowRow],
    after: &[TerminalTextWindowRow],
) -> Result<(), Error<E>> {
    if let (Some(before), Some(after)) = (before.last(), after.first())
        && before.wrap != after.wrap_continuation
    {
        return Err(Error::ProtocolInvalid);
    }
    Ok(())
}

fn request_id<E>() -> Result<RequestId, Error<E>> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::IdentityUnavailable)?;
    let mut text = String::with_capacity(32);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(text, "{byte:02x}");
    }
    RequestId::parse(&text).map_err(|_| Error::IdentityUnavailable)
}
