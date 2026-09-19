//! The single read-only Site query: existing-only rendezvous, one OBLC
//! admission, one snapshot read, then the session is dropped. Nothing here may
//! create directories, acquire locks, start processes, or mutate the Workbench.

#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};
#[cfg(target_os = "linux")]
use std::{env, path::PathBuf};

#[cfg(target_os = "linux")]
use orchbee_client_server_protocol::{
    RequestId as ObcsRequestId, SiteId as ObcsSiteId, TerminalId as ObcsTerminalId,
    TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, ViewerId,
    ViewerRejectedReason, WorkerId as ObcsWorkerId,
};
#[cfg(target_os = "linux")]
use orchbee_local_client_protocol::ConnectClientError;
#[cfg(target_os = "linux")]
use orchbee_local_site_runtime::{
    ModeCheckedLocalSiteRendezvous, NativeCohort, RendezvousError, SiteScope,
};
#[cfg(target_os = "linux")]
use orchbee_terminal_protocol::{AttachmentId, HostEpoch, TerminalTextFrame};
#[cfg(target_os = "linux")]
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchRequestId, WorkbenchSnapshotOutcome,
};

#[cfg(target_os = "linux")]
use super::results::Projection;
#[cfg(target_os = "linux")]
use super::{Failure, FailureKind, stage_details, stage_reason_details, target_details};
#[cfg(target_os = "linux")]
use crate::invocation::{Resource, Target};
#[cfg(target_os = "linux")]
use crate::local_client::workbench::{SnapshotReadError, SnapshotReadFailure};
#[cfg(target_os = "linux")]
use crate::local_client::{LocalClient, OutcomeUnknownReason, SubmissionError};
#[cfg(target_os = "linux")]
use crate::terminal_projection::TerminalProjection;

#[cfg(target_os = "linux")]
const COHORT_BUDGET: Duration = Duration::from_secs(5);
#[cfg(target_os = "linux")]
const NETWORK_BUDGET: Duration = Duration::from_secs(5);
/// An `Empty` poll answer is retried inside the shared network budget; the
/// cadence stays coarse enough that a quiet host is never hammered.
#[cfg(target_os = "linux")]
const EMPTY_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// One admitted Client Session plus the shared network deadline every
/// request on it must finish before. The caller owns the session's drop.
#[cfg(target_os = "linux")]
pub(crate) fn read_snapshot() -> Result<Snapshot, Failure> {
    let (client, _deadline, snapshot) = snapshot_session()?;
    // The Client Session is fully dropped before the snapshot is examined or
    // any output is prepared.
    drop(client);
    Ok(snapshot)
}

/// `terminal read`: the snapshot resolves the typed binding, then one
/// read-capability viewer attachment on the same session and deadline
/// observes the terminal's current projection — or, under `--history`, the
/// retained scrollback window behind it. The attachment is released
/// (best-effort `Detach` plus the session drop) before any output work.
#[cfg(target_os = "linux")]
pub(crate) fn read_terminal(
    target: &Target,
    history: Option<u16>,
) -> Result<(serde_json::Value, Projection), Failure> {
    let (mut client, deadline, snapshot) = snapshot_session()?;
    let source = super::results::snapshot_source(&snapshot);
    let outcome = match history {
        None => read_terminal_frame(&mut client, deadline, &snapshot, target)
            .map(|(row, frame)| Observed::Visible(row, frame)),
        Some(rows) => read_terminal_history(&mut client, deadline, &snapshot, target, rows).map(
            |(row, frame, window)| Observed::History {
                row,
                frame,
                window,
                requested: rows,
            },
        ),
    };
    // The Client Session is fully dropped before the applied frame or the
    // history window is mapped into the bounded document or any output is
    // prepared.
    drop(client);
    match outcome {
        Ok(Observed::Visible(row, frame)) => super::results::terminal_read_doc(row, &frame)
            .map(|doc| (source.clone(), Projection::TerminalRead(doc)))
            .map_err(|failure| failure.source(source)),
        Ok(Observed::History {
            row,
            frame,
            window,
            requested,
        }) => super::results::terminal_history_doc(row, &frame, requested, window.as_ref())
            .map(|doc| (source.clone(), Projection::TerminalHistory(doc)))
            .map_err(|failure| failure.source(source)),
        // Every failure after this point had a validated snapshot: the
        // failure envelope may honestly cite it as the source.
        Err(failure) => Err(failure.source(source)),
    }
}

/// The shared preamble for every network query and mutation: cohort,
/// existing-only rendezvous, socket validation, one admission, one snapshot
/// read — all under the single network deadline. On success the caller owns
/// the still admitted session; on any failure the session is already dropped.
#[cfg(target_os = "linux")]
pub(crate) fn snapshot_session() -> Result<(LocalClient, Instant, Snapshot), Failure> {
    let cohort_deadline = Instant::now()
        .checked_add(COHORT_BUDGET)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    let cohort = NativeCohort::current(cohort_deadline)
        .map_err(|_| Failure::new(FailureKind::CheckFailed).details(stage_details("cohort")))?;
    let site = rendezvous()?;
    match site.client_socket_exists() {
        Ok(true) => {}
        Ok(false) => {
            return Err(
                Failure::new(FailureKind::ServiceUnavailable).details(stage_details("discovery"))
            );
        }
        Err(_) => {
            return Err(Failure::new(FailureKind::CheckFailed).details(stage_details("socket")));
        }
    }
    let deadline = Instant::now()
        .checked_add(NETWORK_BUDGET)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    let mut client =
        LocalClient::connect_before(site.client_socket_path(), cohort.identity(), deadline)
            .map_err(classify_connect)?;
    let intent = SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: request_id()?,
    };
    let result = match client.submit_workbench_snapshot_read_before(intent, deadline) {
        Err(error) => {
            drop(client);
            return Err(classify_read(error));
        }
        Ok(result) => result,
    };
    match result.outcome {
        WorkbenchSnapshotOutcome::Available { snapshot } => Ok((client, deadline, snapshot)),
        WorkbenchSnapshotOutcome::Denied {} => {
            drop(client);
            Err(Failure::new(FailureKind::Denied))
        }
        WorkbenchSnapshotOutcome::Unavailable { .. } => {
            drop(client);
            Err(Failure::new(FailureKind::ServiceUnavailable).details(stage_details("snapshot")))
        }
    }
}

/// What the terminal observation produced: the applied visible frame, or the
/// validated scrollback window behind it.
#[cfg(target_os = "linux")]
enum Observed {
    Visible(super::results::TerminalRow, TerminalTextFrame),
    History {
        row: super::results::TerminalRow,
        frame: TerminalTextFrame,
        window: Option<orchbee_terminal_protocol::TerminalTextRowWindow>,
        requested: u16,
    },
}

/// Resolve the typed binding from the snapshot, attach a read-only viewer to
/// it, and drive attach → poll → feedback → detach inside the same deadline.
/// The applied frame leaves the session so the caller can map it only after
/// the transport is dropped; nothing here creates terminals, controllers,
/// processes, or viewer state beyond this explicit read observation.
#[cfg(target_os = "linux")]
fn read_terminal_frame(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    target: &Target,
) -> Result<(super::results::TerminalRow, TerminalTextFrame), Failure> {
    let (row, viewer_id, _attachment, _epoch, projection) =
        attach_viewer(client, deadline, snapshot, target)?;
    let outcome = poll_terminal_frame(client, deadline, &viewer_id, projection);
    detach_viewer(client, deadline, viewer_id);
    outcome.map(|frame| (row, frame))
}

/// `--history`: the same attach → poll preamble pins the frame's retained
/// scrollback count, then exactly one `ReadRows` asks for the rows that have
/// scrolled off the active screen. `K = min(requested, scrollback,
/// MAXIMUM_FRAME_CELLS / columns)` keeps the absolute start at the recent
/// end; `scrollback == 0` short-circuits before any Host request. No
/// `RowsApplied` is sent: a buffer read creates no pending anchor.
#[cfg(target_os = "linux")]
fn read_terminal_history(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    target: &Target,
    requested: u16,
) -> Result<
    (
        super::results::TerminalRow,
        TerminalTextFrame,
        Option<orchbee_terminal_protocol::TerminalTextRowWindow>,
    ),
    Failure,
> {
    let (row, viewer_id, attachment, epoch, projection) =
        attach_viewer(client, deadline, snapshot, target)?;
    let outcome = poll_terminal_frame(client, deadline, &viewer_id, projection).and_then(|frame| {
        read_history_window(
            client, deadline, &viewer_id, attachment, epoch, &frame, requested,
        )
        .map(|window| (frame, window))
    });
    detach_viewer(client, deadline, viewer_id);
    outcome.map(|(frame, window)| (row, frame, window))
}

/// Resolve the typed binding from the snapshot and attach a read-only viewer
/// to it inside the shared deadline.
#[cfg(target_os = "linux")]
fn attach_viewer(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    target: &Target,
) -> Result<
    (
        super::results::TerminalRow,
        ViewerId,
        AttachmentId,
        HostEpoch,
        TerminalProjection,
    ),
    Failure,
> {
    let identity = match target {
        Target::Id(identity) => identity.as_str(),
        // The parser admits only typed IDs for terminal targets.
        Target::Label(_) => return Err(Failure::new(FailureKind::InternalFailure)),
    };
    let row = super::results::terminal_binding(snapshot, identity).ok_or_else(|| {
        Failure::new(FailureKind::NotFound).details(target_details(Resource::Terminal, identity))
    })?;
    let attach = TerminalViewerRequest::new(
        obcs_request_id()?,
        TerminalViewerAction::AttachBinding {
            site_id: ObcsSiteId::parse(snapshot.site_id.as_str())
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
            worker_id: ObcsWorkerId::parse(row.worker_id.as_str())
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
            terminal_id: ObcsTerminalId::parse(row.terminal_id.as_str())
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
        },
    );
    let result = client
        .submit_terminal_viewer_before(attach, deadline)
        .map_err(classify_viewer)?;
    match result.outcome() {
        TerminalViewerOutcome::Attached {
            attachment_id,
            host_epoch,
        } => {
            let attachment = AttachmentId::parse(attachment_id.as_str())
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
            let epoch = HostEpoch::parse(host_epoch.as_str())
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
            Ok((
                row,
                attachment_id.clone(),
                attachment,
                epoch,
                TerminalProjection::new(attachment, epoch),
            ))
        }
        TerminalViewerOutcome::Rejected { reason } => Err(classify_viewer_rejected(*reason)),
        TerminalViewerOutcome::Unavailable { .. } => Err(viewer_unavailable()),
        _ => Err(viewer_protocol_invalid()),
    }
}

/// The read observation is released no matter how the exchange ended; the
/// detach is best-effort because the session drop that follows already
/// guarantees teardown.
#[cfg(target_os = "linux")]
fn detach_viewer(client: &mut LocalClient, deadline: Instant, viewer_id: ViewerId) {
    if let Ok(request_id) = obcs_request_id() {
        let _ = client.submit_terminal_viewer_before(
            TerminalViewerRequest::new(
                request_id,
                TerminalViewerAction::Detach {
                    attachment_id: viewer_id,
                },
            ),
            deadline,
        );
    }
}

/// Issue the single Host `ReadRows` behind the validated frame and decode its
/// answer. The effective window is `min(requested, scrollback, capacity)`
/// rows ending at the scrollback boundary; a `RowsRejected` maps to its
/// typed failure and any other outcome — including an `Empty` stand-in — is
/// protocol-invalid. Exactly one request is ever sent: no retry.
#[cfg(target_os = "linux")]
fn read_history_window(
    client: &mut LocalClient,
    deadline: Instant,
    viewer_id: &ViewerId,
    attachment: AttachmentId,
    epoch: HostEpoch,
    frame: &TerminalTextFrame,
    requested: u16,
) -> Result<Option<orchbee_terminal_protocol::TerminalTextRowWindow>, Failure> {
    let scrollback = frame.scrollback_rows;
    if scrollback == 0 {
        return Ok(None);
    }
    // The frame geometry was validated by the OBTF decoder: columns ≥ 1.
    let capacity = u64::try_from(orchbee_terminal_protocol::MAXIMUM_FRAME_CELLS)
        .unwrap_or(u64::MAX)
        / u64::from(frame.geometry.columns);
    let count = u64::from(requested).min(scrollback).min(capacity);
    let start_row = scrollback - count;
    let row_count = u16::try_from(count).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let request = TerminalViewerRequest::new(
        obcs_request_id()?,
        TerminalViewerAction::ReadRows {
            attachment_id: viewer_id.clone(),
            start_row,
            row_count,
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
        TerminalViewerOutcome::Record(record) => {
            decode_history_window(record.clone(), attachment, epoch, frame, start_row, count)
                .map(Some)
        }
        TerminalViewerOutcome::RowsRejected { reason } => Err(classify_rows_rejected(*reason)),
        TerminalViewerOutcome::Rejected { reason } => Err(classify_viewer_rejected(*reason)),
        TerminalViewerOutcome::Unavailable { .. } => Err(viewer_unavailable()),
        _ => Err(viewer_protocol_invalid()),
    }
}

/// Independently validate the complete OBTP response record and its OBRW
/// payload against the request and the frame that priced it: envelope role
/// and encoding, Host epoch, and every window field — attachment, sequence,
/// revision, generation, columns, start, count, and the frame-derived total.
#[cfg(target_os = "linux")]
fn decode_history_window(
    record: Vec<u8>,
    attachment: AttachmentId,
    epoch: HostEpoch,
    frame: &TerminalTextFrame,
    start_row: u64,
    count: u64,
) -> Result<orchbee_terminal_protocol::TerminalTextRowWindow, Failure> {
    let envelope = orchbee_terminal_protocol::decode_transport_envelope(&record)
        .map_err(|_| viewer_protocol_invalid())?;
    if envelope.protocol != orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION
        || envelope.role != orchbee_terminal_protocol::MessageRole::Response
        || envelope.optional_message
        || envelope.payload_encoding != orchbee_terminal_protocol::PayloadEncoding::RowWindow
        || envelope.host_epoch != Some(epoch)
        || envelope.payload.len()
            > usize::try_from(orchbee_client_server_protocol::MAXIMUM_READ_ROWS_BYTES)
                .unwrap_or(usize::MAX)
    {
        return Err(viewer_protocol_invalid());
    }
    let window = orchbee_terminal_protocol::decode_terminal_text_row_window(envelope.payload)
        .map_err(|_| viewer_protocol_invalid())?;
    let total_rows = frame
        .scrollback_rows
        .checked_add(u64::from(frame.geometry.rows))
        .ok_or_else(viewer_protocol_invalid)?;
    if window.attachment_id != attachment
        || window.host_epoch != epoch
        || window.sequence != std::num::NonZeroU64::MIN
        || window.revision != frame.route.revision
        || window.generation != frame.route.generation
        || window.columns != frame.geometry.columns
        || window.start_row != start_row
        || u64::try_from(window.rows.len()).unwrap_or(u64::MAX) != count
        || window.total_rows != total_rows
    {
        return Err(viewer_protocol_invalid());
    }
    Ok(window)
}

/// A typed `rows-rejected` answer maps to its fixed failure class: the local
/// budget overrun is a rejection, Host state answers are unavailability, and
/// reasons our fixed request shape cannot honestly produce are protocol
/// violations.
#[cfg(target_os = "linux")]
fn classify_rows_rejected(
    reason: orchbee_client_server_protocol::ViewerRowsRejectedReason,
) -> Failure {
    use orchbee_client_server_protocol::ViewerRowsRejectedReason as Reason;
    let (kind, detail) = match reason {
        Reason::LimitExceeded => (FailureKind::Rejected, Some("limit-exceeded")),
        Reason::StaleRevision => (FailureKind::ServiceUnavailable, Some("stale-revision")),
        Reason::StaleGeneration => (FailureKind::ServiceUnavailable, Some("stale-generation")),
        Reason::HistoryEvicted => (FailureKind::ServiceUnavailable, Some("history-evicted")),
        Reason::MissingResource => (FailureKind::ServiceUnavailable, Some("missing-resource")),
        Reason::ResourceExhausted => (FailureKind::ServiceUnavailable, Some("resource-exhausted")),
        Reason::InvalidPhase => (FailureKind::ServiceUnavailable, Some("invalid-phase")),
        Reason::InvalidField | Reason::InvalidAcknowledgement => {
            (FailureKind::ProtocolInvalid, None)
        }
    };
    let failure = Failure::new(kind);
    match detail {
        Some(reason) => failure.details(history_details(reason)),
        None => failure.details(stage_details("viewer")),
    }
}

/// `read-history` failure details: the typed reason the Host carried, under
/// the viewer stage the read belongs to.
#[cfg(target_os = "linux")]
fn history_details(reason: &str) -> serde_json::Map<String, serde_json::Value> {
    serde_json::Map::from_iter([
        ("stage".to_owned(), serde_json::json!("viewer")),
        ("operation".to_owned(), serde_json::json!("read-history")),
        ("reason".to_owned(), serde_json::json!(reason)),
    ])
}

/// Poll until one record applies to the projection, acknowledging a rejected
/// record with `NeedSnapshot` recovery feedback. `Empty` answers are retried
/// under the shared deadline; there is no second connection, no retry of a
/// failed exchange, and no `FrameApplied` for a frame whose only consumer is
/// this read — `Detach` releases it.
#[cfg(target_os = "linux")]
fn poll_terminal_frame(
    client: &mut LocalClient,
    deadline: Instant,
    viewer_id: &ViewerId,
    mut projection: TerminalProjection,
) -> Result<TerminalTextFrame, Failure> {
    loop {
        let poll = TerminalViewerRequest::new(
            obcs_request_id()?,
            TerminalViewerAction::Poll {
                attachment_id: viewer_id.clone(),
            },
        );
        let result = client
            .submit_terminal_viewer_before(poll, deadline)
            .map_err(classify_viewer)?;
        match result.outcome() {
            TerminalViewerOutcome::Empty {} => std::thread::sleep(EMPTY_POLL_INTERVAL),
            TerminalViewerOutcome::Record(record) => {
                match projection.receive_for_geometry(record, None) {
                    Ok(Some(feedback)) => {
                        if matches!(feedback, TerminalViewerAction::FrameApplied { .. }) {
                            return projection
                                .take_frame()
                                .ok_or_else(|| Failure::new(FailureKind::InternalFailure));
                        }
                        let request = TerminalViewerRequest::new(obcs_request_id()?, feedback);
                        let result = client
                            .submit_terminal_viewer_before(request, deadline)
                            .map_err(classify_viewer)?;
                        match result.outcome() {
                            TerminalViewerOutcome::FeedbackAccepted {} => {}
                            TerminalViewerOutcome::Rejected { reason } => {
                                return Err(classify_viewer_rejected(*reason));
                            }
                            TerminalViewerOutcome::Unavailable { .. } => {
                                return Err(viewer_unavailable());
                            }
                            _ => return Err(viewer_protocol_invalid()),
                        }
                    }
                    // A consumed ProjectionBlocked keeps the loop polling for
                    // an applicable frame; a replayed record was already
                    // rejected above as protocol-invalid.
                    Ok(None) => {}
                    Err(()) => return Err(viewer_protocol_invalid()),
                }
            }
            TerminalViewerOutcome::Rejected { reason } => {
                return Err(classify_viewer_rejected(*reason));
            }
            TerminalViewerOutcome::Unavailable { .. } => return Err(viewer_unavailable()),
            _ => return Err(viewer_protocol_invalid()),
        }
    }
}

#[cfg(target_os = "linux")]
fn classify_viewer(error: SubmissionError) -> Failure {
    match error {
        SubmissionError::SessionUnavailable | SubmissionError::RequestEncoding(_) => {
            Failure::new(FailureKind::InternalFailure)
        }
        SubmissionError::RequestNotSubmitted(_) => {
            Failure::new(FailureKind::TransportLoss).details(stage_details("viewer"))
        }
        SubmissionError::OutcomeUnknown { reason, .. } => match reason {
            OutcomeUnknownReason::Write(_) | OutcomeUnknownReason::Read(_) => {
                Failure::new(FailureKind::TransportLoss).details(stage_details("viewer"))
            }
            OutcomeUnknownReason::CorrelationMismatch { .. } => {
                Failure::new(FailureKind::ProtocolInvalid)
                    .details(stage_reason_details("viewer", "correlation-mismatch"))
            }
            OutcomeUnknownReason::InvalidResult(_)
            | OutcomeUnknownReason::UnexpectedResult { .. } => {
                Failure::new(FailureKind::ProtocolInvalid).details(stage_details("viewer"))
            }
        },
    }
}

/// A rejected viewer action means the read observation could not be kept:
/// contention or a dead terminal is unavailability, while a feedback-state
/// contradiction is a protocol violation.
#[cfg(target_os = "linux")]
fn classify_viewer_rejected(reason: ViewerRejectedReason) -> Failure {
    match reason {
        ViewerRejectedReason::AlreadyAttached | ViewerRejectedReason::TerminalUnavailable => {
            viewer_unavailable()
        }
        ViewerRejectedReason::FeedbackMismatch => viewer_protocol_invalid(),
    }
}

#[cfg(target_os = "linux")]
fn viewer_unavailable() -> Failure {
    Failure::new(FailureKind::ServiceUnavailable).details(stage_details("viewer"))
}

#[cfg(target_os = "linux")]
fn viewer_protocol_invalid() -> Failure {
    Failure::new(FailureKind::ProtocolInvalid).details(stage_details("viewer"))
}

#[cfg(target_os = "linux")]
fn rendezvous() -> Result<ModeCheckedLocalSiteRendezvous, Failure> {
    let base = env::var_os("ORCHBEE_RUNTIME_BASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let scope_text = format!("orchbee-native-{}", nix::unistd::geteuid().as_raw());
    let scope =
        SiteScope::parse(&scope_text).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let scope_directory = base.join(&scope_text);
    ModeCheckedLocalSiteRendezvous::validate_mode_checked(&base, scope)
        .map_err(|error| classify_rendezvous(error, &scope_directory))
}

/// Split rendezvous failures by the typed path they carried: an absent or
/// unsafe Site scope belongs to `runtime-directory`, while anything rooted
/// above it belongs to `runtime-base`. A scope ENOENT is service-unavailable
/// discovery, never a configuration failure.
#[cfg(target_os = "linux")]
fn classify_rendezvous(error: RendezvousError, scope_directory: &std::path::Path) -> Failure {
    let path = rendezvous_path(&error);
    let stage = if path == Some(scope_directory) {
        "runtime-directory"
    } else {
        "runtime-base"
    };
    if let RendezvousError::System { source, .. } = &error
        && source.kind() == std::io::ErrorKind::NotFound
        && path == Some(scope_directory)
    {
        return Failure::new(FailureKind::ServiceUnavailable).details(stage_details("discovery"));
    }
    Failure::new(FailureKind::CheckFailed).details(stage_details(stage))
}

#[cfg(target_os = "linux")]
fn rendezvous_path(error: &RendezvousError) -> Option<&std::path::Path> {
    match error {
        RendezvousError::InvalidSiteScope => None,
        RendezvousError::RuntimeBaseNotAbsolute { path }
        | RendezvousError::RuntimeBaseNotNormalized { path }
        | RendezvousError::DirectorySymlink { path }
        | RendezvousError::NotDirectory { path }
        | RendezvousError::NotRegularLockFile { path, .. }
        | RendezvousError::UnexpectedOwner { path, .. }
        | RendezvousError::UnexpectedMode { path, .. }
        | RendezvousError::UnexpectedLockLinkCount { path, .. }
        | RendezvousError::RuntimeDirectoryIdentityChanged { path }
        | RendezvousError::LockFileSymlink { path, .. }
        | RendezvousError::System { path, .. } => Some(path.as_path()),
    }
}

#[cfg(target_os = "linux")]
fn classify_connect(error: crate::local_client::LocalClientConnectError) -> Failure {
    use orchbee_local_client_protocol::{ProtocolError, RejectionReason};
    match error {
        crate::local_client::LocalClientConnectError::Admission(connect) => match connect {
            ConnectClientError::ConnectFailed(_) => {
                Failure::new(FailureKind::ServiceUnavailable).details(stage_details("admission"))
            }
            ConnectClientError::Io(_) => {
                Failure::new(FailureKind::TransportLoss).details(stage_details("admission"))
            }
            ConnectClientError::Rejected(reason) => match reason {
                RejectionReason::CapacityExceeded => {
                    Failure::new(FailureKind::SessionCapacity).details(stage_details("admission"))
                }
                RejectionReason::HandshakeTimeout => Failure::new(FailureKind::ServiceUnavailable)
                    .details(stage_details("admission")),
                RejectionReason::IncompatibleProtocol | RejectionReason::IncompatibleCohort => {
                    Failure::new(FailureKind::Incompatible).details(stage_details("admission"))
                }
                RejectionReason::WrongMessage | RejectionReason::MalformedFrame => {
                    Failure::new(FailureKind::ProtocolInvalid).details(stage_details("admission"))
                }
            },
            ConnectClientError::Protocol(ProtocolError::IncompatibleVersion { .. }) => {
                Failure::new(FailureKind::Incompatible).details(stage_details("admission"))
            }
            ConnectClientError::Protocol(_) | ConnectClientError::UnexpectedResponse => {
                Failure::new(FailureKind::ProtocolInvalid).details(stage_details("admission"))
            }
        },
        crate::local_client::LocalClientConnectError::RequestIoConfiguration(_) => {
            Failure::new(FailureKind::InternalFailure)
        }
    }
}

#[cfg(target_os = "linux")]
fn classify_read(error: SnapshotReadError) -> Failure {
    use orchbee_workbench_protocol::WorkbenchWireError;
    match error {
        SnapshotReadError::SessionUnavailable | SnapshotReadError::RequestEncoding(_) => {
            Failure::new(FailureKind::InternalFailure)
        }
        SnapshotReadError::RequestNotSubmitted(_) => {
            Failure::new(FailureKind::TransportLoss).details(stage_details("snapshot"))
        }
        SnapshotReadError::Failed { reason, .. } => match reason {
            SnapshotReadFailure::Write(_) | SnapshotReadFailure::ResponseDeadline(_) => {
                Failure::new(FailureKind::TransportLoss).details(stage_details("snapshot"))
            }
            SnapshotReadFailure::Response(WorkbenchWireError::Io(_)) => {
                Failure::new(FailureKind::TransportLoss).details(stage_details("snapshot"))
            }
            SnapshotReadFailure::Response(_) | SnapshotReadFailure::UnexpectedResponse => {
                Failure::new(FailureKind::ProtocolInvalid).details(stage_details("snapshot"))
            }
            SnapshotReadFailure::CorrelationMismatch { .. } => {
                Failure::new(FailureKind::ProtocolInvalid)
                    .details(stage_reason_details("snapshot", "correlation-mismatch"))
            }
        },
    }
}

/// Sixteen fresh bytes become the canonical 32-lowercase-hex request identity.
#[cfg(target_os = "linux")]
fn fresh_request_id() -> Result<String, Failure> {
    use std::io::Read;
    let mut bytes = [0_u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let mut text = String::with_capacity(32);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(text, "{byte:02x}");
    }
    Ok(text)
}

#[cfg(target_os = "linux")]
pub(crate) fn request_id() -> Result<WorkbenchRequestId, Failure> {
    WorkbenchRequestId::parse(&fresh_request_id()?)
        .map_err(|_| Failure::new(FailureKind::InternalFailure))
}

#[cfg(target_os = "linux")]
fn obcs_request_id() -> Result<ObcsRequestId, Failure> {
    ObcsRequestId::parse(&fresh_request_id()?)
        .map_err(|_| Failure::new(FailureKind::InternalFailure))
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn read_snapshot() -> Result<orchbee_workbench_protocol::Snapshot, super::Failure> {
    Err(
        super::Failure::new(super::FailureKind::ServiceUnavailable).details(
            super::stage_reason_details("platform", "unsupported-platform"),
        ),
    )
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn read_terminal(
    _target: &crate::invocation::Target,
    _history: Option<u16>,
) -> Result<(serde_json::Value, super::results::Projection), super::Failure> {
    Err(
        super::Failure::new(super::FailureKind::ServiceUnavailable).details(
            super::stage_reason_details("platform", "unsupported-platform"),
        ),
    )
}
