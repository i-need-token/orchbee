use super::{
    Action, LocalTerminalSession, Outcome, Rejected, TargetLifecycle, TerminalViewerRequest,
    Unavailable, ViewerAttachment, ViewerRejection, WorkerAction, WorkerOperationRequest,
    WorkerOperationResult, WorkerOutcome, WorkerRequest, WorkerRequestError, rejected,
    snapshot_reason, unavailable,
};
use orchbee_capability_node_protocol::{
    OperationId, ViewerHistoryDemand as WorkerHistoryDemand,
    ViewerProjectionDemand as WorkerProjectionDemand, ViewerRowDetail as WorkerRowDetail,
    ViewerRowsRejectedReason as WorkerRowsRejectedReason,
    ViewportRowOrigin as WorkerViewportRowOrigin,
};
use orchbee_client_server_protocol::{
    HistoryDemand, ProjectionDemand, ViewerRowDetail, ViewerRowsRejectedReason, ViewportRowOrigin,
};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, HistoryControl, MessageRole, PayloadEncoding,
    RowWindowDetail, decode_history_control, decode_transport_envelope,
    inspect_terminal_row_window_route,
};

impl LocalTerminalSession<'_> {
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
                site_id,
                worker_id,
                terminal_id,
            } => {
                if !self.admit_viewer_slot() {
                    return unavailable(Unavailable::ServerBusy);
                }
                let issued = self
                    .service
                    .issue_binding_viewer(site_id, worker_id, terminal_id);
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
                origin,
                row_count,
                revision,
                generation,
                request_sequence,
                maximum_bytes,
                ..
            } => WorkerAction::ReadViewportRows {
                origin: match origin {
                    ViewportRowOrigin::Tail { rows_before } => WorkerViewportRowOrigin::Tail {
                        rows_before: *rows_before,
                    },
                    ViewportRowOrigin::Committed { delta } => {
                        WorkerViewportRowOrigin::Committed { delta: *delta }
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
            Action::Attach { .. } | Action::AttachBinding { .. } => {
                unreachable!("attach is handled before existing-viewer operations")
            }
        };
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
                    // A row-read response is not a projection event: it is
                    // validated against the issued request and the pinned
                    // attachment, never fed to the frame-only projection. Poll
                    // alone may carry a kind-53 history event; every other
                    // record still belongs to the frame-only projection, so a
                    // HistoryChanged record on another action remains a strict
                    // route violation.
                    let accepted = match request.action() {
                        Action::ReadProcessInfo { .. } => {
                            valid_process_record(viewer, request_id, &record)
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
                        Action::Poll { .. } => valid_poll_record(viewer, &record),
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
                    Outcome::FeedbackAccepted {}
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
                    ) =>
                {
                    rejected(Rejected::HostRejected)
                }
                WorkerOutcome::Rejected(_) | WorkerOutcome::Attached => {
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

fn valid_process_record(
    viewer: &ViewerAttachment<'_>,
    operation: OperationId,
    record: &[u8],
) -> bool {
    use orchbee_terminal_protocol::{
        ConnectionControl, MAXIMUM_CONTROL_JSON_BYTES, ProcessInfoControl,
        TRANSPORT_ENVELOPE_HEADER_BYTES, decode_connection_control, decode_process_info_control,
    };
    if record.len() > TRANSPORT_ENVELOPE_HEADER_BYTES + MAXIMUM_CONTROL_JSON_BYTES {
        return false;
    }
    let (Ok(envelope), Ok(correlation)) = (
        decode_transport_envelope(record),
        CorrelationId::from_bytes(operation.as_bytes()),
    ) else {
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
    match decode_process_info_control(envelope.kind, envelope.role, envelope.payload) {
        Ok(ProcessInfoControl::ProcessInfo(info)) => {
            info.attachment_id == viewer.projection.attachment_id
                && info.terminal_id.as_bytes() == &viewer.target.identity.terminal_id().as_bytes()
        }
        _ => matches!(
            decode_connection_control(envelope.kind, envelope.role, envelope.payload),
            Ok(ConnectionControl::TypedFailure(_))
        ),
    }
}

/// A Poll record is either an ordinary projection record — still admitted only
/// by the frame-only projection — or a kind-53 `HistoryChanged` event the
/// projection never sees. The event validates its complete envelope plus the
/// decoded payload's exact attachment identity; anything else remains the
/// projection's strict rejection.
fn valid_poll_record(viewer: &mut ViewerAttachment, record: &[u8]) -> bool {
    let Ok(envelope) = decode_transport_envelope(record) else {
        return false;
    };
    if envelope.kind != 53 {
        return viewer.projection.receive(record).is_ok();
    }
    envelope.protocol == CURRENT_PROTOCOL_VERSION
        && envelope.role == MessageRole::Event
        && envelope.correlation_id.is_none()
        && envelope.host_epoch == Some(viewer.projection.host_epoch)
        && !envelope.optional_message
        && envelope.payload_encoding == PayloadEncoding::ControlJson
        && matches!(
            decode_history_control(envelope.kind, envelope.role, envelope.payload),
            Ok(HistoryControl::HistoryChanged(event))
                if event.attachment_id == viewer.projection.attachment_id
        )
}

/// A ReadRows record is accepted only when the complete OBTP envelope and the
/// OBRW header both name this exact request: the Server-issued operation is the
/// Host correlation, and every pinned window field echoes the request.
fn valid_rows_record(
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
        && route.detail == RowWindowDetail::Text
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
