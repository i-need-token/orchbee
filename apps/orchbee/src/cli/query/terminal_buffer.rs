//! One existing-Session viewer owns buffer reads, total budgets and deterministic
//! release for public selection/copy/link queries and one explicit plugin activation;
//! it never acquires terminal input.
use super::{
    LocalClient, TerminalTextFrame, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, ViewerId, attach_viewer_cancellable_with_host, classify_rows_rejected,
    classify_viewer, classify_viewer_rejected, decode_history_window, obcs_request_id,
    poll_terminal_frame_cancellable, stream_snapshot, viewer_protocol_invalid, viewer_unavailable,
};
use crate::{
    cli::{
        Failure, FailureKind, Output, failure_output, output, results,
        source::{ArmedSignals, Finish, SignalWatch},
    },
    invocation::{OutputFormat, Target, TerminalTextPoint},
    localization::Locale,
};
use orchbee_client_server_protocol::{ViewerRowDetail, ViewerRowsRejectedReason};
use orchbee_terminal_protocol::{TerminalTextRowWindow, TerminalTextWindowRow};
use serde_json::{Value, json};
use std::{
    num::NonZeroU64,
    ops::Range,
    time::{Duration, Instant},
};

pub(super) struct ContentFence {
    pub(super) revision: Option<u64>,
    pub(super) generation: Option<u64>,
}
pub(super) struct Prepared {
    pub(super) result: Value,
    pub(super) text: String,
    pub(super) status: u8,
    pub(super) submitted: bool,
}
pub(super) struct BufferReader<'a> {
    snapshot: &'a orchbee_workbench_protocol::Snapshot,
    client: &'a mut LocalClient,
    viewer: &'a ViewerId,
    pub(super) frame: &'a TerminalTextFrame,
    pub(super) terminal: &'a results::TerminalRow,
    host_terminal: &'a orchbee_terminal_protocol::TerminalId,
    pub(super) total: u64,
    deadline: Instant,
    signals: &'a mut ArmedSignals,
    acknowledged: bool,
    sequence: u64,
    remaining_bytes: usize,
    remaining_glyphs: usize,
    effect_detach_attempted: &'a mut bool,
}
pub(super) fn limit() -> Failure {
    classify_rows_rejected(ViewerRowsRejectedReason::LimitExceeded, "read-history")
}
pub(super) fn run(
    target: &Target,
    fence: ContentFence,
    format: OutputFormat,
    locale: Locale,
    schema: &'static str,
    prepare: impl FnOnce(&mut BufferReader<'_>) -> Result<Prepared, Failure>,
) -> Output {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let mut result = execute(
        target,
        fence,
        format,
        schema,
        deadline,
        &mut signals,
        prepare,
    )
    .unwrap_or_else(|failure| failure_output(failure, Some(format), locale));
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled { finalization, .. }) => {
            result.status = 130;
            result.finalization = Some(finalization);
        }
        Err(_) => result.status = 1,
    }
    result
}
fn execute(
    target: &Target,
    fence: ContentFence,
    format: OutputFormat,
    schema: &'static str,
    deadline: Instant,
    signals: &mut ArmedSignals,
    prepare: impl FnOnce(&mut BufferReader<'_>) -> Result<Prepared, Failure>,
) -> Result<Output, Failure> {
    signals.check_before(deadline)?;
    let (mut client, snapshot) = stream_snapshot(deadline, signals.poll_fd())?;
    let source = results::snapshot_source(&snapshot);
    let (terminal, viewer, attachment, epoch, host_terminal, projection) =
        attach_viewer_cancellable_with_host(
            &mut client,
            deadline,
            &snapshot,
            target,
            signals.poll_fd(),
        )
        .map_err(|failure| failure.source(source.clone()))?;
    let mut effect_detach_attempted = false;
    let observed = (|| {
        let frame = poll_terminal_frame_cancellable(
            &mut client,
            deadline,
            &viewer,
            projection,
            signals.poll_fd(),
        )?;
        if fence
            .revision
            .is_some_and(|revision| revision != frame.route.revision)
        {
            return Err(classify_rows_rejected(
                ViewerRowsRejectedReason::StaleRevision,
                "read-history",
            ));
        }
        if fence
            .generation
            .is_some_and(|generation| generation != frame.route.generation.get())
        {
            return Err(classify_rows_rejected(
                ViewerRowsRejectedReason::StaleGeneration,
                "read-history",
            ));
        }
        if frame.route.attachment_id != attachment || frame.route.host_epoch != epoch {
            return Err(viewer_protocol_invalid());
        }
        let total = frame
            .scrollback_rows
            .checked_add(u64::from(frame.geometry.rows))
            .ok_or_else(viewer_protocol_invalid)?;
        let mut reader = BufferReader {
            snapshot: &snapshot,
            client: &mut client,
            viewer: &viewer,
            frame: &frame,
            terminal: &terminal,
            host_terminal: &host_terminal,
            total,
            deadline,
            signals,
            acknowledged: false,
            sequence: 0,
            remaining_bytes: output::LIMIT,
            remaining_glyphs: 1048576,
            effect_detach_attempted: &mut effect_detach_attempted,
        };
        let prepared = prepare(&mut reader)?;
        Ok((frame, prepared))
    })();
    // Read-only paths retain best-effort cleanup. A matched-link effect handoff
    // owns one explicit Detach attempt; uncertainty drops this connection and
    // must never be retried after that attempt.
    if !effect_detach_attempted && let Ok(id) = obcs_request_id() {
        let _ = client.submit_terminal_viewer_cancellable(
            TerminalViewerRequest::new(
                id,
                TerminalViewerAction::Detach {
                    attachment_id: viewer.clone(),
                },
            ),
            Some(deadline),
            signals.poll_fd(),
        );
    }
    drop(client);
    let (frame, mut prepared) = observed.map_err(|failure| failure.source(source.clone()))?;
    if !prepared.submitted {
        signals.check_before(deadline)?;
    }
    let result = prepared
        .result
        .as_object_mut()
        .ok_or_else(viewer_protocol_invalid)?;
    result.insert("terminal".into(), results::terminal_row_json(&terminal));
    result.insert(
        "contentRevision".into(),
        json!(frame.route.revision.to_string()),
    );
    result.insert(
        "contentGeneration".into(),
        json!(frame.route.generation.get().to_string()),
    );
    result.insert("hostEpoch".into(), json!(frame.route.host_epoch.to_text()));
    let stdout = match format {
        OutputFormat::Json => output::encode_document(
            &json!({"schema":schema,"version":1,"source":source,"result":prepared.result}),
        ),
        OutputFormat::Text => output::encode_text(&prepared.text),
    }
    .map_err(|_| Failure::new(FailureKind::OutputTooLarge).source(source))?;
    Ok(Output {
        status: prepared.status,
        stdout,
        stderr: String::new(),
        finalization: None,
    })
}
impl BufferReader<'_> {
    pub(super) fn activate_link(
        &mut self,
        url: Option<&str>,
        row: u64,
        col: u16,
        viewport: Range<u64>,
        explicit: bool,
    ) -> Result<crate::cli::plugins::links::Activation, Failure> {
        use crate::cli::plugins::links;
        let Some(url) = url else {
            return Ok(links::Activation::NoLink);
        };
        let Some(selection) = links::select(url, self.deadline, self.signals)? else {
            return Ok(links::Activation::NoHandler);
        };
        let preflight = links::preflight(url, selection)?;
        let pane_directory = self.link_directory()?;
        let (invocation, member) = links::observe(
            self.client,
            self.snapshot,
            self.terminal,
            pane_directory,
            preflight,
            self.deadline,
            self.signals,
        )?;
        let final_uri = self.hyperlink(row, col)?;
        let same_url = if explicit {
            final_uri.as_deref() == Some(url)
        } else if final_uri.is_none() {
            super::terminal_links::plain(
                self,
                TerminalTextPoint {
                    row: u32::try_from(row).map_err(|_| limit())?,
                    col,
                },
                viewport,
            )?
            .url
            .as_deref()
                == Some(url)
        } else {
            false
        };
        if !same_url {
            return Err(viewer_protocol_invalid());
        }
        links::fence(
            self.client,
            self.snapshot,
            &member,
            self.deadline,
            self.signals,
        )?;
        self.detach_before_effect()?;
        links::submit(
            self.client,
            self.snapshot,
            invocation,
            self.deadline,
            self.signals,
        )
    }

    fn detach_before_effect(&mut self) -> Result<(), Failure> {
        if *self.effect_detach_attempted {
            return Err(viewer_protocol_invalid());
        }
        *self.effect_detach_attempted = true;
        self.signals.check_before(self.deadline)?;
        let response = self
            .client
            .submit_terminal_viewer_cancellable(
                TerminalViewerRequest::new(
                    obcs_request_id()?,
                    TerminalViewerAction::Detach {
                        attachment_id: self.viewer.clone(),
                    },
                ),
                Some(self.deadline),
                self.signals.poll_fd(),
            )
            .map_err(classify_viewer)?;
        self.signals.check_before(self.deadline)?;
        match response.outcome() {
            TerminalViewerOutcome::Detached {} => Ok(()),
            TerminalViewerOutcome::Rejected { reason } => Err(classify_viewer_rejected(*reason)),
            TerminalViewerOutcome::Unavailable { .. } => Err(viewer_unavailable()),
            TerminalViewerOutcome::Attached { .. }
            | TerminalViewerOutcome::Empty { .. }
            | TerminalViewerOutcome::FeedbackAccepted { .. }
            | TerminalViewerOutcome::RowsRejected { .. }
            | TerminalViewerOutcome::Record(..) => Err(viewer_protocol_invalid()),
        }
    }

    fn link_directory(&mut self) -> Result<Option<String>, Failure> {
        if let Some(directory) = self
            .frame
            .current_directory
            .as_deref()
            .and_then(orchbee_local_runtime::windows_reported_directory)
        {
            return Ok(Some(directory));
        }
        self.signals.check_before(self.deadline)?;
        let response = self
            .client
            .submit_terminal_viewer_cancellable(
                TerminalViewerRequest::new(
                    obcs_request_id()?,
                    TerminalViewerAction::ReadProcessInfo {
                        attachment_id: self.viewer.clone(),
                    },
                ),
                Some(self.deadline),
                self.signals.poll_fd(),
            )
            .map_err(classify_viewer)?;
        self.signals.check_before(self.deadline)?;
        match response.outcome() {
            TerminalViewerOutcome::Record(record) => crate::terminal_content::process_directory(
                record,
                self.frame.route.attachment_id,
                self.frame.route.host_epoch,
                self.host_terminal,
            )
            .map_err(|()| viewer_protocol_invalid()),
            TerminalViewerOutcome::Rejected { reason } => Err(classify_viewer_rejected(*reason)),
            TerminalViewerOutcome::Unavailable {
                reason:
                    orchbee_client_server_protocol::ViewerUnavailableReason::TerminalRuntimeUnavailable,
            } => Ok(None),
            TerminalViewerOutcome::Unavailable { .. } => Err(viewer_unavailable()),
            // Action/result admission belongs to the protocol; this step consumes only its requested viewer record or acknowledgment.
            _ => Err(viewer_protocol_invalid()),
        }
    }

    pub(super) fn validate_point(&self, point: TerminalTextPoint) -> Result<(), Failure> {
        if u64::from(point.row) >= self.total || point.col >= self.frame.geometry.columns {
            Err(limit())
        } else {
            Ok(())
        }
    }
    pub(super) fn window(
        &mut self,
        start: u64,
        count: u16,
    ) -> Result<TerminalTextRowWindow, Failure> {
        let record = self.window_record(start, count, ViewerRowDetail::Text)?;
        decode_history_window(
            record,
            self.frame.route.attachment_id,
            self.frame.route.host_epoch,
            self.frame,
            start,
            u64::from(count),
            NonZeroU64::new(self.sequence).ok_or_else(viewer_protocol_invalid)?,
        )
    }
    pub(super) fn hyperlink(&mut self, row: u64, col: u16) -> Result<Option<String>, Failure> {
        let record = self.window_record(row, 1, ViewerRowDetail::Render)?;
        let envelope = orchbee_terminal_protocol::decode_transport_envelope(&record)
            .map_err(|_| viewer_protocol_invalid())?;
        if envelope.protocol != orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION
            || envelope.role != orchbee_terminal_protocol::MessageRole::Response
            || envelope.optional_message
            || envelope.payload_encoding != orchbee_terminal_protocol::PayloadEncoding::RowWindow
            || envelope.host_epoch != Some(self.frame.route.host_epoch)
        {
            return Err(viewer_protocol_invalid());
        }
        let window = orchbee_terminal_protocol::decode_terminal_render_row_window(envelope.payload)
            .map_err(|_| viewer_protocol_invalid())?;
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
            return Err(viewer_protocol_invalid());
        }
        let row = window
            .projection()
            .rows()
            .next()
            .ok_or_else(viewer_protocol_invalid)?;
        crate::terminal_links::explicit_url(row, col).map_err(|()| viewer_protocol_invalid())
    }
    fn window_record(
        &mut self,
        start: u64,
        count: u16,
        detail: ViewerRowDetail,
    ) -> Result<Vec<u8>, Failure> {
        self.signals.check_before(self.deadline)?;
        if count == 0
            || count > 4096
            || start >= self.total
            || u64::from(count) > self.total - start
        {
            return Err(limit());
        }
        if !self.acknowledged {
            let request = TerminalViewerRequest::new(
                obcs_request_id()?,
                TerminalViewerAction::FrameApplied {
                    attachment_id: self.viewer.clone(),
                    sequence: self.frame.route.sequence,
                    generation: self.frame.route.generation,
                    revision: self.frame.route.revision,
                },
            );
            match self
                .client
                .submit_terminal_viewer_cancellable(
                    request,
                    Some(self.deadline),
                    self.signals.poll_fd(),
                )
                .map_err(classify_viewer)?
                .outcome()
            {
                TerminalViewerOutcome::FeedbackAccepted {} => self.acknowledged = true,
                TerminalViewerOutcome::Rejected { reason } => {
                    return Err(classify_viewer_rejected(*reason));
                }
                TerminalViewerOutcome::Unavailable { .. } => return Err(viewer_unavailable()),
                TerminalViewerOutcome::Attached { .. }
                | TerminalViewerOutcome::Empty { .. }
                | TerminalViewerOutcome::Detached { .. }
                | TerminalViewerOutcome::RowsRejected { .. }
                | TerminalViewerOutcome::Record(..) => return Err(viewer_protocol_invalid()),
            }
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(viewer_protocol_invalid)?;
        let sequence = NonZeroU64::new(self.sequence).ok_or_else(viewer_protocol_invalid)?;
        let request = TerminalViewerRequest::new(
            obcs_request_id()?,
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
        );
        let response = self
            .client
            .submit_terminal_viewer_cancellable(
                request,
                Some(self.deadline),
                self.signals.poll_fd(),
            )
            .map_err(classify_viewer)?;
        match response.outcome() {
            TerminalViewerOutcome::Record(record) => {
                let payload = orchbee_terminal_protocol::decode_transport_envelope(record)
                    .map_err(|_| viewer_protocol_invalid())?
                    .payload;
                if payload.len() > self.remaining_bytes {
                    return Err(limit());
                }
                self.remaining_bytes -= payload.len();
                Ok(record.clone())
            }
            TerminalViewerOutcome::RowsRejected { reason } => {
                Err(classify_rows_rejected(*reason, "read-history"))
            }
            TerminalViewerOutcome::Rejected { reason } => Err(classify_viewer_rejected(*reason)),
            TerminalViewerOutcome::Unavailable { .. } => Err(viewer_unavailable()),
            TerminalViewerOutcome::Attached { .. }
            | TerminalViewerOutcome::Empty { .. }
            | TerminalViewerOutcome::FeedbackAccepted { .. }
            | TerminalViewerOutcome::Detached { .. } => Err(viewer_protocol_invalid()),
        }
    }
    pub(super) fn rows(
        &mut self,
        range: Range<u64>,
    ) -> Result<Vec<TerminalTextWindowRow>, Failure> {
        if range.start >= range.end
            || range.end > self.total
            || range.end - range.start > 65536
            || range.end > u64::from(u32::MAX) + 1
        {
            return Err(limit());
        }
        let chunk = (orchbee_terminal_protocol::MAXIMUM_FRAME_CELLS
            / usize::from(self.frame.geometry.columns))
        .min(4096) as u64;
        let mut rows: Vec<TerminalTextWindowRow> = Vec::new();
        let mut at = range.start;
        while at < range.end {
            let count = (range.end - at).min(chunk) as u16;
            let window = self.window(at, count)?;
            if let (Some(prior), Some(next)) = (rows.last(), window.rows.first())
                && prior.wrap != next.wrap_continuation
            {
                return Err(viewer_protocol_invalid());
            }
            for row in &window.rows {
                let glyphs = if row.identity_ascii {
                    row.text.len()
                } else {
                    row.mappings.len()
                };
                if glyphs > self.remaining_glyphs {
                    return Err(limit());
                }
                self.remaining_glyphs -= glyphs;
            }
            rows.extend(window.rows);
            at += u64::from(count);
        }
        Ok(rows)
    }
}
