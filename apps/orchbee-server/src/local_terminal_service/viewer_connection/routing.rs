//! Routes existing-viewer controls to the pinned Worker and validates correlated record
//! targets, feedback, and row detail before returning them. It neither renders the opaque
//! records nor rebinds a viewer to a replacement terminal.

use super::{
    Action, LocalTerminalConnection, Outcome, Rejected, TargetLifecycle, TerminalViewerRequest,
    Unavailable, ViewerAttachment, ViewerRejection, WorkerAction, WorkerOperationRequest,
    WorkerOperationResult, WorkerOutcome, WorkerRequest, WorkerRequestError, rejected,
    snapshot_reason, unavailable,
};
use orchbee_capability_node_protocol::{
    OperationId, ViewerHistoryDemand as WorkerHistoryDemand,
    ViewerProjectionDemand as WorkerProjectionDemand, ViewerRowDetail as WorkerRowDetail,
    ViewerRowsRejectedReason as WorkerRowsRejectedReason,
    ViewerSelectionCommand as WorkerSelectionCommand,
    ViewerSelectionExportCancelReason as WorkerSelectionExportCancelReason,
    ViewerSelectionExportFormat as WorkerSelectionExportFormat,
    ViewerSelectionGestureAction as WorkerSelectionGestureAction,
    ViewerSelectionGestureGeometry as WorkerSelectionGestureGeometry,
    ViewerSelectionGesturePosition as WorkerSelectionGesturePosition,
    ViewerSelectionPoint as WorkerSelectionPoint,
    ViewerSelectionViewportPoint as WorkerSelectionViewportPoint,
    ViewportRowOrigin as WorkerViewportRowOrigin,
};
use orchbee_client_server_protocol::{
    HistoryDemand, ProjectionDemand, ViewerRowDetail, ViewerRowsRejectedReason,
    ViewerSelectionCommand, ViewerSelectionExportCancelReason, ViewerSelectionExportFormat,
    ViewerSelectionGestureAction, ViewerSelectionPoint, ViewportRowOrigin,
};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, HistoryControl, MessageRole,
    PayloadEncoding, RowWindowDetail, SelectionControl, SelectionExportControl,
    SelectionExportFormat, SelectionExportStatus, TerminalEffectEvent, decode_connection_control,
    decode_history_control, decode_selection_control, decode_selection_export_chunk_route,
    decode_selection_export_control, decode_terminal_effect_event, decode_transport_envelope,
    inspect_terminal_row_window_route,
};

impl LocalTerminalConnection<'_> {
    pub(super) fn execute_viewer(&mut self, request: &TerminalViewerRequest) -> Outcome {
        match request.action() {
            Action::Attach {
                terminal_create_request_id,
            } => {
                if !self.admit_viewer_slot() {
                    return unavailable(Unavailable::ServerBusy);
                }
                let issued = self.service.issue_viewer(terminal_create_request_id);
                return self.attach(issued);
            }
            Action::AttachBinding {
                session_id,
                worker_id,
                terminal_id,
            } => {
                if !self.admit_viewer_slot() {
                    return unavailable(Unavailable::ServerBusy);
                }
                let issued = self
                    .service
                    .issue_binding_viewer(session_id, worker_id, terminal_id);
                return self.attach(issued);
            }
            Action::Poll { .. }
            | Action::ReadProcessInfo { .. }
            | Action::FrameApplied { .. }
            | Action::NeedSnapshot { .. }
            | Action::ReadRows { .. }
            | Action::SetProjectionDemand { .. }
            | Action::SetHistoryDemand { .. }
            | Action::ReadViewportRows { .. }
            | Action::RowsApplied { .. }
            | Action::HistoryObserved { .. }
            | Action::SelectionGesture { .. }
            | Action::SelectionCommand { .. }
            | Action::SelectionExport { .. }
            | Action::SelectionExportChunkApplied { .. }
            | Action::SelectionExportCancel { .. }
            | Action::Detach { .. } => {}
        }
        let index = request.action().attachment_id().and_then(|attachment_id| {
            self.viewers.iter().position(|entry| {
                entry
                    .as_ref()
                    .is_some_and(|viewer| &viewer.attachment_id == attachment_id)
            })
        });
        let Some(index) = index else {
            return if matches!(request.action(), Action::Detach { .. }) {
                Outcome::Detached {}
            } else {
                rejected(Rejected::TerminalUnavailable)
            };
        };
        let Some(viewer) = self.viewers[index].as_mut() else {
            unreachable!("selected occupied viewer slot")
        };
        if self.service.target_lifecycle(viewer.target) == Some(TargetLifecycle::Disposed) {
            self.viewers[index].take();
            return if matches!(request.action(), Action::Detach { .. }) {
                Outcome::Detached {}
            } else {
                unavailable(Unavailable::TerminalRuntimeUnavailable)
            };
        }
        if !self.service.viewer_worker_is_current(&viewer.handle) {
            self.viewers[index].take();
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        }
        // A pending frame flight no longer short-circuits Poll: the Worker
        // still holds the frame record until its feedback, while queued
        // HistoryChanged events stay deliverable through the same action.
        let feedback = matches!(
            request.action(),
            Action::FrameApplied { .. } | Action::NeedSnapshot { .. }
        );
        if feedback && !viewer.projection.accepts_feedback(request.action()) {
            return rejected(Rejected::FeedbackMismatch);
        }
        let Some(request_id) = self.service.next_viewer_operation() else {
            return unavailable(Unavailable::ServerBusy);
        };
        let Some(template) = viewer.cleanup else {
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        };
        let action = match request.action() {
            Action::Poll { .. } => WorkerAction::Poll,
            Action::Detach { .. } => WorkerAction::Detach,
            Action::FrameApplied {
                sequence,
                generation,
                revision,
                ..
            } => WorkerAction::FrameApplied {
                sequence: *sequence,
                generation: *generation,
                revision: *revision,
            },
            Action::NeedSnapshot {
                sequence, reason, ..
            } => WorkerAction::NeedSnapshot {
                sequence: *sequence,
                reason: snapshot_reason(*reason),
            },
            Action::ReadProcessInfo { .. } => WorkerAction::ReadProcessInfo,
            Action::ReadRows {
                detail,
                start_row,
                row_count,
                revision,
                generation,
                request_sequence,
                maximum_bytes,
                ..
            } => WorkerAction::ReadRows {
                detail: match detail {
                    ViewerRowDetail::Text => WorkerRowDetail::Text,
                    ViewerRowDetail::Render => WorkerRowDetail::Render,
                },
                start_row: *start_row,
                row_count: *row_count,
                revision: *revision,
                generation: *generation,
                request_sequence: *request_sequence,
                maximum_bytes: *maximum_bytes,
            },
            Action::SetProjectionDemand { demand, .. } => WorkerAction::SetProjectionDemand {
                demand: match demand {
                    ProjectionDemand::Active => WorkerProjectionDemand::Active,
                    ProjectionDemand::Paused => WorkerProjectionDemand::Paused,
                },
            },
            Action::SetHistoryDemand { demand, .. } => WorkerAction::SetHistoryDemand {
                demand: match demand {
                    HistoryDemand::FollowingBottom => WorkerHistoryDemand::FollowingBottom,
                    HistoryDemand::ViewingHistory => WorkerHistoryDemand::ViewingHistory,
                },
            },
            Action::ReadViewportRows {
                detail,
                origin,
                row_count,
                revision,
                generation,
                request_sequence,
                maximum_bytes,
                ..
            } => WorkerAction::ReadViewportRows {
                detail: match detail {
                    ViewerRowDetail::Text => WorkerRowDetail::Text,
                    ViewerRowDetail::Render => WorkerRowDetail::Render,
                },
                origin: match origin {
                    ViewportRowOrigin::Tail { rows_before } => WorkerViewportRowOrigin::Tail {
                        rows_before: *rows_before,
                    },
                    ViewportRowOrigin::Committed { delta } => {
                        WorkerViewportRowOrigin::Committed { delta: *delta }
                    }
                    ViewportRowOrigin::Absolute { row } => {
                        WorkerViewportRowOrigin::Absolute { row: *row }
                    }
                },
                row_count: *row_count,
                revision: *revision,
                generation: *generation,
                request_sequence: *request_sequence,
                maximum_bytes: *maximum_bytes,
            },
            Action::RowsApplied {
                request_sequence,
                generation,
                revision,
                ..
            } => WorkerAction::RowsApplied {
                request_sequence: *request_sequence,
                generation: *generation,
                revision: *revision,
            },
            Action::HistoryObserved {
                sequence,
                generation,
                revision,
                ..
            } => WorkerAction::HistoryObserved {
                sequence: *sequence,
                generation: *generation,
                revision: *revision,
            },
            Action::SelectionGesture {
                generation,
                gesture,
                ..
            } => WorkerAction::SelectionGesture {
                generation: *generation,
                gesture: worker_selection_gesture(*gesture),
            },
            Action::SelectionCommand {
                generation,
                revision,
                command,
                ..
            } => WorkerAction::SelectionCommand {
                generation: *generation,
                revision: *revision,
                command: worker_selection_command(*command),
            },
            Action::SelectionExport {
                export_sequence,
                format,
                ..
            } => WorkerAction::SelectionExport {
                export_sequence: *export_sequence,
                format: worker_selection_export_format(*format),
            },
            Action::SelectionExportChunkApplied {
                export_sequence,
                chunk_sequence,
                ..
            } => WorkerAction::SelectionExportChunkApplied {
                export_sequence: *export_sequence,
                chunk_sequence: *chunk_sequence,
            },
            Action::SelectionExportCancel {
                export_sequence,
                reason,
                ..
            } => WorkerAction::SelectionExportCancel {
                export_sequence: *export_sequence,
                reason: worker_selection_export_cancel_reason(*reason),
            },
            Action::Attach { .. } | Action::AttachBinding { .. } => {
                unreachable!("attach is handled before existing-viewer operations")
            }
        };
        let process_started = std::time::Instant::now();
        let completion = viewer
            .handle
            .request(WorkerOperationRequest::Viewer(WorkerRequest {
                request_id,
                action,
                ..template
            }));
        if !self.service.viewer_worker_is_current(&viewer.handle) {
            self.viewers[index].take();
            return unavailable(Unavailable::TerminalRuntimeUnavailable);
        }
        match completion {
            Ok(WorkerOperationResult::Viewer(result)) => match result.outcome {
                WorkerOutcome::Empty => Outcome::Empty {},
                WorkerOutcome::Record(record) => {
                    // Row-read and correlated selection responses validate
                    // against their issued request and pinned attachment rather
                    // than the frame projection. Poll may carry history and
                    // selection and pane-effect events through their dedicated routes; every
                    // other record remains projection-owned. A record on the
                    // wrong action is therefore still a strict route violation.
                    let accepted = match request.action() {
                        Action::ReadProcessInfo { .. } => {
                            valid_process_record(viewer, request_id, &record, process_started)
                        }
                        Action::ReadRows { .. } => {
                            valid_rows_record(request.action(), viewer, request_id, &record)
                        }
                        Action::ReadViewportRows { .. } => valid_viewport_rows_record(
                            request.action(),
                            viewer,
                            request_id,
                            &record,
                        ),
                        Action::SelectionGesture { .. }
                        | Action::SelectionCommand { .. }
                        | Action::SelectionExport { .. }
                        | Action::SelectionExportChunkApplied { .. }
                        | Action::SelectionExportCancel { .. } => valid_selection_action_record(
                            request.action(),
                            viewer,
                            request_id,
                            &record,
                        ),
                        Action::Poll { .. } => valid_poll_record(viewer, &record),
                        // Only the selected viewer actions produce the specialized record or feedback checked here.
                        _ => viewer.projection.receive(&record).is_ok(),
                    };
                    if accepted {
                        Outcome::Record(record)
                    } else {
                        self.viewers[index].take();
                        unavailable(Unavailable::TerminalRuntimeUnavailable)
                    }
                }
                WorkerOutcome::RowsRejected(reason) => {
                    // A typed row rejection answers the read without disturbing
                    // the healthy viewer; the carrier already proved the pairing,
                    // and any other action turns it into a viewer violation.
                    if matches!(
                        request.action(),
                        Action::ReadRows { .. } | Action::ReadViewportRows { .. }
                    ) {
                        Outcome::RowsRejected {
                            reason: rows_rejected_reason(reason),
                        }
                    } else {
                        self.viewers[index].take();
                        unavailable(Unavailable::TerminalRuntimeUnavailable)
                    }
                }
                WorkerOutcome::FeedbackAccepted => {
                    if matches!(
                        request.action(),
                        Action::FrameApplied { .. } | Action::NeedSnapshot { .. }
                    ) {
                        viewer.projection.complete_feedback(request.action());
                    }
                    if matches!(
                        request.action(),
                        Action::FrameApplied { .. }
                            | Action::NeedSnapshot { .. }
                            | Action::SetProjectionDemand { .. }
                            | Action::SetHistoryDemand { .. }
                            | Action::RowsApplied { .. }
                            | Action::HistoryObserved { .. }
                            | Action::SelectionExportChunkApplied { .. }
                    ) {
                        Outcome::FeedbackAccepted {}
                    } else {
                        self.viewers[index].take();
                        unavailable(Unavailable::TerminalRuntimeUnavailable)
                    }
                }
                WorkerOutcome::Detached => {
                    viewer.cleanup = None;
                    self.viewers[index].take();
                    Outcome::Detached {}
                }
                WorkerOutcome::Rejected(ViewerRejection::FeedbackMismatch) => {
                    rejected(Rejected::FeedbackMismatch)
                }
                WorkerOutcome::Rejected(ViewerRejection::HostRejected)
                    if matches!(
                        request.action(),
                        Action::SetProjectionDemand { .. }
                            | Action::SetHistoryDemand { .. }
                            | Action::RowsApplied { .. }
                            | Action::HistoryObserved { .. }
                            | Action::SelectionGesture { .. }
                            | Action::SelectionCommand { .. }
                            | Action::SelectionExport { .. }
                            | Action::SelectionExportChunkApplied { .. }
                            | Action::SelectionExportCancel { .. }
                    ) =>
                {
                    rejected(Rejected::HostRejected)
                }
                WorkerOutcome::Rejected(_)
                | WorkerOutcome::Attached
                | WorkerOutcome::LifecycleObserved { .. } => {
                    self.viewers[index].take();
                    unavailable(Unavailable::TerminalRuntimeUnavailable)
                }
            },
            Err(WorkerRequestError::Backpressure) => unavailable(Unavailable::ServerBusy),
            _ => {
                self.viewers[index].take();
                unavailable(Unavailable::TerminalRuntimeUnavailable)
            }
        }
    }
}

const fn worker_selection_gesture(
    gesture: ViewerSelectionGestureAction,
) -> WorkerSelectionGestureAction {
    match gesture {
        ViewerSelectionGestureAction::Start {
            click_count,
            point,
            position,
            select_output,
        } => WorkerSelectionGestureAction::Start {
            click_count,
            point: WorkerSelectionViewportPoint {
                column: point.column,
                row: point.row,
            },
            position: WorkerSelectionGesturePosition {
                x: position.x,
                y: position.y,
            },
            select_output,
        },
        ViewerSelectionGestureAction::Update {
            geometry,
            point,
            position,
            rectangle,
        } => WorkerSelectionGestureAction::Update {
            geometry: WorkerSelectionGestureGeometry {
                cell_width: geometry.cell_width,
                columns: geometry.columns,
                padding_left: geometry.padding_left,
                screen_height: geometry.screen_height,
            },
            point: WorkerSelectionViewportPoint {
                column: point.column,
                row: point.row,
            },
            position: WorkerSelectionGesturePosition {
                x: position.x,
                y: position.y,
            },
            rectangle,
        },
        ViewerSelectionGestureAction::End { point } => WorkerSelectionGestureAction::End {
            point: WorkerSelectionViewportPoint {
                column: point.column,
                row: point.row,
            },
        },
        ViewerSelectionGestureAction::Cancel => WorkerSelectionGestureAction::Cancel,
    }
}

const fn worker_selection_point(point: ViewerSelectionPoint) -> WorkerSelectionPoint {
    WorkerSelectionPoint {
        column: point.column,
        row: point.row,
    }
}

const fn worker_selection_command(command: ViewerSelectionCommand) -> WorkerSelectionCommand {
    match command {
        ViewerSelectionCommand::Clear => WorkerSelectionCommand::Clear,
        ViewerSelectionCommand::SelectAll => WorkerSelectionCommand::SelectAll,
        ViewerSelectionCommand::SetRange {
            end,
            rectangle,
            start,
        } => WorkerSelectionCommand::SetRange {
            end: worker_selection_point(end),
            rectangle,
            start: worker_selection_point(start),
        },
        ViewerSelectionCommand::SelectOutput { point } => WorkerSelectionCommand::SelectOutput {
            point: worker_selection_point(point),
        },
    }
}

const fn worker_selection_export_format(
    format: ViewerSelectionExportFormat,
) -> WorkerSelectionExportFormat {
    match format {
        ViewerSelectionExportFormat::Plain => WorkerSelectionExportFormat::Plain,
    }
}

const fn worker_selection_export_cancel_reason(
    reason: ViewerSelectionExportCancelReason,
) -> WorkerSelectionExportCancelReason {
    match reason {
        ViewerSelectionExportCancelReason::ClipboardFailed => {
            WorkerSelectionExportCancelReason::ClipboardFailed
        }
        ViewerSelectionExportCancelReason::Disposed => WorkerSelectionExportCancelReason::Disposed,
        ViewerSelectionExportCancelReason::Replaced => WorkerSelectionExportCancelReason::Replaced,
        ViewerSelectionExportCancelReason::User => WorkerSelectionExportCancelReason::User,
    }
}

/// Selection acknowledgements are data-bearing Host records. Server repeats
/// the complete envelope and route validation before forwarding opaque bytes
/// to the exact Client attachment.
fn valid_selection_action_record(
    action: &Action,
    viewer: &ViewerAttachment,
    operation: OperationId,
    record: &[u8],
) -> bool {
    let Ok(envelope) = decode_transport_envelope(record) else {
        return false;
    };
    let Ok(correlation) = CorrelationId::from_bytes(operation.as_bytes()) else {
        return false;
    };
    if envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Response
        || envelope.correlation_id != Some(correlation)
        || envelope.host_epoch != Some(viewer.projection.host_epoch)
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::ControlJson
    {
        return false;
    }
    // The Host refuses a selection control that its own state overtook: a gesture for a
    // superseded generation, or the cancellation of an export that already finished. That
    // correlated failure is the request's answer, which the Client settles; the viewer stays.
    if matches!(
        decode_connection_control(envelope.kind, envelope.role, envelope.payload),
        Ok(ConnectionControl::TypedFailure(_))
    ) {
        return true;
    }
    match action {
        Action::SelectionGesture { .. } | Action::SelectionCommand { .. } => matches!(
            decode_selection_control(envelope.kind, envelope.role, envelope.payload),
            Ok(SelectionControl::ChangedResponse(changed))
                if changed.attachment_id == viewer.projection.attachment_id
        ),
        Action::SelectionExport {
            export_sequence,
            format,
            ..
        } => matches!(
            decode_selection_export_control(envelope.kind, envelope.role, envelope.payload),
            Ok(SelectionExportControl::StartedResponse(started))
                if started.attachment_id == viewer.projection.attachment_id
                    && started.export_sequence == *export_sequence
                    && started.format == match format {
                        ViewerSelectionExportFormat::Plain => SelectionExportFormat::Plain,
                    }
        ),
        Action::SelectionExportCancel {
            export_sequence, ..
        } => matches!(
            decode_selection_export_control(envelope.kind, envelope.role, envelope.payload),
            Ok(SelectionExportControl::FinishedResponse(finished))
                if finished.attachment_id == viewer.projection.attachment_id
                    && finished.export_sequence == *export_sequence
                    && finished.status == SelectionExportStatus::Cancelled
        ),
        // An applied chunk succeeds with feedback, never with a record; other actions are
        // validated elsewhere.
        _ => false,
    }
}

pub(super) fn valid_process_record(
    viewer: &ViewerAttachment<'_>,
    operation: OperationId,
    record: &[u8],
    started: std::time::Instant,
) -> bool {
    match process_record(viewer, operation, record) {
        Ok(Some(info)) => {
            viewer.service.observe_host_process(
                &viewer.handle,
                viewer.target,
                &info,
                None,
                None,
                started,
            );
            true
        }
        Ok(None) => true,
        Err(()) => false,
    }
}
/// Validates complete correlated process evidence without publishing it.
pub(super) fn process_record(
    viewer: &ViewerAttachment<'_>,
    operation: OperationId,
    record: &[u8],
) -> Result<Option<orchbee_terminal_protocol::TerminalProcessInfo>, ()> {
    use orchbee_terminal_protocol::{
        MAXIMUM_CONTROL_JSON_BYTES, ProcessInfoControl, TRANSPORT_ENVELOPE_HEADER_BYTES,
        decode_process_info_control,
    };
    if record.len() > TRANSPORT_ENVELOPE_HEADER_BYTES + MAXIMUM_CONTROL_JSON_BYTES {
        return Err(());
    }
    let envelope = decode_transport_envelope(record).map_err(|_| ())?;
    let correlation = CorrelationId::from_bytes(operation.as_bytes()).map_err(|_| ())?;
    if envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Response
        || envelope.correlation_id != Some(correlation)
        || envelope.host_epoch != Some(viewer.projection.host_epoch)
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::ControlJson
    {
        return Err(());
    }
    if let Ok(ProcessInfoControl::ProcessInfo(info)) =
        decode_process_info_control(envelope.kind, envelope.role, envelope.payload)
    {
        if info.attachment_id != viewer.projection.attachment_id
            || info.terminal_id.as_bytes() != &viewer.target.identity.terminal_id().as_bytes()
        {
            return Err(());
        }
        return Ok(Some(info));
    }
    if matches!(
        decode_connection_control(envelope.kind, envelope.role, envelope.payload),
        Ok(ConnectionControl::TypedFailure(_))
    ) {
        Ok(None)
    } else {
        Err(())
    }
}

/// A Poll record is either an ordinary projection record or a dedicated
/// non-frame event routed here. Every routed event validates its complete
/// envelope and exact attachment before its bytes leave Server.
pub(super) fn valid_poll_record(viewer: &mut ViewerAttachment, record: &[u8]) -> bool {
    let Ok(envelope) = decode_transport_envelope(record) else {
        return false;
    };
    if !matches!(envelope.kind, 15 | 19 | 53 | 55 | 56 | 62 | 65 | 67) {
        return viewer.projection.receive(record).is_ok();
    }
    if envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Event
        || envelope.correlation_id.is_some()
        || envelope.host_epoch != Some(viewer.projection.host_epoch)
        || envelope.optional_message
    {
        return false;
    }
    match envelope.kind {
        15 | 19 => {
            if envelope.payload_encoding != PayloadEncoding::ControlJson {
                return false;
            }
            let (identity, normal_exit) = match orchbee_terminal_protocol::decode_lifecycle_control(
                envelope.kind,
                envelope.role,
                envelope.payload,
            ) {
                Ok(orchbee_terminal_protocol::LifecycleControl::TerminalExited(event)) => (
                    event.identity,
                    event.exit == orchbee_terminal_protocol::ExitResult::ExitCode(0),
                ),
                Ok(orchbee_terminal_protocol::LifecycleControl::TerminalFailed(event)) => {
                    (event.identity, false)
                }
                // This lifecycle exchange requires its exact target-bearing acknowledgment; other control records cannot confirm it.
                _ => return false,
            };
            let exact = identity.terminal_id.as_bytes()
                == &viewer.target.identity.terminal_id().as_bytes()
                && identity.creation_id.as_bytes()
                    == &viewer.target.identity.creation_id().as_bytes();
            // Every viewer of this exact terminal, including Server sampling viewers, reaches
            // this check; the Session alone decides whether the exited Shell's Panel closes.
            if exact && normal_exit {
                viewer
                    .service
                    .close_after_normal_exit(&viewer.handle, viewer.target);
            }
            exact
        }
        55 | 56 => {
            envelope.payload_encoding == PayloadEncoding::ControlJson
                && match decode_terminal_effect_event(
                    envelope.kind,
                    envelope.role,
                    envelope.payload,
                ) {
                    Ok(TerminalEffectEvent::Bell(event)) => {
                        event.attachment_id == viewer.projection.attachment_id
                    }
                    Ok(TerminalEffectEvent::ClipboardWriteRequest(event)) => {
                        event.attachment_id == viewer.projection.attachment_id
                    }
                    // This event stream observes only its declared lifecycle/effect records; unrelated records cannot advance it.
                    _ => false,
                }
        }
        53 => {
            envelope.payload_encoding == PayloadEncoding::ControlJson
                && matches!(
                decode_history_control(envelope.kind, envelope.role, envelope.payload),
                Ok(HistoryControl::HistoryChanged(event))
                    if event.attachment_id == viewer.projection.attachment_id
                    )
        }
        62 => {
            envelope.payload_encoding == PayloadEncoding::ControlJson
                && matches!(
                    decode_selection_control(envelope.kind, envelope.role, envelope.payload),
                    Ok(SelectionControl::ChangedEvent(changed))
                        if changed.attachment_id == viewer.projection.attachment_id
                )
        }
        65 => {
            envelope.payload_encoding == PayloadEncoding::RawBytes
                && matches!(
                    decode_selection_export_chunk_route(
                        envelope.kind,
                        envelope.role,
                        envelope.payload,
                    ),
                    Ok(route) if route.attachment_id == viewer.projection.attachment_id
                )
        }
        67 => {
            envelope.payload_encoding == PayloadEncoding::ControlJson
                && matches!(
                    decode_selection_export_control(
                        envelope.kind,
                        envelope.role,
                        envelope.payload,
                    ),
                    Ok(SelectionExportControl::FinishedEvent(finished))
                        if finished.attachment_id == viewer.projection.attachment_id
                )
        }
        _ => false,
    }
}

/// A ReadRows record is accepted only when the complete OBTP envelope and the
/// OBRW header both name this exact request: the Server-issued operation is the
/// Host correlation, and every pinned window field echoes the request.
pub(super) fn valid_rows_record(
    action: &Action,
    viewer: &ViewerAttachment,
    operation: OperationId,
    record: &[u8],
) -> bool {
    let Action::ReadRows {
        detail,
        start_row,
        row_count,
        revision,
        generation,
        request_sequence,
        maximum_bytes,
        ..
    } = action
    else {
        return false;
    };
    let Ok(envelope) = decode_transport_envelope(record) else {
        return false;
    };
    let Ok(correlation) = CorrelationId::from_bytes(operation.as_bytes()) else {
        return false;
    };
    // `decode_transport_envelope` already proved kind 46 is the only
    // RowWindow-encoded response; the requested detail must match exactly.
    if envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Response
        || envelope.correlation_id != Some(correlation)
        || envelope.host_epoch != Some(viewer.projection.host_epoch)
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::RowWindow
        || envelope.payload.len() > usize::try_from(*maximum_bytes).unwrap_or(usize::MAX)
    {
        return false;
    }
    let Ok(route) = inspect_terminal_row_window_route(envelope.payload) else {
        return false;
    };
    route.attachment_id == viewer.projection.attachment_id
        && route.host_epoch == viewer.projection.host_epoch
        && route.sequence == *request_sequence
        && route.revision == *revision
        && route.generation == *generation
        && route.start_row == *start_row
        && route.row_count == *row_count
        && route.detail
            == match detail {
                ViewerRowDetail::Text => RowWindowDetail::Text,
                ViewerRowDetail::Render => RowWindowDetail::Render,
            }
}

/// A viewport-read record is accepted on the same envelope, correlation, and
/// pinned-window checks as a buffer read, except start row and revision: the
/// Host serves the current view and captures the window identity it resolved,
/// so the binding never requires request equality for those fields.
fn valid_viewport_rows_record(
    action: &Action,
    viewer: &ViewerAttachment,
    operation: OperationId,
    record: &[u8],
) -> bool {
    let Action::ReadViewportRows {
        detail,
        row_count,
        generation,
        request_sequence,
        maximum_bytes,
        ..
    } = action
    else {
        return false;
    };
    let Ok(envelope) = decode_transport_envelope(record) else {
        return false;
    };
    let Ok(correlation) = CorrelationId::from_bytes(operation.as_bytes()) else {
        return false;
    };
    if envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Response
        || envelope.correlation_id != Some(correlation)
        || envelope.host_epoch != Some(viewer.projection.host_epoch)
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::RowWindow
        || envelope.payload.len() > usize::try_from(*maximum_bytes).unwrap_or(usize::MAX)
    {
        return false;
    }
    let Ok(route) = inspect_terminal_row_window_route(envelope.payload) else {
        return false;
    };
    route.attachment_id == viewer.projection.attachment_id
        && route.host_epoch == viewer.projection.host_epoch
        && route.sequence == *request_sequence
        && route.generation == *generation
        && route.row_count == *row_count
        && route.detail
            == match detail {
                ViewerRowDetail::Text => RowWindowDetail::Text,
                ViewerRowDetail::Render => RowWindowDetail::Render,
            }
}

const fn rows_rejected_reason(reason: WorkerRowsRejectedReason) -> ViewerRowsRejectedReason {
    match reason {
        WorkerRowsRejectedReason::StaleRevision => ViewerRowsRejectedReason::StaleRevision,
        WorkerRowsRejectedReason::StaleGeneration => ViewerRowsRejectedReason::StaleGeneration,
        WorkerRowsRejectedReason::HistoryEvicted => ViewerRowsRejectedReason::HistoryEvicted,
        WorkerRowsRejectedReason::MissingResource => ViewerRowsRejectedReason::MissingResource,
        WorkerRowsRejectedReason::InvalidField => ViewerRowsRejectedReason::InvalidField,
        WorkerRowsRejectedReason::InvalidAcknowledgement => {
            ViewerRowsRejectedReason::InvalidAcknowledgement
        }
        WorkerRowsRejectedReason::LimitExceeded => ViewerRowsRejectedReason::LimitExceeded,
        WorkerRowsRejectedReason::ResourceExhausted => ViewerRowsRejectedReason::ResourceExhausted,
        WorkerRowsRejectedReason::InvalidPhase => ViewerRowsRejectedReason::InvalidPhase,
    }
}

#[cfg(test)]
mod tests;
