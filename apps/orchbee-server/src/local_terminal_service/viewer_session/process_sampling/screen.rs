//! Correlated process/frame/tail/process capture on the sampler's own temporary viewer.
use super::super::{
    Action, ViewerAttachment, WorkerAction, WorkerOperationRequest, WorkerOperationResult,
    WorkerOutcome, WorkerRequest, routing,
};
use crate::{
    agent_screen::{ScreenDetector, ScreenInput},
    site_coordinator::identify_host_agent,
};
use orchbee_capability_node_protocol::{OperationId, ViewerRowDetail, ViewerSnapshotReason};
use orchbee_terminal_protocol::{
    TerminalProcessInfo, TerminalProgress, TerminalTextFrame, decode_terminal_text_frame,
    decode_terminal_text_row_window, decode_transport_envelope,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
#[cfg(windows)]
mod events;
#[cfg(windows)]
mod graphics;
#[cfg(windows)]
pub(in crate::local_terminal_service) use events::event_capture;
#[cfg(windows)]
pub(in crate::local_terminal_service) use graphics::graphics_png;

fn request(
    viewer: &ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
    action: WorkerAction,
) -> Option<(OperationId, WorkerOutcome)> {
    if stop.load(Ordering::Acquire)
        || !viewer.service.viewer_worker_is_current(&viewer.handle)
        || viewer.service.target_lifecycle(viewer.target) != Some(super::TargetLifecycle::Unfenced)
    {
        return None;
    }
    let request_id = viewer.service.next_viewer_operation()?;
    let result = viewer
        .handle
        .request(WorkerOperationRequest::Viewer(WorkerRequest {
            request_id,
            action,
            ..template
        }));
    // Always consume Required completion, including cancellation that arrives during I/O.
    if stop.load(Ordering::Acquire) {
        return None;
    }
    match result.ok()? {
        WorkerOperationResult::Viewer(result)
            if result.request_id == request_id
                && result.attachment_id == template.attachment_id =>
        {
            Some((request_id, result.outcome))
        }
        _ => None,
    }
}
fn process(
    viewer: &ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
) -> Option<TerminalProcessInfo> {
    let (id, WorkerOutcome::Record(record)) =
        request(viewer, template, stop, WorkerAction::ReadProcessInfo)?
    else {
        return None;
    };
    routing::process_record(viewer, id, &record).ok().flatten()
}
fn frame(
    viewer: &mut ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
) -> Option<TerminalTextFrame> {
    for _ in 0..8 {
        let (_, outcome) = request(viewer, template, stop, WorkerAction::Poll)?;
        match outcome {
            WorkerOutcome::Empty => std::thread::sleep(Duration::from_millis(10)),
            WorkerOutcome::Record(record) => {
                if !routing::valid_poll_record(viewer, &record) {
                    return None;
                }
                let envelope = decode_transport_envelope(&record).ok()?;
                if envelope.kind == 53 {
                    continue;
                }
                if envelope.kind != 41 {
                    return None;
                }
                return decode_terminal_text_frame(envelope.payload).ok();
            }
            _ => return None,
        }
    }
    None
}
fn fresh_frame(
    viewer: &mut ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
) -> Option<TerminalTextFrame> {
    let initial = frame(viewer, template, stop)?;
    // Initial projection can predate process-before. Reset only our attachment to force a later capture.
    let feedback = Action::NeedSnapshot {
        attachment_id: viewer.attachment_id.clone(),
        sequence: initial.route.sequence,
        reason: orchbee_client_server_protocol::ViewerSnapshotReason::StoreReset,
    };
    if !viewer.projection.accepts_feedback(&feedback) {
        return None;
    }
    let (_, WorkerOutcome::FeedbackAccepted) = request(
        viewer,
        template,
        stop,
        WorkerAction::NeedSnapshot {
            sequence: initial.route.sequence,
            reason: ViewerSnapshotReason::StoreReset,
        },
    )?
    else {
        return None;
    };
    viewer.projection.complete_feedback(&feedback);
    frame(viewer, template, stop)
}
pub(super) fn directory(
    viewer: &mut ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
) -> Option<orchbee_workbench_protocol::WorkbenchDirectoryPath> {
    let reported = fresh_frame(viewer, template, stop)
        .and_then(|f| {
            f.current_directory
                .as_deref()
                .and_then(orchbee_local_site_runtime::windows_reported_directory)
        })
        .and_then(|p| orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(&p).ok());
    reported.or_else(|| {
        let sample = process(viewer, template, stop)?;
        let path = sample
            .processes
            .iter()
            .find(|p| p.pid == sample.root_pid)?
            .current_directory
            .as_deref()?;
        orchbee_local_site_runtime::windows_reported_directory(path)
            .and_then(|p| orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(&p).ok())
    })
}

pub(in crate::local_terminal_service) fn configured_capture(
    viewer: &mut ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
    selection: Option<&orchbee_workbench_protocol::ConfiguredCommandSelection>,
) -> Result<
    crate::local_terminal_service::configured_commands::Captured,
    orchbee_workbench_protocol::ConfiguredCommandFailure,
> {
    use orchbee_workbench_protocol::ConfiguredCommandFailure as F;
    let frame = fresh_frame(viewer, template, stop).ok_or(F::Unavailable)?;
    let mut directory = frame
        .current_directory
        .as_deref()
        .and_then(orchbee_local_site_runtime::windows_reported_directory)
        .and_then(|path| orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(&path).ok());
    let selected_text = if let Some(selection) = selection {
        selection.validate().map_err(|_| F::InvalidIntent)?;
        if frame.route.revision != selection.revision.value() {
            return Err(F::StaleSource);
        }
        let start = selection
            .anchor
            .row
            .value()
            .min(selection.cursor.row.value());
        let count = selection
            .anchor
            .row
            .value()
            .abs_diff(selection.cursor.row.value())
            + 1;
        let action = Action::ReadRows {
            attachment_id: viewer.attachment_id.clone(),
            detail: orchbee_client_server_protocol::ViewerRowDetail::Text,
            start_row: start,
            row_count: count as u16,
            revision: frame.route.revision,
            generation: frame.route.generation,
            request_sequence: frame.route.sequence,
            maximum_bytes: 8 * 1024 * 1024,
        };
        let (operation, WorkerOutcome::Record(record)) = request(
            viewer,
            template,
            stop,
            WorkerAction::ReadRows {
                detail: ViewerRowDetail::Text,
                start_row: start,
                row_count: count as u16,
                revision: frame.route.revision,
                generation: frame.route.generation,
                request_sequence: frame.route.sequence,
                maximum_bytes: 8 * 1024 * 1024,
            },
        )
        .ok_or(F::StaleSource)?
        else {
            return Err(F::StaleSource);
        };
        if !routing::valid_rows_record(&action, viewer, operation, &record) {
            return Err(F::StaleSource);
        }
        let envelope = decode_transport_envelope(&record).map_err(|_| F::StaleSource)?;
        let window =
            decode_terminal_text_row_window(envelope.payload).map_err(|_| F::StaleSource)?;
        if window.columns != frame.geometry.columns
            || window.total_rows
                != frame
                    .scrollback_rows
                    .checked_add(u64::from(frame.geometry.rows))
                    .ok_or(F::StaleSource)?
        {
            return Err(F::StaleSource);
        }
        let point = |p: orchbee_workbench_protocol::ConfiguredCommandTextPoint| {
            orchbee_terminal_protocol::SelectionPoint {
                row: p.row.value(),
                column: p.col,
            }
        };
        Some(
            orchbee_terminal_protocol::extract_terminal_text_selection(
                &window,
                point(selection.anchor),
                point(selection.cursor),
                8192,
            )
            .map_err(|_| F::InvalidIntent)?,
        )
    } else {
        None
    };
    if directory.is_none() {
        directory = process(viewer, template, stop)
            .and_then(|sample| {
                sample
                    .processes
                    .into_iter()
                    .find(|p| p.pid == sample.root_pid)
            })
            .and_then(|p| p.current_directory)
            .and_then(|path| orchbee_local_site_runtime::windows_reported_directory(&path))
            .and_then(|path| orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(&path).ok());
    }
    Ok(
        crate::local_terminal_service::configured_commands::Captured {
            directory,
            selected_text,
        },
    )
}

fn progress(value: Option<TerminalProgress>) -> String {
    use TerminalProgress::{Error, Indeterminate, Pause, Remove, Set};
    let (tag, value) = match value {
        None => return String::new(),
        Some(Remove) => return "4;0;0".into(),
        Some(Indeterminate) => return "4;3;0".into(),
        Some(Set(v)) => (1, v),
        Some(Error(v)) => (2, v),
        Some(Pause(v)) => (4, v),
    };
    format!("4;{tag};{}", value.map_or(-1, |v| i32::from(v.percent())))
}
struct CapturedScreen {
    text: String,
    title: String,
    progress: String,
    sequence: u64,
    generation: u64,
    revision: u64,
}
impl CapturedScreen {
    fn input(&self) -> ScreenInput<'_> {
        ScreenInput {
            screen: &self.text,
            title: &self.title,
            progress: &self.progress,
        }
    }
}
fn capture(
    viewer: &mut ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
) -> Option<CapturedScreen> {
    let frame = fresh_frame(viewer, template, stop)?;
    let action = Action::ReadRows {
        attachment_id: viewer.attachment_id.clone(),
        detail: orchbee_client_server_protocol::ViewerRowDetail::Text,
        start_row: frame.scrollback_rows,
        row_count: frame.geometry.rows,
        revision: frame.route.revision,
        generation: frame.route.generation,
        request_sequence: frame.route.sequence,
        maximum_bytes: 8 * 1024 * 1024,
    };
    let (operation, WorkerOutcome::Record(record)) = request(
        viewer,
        template,
        stop,
        WorkerAction::ReadRows {
            detail: ViewerRowDetail::Text,
            start_row: frame.scrollback_rows,
            row_count: frame.geometry.rows,
            revision: frame.route.revision,
            generation: frame.route.generation,
            request_sequence: frame.route.sequence,
            maximum_bytes: 8 * 1024 * 1024,
        },
    )?
    else {
        return None;
    };
    if !routing::valid_rows_record(&action, viewer, operation, &record) {
        return None;
    }
    let envelope = decode_transport_envelope(&record).ok()?;
    let window = decode_terminal_text_row_window(envelope.payload).ok()?;
    if window.columns != frame.geometry.columns
        || window.total_rows
            != frame
                .scrollback_rows
                .checked_add(u64::from(frame.geometry.rows))?
    {
        return None;
    }
    let text = bounded_text(window.rows.iter().map(|r| r.text.as_ref()))?;
    Some(CapturedScreen {
        text,
        title: frame.title.as_deref().unwrap_or("").to_owned(),
        progress: progress(frame.progress),
        sequence: frame.route.sequence.get(),
        generation: frame.route.generation.get(),
        revision: frame.route.revision,
    })
}
fn screen(
    viewer: &mut ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
    detector: &ScreenDetector,
    provider: orchbee_agent_observation_protocol::AgentReportProvider,
) -> Option<crate::agent_screen::Detection> {
    detector
        .detect(Some(provider), capture(viewer, template, stop)?.input())
        .ok()
}
/// A query predicts against the selected report provider; screen text never infers provider identity.
pub(super) fn explain(
    viewer: &mut ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
    detector: &ScreenDetector,
    provider: orchbee_agent_observation_protocol::AgentReportProvider,
) -> Option<super::CapturedAgentExplanation> {
    let before = process(viewer, template, stop)?;
    let screen = capture(viewer, template, stop)?;
    let after = process(viewer, template, stop)?;
    let selected = identify_host_agent(&before);
    let root = |p: &TerminalProcessInfo| {
        p.processes
            .iter()
            .find(|p0| p0.pid == p.root_pid)
            .map(|p0| (p0.pid, p0.creation_time))
            .filter(|(_, created)| *created != 0)
    };
    if root(&before).is_none()
        || root(&before) != root(&after)
        || selected != identify_host_agent(&after)
    {
        return None;
    }
    if selected.is_none() {
        let identities = |p: &TerminalProcessInfo| {
            p.processes
                .iter()
                .map(|p0| (p0.pid, p0.creation_time))
                .collect::<std::collections::BTreeSet<_>>()
        };
        if identities(&before) != identities(&after) {
            return None;
        }
    }
    Some(super::CapturedAgentExplanation {
        explanation: detector.explain(provider, screen.input()).ok()?,
        sample: orchbee_workbench_protocol::AgentDetectionSample {
            frame_sequence: orchbee_workbench_protocol::WorkbenchRevision::new(screen.sequence),
            frame_generation: orchbee_workbench_protocol::WorkbenchRevision::new(screen.generation),
            frame_revision: orchbee_workbench_protocol::WorkbenchRevision::new(screen.revision),
            root_pid: before.root_pid,
            agent_pid: selected.map(|p| p.pid()),
        },
    })
}
fn bounded_text<'a>(rows: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut text = String::new();
    for row in rows {
        let row = row.trim_end();
        if text.len().checked_add(row.len())?.checked_add(1)? > 64 * 1024 {
            return None;
        }
        text.push_str(row);
        text.push('\n');
    }
    text.truncate(text.trim_end().len());
    Some(text)
}
pub(super) fn observe(
    viewer: &mut ViewerAttachment<'_>,
    template: WorkerRequest,
    stop: &AtomicBool,
    detector: &ScreenDetector,
) {
    let started = Instant::now();
    let lifecycle = request(
        viewer,
        template,
        stop,
        WorkerAction::ReadLifecycle {
            target: viewer.target.identity,
        },
    )
    .and_then(|(_, outcome)| match outcome {
        WorkerOutcome::LifecycleObserved {
            target,
            revision,
            settled,
        } if target == viewer.target.identity => Some((revision, settled)),
        _ => None,
    });
    let version = fresh_frame(viewer, template, stop)
        .map(|frame| (frame.route.generation.get(), frame.route.revision));
    if !stop.load(Ordering::Acquire) {
        viewer.service.observe_terminal_projection(
            &viewer.handle,
            viewer.target,
            version,
            lifecycle,
            started,
        );
    }
    let Some(before) = process(viewer, template, stop) else {
        return;
    };
    let selected = identify_host_agent(&before);
    let (sample, detection) = if let Some(selected) = selected {
        let detection = screen(viewer, template, stop, detector, selected.provider);
        match process(viewer, template, stop) {
            Some(after) => {
                let detection = detection.filter(|_| identify_host_agent(&after) == Some(selected));
                (after, detection)
            }
            None => (before, None),
        }
    } else {
        (before, None)
    };
    if !stop.load(Ordering::Acquire) {
        viewer.service.observe_host_process(
            &viewer.handle,
            viewer.target,
            &sample,
            detection,
            started,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_lines_unicode_trailing_blanks_and_whole_input_bounds() {
        assert_eq!(
            bounded_text(["  提示  ", "answer", "  ", ""].into_iter()).as_deref(),
            Some("  提示\nanswer")
        );
        assert!(bounded_text(["x".repeat(64 * 1024)].iter().map(String::as_str)).is_none());
        assert_eq!(bounded_text(["   ", ""].into_iter()).as_deref(), Some(""));
    }
    #[test]
    fn progress_keeps_unknown_absence_distinct_from_explicit_removal() {
        assert_eq!(progress(None), "");
        assert_eq!(progress(Some(TerminalProgress::Remove)), "4;0;0");
        assert_eq!(progress(Some(TerminalProgress::Set(None))), "4;1;-1");
        assert_eq!(progress(Some(TerminalProgress::Indeterminate)), "4;3;0");
    }
}
