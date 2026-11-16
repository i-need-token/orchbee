//! Capture bounded physical/logical rows from one actual frame version; Client viewport is explicit.
use super::{
    Action, ViewerAttachment, ViewerRowDetail, WorkerAction, WorkerOutcome, WorkerRequest,
    fresh_frame, request, routing,
};
use crate::local_terminal_service::semantic_events::OutputRead;
use orchbee_terminal_protocol::{
    TerminalTextFrame, TerminalTextStyle, decode_terminal_render_row_window,
    decode_terminal_text_row_window, decode_transport_envelope, terminal_sgr_style,
};
use orchbee_workbench_protocol::{
    ClientViewportObservation, SemanticEventsFailure as Failure, SemanticOutputSource as Source,
    WorkbenchRevision,
};
use std::sync::atomic::AtomicBool;
const MAXIMUM_TEXT: usize = 8 * 1024 * 1024;
const MAXIMUM_LINE: usize = 64 * 1024;
const MAXIMUM_ROWS: usize = 65_536;
struct Row {
    text: String,
    from_previous: bool,
}

pub(in crate::local_terminal_service) fn event_capture(
    viewer: &mut ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
    read: &OutputRead,
) -> Result<(WorkbenchRevision, WorkbenchRevision, Vec<String>), Failure> {
    let frame = fresh_frame(viewer, template, stop).ok_or(Failure::SourceUnavailable)?;
    let lines = frame_lines(viewer, template, stop, &frame, read)?;
    Ok((
        WorkbenchRevision::new(frame.route.generation.get()),
        WorkbenchRevision::new(frame.route.revision),
        lines,
    ))
}

/// One terminal's recent primary-screen lines for restart replay.
pub(in crate::local_terminal_service) enum ReplayCapture {
    /// The content generation and revision still equal the caller's last capture.
    Unchanged,
    /// A full-screen program owns the alternate screen; its content is never kept.
    Alternate,
    /// Logical lines with their styles, oldest first, at this content generation and revision.
    Lines {
        version: (u64, u64),
        lines: Vec<String>,
    },
}

/// Reads the last `lines` logical lines with styles unless the content is unchanged since
/// `known` or the alternate screen is active.
pub(in crate::local_terminal_service) fn replay_capture(
    viewer: &mut ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
    lines: u16,
    known: Option<(u64, u64)>,
) -> Result<ReplayCapture, Failure> {
    let frame = fresh_frame(viewer, template, stop).ok_or(Failure::SourceUnavailable)?;
    if frame.alternate_screen {
        return Ok(ReplayCapture::Alternate);
    }
    let version = (frame.route.generation.get(), frame.route.revision);
    if known == Some(version) {
        return Ok(ReplayCapture::Unchanged);
    }
    let read = OutputRead {
        source: Source::RecentUnwrapped,
        lines,
        strip_ansi: false,
        viewport: None,
    };
    let lines = frame_lines(viewer, template, stop, &frame, &read)?;
    Ok(ReplayCapture::Lines { version, lines })
}

fn frame_lines(
    viewer: &mut ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
    frame: &TerminalTextFrame,
    read: &OutputRead,
) -> Result<Vec<String>, Failure> {
    let (floor, end) = range(viewer, frame, read)?;
    let mut start = end.saturating_sub(u64::from(read.lines)).max(floor);
    let mut rows = read_rows(viewer, template, stop, frame, start, end, read.strip_ansi)?;
    if read.source == Source::RecentUnwrapped {
        loop {
            let logical = rows.iter().filter(|row| !row.from_previous).count();
            if start == floor || logical >= usize::from(read.lines) {
                break;
            }
            let next = start.saturating_sub(64).max(floor);
            let mut preceding =
                read_rows(viewer, template, stop, frame, next, start, read.strip_ansi)?;
            preceding.append(&mut rows);
            rows = preceding;
            enforce_total(&rows)?;
            start = next;
        }
        if rows.first().is_some_and(|row| row.from_previous) {
            if rows.iter().filter(|row| !row.from_previous).count() < usize::from(read.lines) {
                return Err(Failure::SourceUnavailable);
            }
            let prefix = rows
                .iter()
                .position(|row| !row.from_previous)
                .ok_or(Failure::SourceUnavailable)?;
            rows.drain(..prefix);
        }
    }
    while rows.last().is_some_and(|row| row.text.trim().is_empty()) {
        rows.pop();
    }
    let mut lines: Vec<String> = Vec::new();
    for row in rows {
        if read.source == Source::RecentUnwrapped && row.from_previous {
            let prior = lines.last_mut().ok_or(Failure::SourceUnavailable)?;
            append(prior, &row.text)?;
        } else {
            lines.push(row.text);
        }
    }
    if lines.len() > usize::from(read.lines) {
        lines.drain(..lines.len() - usize::from(read.lines));
    }
    Ok(lines)
}
fn range(
    viewer: &ViewerAttachment<'_>,
    frame: &TerminalTextFrame,
    read: &OutputRead,
) -> Result<(u64, u64), Failure> {
    let total = frame
        .scrollback_rows
        .checked_add(u64::from(frame.geometry.rows))
        .ok_or(Failure::SourceUnavailable)?;
    match read.source {
        Source::Viewport => {
            let Some(ClientViewportObservation::Observed { source, scroll, .. }) = &read.viewport
            else {
                return Err(Failure::InvalidIntent);
            };
            let epoch = format!(
                "{:032x}",
                u128::from_be_bytes(viewer.target.identity.host_epoch().as_bytes())
            );
            if source.host_epoch != epoch
                || source.generation != frame.route.generation.get()
                || source.revision != frame.route.revision
                || source.columns != frame.geometry.columns
                || source.total_rows != total
                || source.alternate_screen != frame.alternate_screen
            {
                return Err(Failure::SourceChanged);
            }
            let end = source
                .start_row
                .checked_add(u64::from(scroll.viewport_rows))
                .ok_or(Failure::SourceChanged)?
                .min(total);
            Ok((source.start_row, end))
        }
        Source::Detection => Ok((frame.scrollback_rows, total)),
        Source::History => Ok((0, frame.scrollback_rows)),
        Source::Recent | Source::RecentUnwrapped => {
            let last = frame
                .rows
                .iter()
                .filter(|row| {
                    row.cells
                        .iter()
                        .any(|cell| cell.width < 2 && !cell.text.trim().is_empty())
                })
                .map(|row| row.index)
                .max()
                .unwrap_or(frame.cursor.row)
                .max(frame.cursor.row);
            let end = if frame.alternate_screen {
                total
            } else {
                frame
                    .scrollback_rows
                    .checked_add(u64::from(last) + 1)
                    .ok_or(Failure::SourceUnavailable)?
                    .min(total)
            };
            Ok((
                if frame.alternate_screen {
                    frame.scrollback_rows
                } else {
                    0
                },
                end,
            ))
        }
    }
}
fn read_rows(
    viewer: &mut ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
    frame: &TerminalTextFrame,
    start: u64,
    end: u64,
    strip_ansi: bool,
) -> Result<Vec<Row>, Failure> {
    let detail = if strip_ansi {
        ViewerRowDetail::Text
    } else {
        ViewerRowDetail::Render
    };
    let client_detail = if strip_ansi {
        orchbee_client_server_protocol::ViewerRowDetail::Text
    } else {
        orchbee_client_server_protocol::ViewerRowDetail::Render
    };
    let mut rows = Vec::new();
    let total = frame
        .scrollback_rows
        .checked_add(u64::from(frame.geometry.rows))
        .ok_or(Failure::SourceUnavailable)?;
    let mut index = start;
    while index < end {
        let count = u16::try_from((end - index).min(64)).map_err(|_| Failure::SourceUnavailable)?;
        let action = Action::ReadRows {
            attachment_id: viewer.attachment_id.clone(),
            detail: client_detail,
            start_row: index,
            row_count: count,
            revision: frame.route.revision,
            generation: frame.route.generation,
            request_sequence: frame.route.sequence,
            maximum_bytes: MAXIMUM_TEXT as u32,
        };
        let (operation, outcome) = request(
            viewer,
            template,
            stop,
            WorkerAction::ReadRows {
                detail,
                start_row: index,
                row_count: count,
                revision: frame.route.revision,
                generation: frame.route.generation,
                request_sequence: frame.route.sequence,
                maximum_bytes: MAXIMUM_TEXT as u32,
            },
        )
        .ok_or(Failure::SourceUnavailable)?;
        let record = match outcome {
            WorkerOutcome::Record(record) => record,
            WorkerOutcome::RowsRejected(
                orchbee_capability_node_protocol::ViewerRowsRejectedReason::StaleRevision
                | orchbee_capability_node_protocol::ViewerRowsRejectedReason::StaleGeneration
                | orchbee_capability_node_protocol::ViewerRowsRejectedReason::HistoryEvicted,
            ) => return Err(Failure::SourceChanged),
            // Action/result admission belongs to the protocol; this step consumes only its requested viewer record or acknowledgment.
            _ => return Err(Failure::SourceUnavailable),
        };
        if !routing::valid_rows_record(&action, viewer, operation, &record) {
            return Err(Failure::SourceChanged);
        }
        let envelope = decode_transport_envelope(&record).map_err(|_| Failure::SourceChanged)?;
        if strip_ansi {
            let window = decode_terminal_text_row_window(envelope.payload)
                .map_err(|_| Failure::SourceChanged)?;
            if window.columns != frame.geometry.columns
                || window.total_rows != total
                || window.rows.len() != usize::from(count)
            {
                return Err(Failure::SourceChanged);
            }
            for row in window.rows {
                let text = row.text.trim_end().to_string();
                if text.len() > MAXIMUM_LINE {
                    return Err(Failure::SourceUnavailable);
                }
                rows.push(Row {
                    text,
                    from_previous: row.wrap_continuation,
                });
            }
        } else {
            let window = decode_terminal_render_row_window(envelope.payload)
                .map_err(|_| Failure::SourceChanged)?;
            if window.route().total_rows != total
                || window.projection().geometry().columns != frame.geometry.columns
                || window.projection().rows().len() != usize::from(count)
            {
                return Err(Failure::SourceChanged);
            }
            for row in window.projection().rows() {
                let cells: Vec<_> = row.cells().collect();
                let kept = cells
                    .iter()
                    .rposition(|cell| {
                        cell.width != 2 && (!cell.text.trim().is_empty() || !unstyled(cell.style))
                    })
                    .map_or(0, |index| index + 1);
                let mut text = String::new();
                let mut last = None;
                for cell in cells.into_iter().take(kept).filter(|cell| cell.width != 2) {
                    if last != Some(cell.style) {
                        append(&mut text, &terminal_sgr_style(cell.style).to_string())?;
                        last = Some(cell.style);
                    }
                    append(&mut text, cell.text)?;
                }
                if !text.is_empty() {
                    append(&mut text, "\x1b[0m")?;
                }
                rows.push(Row {
                    text,
                    from_previous: row.flags() & 2 != 0,
                });
            }
        }
        enforce_total(&rows)?;
        index += u64::from(count);
    }
    Ok(rows)
}
fn append(text: &mut String, next: &str) -> Result<(), Failure> {
    if text.len().saturating_add(next.len()) > MAXIMUM_LINE {
        return Err(Failure::SourceUnavailable);
    }
    text.push_str(next);
    Ok(())
}
fn enforce_total(rows: &[Row]) -> Result<(), Failure> {
    if rows.len() > MAXIMUM_ROWS
        || rows.iter().fold(0usize, |bytes, row| {
            bytes
                .saturating_add(row.text.capacity())
                .saturating_add(std::mem::size_of::<Row>())
        }) > MAXIMUM_TEXT
    {
        return Err(Failure::SourceUnavailable);
    }
    Ok(())
}
fn unstyled(style: TerminalTextStyle) -> bool {
    [style.foreground, style.background, style.underline_color]
        .into_iter()
        .all(|color| {
            matches!(
                color,
                orchbee_terminal_protocol::COLOR_DEFAULT | orchbee_terminal_protocol::COLOR_NONE
            )
        })
        && style.flags == 0
        && style.underline == 0
}
