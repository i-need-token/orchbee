//! Translates generation-fenced viewer actions into Host attachment, read, demand, selection,
//! and feedback exchanges and updates connection-owned viewer state. It routes opaque
//! projections without rendering them or deciding product exposure and Client authorization.

use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    error::Error,
    num::NonZeroU64,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    MAXIMUM_VIEWER_RECORD_BYTES, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, TerminalViewerResult, ViewerHistoryDemand, ViewerProjectionDemand,
    ViewerRejection, ViewerRowDetail, ViewerRowsRejectedReason, ViewerSelectionCommand,
    ViewerSelectionExportCancelReason, ViewerSelectionExportFormat, ViewerSelectionGestureAction,
    ViewerSelectionPoint, ViewerSnapshotReason, ViewportRowOrigin,
};
use orchbee_local_worker_protocol::WorkerId;
use orchbee_terminal_protocol::{
    AttachTerminal, AttachmentCapabilityState, AttachmentControl, AttachmentId, AttachmentTarget,
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, EncodedRowAccessControl, Envelope,
    FrameApplied, HistoryControl, HistoryDemand, HistoryObserved, MessageRole, NeedSnapshot,
    PayloadEncoding, ProjectionDemand, ProjectionDemandControl, ProjectionFeedbackControl,
    ReadRows, RowAccessControl, RowRevisionPolicy, RowWindowDetail, RowWindowOrigin,
    RowWindowPurpose, RowsApplied, SelectionCommand, SelectionCommandKind, SelectionControl,
    SelectionExport, SelectionExportCancel, SelectionExportCancelReason,
    SelectionExportChunkApplied, SelectionExportControl, SelectionExportFormat,
    SelectionExportStatus, SelectionGesture, SelectionGestureAction, SelectionGestureGeometry,
    SelectionGesturePosition, SelectionPoint, SelectionViewportPoint, SetHistoryDemand,
    SetProjectionDemand, SnapshotReason, TRANSPORT_ENVELOPE_HEADER_BYTES, TerminalId,
    decode_attachment_control, decode_connection_control, decode_history_control,
    decode_projection_demand_control, decode_projection_feedback_control,
    decode_row_access_control, decode_selection_control, decode_selection_export_control,
    decode_transport_envelope, encode_attachment_control, encode_history_control,
    encode_projection_demand_control, encode_projection_feedback_control,
    encode_row_access_control, encode_selection_control, encode_selection_export_control,
    encode_transport_envelope, message_contract,
};

use crate::diagnostic_events::Site;
use crate::host_connection::{HostBrokerConnection, HostExchange, NotStarted, exchange};
use crate::worker_identity::fresh_identity;

mod graphics_decode;
mod lifecycle;
pub(crate) use graphics_decode::execute as decode_graphics;
mod process_info;

fn invalid(error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string())
}

pub(crate) use crate::host_connection::{UnexpectedAnswer, response_envelope};

pub(crate) fn execute(
    request: TerminalViewerRequest,
    worker_id: WorkerId,
    generation: NonZeroU64,
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<TerminalViewerResult, Box<dyn Error>> {
    let outcome = if request.worker_id.as_bytes() != worker_id.as_bytes() {
        TerminalViewerOutcome::Rejected(ViewerRejection::WrongWorker)
    } else if request.worker_generation.get() != generation.get() {
        TerminalViewerOutcome::Rejected(ViewerRejection::StaleWorkerGeneration)
    } else {
        match apply(request, host, server) {
            Ok(outcome) => outcome,
            Err(error) if NotStarted::is(error.as_ref()) => not_started(request.action),
            Err(error) => contain(error, host, request.attachment_id)?,
        }
    };
    Ok(TerminalViewerResult {
        request_id: request.request_id,
        attachment_id: request.attachment_id,
        outcome,
    })
}

/// The answer to a viewer request whose first Host exchange had no room in the request. A
/// request's first exchange starts within the record budget of its arrival, well inside the start
/// limit, so this answers only a Worker that stalled for seconds; nothing reached the Host and the
/// viewer stays as it was.
const fn not_started(action: TerminalViewerAction) -> TerminalViewerOutcome {
    match action {
        // The Server answers this one as busy, and both Clients attach again.
        TerminalViewerAction::Attach { .. } => {
            TerminalViewerOutcome::Rejected(ViewerRejection::CapacityExceeded)
        }
        TerminalViewerAction::ReadRows { .. } | TerminalViewerAction::ReadViewportRows { .. } => {
            TerminalViewerOutcome::RowsRejected(ViewerRowsRejectedReason::ResourceExhausted)
        }
        _ => TerminalViewerOutcome::Rejected(ViewerRejection::HostRejected),
    }
}

/// An unexpected answer refuses this request and closes its viewer, which a later cleanup
/// detaches; every other failure still ends the connection.
fn contain(
    error: Box<dyn Error>,
    host: &HostBrokerConnection,
    attachment: orchbee_capability_node_protocol::OperationId,
) -> Result<TerminalViewerOutcome, Box<dyn Error>> {
    let answer = error.downcast::<UnexpectedAnswer>()?;
    crate::diagnostic_events::host_unexpected(
        host.pacing.site(),
        answer.kind,
        crate::diagnostic_events::Anomaly::Answer,
    );
    host.viewers.borrow_mut().close(attachment);
    Ok(TerminalViewerOutcome::Rejected(
        ViewerRejection::HostRejected,
    ))
}

/// Cleanup starts Host exchanges only within this share of time after it begins, and only while
/// the request's bound has room. It runs after the request's own work, so it never takes that
/// work's room; whatever it does not reach waits for the next cleanup.
pub(crate) const CLEANUP_SHARE: Duration = Duration::from_secs(2);

/// Detaches what the connection no longer uses — deferred Detaches first, then closed viewers
/// with their controllers — and brings each controller's demand in line with its viewer.
///
/// Queue saturation retires only that viewer, through the same Host writer; it cannot retain
/// another output frame. Any correlated answer ends an attachment: a refused Detach counts as
/// detached, and another answer is recorded and the attachment forgotten all the same. Only a
/// failed exchange is an error.
pub(crate) fn cleanup(
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<(), Box<dyn Error>> {
    let site = host.pacing.site();
    host.pacing.set_site(Site::Cleanup);
    let cleaned = clean(host, server, Instant::now() + CLEANUP_SHARE);
    host.pacing.set_site(site);
    NotStarted::optional(cleaned).map(drop)
}

fn clean(
    host: &HostBrokerConnection,
    server: &UnixStream,
    share: Instant,
) -> Result<(), Box<dyn Error>> {
    let within = || {
        if Instant::now() < share {
            Ok(())
        } else {
            Err(Box::new(NotStarted) as Box<dyn Error>)
        }
    };
    for id in host.pending_detaches() {
        within()?;
        settle_deferred_detach(host, server, id)?;
    }
    let closed = host.viewers.borrow().closed_ids();
    for bytes in closed {
        let id = orchbee_capability_node_protocol::OperationId::from_bytes(bytes)?;
        within()?;
        crate::terminal_control_operation::release_viewer(host, server, id)?;
        within()?;
        let correlation = CorrelationId::from_bytes(fresh_identity()?)?;
        let answer =
            detach_attachment(host, server, correlation, AttachmentId::from_bytes(bytes)?)?;
        record_cleanup_answer(&answer);
        host.forget_viewer(id);
    }
    let grants = host.controllers.borrow().grants();
    for grant in grants {
        let viewer = host.viewers.borrow().demand(grant.viewer_id);
        let Some(demand) =
            viewer.filter(|demand| grant.capability_revision.is_some() && *demand != grant.demand)
        else {
            continue;
        };
        within()?;
        let correlation = CorrelationId::from_bytes(fresh_identity()?)?;
        follow_viewer_demand(host, server, correlation, grant, demand)?;
    }
    Ok(())
}

/// Sends every Detach that no earlier request had room for, oldest first. A Detach this request
/// has no room for stays pending and ends the call with `NotStarted`; only a failed exchange is
/// another error.
pub(crate) fn settle_deferred_detaches(
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<(), Box<dyn Error>> {
    for id in host.pending_detaches() {
        settle_deferred_detach(host, server, id)?;
    }
    Ok(())
}

/// Any correlated answer settles a deferred Detach; one other than the acceptance is recorded.
fn settle_deferred_detach(
    host: &HostBrokerConnection,
    server: &UnixStream,
    id: [u8; 16],
) -> Result<(), Box<dyn Error>> {
    let correlation = CorrelationId::from_bytes(fresh_identity()?)?;
    let answer = detach_attachment(host, server, correlation, AttachmentId::from_bytes(id)?)?;
    record_cleanup_answer(&answer);
    host.settle_detach(id);
    Ok(())
}

fn record_cleanup_answer(answer: &DetachAnswer) {
    match answer {
        DetachAnswer::Detached => {}
        DetachAnswer::Refused(kind) | DetachAnswer::Unexpected(UnexpectedAnswer { kind }) => {
            crate::diagnostic_events::host_unexpected(
                Site::Cleanup,
                *kind,
                crate::diagnostic_events::Anomaly::Answer,
            );
        }
    }
}

/// Sets a controller's demand to its viewer's. A refusal or another answer means the controller
/// can no longer follow its viewer: it is recorded and released. [`NotStarted`] leaves it for a
/// later cleanup.
fn follow_viewer_demand(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    grant: crate::host_connection::ControlGrant,
    demand: ProjectionDemand,
) -> Result<(), Box<dyn Error>> {
    let controller = AttachmentId::from_bytes(grant.controller_id.as_bytes())?;
    let kind = match set_attachment_demand(host, server, correlation, controller, demand) {
        Ok(DemandAnswer::Accepted) => {
            host.controllers
                .borrow_mut()
                .set_demand(grant.controller_id, demand);
            return Ok(());
        }
        Ok(DemandAnswer::Refused(kind)) => kind,
        Err(error) => error.downcast::<UnexpectedAnswer>()?.kind,
    };
    crate::diagnostic_events::host_unexpected(
        host.pacing.site(),
        kind,
        crate::diagnostic_events::Anomaly::Answer,
    );
    crate::terminal_control_operation::release_viewer(host, server, grant.viewer_id)
}

fn apply(
    request: TerminalViewerRequest,
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<TerminalViewerOutcome, Box<dyn Error>> {
    let id = AttachmentId::from_bytes(request.attachment_id.as_bytes())?;
    let correlation = CorrelationId::from_bytes(request.request_id.as_bytes())?;
    match request.action {
        TerminalViewerAction::Attach {
            host_epoch,
            terminal_id,
        } => {
            if host_epoch.as_bytes() != *host.host_epoch().as_bytes() {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::StaleHostEpoch,
                ));
            }
            if host.disposed_terminal(terminal_id) || host.disposed_viewer(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            if host.controllers.borrow().contains(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentAlreadyExists,
                ));
            }
            let admission = host.viewers.borrow_mut().admit(&request);
            if let Err(reason) = admission {
                return Ok(TerminalViewerOutcome::Rejected(reason));
            }
            host.note_attached(request.attachment_id);
            let terminal_id = TerminalId::from_bytes(terminal_id.as_bytes())?;
            let capabilities = AttachmentCapabilityState {
                read: true,
                input: false,
                resize: false,
            };
            let command =
                encode_attachment_control(&AttachmentControl::AttachTerminal(AttachTerminal {
                    attachment_id: id,
                    capabilities,
                    request_resize_lease: false,
                    terminal_id,
                }))?;
            let Some(response) = NotStarted::optional(exchange(
                host,
                server,
                correlation,
                command.kind,
                &command.payload,
                PayloadEncoding::ControlJson,
            ))?
            else {
                // Nothing reached the Host. The Server answers this as busy and the Client
                // attaches again.
                host.viewers.borrow_mut().remove(request.attachment_id);
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::CapacityExceeded,
                ));
            };
            let envelope = decode_transport_envelope(&response)?;
            if matches!(
                decode_connection_control(envelope.kind, envelope.role, envelope.payload),
                Ok(ConnectionControl::TypedFailure(_))
            ) {
                host.viewers.borrow_mut().remove(request.attachment_id);
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::HostRejected,
                ));
            }
            let Ok(AttachmentControl::AttachmentCapabilities(accepted)) =
                decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
            else {
                return Err(UnexpectedAnswer::of(&response).into());
            };
            // Capabilities beyond the Server's grant are never used: the viewer is closed.
            if accepted.attachment_id != id
                || accepted.terminal_id != terminal_id
                || accepted.capabilities != capabilities
                || accepted.resize_lease
            {
                return Err(UnexpectedAnswer::of(&response).into());
            }
            let demand = SetProjectionDemand {
                attachment_id: id,
                demand: ProjectionDemand::Active,
            };
            let command = encode_projection_demand_control(
                &ProjectionDemandControl::SetProjectionDemand(demand),
            )?;
            let Some(response) = NotStarted::optional(exchange(
                host,
                server,
                correlation,
                command.kind,
                &command.payload,
                PayloadEncoding::ControlJson,
            ))?
            else {
                // The new Host attachment stays paused until a later cleanup detaches it; the
                // Client attaches again as for any busy refusal.
                host.viewers.borrow_mut().close(request.attachment_id);
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::CapacityExceeded,
                ));
            };
            let envelope = decode_transport_envelope(&response)?;
            let Ok(ProjectionDemandControl::ProjectionDemandAccepted(accepted)) =
                decode_projection_demand_control(envelope.kind, envelope.role, envelope.payload)
            else {
                return Err(UnexpectedAnswer::of(&response).into());
            };
            if accepted.attachment_id != id || accepted.demand != ProjectionDemand::Active {
                return Err(UnexpectedAnswer::of(&response).into());
            }
            match lifecycle::snapshot(
                host,
                server,
                orchbee_capability_node_protocol::HostTerminalId::from_bytes(
                    *terminal_id.as_bytes(),
                )?,
            )? {
                Ok(Some(record)) => host.viewers.borrow_mut().receive(record, host.host_epoch()),
                Ok(None) => {}
                // A refused Attach grants nothing, so the new Host attachment goes first.
                Err(reason) => {
                    retire_viewer(host, server, correlation, request.attachment_id)?;
                    return Ok(TerminalViewerOutcome::Rejected(reason));
                }
            }
            Ok(TerminalViewerOutcome::Attached)
        }
        TerminalViewerAction::Poll => Ok(host.viewers.borrow_mut().poll(request.attachment_id)),
        TerminalViewerAction::ReadLifecycle { target } => {
            lifecycle::read(host, server, request.attachment_id, target)
        }
        TerminalViewerAction::ReadProcessInfo => {
            process_info::read(host, server, request.attachment_id, correlation)
        }
        TerminalViewerAction::ReadRows {
            detail,
            start_row,
            row_count,
            revision,
            generation,
            request_sequence,
            maximum_bytes,
        } => read_rows(
            host,
            server,
            correlation,
            request.attachment_id,
            ReadRows {
                attachment_id: id,
                detail: match detail {
                    ViewerRowDetail::Text => RowWindowDetail::Text,
                    ViewerRowDetail::Render => RowWindowDetail::Render,
                },
                generation,
                maximum_bytes,
                origin: RowWindowOrigin::Absolute { row: start_row },
                purpose: RowWindowPurpose::Buffer,
                request_sequence,
                revision,
                revision_policy: RowRevisionPolicy::Exact,
                row_count,
            },
        ),
        TerminalViewerAction::ReadViewportRows {
            detail,
            origin,
            row_count,
            revision,
            generation,
            request_sequence,
            maximum_bytes,
        } => {
            // A relative origin reads the Host's current view; an absolute row names a place in
            // one revision of history, so a changed history refuses it rather than moving it.
            let (origin, revision_policy) = match origin {
                ViewportRowOrigin::Tail { rows_before } => (
                    RowWindowOrigin::Tail { rows_before },
                    RowRevisionPolicy::CurrentView,
                ),
                ViewportRowOrigin::Committed { delta } => (
                    RowWindowOrigin::Committed { delta },
                    RowRevisionPolicy::CurrentView,
                ),
                ViewportRowOrigin::Absolute { row } => {
                    (RowWindowOrigin::Absolute { row }, RowRevisionPolicy::Exact)
                }
            };
            read_rows(
                host,
                server,
                correlation,
                request.attachment_id,
                ReadRows {
                    attachment_id: id,
                    detail: match detail {
                        ViewerRowDetail::Text => RowWindowDetail::Text,
                        ViewerRowDetail::Render => RowWindowDetail::Render,
                    },
                    generation,
                    maximum_bytes,
                    origin,
                    purpose: RowWindowPurpose::Viewport,
                    request_sequence,
                    revision,
                    revision_policy,
                    row_count,
                },
            )
        }
        TerminalViewerAction::Detach => {
            if !host.viewers.borrow().contains(request.attachment_id) {
                if host.disposed_viewer(request.attachment_id) {
                    return Ok(TerminalViewerOutcome::Detached);
                }
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            crate::terminal_control_operation::release_viewer(host, server, request.attachment_id)?;
            retire_viewer(host, server, correlation, request.attachment_id)?;
            Ok(TerminalViewerOutcome::Detached)
        }
        TerminalViewerAction::SetProjectionDemand { demand } => {
            if !host.viewers.borrow().open(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            let demand = match demand {
                ViewerProjectionDemand::Active => ProjectionDemand::Active,
                ViewerProjectionDemand::Paused => ProjectionDemand::Paused,
            };
            // The controller goes first: pausing it repairs its held input and focus claim while
            // the viewer is still presented.
            let grant = host.controllers.borrow().for_viewer(request.attachment_id);
            if let Some(grant) = grant {
                let controller = AttachmentId::from_bytes(grant.controller_id.as_bytes())?;
                match set_attachment_demand(host, server, correlation, controller, demand)? {
                    DemandAnswer::Accepted => host
                        .controllers
                        .borrow_mut()
                        .set_demand(grant.controller_id, demand),
                    DemandAnswer::Refused(_) => {
                        return Ok(TerminalViewerOutcome::Rejected(
                            ViewerRejection::HostRejected,
                        ));
                    }
                }
            }
            // A slow controller answer can leave the viewer's own change no room: nothing of the
            // viewer changes, the request is refused for the Client to send again, and a later
            // cleanup brings the controller back in line with its viewer.
            let Some(DemandAnswer::Accepted) =
                NotStarted::optional(set_attachment_demand(host, server, correlation, id, demand))?
            else {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::HostRejected,
                ));
            };
            host.viewers
                .borrow_mut()
                .set_demand(request.attachment_id, demand);
            Ok(TerminalViewerOutcome::FeedbackAccepted)
        }
        TerminalViewerAction::SetHistoryDemand { demand } => {
            if !host.viewers.borrow().open(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            let demand = match demand {
                ViewerHistoryDemand::FollowingBottom => HistoryDemand::FollowingBottom,
                ViewerHistoryDemand::ViewingHistory => HistoryDemand::ViewingHistory,
            };
            let command =
                encode_history_control(&HistoryControl::SetHistoryDemand(SetHistoryDemand {
                    attachment_id: id,
                    demand,
                }))?;
            let Some(response) =
                ack_round_trip(host, server, correlation, command.kind, &command.payload)?
            else {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::HostRejected,
                ));
            };
            let envelope = decode_transport_envelope(&response)?;
            let Ok(HistoryControl::HistoryDemandAccepted(accepted)) =
                decode_history_control(envelope.kind, envelope.role, envelope.payload)
            else {
                return Err(UnexpectedAnswer::of(&response).into());
            };
            if accepted.attachment_id != id || accepted.demand != demand {
                return Err(UnexpectedAnswer::of(&response).into());
            }
            Ok(TerminalViewerOutcome::FeedbackAccepted)
        }
        TerminalViewerAction::RowsApplied {
            request_sequence,
            generation,
            revision,
        } => {
            if !host.viewers.borrow().open(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            let applied = RowsApplied {
                attachment_id: id,
                generation,
                request_sequence,
                revision,
            };
            let command =
                encode_row_access_control(&RowAccessControl::RowsAppliedRequest(applied))?;
            let Some(response) =
                ack_round_trip(host, server, correlation, command.kind, &command.payload)?
            else {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::HostRejected,
                ));
            };
            let envelope = decode_transport_envelope(&response)?;
            if decode_row_access_control(envelope.kind, envelope.role, envelope.payload).ok()
                != Some(RowAccessControl::RowsAppliedResponse(applied))
            {
                return Err(UnexpectedAnswer::of(&response).into());
            }
            Ok(TerminalViewerOutcome::FeedbackAccepted)
        }
        TerminalViewerAction::HistoryObserved {
            sequence,
            generation,
            revision,
        } => {
            if !host.viewers.borrow().open(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            let observed = HistoryObserved {
                attachment_id: id,
                generation,
                revision,
                sequence,
            };
            let command =
                encode_history_control(&HistoryControl::HistoryObservedRequest(observed))?;
            let Some(response) =
                ack_round_trip(host, server, correlation, command.kind, &command.payload)?
            else {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::HostRejected,
                ));
            };
            let envelope = decode_transport_envelope(&response)?;
            if decode_history_control(envelope.kind, envelope.role, envelope.payload).ok()
                != Some(HistoryControl::HistoryObservedResponse(observed))
            {
                return Err(UnexpectedAnswer::of(&response).into());
            }
            Ok(TerminalViewerOutcome::FeedbackAccepted)
        }
        TerminalViewerAction::SelectionGesture {
            generation,
            gesture,
        } => {
            if !host.viewers.borrow().open(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            selection_changed_exchange(
                host,
                server,
                correlation,
                id,
                &SelectionControl::Gesture(SelectionGesture {
                    action: selection_gesture_action(gesture),
                    attachment_id: id,
                    generation,
                }),
            )
        }
        TerminalViewerAction::SelectionCommand {
            generation,
            revision,
            command,
        } => {
            if !host.viewers.borrow().open(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            selection_changed_exchange(
                host,
                server,
                correlation,
                id,
                &SelectionControl::Command(SelectionCommand {
                    attachment_id: id,
                    command: selection_command_kind(command),
                    generation,
                    revision,
                }),
            )
        }
        TerminalViewerAction::SelectionExport {
            export_sequence,
            format,
        } => {
            if !host.viewers.borrow().open(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            let format = match format {
                ViewerSelectionExportFormat::Plain => SelectionExportFormat::Plain,
            };
            let command = encode_selection_export_control(&SelectionExportControl::ExportRequest(
                SelectionExport {
                    attachment_id: id,
                    export_sequence,
                    format,
                },
            ))?;
            let response = match selection_round_trip(
                host,
                server,
                correlation,
                command.kind,
                &command.payload,
            )? {
                SelectionReply::Answer(response) => response,
                SelectionReply::Failure(response) => {
                    return Ok(TerminalViewerOutcome::Record(response));
                }
            };
            let envelope = decode_transport_envelope(&response)?;
            let Ok(SelectionExportControl::StartedResponse(started)) =
                decode_selection_export_control(envelope.kind, envelope.role, envelope.payload)
            else {
                return Err(UnexpectedAnswer::of(&response).into());
            };
            if started.attachment_id != id
                || started.export_sequence != export_sequence
                || started.format != format
            {
                return Err(UnexpectedAnswer::of(&response).into());
            }
            Ok(TerminalViewerOutcome::Record(response))
        }
        TerminalViewerAction::SelectionExportChunkApplied {
            export_sequence,
            chunk_sequence,
        } => {
            if !host.viewers.borrow().open(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            let applied = SelectionExportChunkApplied {
                attachment_id: id,
                chunk_sequence,
                export_sequence,
            };
            let command = encode_selection_export_control(
                &SelectionExportControl::ChunkAppliedRequest(applied),
            )?;
            let response = match selection_round_trip(
                host,
                server,
                correlation,
                command.kind,
                &command.payload,
            )? {
                SelectionReply::Answer(response) => response,
                SelectionReply::Failure(response) => {
                    return Ok(TerminalViewerOutcome::Record(response));
                }
            };
            let envelope = decode_transport_envelope(&response)?;
            if decode_selection_export_control(envelope.kind, envelope.role, envelope.payload).ok()
                != Some(SelectionExportControl::ChunkAppliedResponse(applied))
            {
                return Err(UnexpectedAnswer::of(&response).into());
            }
            Ok(TerminalViewerOutcome::FeedbackAccepted)
        }
        TerminalViewerAction::SelectionExportCancel {
            export_sequence,
            reason,
        } => {
            if !host.viewers.borrow().open(request.attachment_id) {
                return Ok(TerminalViewerOutcome::Rejected(
                    ViewerRejection::AttachmentUnavailable,
                ));
            }
            let reason = match reason {
                ViewerSelectionExportCancelReason::ClipboardFailed => {
                    SelectionExportCancelReason::ClipboardFailed
                }
                ViewerSelectionExportCancelReason::Disposed => {
                    SelectionExportCancelReason::Disposed
                }
                ViewerSelectionExportCancelReason::Replaced => {
                    SelectionExportCancelReason::Replaced
                }
                ViewerSelectionExportCancelReason::User => SelectionExportCancelReason::User,
            };
            let command = encode_selection_export_control(&SelectionExportControl::CancelRequest(
                SelectionExportCancel {
                    attachment_id: id,
                    export_sequence,
                    reason,
                },
            ))?;
            let response = match selection_round_trip(
                host,
                server,
                correlation,
                command.kind,
                &command.payload,
            )? {
                SelectionReply::Answer(response) => response,
                SelectionReply::Failure(response) => {
                    return Ok(TerminalViewerOutcome::Record(response));
                }
            };
            let envelope = decode_transport_envelope(&response)?;
            let Ok(SelectionExportControl::FinishedResponse(finished)) =
                decode_selection_export_control(envelope.kind, envelope.role, envelope.payload)
            else {
                return Err(UnexpectedAnswer::of(&response).into());
            };
            if finished.attachment_id != id
                || finished.export_sequence != export_sequence
                || finished.status != SelectionExportStatus::Cancelled
            {
                return Err(UnexpectedAnswer::of(&response).into());
            }
            Ok(TerminalViewerOutcome::Record(response))
        }
        TerminalViewerAction::FrameApplied { .. } | TerminalViewerAction::NeedSnapshot { .. } => {
            // The exchange starts, and pending events drain, before feedback state changes, so a
            // feedback the request had no room for leaves that state as it was.
            let mut feedback_exchange = HostExchange::open(host, server)?;
            feedback_exchange
                .prepare()
                .map_err(|class| feedback_exchange.error(class))?;
            let admission = host.viewers.borrow_mut().feedback(&request);
            if let Err(reason) = admission {
                return Ok(TerminalViewerOutcome::Rejected(reason));
            }
            let (command, expected) = match request.action {
                TerminalViewerAction::FrameApplied {
                    sequence,
                    generation,
                    revision,
                } => {
                    let feedback = FrameApplied {
                        attachment_id: id,
                        sequence,
                        generation,
                        revision,
                    };
                    (
                        ProjectionFeedbackControl::FrameAppliedRequest(feedback),
                        ProjectionFeedbackControl::FrameAppliedResponse(feedback),
                    )
                }
                TerminalViewerAction::NeedSnapshot { sequence, reason } => {
                    let feedback = NeedSnapshot {
                        attachment_id: id,
                        sequence,
                        reason: snapshot_reason(reason),
                    };
                    (
                        ProjectionFeedbackControl::NeedSnapshotRequest(feedback),
                        ProjectionFeedbackControl::NeedSnapshotResponse(feedback),
                    )
                }
                // The enclosing path selects a viewer feedback subset; other actions require their own exchanges.
                _ => unreachable!("feedback action was matched"),
            };
            let command = encode_projection_feedback_control(&command)?;
            let response = feedback_exchange.request(
                correlation,
                command.kind,
                &command.payload,
                PayloadEncoding::ControlJson,
            )?;
            let envelope = decode_transport_envelope(&response)?;
            if decode_projection_feedback_control(envelope.kind, envelope.role, envelope.payload)
                .ok()
                != Some(expected)
            {
                return Err(UnexpectedAnswer::of(&response).into());
            }
            Ok(TerminalViewerOutcome::FeedbackAccepted)
        }
    }
}

/// The Host's answer to a viewer Detach.
enum DetachAnswer {
    Detached,
    /// A correlated refusal of this kind; the attachment counts as detached.
    Refused(u16),
    /// Another correlated answer; the attachment's fate is unknown.
    Unexpected(UnexpectedAnswer),
}

/// Detaches one viewer attachment from the Host. Only a failed exchange is an error.
fn detach_attachment(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    id: AttachmentId,
) -> Result<DetachAnswer, Box<dyn Error>> {
    let target = AttachmentTarget { attachment_id: id };
    let command = encode_attachment_control(&AttachmentControl::DetachTerminal(target))?;
    let response = match exchange(
        host,
        server,
        correlation,
        command.kind,
        &command.payload,
        PayloadEncoding::ControlJson,
    ) {
        Ok(response) => response,
        Err(error) => {
            return error
                .downcast::<UnexpectedAnswer>()
                .map(|answer| DetachAnswer::Unexpected(*answer));
        }
    };
    let envelope = decode_transport_envelope(&response)?;
    if matches!(
        decode_connection_control(envelope.kind, envelope.role, envelope.payload),
        Ok(ConnectionControl::TypedFailure(_))
    ) {
        return Ok(DetachAnswer::Refused(envelope.kind));
    }
    Ok(
        match decode_attachment_control(envelope.kind, envelope.role, envelope.payload) {
            Ok(AttachmentControl::DetachTerminalAccepted(accepted)) if accepted == target => {
                DetachAnswer::Detached
            }
            _ => DetachAnswer::Unexpected(UnexpectedAnswer::of(&response)),
        },
    )
}

/// Detaches a viewer this Worker no longer uses and forgets it. A Detach the request has no room
/// for is left to a later cleanup, the viewer closed meanwhile.
fn retire_viewer(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    attachment: orchbee_capability_node_protocol::OperationId,
) -> Result<(), Box<dyn Error>> {
    let id = AttachmentId::from_bytes(attachment.as_bytes())?;
    if NotStarted::optional(detach_viewer(host, server, correlation, id))?.is_some() {
        host.forget_viewer(attachment);
    } else {
        host.viewers.borrow_mut().close(attachment);
    }
    Ok(())
}

/// Detaches a viewer for its own request: a refusal counts as detached and is recorded, and
/// another answer is the request's unexpected answer.
fn detach_viewer(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    id: AttachmentId,
) -> Result<(), Box<dyn Error>> {
    match detach_attachment(host, server, correlation, id)? {
        DetachAnswer::Detached => Ok(()),
        DetachAnswer::Refused(kind) => {
            crate::diagnostic_events::host_unexpected(
                host.pacing.site(),
                kind,
                crate::diagnostic_events::Anomaly::Answer,
            );
            Ok(())
        }
        DetachAnswer::Unexpected(answer) => Err(answer.into()),
    }
}

const fn selection_gesture_action(gesture: ViewerSelectionGestureAction) -> SelectionGestureAction {
    match gesture {
        ViewerSelectionGestureAction::Start {
            click_count,
            point,
            position,
            select_output,
        } => SelectionGestureAction::Start {
            click_count,
            point: SelectionViewportPoint {
                column: point.column,
                row: point.row,
            },
            position: SelectionGesturePosition {
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
        } => SelectionGestureAction::Update {
            geometry: SelectionGestureGeometry {
                cell_width: geometry.cell_width,
                columns: geometry.columns,
                padding_left: geometry.padding_left,
                screen_height: geometry.screen_height,
            },
            point: SelectionViewportPoint {
                column: point.column,
                row: point.row,
            },
            position: SelectionGesturePosition {
                x: position.x,
                y: position.y,
            },
            rectangle,
        },
        ViewerSelectionGestureAction::End { point } => SelectionGestureAction::End {
            point: SelectionViewportPoint {
                column: point.column,
                row: point.row,
            },
        },
        ViewerSelectionGestureAction::Cancel => SelectionGestureAction::Cancel,
    }
}

const fn selection_point(point: ViewerSelectionPoint) -> SelectionPoint {
    SelectionPoint {
        column: point.column,
        row: point.row,
    }
}

const fn selection_command_kind(command: ViewerSelectionCommand) -> SelectionCommandKind {
    match command {
        ViewerSelectionCommand::Clear => SelectionCommandKind::Clear,
        ViewerSelectionCommand::SelectAll => SelectionCommandKind::SelectAll,
        ViewerSelectionCommand::SetRange {
            end,
            rectangle,
            start,
        } => SelectionCommandKind::SetRange {
            end: selection_point(end),
            rectangle,
            start: selection_point(start),
        },
        ViewerSelectionCommand::SelectOutput { point } => SelectionCommandKind::SelectOutput {
            point: selection_point(point),
        },
    }
}

const fn snapshot_reason(reason: ViewerSnapshotReason) -> SnapshotReason {
    match reason {
        ViewerSnapshotReason::BaseMismatch => SnapshotReason::BaseMismatch,
        ViewerSnapshotReason::DecoderRejected => SnapshotReason::DecoderRejected,
        ViewerSnapshotReason::DimensionsMismatch => SnapshotReason::DimensionsMismatch,
        ViewerSnapshotReason::GenerationMismatch => SnapshotReason::GenerationMismatch,
        ViewerSnapshotReason::IdentityMismatch => SnapshotReason::IdentityMismatch,
        ViewerSnapshotReason::SequenceGap => SnapshotReason::SequenceGap,
        ViewerSnapshotReason::StoreReset => SnapshotReason::StoreReset,
    }
}

/// A row read needs an admitted, open viewer; unknown or closed attachments are
/// the existing unavailability, never a row rejection. The answer is either the
/// correlated row-window record or a typed failure mapped to a rows rejection.
fn read_rows(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    attachment: orchbee_capability_node_protocol::OperationId,
    read: ReadRows,
) -> Result<TerminalViewerOutcome, Box<dyn Error>> {
    if !host.viewers.borrow().open(attachment) {
        return Ok(TerminalViewerOutcome::Rejected(
            ViewerRejection::AttachmentUnavailable,
        ));
    }
    // The effective budget is the smallest of the requested bound, the
    // negotiated row-window limit, and what the viewer record carrier can
    // hold after its envelope header.
    let carrier_payload =
        u32::try_from(MAXIMUM_VIEWER_RECORD_BYTES.saturating_sub(TRANSPORT_ENVELOPE_HEADER_BYTES))
            .unwrap_or(u32::MAX);
    let effective_maximum = read
        .maximum_bytes
        .min(host.negotiated_limits().row_window_bytes)
        .min(carrier_payload);
    let command = encode_row_access_control(&RowAccessControl::ReadRows(ReadRows {
        maximum_bytes: effective_maximum,
        ..read
    }))?;
    let response = exchange_rows(
        host,
        server,
        correlation,
        &command,
        TRANSPORT_ENVELOPE_HEADER_BYTES.saturating_add(effective_maximum as usize),
    )?;
    let envelope = decode_transport_envelope(&response)?;
    if envelope.payload_encoding == PayloadEncoding::RowWindow {
        // The bounded exchange already proved the response record fits
        // the effective budget; confirm the payload length itself.
        if envelope.payload.len() > effective_maximum as usize {
            return Err(UnexpectedAnswer::of(&response).into());
        }
        return Ok(TerminalViewerOutcome::Record(response));
    }
    let Ok(ConnectionControl::TypedFailure(failure)) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)
    else {
        return Err(UnexpectedAnswer::of(&response).into());
    };
    let Some(reason) = rows_rejection_reason(&failure.code) else {
        return Err(UnexpectedAnswer::of(&response).into());
    };
    Ok(TerminalViewerOutcome::RowsRejected(reason))
}

/// A control acknowledgement round trip answers either the control family's
/// response or a typed failure; the caller maps the failure to host-rejected.
fn ack_round_trip(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    kind: u16,
    payload: &[u8],
) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    let response = exchange(
        host,
        server,
        correlation,
        kind,
        payload,
        PayloadEncoding::ControlJson,
    )?;
    let envelope = decode_transport_envelope(&response)?;
    if matches!(
        decode_connection_control(envelope.kind, envelope.role, envelope.payload),
        Ok(ConnectionControl::TypedFailure(_))
    ) {
        return Ok(None);
    }
    Ok(Some(response))
}

/// The Host's answer to a demand change.
pub(crate) enum DemandAnswer {
    Accepted,
    /// A correlated typed failure of this kind: the attachment is gone or cannot take the demand.
    Refused(u16),
}

/// Sets an attachment's demand; another correlated answer is an [`UnexpectedAnswer`].
pub(crate) fn set_attachment_demand(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    attachment_id: AttachmentId,
    demand: ProjectionDemand,
) -> Result<DemandAnswer, Box<dyn Error>> {
    let command = encode_projection_demand_control(&ProjectionDemandControl::SetProjectionDemand(
        SetProjectionDemand {
            attachment_id,
            demand,
        },
    ))?;
    let response = exchange(
        host,
        server,
        correlation,
        command.kind,
        &command.payload,
        PayloadEncoding::ControlJson,
    )?;
    let envelope = decode_transport_envelope(&response)?;
    if matches!(
        decode_connection_control(envelope.kind, envelope.role, envelope.payload),
        Ok(ConnectionControl::TypedFailure(_))
    ) {
        return Ok(DemandAnswer::Refused(envelope.kind));
    }
    let Ok(ProjectionDemandControl::ProjectionDemandAccepted(accepted)) =
        decode_projection_demand_control(envelope.kind, envelope.role, envelope.payload)
    else {
        return Err(UnexpectedAnswer::of(&response).into());
    };
    if accepted.attachment_id != attachment_id || accepted.demand != demand {
        return Err(UnexpectedAnswer::of(&response).into());
    }
    Ok(DemandAnswer::Accepted)
}

enum SelectionReply {
    Answer(Vec<u8>),
    Failure(Vec<u8>),
}

// Selection cancellation needs the Host failure code. Preserve that exact
// correlated response in the existing opaque record carrier; other control
// families retain their established host-rejected mapping.
fn selection_round_trip(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    kind: u16,
    payload: &[u8],
) -> Result<SelectionReply, Box<dyn Error>> {
    let response = exchange(
        host,
        server,
        correlation,
        kind,
        payload,
        PayloadEncoding::ControlJson,
    )?;
    let envelope = decode_transport_envelope(&response)?;
    if matches!(
        decode_connection_control(envelope.kind, envelope.role, envelope.payload),
        Ok(ConnectionControl::TypedFailure(_))
    ) {
        Ok(SelectionReply::Failure(response))
    } else {
        Ok(SelectionReply::Answer(response))
    }
}

/// A gesture and a command both answer with this viewer's selection, or with the Host's refusal.
fn selection_changed_exchange(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    id: AttachmentId,
    control: &SelectionControl,
) -> Result<TerminalViewerOutcome, Box<dyn Error>> {
    let encoded = encode_selection_control(control)?;
    let response =
        match selection_round_trip(host, server, correlation, encoded.kind, &encoded.payload)? {
            SelectionReply::Answer(response) => response,
            SelectionReply::Failure(response) => {
                return Ok(TerminalViewerOutcome::Record(response));
            }
        };
    let envelope = decode_transport_envelope(&response)?;
    let Ok(SelectionControl::ChangedResponse(changed)) =
        decode_selection_control(envelope.kind, envelope.role, envelope.payload)
    else {
        return Err(UnexpectedAnswer::of(&response).into());
    };
    if changed.attachment_id != id {
        return Err(UnexpectedAnswer::of(&response).into());
    }
    Ok(TerminalViewerOutcome::Record(response))
}

fn rows_rejection_reason(code: &str) -> Option<ViewerRowsRejectedReason> {
    Some(match code {
        "stale-revision" => ViewerRowsRejectedReason::StaleRevision,
        "stale-generation" => ViewerRowsRejectedReason::StaleGeneration,
        "history-evicted" => ViewerRowsRejectedReason::HistoryEvicted,
        "missing-resource" => ViewerRowsRejectedReason::MissingResource,
        "invalid-field" => ViewerRowsRejectedReason::InvalidField,
        "invalid-acknowledgement" => ViewerRowsRejectedReason::InvalidAcknowledgement,
        "limit-exceeded" => ViewerRowsRejectedReason::LimitExceeded,
        "resource-exhausted" => ViewerRowsRejectedReason::ResourceExhausted,
        "invalid-phase" => ViewerRowsRejectedReason::InvalidPhase,
        _ => return None,
    })
}

/// A row read accepts either the correlated RowWindow response or a control
/// TypedFailure; every other kind, encoding, or role is a Host violation.
fn exchange_rows(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    command: &EncodedRowAccessControl,
    maximum_record_bytes: usize,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let expected = message_contract(command.kind)
        .and_then(|contract| contract.request)
        .map(|request| request.success_kind)
        .ok_or_else(|| invalid("Host row request kind has no response contract"))?;
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(correlation),
        host_epoch: Some(host.host_epoch()),
        kind: command.kind,
        optional_message: false,
        payload: &command.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Request,
    })?;
    let mut exchange = HostExchange::open(host, server)?;
    exchange.prepare().map_err(|class| exchange.error(class))?;
    let record = exchange
        .round_trip_bounded(&record, maximum_record_bytes)
        .map_err(|class| exchange.error(class))?;
    let envelope = response_envelope(&record, correlation, host.host_epoch())?;
    let row_window =
        envelope.kind == expected && envelope.payload_encoding == PayloadEncoding::RowWindow;
    let typed_failure = envelope.payload_encoding == PayloadEncoding::ControlJson
        && matches!(
            decode_connection_control(envelope.kind, envelope.role, envelope.payload),
            Ok(ConnectionControl::TypedFailure(_))
        );
    if !(row_window || typed_failure) {
        return Err(UnexpectedAnswer::of(&record).into());
    }
    Ok(record)
}
