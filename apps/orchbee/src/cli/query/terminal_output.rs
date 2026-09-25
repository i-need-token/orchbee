//! One source-selected read on the existing read-only viewer lifecycle.

use super::*;
use crate::cli::terminal_output::{self, SampleRange};
use crate::invocation::{OutputSelection, OutputSource, OutputTarget};

pub(crate) fn read_output(
    target: &OutputTarget,
    selection: OutputSelection,
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
        let (terminal, viewer, attachment, epoch, projection) =
            attach_viewer(&mut client, deadline, &snapshot, &target)?;
        let observed =
            poll_terminal_frame(&mut client, deadline, &viewer, projection).and_then(|frame| {
                acknowledge_frame(&mut client, deadline, &viewer, &frame)?;
                let visible = terminal_output::frame_rows(&frame);
                let range = terminal_output::range(&frame, selection, &visible)?;
                let rows = if range.count == 0 {
                    Vec::new()
                } else if selection.source == OutputSource::Visible {
                    visible
                } else {
                    read_rows(
                        &mut client,
                        deadline,
                        &viewer,
                        attachment,
                        epoch,
                        &frame,
                        range,
                    )?
                };
                Ok((frame, range, rows))
            });
        detach_viewer(&mut client, deadline, viewer);
        observed.map(|(frame, range, rows)| (terminal, frame, range, rows))
    })();
    drop(client);
    outcome
        .and_then(|(terminal, frame, range, rows)| {
            terminal_output::document(terminal, &frame, selection, range, rows)
                .map(|doc| (source.clone(), Projection::TerminalOutput(doc)))
        })
        .map_err(|failure| failure.source(source))
}

fn acknowledge_frame(
    client: &mut LocalClient,
    deadline: Instant,
    viewer: &ViewerId,
    frame: &TerminalTextFrame,
) -> Result<(), Failure> {
    let request = TerminalViewerRequest::new(
        obcs_request_id()?,
        TerminalViewerAction::FrameApplied {
            attachment_id: viewer.clone(),
            sequence: frame.route.sequence,
            generation: frame.route.generation,
            revision: frame.route.revision,
        },
    );
    let result = client
        .submit_terminal_viewer_before(request, deadline)
        .map_err(classify_viewer)?;
    match result.outcome() {
        TerminalViewerOutcome::FeedbackAccepted {} => Ok(()),
        TerminalViewerOutcome::Rejected { reason } => Err(classify_viewer_rejected(*reason)),
        TerminalViewerOutcome::Unavailable { .. } => Err(viewer_unavailable()),
        _ => Err(viewer_protocol_invalid()),
    }
}

fn read_rows(
    client: &mut LocalClient,
    deadline: Instant,
    viewer: &ViewerId,
    attachment: AttachmentId,
    epoch: HostEpoch,
    frame: &TerminalTextFrame,
    range: SampleRange,
) -> Result<Vec<terminal_output::OutputRow>, Failure> {
    let request = TerminalViewerRequest::new(
        obcs_request_id()?,
        TerminalViewerAction::ReadRows {
            attachment_id: viewer.clone(),
            start_row: range.start,
            row_count: range.count,
            revision: frame.route.revision,
            generation: frame.route.generation,
            request_sequence: std::num::NonZeroU64::MIN,
            maximum_bytes: orchbee_client_server_protocol::MAXIMUM_READ_ROWS_BYTES,
        },
    );
    let result = client
        .submit_terminal_viewer_before(request, deadline)
        .map_err(classify_viewer)?;
    match result.outcome() {
        TerminalViewerOutcome::Record(record) => decode_history_window(
            record.clone(),
            attachment,
            epoch,
            frame,
            range.start,
            u64::from(range.count),
            std::num::NonZeroU64::MIN,
        )
        .map(|window| terminal_output::window_rows(&window)),
        TerminalViewerOutcome::RowsRejected { reason } => {
            Err(classify_rows_rejected(*reason, "read-history"))
        }
        TerminalViewerOutcome::Rejected { reason } => Err(classify_viewer_rejected(*reason)),
        TerminalViewerOutcome::Unavailable { .. } => Err(viewer_unavailable()),
        _ => Err(viewer_protocol_invalid()),
    }
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
    let visible = terminal_output::frame_rows(frame);
    let range = terminal_output::range(frame, selection, &visible)?;
    let mut rows = if range.count == 0 {
        Vec::new()
    } else if selection.source == OutputSource::Visible {
        visible
    } else {
        let sequence = run.next_sequence()?;
        let request = TerminalViewerRequest::new(
            obcs_request_id()?,
            TerminalViewerAction::ReadRows {
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
