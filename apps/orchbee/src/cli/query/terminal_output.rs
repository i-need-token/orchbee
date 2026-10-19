//! One source-selected read on the existing read-only viewer lifecycle.

use super::{
    Failure, FailureKind, Instant, LocalClient, Projection, TerminalTextFrame,
    TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, ViewerId, WaitRun,
    WaitSample, acknowledge_frame, aged_pin, agent_terminal, attach_viewer, classify_rows_rejected,
    classify_viewer, classify_viewer_rejected, decode_history_window, detach_viewer,
    obcs_request_id, read_settled, snapshot_session, viewer_protocol_invalid, viewer_unavailable,
};
use crate::cli::terminal_output::{self, SampleRange};
use crate::invocation::{OutputSelection, OutputSource, OutputTarget};
use crate::terminal_projection::TerminalProjection;
use orchbee_client_server_protocol::ViewerRowsRejectedReason;
use std::num::NonZeroU64;

enum Sample {
    Text(Vec<terminal_output::OutputRow>),
    Render(Vec<u8>, NonZeroU64),
    Visible,
}

pub(crate) fn read_output(
    target: &OutputTarget,
    selection: OutputSelection,
    ansi: bool,
) -> Result<(serde_json::Value, Projection), Failure> {
    let (mut client, deadline, snapshot) = snapshot_session()?;
    let source = super::super::results::snapshot_source(&snapshot);
    let target = match target {
        OutputTarget::Terminal(target) => target.clone(),
        OutputTarget::Agent(target) => {
            agent_terminal::agent_terminal_target(&mut client, &snapshot, target, deadline)?
        }
    };
    let outcome = (|| {
        let (terminal, viewer, _attachment, _epoch, mut projection) =
            attach_viewer(&mut client, deadline, &snapshot, &target)?;
        let observed = observe(
            &mut client,
            deadline,
            &viewer,
            &mut projection,
            selection,
            ansi,
        );
        detach_viewer(&mut client, deadline, viewer);
        observed.map(|(frame, range, rows)| (terminal, frame, range, rows))
    })();
    drop(client);
    outcome
        .and_then(|(terminal, frame, range, rows)| {
            let mut doc = match rows {
                Sample::Text(rows) if ansi && selection.source == OutputSource::Detection => {
                    terminal_output::prepared_document(
                        terminal,
                        &frame,
                        selection,
                        range,
                        terminal_output::ansi::detection(rows, selection)?,
                    )
                }
                Sample::Text(rows) => {
                    terminal_output::document(terminal, &frame, selection, range, rows)?
                }
                Sample::Visible if ansi => terminal_output::prepared_document(
                    terminal,
                    &frame,
                    selection,
                    range,
                    terminal_output::ansi::frame(&frame, selection)?,
                ),
                Sample::Visible => terminal_output::document(
                    terminal,
                    &frame,
                    selection,
                    range,
                    terminal_output::frame_rows(&frame, selection)?,
                )?,
                Sample::Render(record, sequence) => terminal_output::prepared_document(
                    terminal,
                    &frame,
                    selection,
                    range,
                    terminal_output::ansi::window(
                        &decode_render(&record, &frame, range, sequence)?,
                        selection,
                    )?,
                ),
            };
            // Detection follows the reference source's plain-text representation even under ANSI.
            doc.ansi = ansi;
            Ok((source.clone(), Projection::TerminalOutput(doc)))
        })
        .map_err(|failure| failure.source(source))
}

/// Read the selected rows of one consistent frame. Every applied frame is
/// acknowledged before its rows are pinned, so an aged pin can be abandoned
/// for the next frame by the shared settle loop.
fn observe(
    client: &mut LocalClient,
    deadline: Instant,
    viewer: &ViewerId,
    projection: &mut TerminalProjection,
    selection: OutputSelection,
    ansi: bool,
) -> Result<(TerminalTextFrame, SampleRange, Sample), Failure> {
    read_settled(
        client,
        deadline,
        viewer,
        projection,
        |client, frame, sequence| {
            acknowledge_frame(client, deadline, viewer, frame)?;
            let range = terminal_output::range(frame, selection)?;
            if range.count == 0 {
                return Ok(Ok((range, Sample::Text(Vec::new()))));
            }
            if selection.source == OutputSource::Visible {
                return Ok(Ok((range, Sample::Visible)));
            }
            Ok(read_rows(
                client,
                deadline,
                viewer,
                frame,
                range,
                ansi && selection.source != OutputSource::Detection,
                sequence,
            )?
            .map(|rows| (range, rows)))
        },
    )
    .map(|(frame, (range, rows))| (frame, range, rows))
}

/// One `ReadRows` pinned to `frame`; an aged pin answers its retryable reason.
fn read_rows(
    client: &mut LocalClient,
    deadline: Instant,
    viewer: &ViewerId,
    frame: &TerminalTextFrame,
    range: SampleRange,
    ansi: bool,
    sequence: NonZeroU64,
) -> Result<Result<Sample, ViewerRowsRejectedReason>, Failure> {
    let request = TerminalViewerRequest::new(
        obcs_request_id()?,
        TerminalViewerAction::ReadRows {
            detail: if ansi {
                orchbee_client_server_protocol::ViewerRowDetail::Render
            } else {
                orchbee_client_server_protocol::ViewerRowDetail::Text
            },
            attachment_id: viewer.clone(),
            start_row: range.start,
            row_count: range.count,
            revision: frame.route.revision,
            generation: frame.route.generation,
            request_sequence: sequence,
            maximum_bytes: orchbee_client_server_protocol::MAXIMUM_READ_ROWS_BYTES,
        },
    );
    let result = client
        .submit_terminal_viewer_before(request, deadline)
        .map_err(classify_viewer)?;
    match result.outcome() {
        TerminalViewerOutcome::Record(record) if ansi => {
            Ok(Ok(Sample::Render(record.clone(), sequence)))
        }
        TerminalViewerOutcome::Record(record) => decode_history_window(
            record.clone(),
            frame.route.attachment_id,
            frame.route.host_epoch,
            frame,
            range.start,
            u64::from(range.count),
            sequence,
        )
        .map(|window| Ok(Sample::Text(terminal_output::window_rows(&window)))),
        TerminalViewerOutcome::RowsRejected { reason } => aged_pin(*reason),
        TerminalViewerOutcome::Rejected { reason } => Err(classify_viewer_rejected(*reason)),
        TerminalViewerOutcome::Unavailable { .. } => Err(viewer_unavailable()),
        _ => Err(viewer_protocol_invalid()),
    }
}

/// Validate the complete styled record before exposing any cell to the serializer.
fn decode_render<'a>(
    record: &'a [u8],
    frame: &TerminalTextFrame,
    range: SampleRange,
    sequence: NonZeroU64,
) -> Result<orchbee_terminal_protocol::TerminalRenderRowWindow<'a>, Failure> {
    use orchbee_terminal_protocol::{
        CURRENT_PROTOCOL_VERSION, MessageRole, PayloadEncoding, decode_terminal_render_row_window,
        decode_transport_envelope,
    };
    let envelope = decode_transport_envelope(record).map_err(|_| viewer_protocol_invalid())?;
    if envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Response
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::RowWindow
        || envelope.host_epoch != Some(frame.route.host_epoch)
        || envelope.payload.len() > orchbee_client_server_protocol::MAXIMUM_READ_ROWS_BYTES as usize
    {
        return Err(viewer_protocol_invalid());
    }
    let window = decode_terminal_render_row_window(envelope.payload)
        .map_err(|_| viewer_protocol_invalid())?;
    let route = window.route();
    if route.attachment_id != frame.route.attachment_id
        || route.host_epoch != frame.route.host_epoch
        || route.sequence != sequence
        || route.revision != frame.route.revision
        || route.generation != frame.route.generation
        || route.columns != frame.geometry.columns
        || route.start_row != range.start
        || route.row_count != range.count
        || route.total_rows != range.total
    {
        return Err(viewer_protocol_invalid());
    }
    Ok(window)
}

/// One complete source sample inside the wait's existing absolute budget.
/// A stale pin contributes no observation; an empty valid selection does.
pub(super) fn wait_sample(
    client: &mut LocalClient,
    run: &mut WaitRun<'_>,
    viewer: &ViewerId,
    frame: &TerminalTextFrame,
    selection: OutputSelection,
    matcher: &super::super::results::WaitMatcher<'_>,
) -> Result<Option<WaitSample>, Failure> {
    let range = terminal_output::range(frame, selection)?;
    let mut rows = if range.count == 0 {
        Vec::new()
    } else if selection.source == OutputSource::Visible {
        terminal_output::frame_rows(frame, selection)?
    } else {
        let sequence = run.next_sequence()?;
        let request = TerminalViewerRequest::new(
            obcs_request_id()?,
            TerminalViewerAction::ReadRows {
                detail: orchbee_client_server_protocol::ViewerRowDetail::Text,
                attachment_id: viewer.clone(),
                start_row: range.start,
                row_count: range.count,
                revision: frame.route.revision,
                generation: frame.route.generation,
                request_sequence: sequence,
                maximum_bytes: orchbee_client_server_protocol::MAXIMUM_READ_ROWS_BYTES,
            },
        );
        let result = client.submit_terminal_viewer_before(request, run.step_deadline()?);
        run.check()?;
        let result = result.map_err(classify_viewer)?;
        match result.outcome() {
            TerminalViewerOutcome::Record(record) => {
                terminal_output::window_rows(&decode_history_window(
                    record.clone(),
                    frame.route.attachment_id,
                    frame.route.host_epoch,
                    frame,
                    range.start,
                    u64::from(range.count),
                    sequence,
                )?)
            }
            TerminalViewerOutcome::RowsRejected { reason } => {
                use orchbee_client_server_protocol::ViewerRowsRejectedReason as Reason;
                return match reason {
                    Reason::StaleRevision | Reason::StaleGeneration | Reason::HistoryEvicted => {
                        Ok(None)
                    }
                    other => Err(classify_rows_rejected(*other, "wait-output")),
                };
            }
            TerminalViewerOutcome::Rejected { reason } => {
                return Err(classify_viewer_rejected(*reason));
            }
            TerminalViewerOutcome::Unavailable { .. } => return Err(viewer_unavailable()),
            _ => return Err(viewer_protocol_invalid()),
        }
    };
    terminal_output::prepare_rows(&mut rows, selection);
    let matched = super::super::results::match_rows(
        rows.iter().map(|row| {
            (
                row.index,
                row.text.as_str(),
                row.from_previous,
                row.into_next,
            )
        }),
        matcher,
        matches!(
            selection.source,
            OutputSource::Recent | OutputSource::RecentUnwrapped
        ),
    );
    run.observations = run
        .observations
        .checked_add(1)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    run.check()?;
    let requested_rows = match selection.source {
        OutputSource::Visible | OutputSource::Detection => frame.geometry.rows,
        OutputSource::Recent | OutputSource::RecentUnwrapped => selection.lines.unwrap_or(80),
    };
    Ok(matched.map(|matched| WaitSample {
        observation: super::super::results::WaitObservation {
            matched,
            revision: frame.route.revision,
            generation: frame.route.generation.get(),
            window_start: range.start,
            window_rows: u64::from(range.count),
            total_rows: range.total,
        },
        requested_rows,
        capacity_omitted: Some(range.capacity_omitted),
    }))
}
