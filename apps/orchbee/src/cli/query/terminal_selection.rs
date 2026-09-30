//! One revision-pinned buffer read; it does not own the user's selection.
use super::{
    LocalClient, TerminalTextFrame, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, ViewerId, attach_viewer_cancellable, classify_rows_rejected,
    classify_viewer, classify_viewer_rejected, decode_history_window, obcs_request_id,
    poll_terminal_frame_cancellable, stream_snapshot, viewer_protocol_invalid, viewer_unavailable,
};
use crate::{
    cli::{
        Failure, FailureKind, Output, failure_output, output, results,
        source::{ArmedSignals, Finish, SignalWatch},
    },
    invocation::{OutputFormat, TerminalSelectionCommand, TerminalTextPoint},
    localization::Locale,
};
use orchbee_client_server_protocol::{ViewerRowDetail, ViewerRowsRejectedReason};
use orchbee_terminal_protocol::{TerminalTextRowWindow, TerminalTextWindowRow};
use serde_json::{Value, json};
use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

pub(crate) fn run(
    command: &TerminalSelectionCommand,
    format: OutputFormat,
    locale: Locale,
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
    let mut result = execute(command, format, locale, deadline, &mut signals)
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
    command: &TerminalSelectionCommand,
    format: OutputFormat,
    locale: Locale,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Output, Failure> {
    signals.check_before(deadline)?;
    let (mut client, snapshot) = stream_snapshot(deadline, signals.poll_fd())?;
    let source = results::snapshot_source(&snapshot);
    let (terminal, viewer, attachment, epoch, projection) = attach_viewer_cancellable(
        &mut client,
        deadline,
        &snapshot,
        &command.target,
        signals.poll_fd(),
    )
    .map_err(|failure| failure.source(source.clone()))?;
    let observed = (|| {
        let frame = poll_terminal_frame_cancellable(
            &mut client,
            deadline,
            &viewer,
            projection,
            signals.poll_fd(),
        )?;
        if command
            .revision
            .is_some_and(|revision| revision != frame.route.revision)
        {
            return Err(classify_rows_rejected(
                ViewerRowsRejectedReason::StaleRevision,
                "read-history",
            ));
        }
        if command
            .generation
            .is_some_and(|generation| generation != frame.route.generation.get())
        {
            return Err(classify_rows_rejected(
                ViewerRowsRejectedReason::StaleGeneration,
                "read-history",
            ));
        }
        let (start, end) = ordered(command.anchor, command.cursor);
        let total = frame
            .scrollback_rows
            .checked_add(u64::from(frame.geometry.rows))
            .ok_or_else(viewer_protocol_invalid)?;
        if u64::from(end.row) >= total
            || start.col >= frame.geometry.columns
            || end.col >= frame.geometry.columns
        {
            return Err(classify_rows_rejected(
                ViewerRowsRejectedReason::LimitExceeded,
                "read-history",
            ));
        }
        let feedback = TerminalViewerRequest::new(
            obcs_request_id()?,
            TerminalViewerAction::FrameApplied {
                attachment_id: viewer.clone(),
                sequence: frame.route.sequence,
                generation: frame.route.generation,
                revision: frame.route.revision,
            },
        );
        match client
            .submit_terminal_viewer_cancellable(feedback, Some(deadline), signals.poll_fd())
            .map_err(classify_viewer)?
            .outcome()
        {
            TerminalViewerOutcome::FeedbackAccepted {} => {}
            TerminalViewerOutcome::Rejected { reason } => {
                return Err(classify_viewer_rejected(*reason));
            }
            TerminalViewerOutcome::Unavailable { .. } => return Err(viewer_unavailable()),
            _ => return Err(viewer_protocol_invalid()),
        }
        let window = read_window(&mut client, &viewer, &frame, start, end, deadline, signals)?;
        if window.attachment_id != attachment || window.host_epoch != epoch {
            return Err(viewer_protocol_invalid());
        }
        let text = extract(&window, start, end)?;
        Ok((frame, text))
    })();
    // A cancelled/failed Detach still ends the connection's owned viewer on Drop.
    if let Ok(id) = obcs_request_id() {
        let _ = client.submit_terminal_viewer_cancellable(
            TerminalViewerRequest::new(
                id,
                TerminalViewerAction::Detach {
                    attachment_id: viewer,
                },
            ),
            Some(deadline),
            signals.poll_fd(),
        );
    }
    drop(client);
    let (frame, text) = observed.map_err(|failure| failure.source(source.clone()))?;
    signals.check_before(deadline)?;
    let document = json!({
        "schema":"orchbee.terminal.selection", "version":1, "source":source,
        "result":{
            "terminal":results::terminal_row_json(&terminal),
            "anchor":point(command.anchor), "cursor":point(command.cursor),
            "contentRevision":frame.route.revision.to_string(),
            "contentGeneration":frame.route.generation.get().to_string(),
            "hostEpoch":frame.route.host_epoch.to_text(), "text":text,
        },
    });
    let stdout = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => output::encode_text(&locale.cli_terminal_selection(
            &terminal.terminal_id,
            frame.route.revision,
            &text,
        )),
    }
    .map_err(|_| Failure::new(FailureKind::OutputTooLarge).source(source))?;
    Ok(Output {
        status: 0,
        stdout,
        stderr: String::new(),
        finalization: None,
    })
}

fn read_window(
    client: &mut LocalClient,
    viewer: &ViewerId,
    frame: &TerminalTextFrame,
    start: TerminalTextPoint,
    end: TerminalTextPoint,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<TerminalTextRowWindow, Failure> {
    let count = u16::try_from(u64::from(end.row) - u64::from(start.row) + 1)
        .map_err(|_| Failure::new(FailureKind::InvalidUsage))?;
    let request = TerminalViewerRequest::new(
        obcs_request_id()?,
        TerminalViewerAction::ReadRows {
            detail: ViewerRowDetail::Text,
            attachment_id: viewer.clone(),
            start_row: u64::from(start.row),
            row_count: count,
            revision: frame.route.revision,
            generation: frame.route.generation,
            request_sequence: NonZeroU64::MIN,
            maximum_bytes: orchbee_client_server_protocol::MAXIMUM_READ_ROWS_BYTES,
        },
    );
    let result = client
        .submit_terminal_viewer_cancellable(request, Some(deadline), signals.poll_fd())
        .map_err(classify_viewer)?;
    match result.outcome() {
        TerminalViewerOutcome::Record(record) => decode_history_window(
            record.clone(),
            frame.route.attachment_id,
            frame.route.host_epoch,
            frame,
            u64::from(start.row),
            u64::from(count),
            NonZeroU64::MIN,
        ),
        TerminalViewerOutcome::RowsRejected { reason } => {
            Err(classify_rows_rejected(*reason, "read-history"))
        }
        TerminalViewerOutcome::Rejected { reason } => Err(classify_viewer_rejected(*reason)),
        TerminalViewerOutcome::Unavailable { .. } => Err(viewer_unavailable()),
        _ => Err(viewer_protocol_invalid()),
    }
}
fn ordered(a: TerminalTextPoint, b: TerminalTextPoint) -> (TerminalTextPoint, TerminalTextPoint) {
    if a <= b { (a, b) } else { (b, a) }
}
fn point(point: TerminalTextPoint) -> Value {
    json!({"row":point.row,"col":point.col})
}

fn extract(
    window: &TerminalTextRowWindow,
    a: TerminalTextPoint,
    b: TerminalTextPoint,
) -> Result<String, Failure> {
    let (start, end) = ordered(a, b);
    if window.columns == 0
        || start.col >= window.columns
        || end.col >= window.columns
        || window.start_row != u64::from(start.row)
        || u64::from(end.row) >= window.total_rows
        || window.rows.len() as u64 != u64::from(end.row) - u64::from(start.row) + 1
    {
        return Err(viewer_protocol_invalid());
    }
    let mut text = String::new();
    for (index, row) in window.rows.iter().enumerate() {
        if row.index != u64::from(start.row) + index as u64 {
            return Err(viewer_protocol_invalid());
        }
        let first = if row.index == u64::from(start.row) {
            start.col
        } else {
            0
        };
        let last = if row.index == u64::from(end.row) {
            end.col
        } else {
            window.columns - 1
        };
        let part = slice(row, first, last)?;
        let joined = row.wrap
            && window
                .rows
                .get(index + 1)
                .is_some_and(|next| next.wrap_continuation);
        text.push_str(if joined {
            part
        } else {
            part.trim_end_matches(' ')
        });
        if index + 1 < window.rows.len() && !joined {
            text.push('\n');
        }
    }
    Ok(text)
}
fn slice(row: &TerminalTextWindowRow, first: u16, last: u16) -> Result<&str, Failure> {
    if row.identity_ascii {
        return row
            .text
            .get(
                usize::from(first).min(row.text.len())..(usize::from(last) + 1).min(row.text.len()),
            )
            .ok_or_else(viewer_protocol_invalid);
    }
    let intersects = |index: usize| {
        let cell = row.mappings[index];
        cell.column <= last && u32::from(cell.column) + u32::from(cell.width) > u32::from(first)
    };
    let Some(start) = (0..row.mappings.len()).find(|index| intersects(*index)) else {
        return Ok("");
    };
    let end = (start..row.mappings.len())
        .take_while(|index| intersects(*index))
        .last()
        .unwrap_or(start);
    let from = row.mappings[start].byte_offset as usize;
    let to = row
        .mappings
        .get(end + 1)
        .map_or(row.text.len(), |cell| cell.byte_offset as usize);
    row.text.get(from..to).ok_or_else(viewer_protocol_invalid)
}

#[cfg(test)]
mod tests;
