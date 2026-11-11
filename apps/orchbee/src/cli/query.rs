//! Existing-only Session admission and bounded query exchanges. Snapshot queries
//! are read-only; the names child owns one explicit Agent alias exchange.
//! No path activates a Session. Selection/copy/link reads do not start processes;
//! explicit link activation delegates one plugin Job to the existing execution owner.

mod terminal_buffer;
pub(super) mod terminal_copy;
pub(super) mod terminal_links;
mod terminal_output;
pub(super) mod terminal_selection;
pub(crate) use terminal_output::read_output;

pub(crate) fn wait_source_output(
    target: &Target,
    pattern: &crate::invocation::WaitPattern,
    selection: crate::invocation::OutputSelection,
    timeout_ms: Option<u32>,
    after: Option<&crate::invocation::OutputAfter>,
) -> Result<(serde_json::Value, Projection), Failure> {
    super::execution::admit(super::execution::Capability::CanonicalWorkbenchRead)?;
    wait_output_inner(target, pattern, selection, timeout_ms, after.cloned())
}

struct WaitSample {
    observation: super::results::WaitObservation,
    requested_rows: u16,
    capacity_omitted: Option<u64>,
}

pub(in crate::cli) mod named_terminal;
pub(crate) use named_terminal::named_terminal_target;
mod agent_management;
pub(in crate::cli) mod terminal_agent;
pub(super) mod terminal_metadata;
pub(in crate::cli) use agent_management::read_agent_management;
mod agent_journal;
pub(crate) use agent_journal::{AgentJournal, JournalError};
mod agent_explanation;
mod agent_focus;
mod agent_names;
mod agent_names_failure;
pub(in crate::cli) use agent_names_failure::rejection as agent_names_rejection;

pub(in crate::cli) mod agent_target;
pub(in crate::cli) fn read_agent_snapshot(
    client: &mut LocalClient,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<Snapshot, Failure> {
    agent_target::read_snapshot(client, deadline, wake)
}
pub(crate) use agent_focus::{
    agent_attention_target, agent_focus_target, attention_changed_sessions,
    attention_covers_observation, authenticate_attention,
};
mod process_info;
pub(crate) use process_info::panel_process_info;
pub(crate) use process_info::read_terminal_processes;
pub(in crate::cli) use process_info::{classify_process, metadata_failure, panel_agent_failure};
pub(crate) use process_info::{primary_terminal, read_terminal_processes_cancellable};

mod agent_terminal;
pub(in crate::cli) use agent_terminal::classify_observation;
pub(crate) use agent_terminal::resolve_agent_terminal;
pub(in crate::cli) use agent_terminal::resolve_session_terminal;
pub(crate) use agent_terminal::verify_agent_terminal;
pub(crate) use agent_terminal::{AgentObservationRead, read_agent_observation};

use super::command_interrupt::{ArmedSignals as AgentSignals, Finish as AgentFinish};
use super::results::Projection;
use super::{
    Failure, FailureKind, SourceFinalization, stage_details, stage_reason_details, target_details,
};
use crate::invocation::{Resource, Target};
use crate::local_client::{AgentExchangeError, AgentExchangeFailure, AgentReadOperation};
use crate::local_client::{
    LaunchProfileCatalogReadError, LaunchProfileCatalogReadFailure, SnapshotReadError,
    SnapshotReadFailure, WorkbenchWorkspaceCloseFailure, WorkbenchWorkspaceCloseNotSubmitted,
    WorkbenchWorkspaceCloseReadError, WorktreeListReadError, WorktreeListReadFailure,
};
use crate::local_client::{LocalClient, OutcomeUnknownReason, SubmissionError};
use crate::local_client::{exchange_wake_fault, exchange_wake_ready};
use crate::terminal_projection::TerminalProjection;
use crate::termination_signal::{TerminationSignal, TerminationSource};
use orchbee_agent_observation_protocol::{
    AgentExplainOutcome, AgentGetOutcome, AgentListOutcome, AgentReadRequest, AgentReadResult,
    AgentSafeInteger, AgentSessionId, AgentWireError, ObapRequestId,
};
use orchbee_client_server_protocol::{
    RequestId as ObcsRequestId, TerminalId as ObcsTerminalId, TerminalLaunchReadOutcome,
    TerminalLaunchReadRequest, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
    ViewerId, ViewerRejectedReason,
};
use orchbee_local_client_protocol::ConnectClientError;
use orchbee_local_ipc::LocalIo as BorrowedFd;
use orchbee_local_runtime::{
    Cohort, ModeCheckedLocalSessionRendezvous, RendezvousError, SessionScope,
};
use orchbee_terminal_protocol::{
    AttachmentId, HostEpoch, TerminalId as HostTerminalId, TerminalTextFrame,
};
use orchbee_workbench_protocol::{
    LaunchProfileCatalogOutcome, LaunchProfileSelector, ProtocolVersion, Snapshot,
    SnapshotReadIntent, WorkbenchDirectoryId, WorkbenchLaunchProfileCatalogRead,
    WorkbenchSnapshotOutcome, WorkbenchSnapshotUnavailableReason,
    WorkbenchTabCloseUnavailableReason, WorkbenchWorkspaceCloseRead,
    WorkbenchWorkspaceCloseReadOutcome, WorkbenchWorkspaceId, WorkbenchWorktreeListOutcome,
    WorkbenchWorktreeListRead, WorktreeListStage, WorktreeListTooLargeOrigin,
    WorktreeListUnavailableReason,
};
use std::time::{Duration, Instant};

mod agent_list_unarmed;

const COHORT_BUDGET: Duration = Duration::from_secs(5);
/// The Agent read's whole-command budget: one absolute deadline sampled
/// once at invocation entry and shared by the preamble, the admission, and
/// the single OBAP exchange — never refreshed between stages.
pub(super) const AGENT_TOTAL_BUDGET: Duration = Duration::from_secs(5);
const NETWORK_BUDGET: Duration = Duration::from_secs(5);
/// An `Empty` poll answer is retried inside the shared network budget; the
/// cadence stays coarse enough that a quiet host is never hammered.
const EMPTY_POLL_INTERVAL: Duration = Duration::from_millis(10);
/// A quiet wait backs off on consecutive `Empty` answers up to this ceiling;
/// any real frame record resets the cadence to `EMPTY_POLL_INTERVAL`.
const WAIT_POLL_BACKOFF: Duration = Duration::from_millis(100);

/// One admitted Client Connection plus the shared network deadline every
/// request on it must finish before. The caller owns the session's drop.
pub(crate) fn read_snapshot() -> Result<Snapshot, Failure> {
    super::execution::admit(super::execution::Capability::CanonicalWorkbenchRead)?;
    let (client, _deadline, snapshot) = snapshot_session()?;
    // The Client Connection is fully dropped before the snapshot is examined or
    // any output is prepared.
    drop(client);
    Ok(snapshot)
}

/// `terminal get`: the shared snapshot path admits the transport; the caller
/// supplies the exact typed target id. The answer is the Server's own join under
/// the coordinator lock: the current binding, the committed revision, and the
/// retained launch fact. A stale snapshot route is never echoed back.
pub(crate) fn terminal_get(target: &Target) -> Result<(serde_json::Value, Projection), Failure> {
    super::execution::admit(super::execution::Capability::CanonicalWorkbenchRead)?;
    let (mut client, deadline, _) = snapshot_session()?;
    // A typed ID passes through to the Server's launch read unchanged — it
    // answers binding state itself. A compact selector expands to the same exact ID.
    let identity = match target {
        Target::Id(identity) => identity.clone(),
        Target::Selector(_) => target
            .selector_identity()
            .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?,
        // The parser admits only typed IDs and selectors for terminal targets.
        Target::Label(_) | Target::TerminalOrPanelId(_) => {
            return Err(Failure::new(FailureKind::InternalFailure));
        }
    };
    let request = TerminalLaunchReadRequest::new(
        obcs_request_id()?,
        ObcsTerminalId::parse(identity.as_str())
            .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
    );
    let outcome = client
        .submit_terminal_launch_read_before(request, deadline)
        .map_err(classify_launch_read);
    // The Client Connection is fully dropped before the answer is mapped into the
    // document or any output is prepared.
    drop(client);
    let result = outcome?;
    match result.outcome() {
        TerminalLaunchReadOutcome::Found {
            session_id,
            workbench_revision,
            terminal,
            launch,
            launch_profile,
            launch_selection,
        } => {
            let bound_to = terminal.bound_to();
            let row = super::results::TerminalRow {
                terminal_id: terminal.terminal_id().as_str().to_owned(),
                worker_id: terminal.worker_id().as_str().to_owned(),
                presentation_id: bound_to.presentation_id().as_str().to_owned(),
                panel_id: bound_to.panel_id().as_str().to_owned(),
                tab_id: bound_to.tab_id().as_str().to_owned(),
                terminal_context_id: bound_to.terminal_context_id().as_str().to_owned(),
            };
            Ok((
                super::results::launch_read_source(session_id.as_str(), *workbench_revision),
                Projection::TerminalGet(super::results::TerminalGetDoc {
                    terminal: row,
                    launch: (**launch).clone(),
                    launch_profile: launch_profile.clone(),
                    launch_selection: launch_selection.as_deref().cloned(),
                }),
            ))
        }
        TerminalLaunchReadOutcome::NotFound => Err(Failure::new(FailureKind::NotFound)
            .details(target_details(Resource::Terminal, identity.as_str()))),
        TerminalLaunchReadOutcome::Unavailable { .. } => {
            Err(Failure::new(FailureKind::ServiceUnavailable).details(stage_details("launch-read")))
        }
    }
}

/// `terminal read`: the snapshot resolves the typed binding, then one
/// read-capability viewer attachment on the same session and deadline
/// observes the terminal's current projection — or, under `--source recent`, the
/// retained scrollback window behind it. The attachment is released
/// (best-effort `Detach` plus the session drop) before any output work.
#[cfg(any(target_os = "linux", windows))]
#[cfg(test)]
pub(crate) fn read_terminal(
    target: &Target,
    history: Option<u16>,
    presentation: super::results::ReadPresentation,
) -> Result<(serde_json::Value, Projection), Failure> {
    let (client, deadline, snapshot) = snapshot_session()?;
    read_terminal_on_session(client, deadline, &snapshot, target, history, presentation)
}

#[cfg(any(target_os = "linux", windows))]
#[cfg(test)]
fn read_terminal_on_session(
    mut client: LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    target: &Target,
    history: Option<u16>,
    presentation: super::results::ReadPresentation,
) -> Result<(serde_json::Value, Projection), Failure> {
    let source = super::results::snapshot_source(snapshot);
    let outcome = match history {
        None => read_terminal_frame(&mut client, deadline, snapshot, target)
            .map(|(row, frame)| Observed::Visible(row, frame)),
        Some(rows) => read_terminal_history(&mut client, deadline, snapshot, target, rows).map(
            |(row, frame, window)| Observed::History {
                row,
                frame,
                window,
                requested: rows,
            },
        ),
    };
    // The Client Connection is fully dropped before the applied frame or the
    // history window is mapped into the bounded document or any output is
    // prepared.
    drop(client);
    match outcome {
        Ok(Observed::Visible(row, frame)) => {
            super::results::terminal_read_doc(row, &frame, presentation)
                .map(|doc| (source.clone(), Projection::TerminalRead(doc)))
                .map_err(|failure| failure.source(source))
        }
        Ok(Observed::History {
            row,
            frame,
            window,
            requested,
        }) => super::results::terminal_history_doc(
            row,
            &frame,
            requested,
            window.as_ref(),
            presentation,
        )
        .map(|doc| (source.clone(), Projection::TerminalHistory(doc)))
        .map_err(|failure| failure.source(source)),
        // Every failure after this point had a validated snapshot: the
        // failure envelope may honestly cite it as the source.
        Err(failure) => Err(failure.source(source)),
    }
}

/// `worktree list`: one absolute deadline covers the preamble, the snapshot
/// read, and the worktree-list exchange. The snapshot resolves only the
/// project identity — directory membership is the Server's authority, so the
/// worktree-list read is the command's family boundary and every failure it
/// produces wears the dedicated worktree failure envelope.
pub(crate) fn worktree_list(
    directory: &WorkbenchDirectoryId,
) -> Result<(serde_json::Value, Projection), Failure> {
    super::execution::admit(super::execution::Capability::CanonicalWorkbenchRead)?;
    let deadline = deadline_after(NETWORK_BUDGET)?;
    let (mut client, deadline, snapshot) = snapshot_session_until(deadline)?;
    let source = super::results::snapshot_source(&snapshot);
    let outcome = worktree_list_exchange(&mut client, deadline, directory);
    // The Client Connection is fully dropped before the answer is mapped into
    // the bounded document or any output is prepared.
    drop(client);
    outcome
        .map(|(source, doc)| (source, Projection::WorktreeList(doc)))
        .map_err(|failure| {
            // Failures past the snapshot boundary may honestly cite it; only
            // post-query failures carry the worktree envelope schema.
            failure.source(source)
        })
}

/// Resolve the project on the snapshot, then submit the single worktree-list
/// read. A pre-boundary failure (project resolution) keeps the generic
/// envelope; everything from the typed exchange onward is worktree-family.
fn worktree_list_exchange(
    client: &mut LocalClient,
    deadline: Instant,
    directory: &WorkbenchDirectoryId,
) -> Result<(serde_json::Value, super::results::WorktreeListDoc), Failure> {
    let request = WorkbenchWorktreeListRead {
        protocol_version: ProtocolVersion,
        request_id: request_id()?,
        directory_id: directory.clone(),
    };
    let result = client
        .submit_workbench_worktree_list_read_before(request, deadline)
        .map_err(classify_worktree_read)?;
    match result.outcome {
        WorkbenchWorktreeListOutcome::Found {
            session_id,
            workbench_revision,
            directory_id,
            path,
            entries,
        } => Ok((
            serde_json::json!({
                "kind": "worktree-list",
                "sessionId": session_id.as_str(),
                "revision": workbench_revision,
            }),
            super::results::WorktreeListDoc {
                directory_id: directory_id.as_str().to_owned(),
                path: path.as_str().to_owned(),
                entries,
            },
        )),
        outcome => Err(classify_worktree_outcome(outcome, directory)),
    }
}

/// The worktree-list outcome family → the CLI failure kinds. Every failure
/// produced here is post-boundary and carries the dedicated envelope schema.
fn classify_worktree_outcome(
    outcome: WorkbenchWorktreeListOutcome,
    directory: &WorkbenchDirectoryId,
) -> Failure {
    let failure = |kind| Failure::new(kind).failure_schema(super::schema::WORKTREE_FAILURE_ID);
    match outcome {
        WorkbenchWorktreeListOutcome::NotFound {} => {
            failure(FailureKind::NotFound).details(serde_json::Map::from_iter([
                ("resource".to_owned(), serde_json::json!("directory")),
                ("target".to_owned(), serde_json::json!(directory.as_str())),
            ]))
        }
        WorkbenchWorktreeListOutcome::Denied {} => {
            failure(FailureKind::Denied).details(stage_details("worktree-list"))
        }
        WorkbenchWorktreeListOutcome::Unavailable { reason } => {
            let reason = match reason {
                WorktreeListUnavailableReason::ServerBusy => "server-busy",
                WorktreeListUnavailableReason::WorkerUnavailable => "worker-unavailable",
                WorktreeListUnavailableReason::JobDead => "job-dead",
                WorktreeListUnavailableReason::JobUnknown => "job-unknown",
            };
            failure(FailureKind::ServiceUnavailable)
                .details(stage_reason_details("worktree-list", reason))
        }
        WorkbenchWorktreeListOutcome::GitFailed { stage, status } => {
            // probe/list always carry the exact Git exit status — an explicit
            // null when the process was signaled, never an invented code.
            let mut details = stage_details(worktree_git_stage(stage));
            details.insert("status".to_owned(), serde_json::json!(status));
            failure(FailureKind::CheckFailed).details(details)
        }
        // Both spawn arms are the Worker's process-start failure, not a Git
        // exit: service-unavailable with the worker-unavailable reason.
        WorkbenchWorktreeListOutcome::SpawnUnavailable { .. }
        | WorkbenchWorktreeListOutcome::SpawnFailed { .. } => {
            failure(FailureKind::ServiceUnavailable)
                .details(stage_reason_details("worktree-list", "worker-unavailable"))
        }
        WorkbenchWorktreeListOutcome::TooLarge { origin } => {
            let (origin, limit_bytes) = match origin {
                WorktreeListTooLargeOrigin::GitProbe => ("git-probe", 65536_u32),
                WorktreeListTooLargeOrigin::GitList => ("git-list", 65536_u32),
                WorktreeListTooLargeOrigin::WorkerResult => ("worker-result", 7936_u32),
                WorktreeListTooLargeOrigin::WorkbenchResult => ("workbench-result", 32768_u32),
            };
            failure(FailureKind::OutputTooLarge).details(serde_json::Map::from_iter([
                ("stage".to_owned(), serde_json::json!("worktree-list")),
                ("origin".to_owned(), serde_json::json!(origin)),
                ("limitBytes".to_owned(), serde_json::json!(limit_bytes)),
            ]))
        }
        WorkbenchWorktreeListOutcome::Timeout {} => {
            failure(FailureKind::Timeout).details(serde_json::Map::from_iter([
                ("stage".to_owned(), serde_json::json!("worktree-list")),
                ("timeoutMs".to_owned(), serde_json::json!(5000)),
            ]))
        }
        WorkbenchWorktreeListOutcome::OwnerIo { .. } => failure(FailureKind::ServiceUnavailable)
            .details(stage_reason_details("worktree-list", "worker-unavailable")),
        WorkbenchWorktreeListOutcome::TransportLoss {} => {
            failure(FailureKind::TransportLoss).details(stage_details("worktree-list"))
        }
        WorkbenchWorktreeListOutcome::MalformedListing {} => {
            failure(FailureKind::CheckFailed).details(stage_details("parse"))
        }
        WorkbenchWorktreeListOutcome::Found { .. } => {
            unreachable!("the found arm returns before classification")
        }
    }
}

const fn worktree_git_stage(stage: WorktreeListStage) -> &'static str {
    match stage {
        WorktreeListStage::Probe => "probe",
        WorktreeListStage::List => "list",
    }
}

/// `workspace close-status`: one typed retained-record read on the admitted
/// session, keyed by the stable `(project, workspace)` pair. The record
/// outlives the Workspace's active projection, so the snapshot supplies only
/// the session's session — membership is never re-resolved locally. Every
/// failure past the session boundary carries the dedicated close envelope.
pub(crate) fn workspace_close_status(
    workspace: &WorkbenchWorkspaceId,
) -> Result<(serde_json::Value, Projection), Failure> {
    super::execution::admit(super::execution::Capability::CanonicalWorkbenchRead)?;
    let deadline = deadline_after(NETWORK_BUDGET)?;
    let (mut client, deadline, snapshot) = snapshot_session_until(deadline)?;
    let source = super::results::snapshot_source(&snapshot);
    let request = WorkbenchWorkspaceCloseRead {
        protocol_version: ProtocolVersion,
        request_id: request_id()?,
        session_id: snapshot.session_id.clone(),
        workspace_id: workspace.clone(),
    };
    let outcome = client
        .submit_workbench_workspace_close_read(request, None, deadline)
        .map_err(classify_close_status_read);
    // The Client Connection is fully dropped before the answer is mapped into
    // the bounded document or any output is prepared.
    drop(client);
    outcome
        .and_then(classify_close_status_outcome)
        .map(|outcome| (source.clone(), Projection::WorkspaceCloseStatus(outcome)))
        .map_err(|failure| failure.source(source))
}

/// The close read outcome family → projection or CLI failure. Only the
/// record-bearing answers reach the result document; refusals leave as
/// failures under the close envelope.
fn classify_close_status_outcome(
    outcome: WorkbenchWorkspaceCloseReadOutcome,
) -> Result<WorkbenchWorkspaceCloseReadOutcome, Failure> {
    let failure = |kind| Failure::new(kind).failure_schema(super::schema::CLOSE_FAILURE_ID);
    match outcome {
        WorkbenchWorkspaceCloseReadOutcome::NotClosing { .. }
        | WorkbenchWorkspaceCloseReadOutcome::Available { .. } => Ok(outcome),
        WorkbenchWorkspaceCloseReadOutcome::Denied {} => {
            Err(failure(FailureKind::Denied).details(stage_details("close-status")))
        }
        WorkbenchWorkspaceCloseReadOutcome::Unavailable { reason } => {
            let reason = match reason {
                WorkbenchTabCloseUnavailableReason::ServerBusy => "server-busy",
            };
            Err(failure(FailureKind::ServiceUnavailable)
                .details(stage_reason_details("close-status", reason)))
        }
    }
}

/// The close status exchange failures, classified under the close failure
/// envelope: a deadline-expired response is the command's own timeout, and a
/// scope-mismatched answer is a protocol fault, never a silent retry.
fn classify_close_status_read(error: WorkbenchWorkspaceCloseReadError) -> Failure {
    use orchbee_workbench_protocol::WorkbenchWireError;
    let failure = |kind| Failure::new(kind).failure_schema(super::schema::CLOSE_FAILURE_ID);
    match error {
        WorkbenchWorkspaceCloseReadError::NotSubmitted(reason) => match reason {
            WorkbenchWorkspaceCloseNotSubmitted::ConnectionUnavailable
            | WorkbenchWorkspaceCloseNotSubmitted::RequestEncoding(_) => {
                failure(FailureKind::InternalFailure)
            }
            WorkbenchWorkspaceCloseNotSubmitted::RequestNotSubmitted(_) => {
                failure(FailureKind::TransportLoss).details(stage_details("close-status"))
            }
        },
        WorkbenchWorkspaceCloseReadError::Failed { reason, .. } => match reason {
            WorkbenchWorkspaceCloseFailure::Write(_) => {
                failure(FailureKind::TransportLoss).details(stage_details("close-status"))
            }
            WorkbenchWorkspaceCloseFailure::ResponseDeadline(_) => failure(FailureKind::Timeout)
                .details(serde_json::Map::from_iter([
                    ("stage".to_owned(), serde_json::json!("close-status")),
                    ("timeoutMs".to_owned(), serde_json::json!(5000)),
                ])),
            WorkbenchWorkspaceCloseFailure::Response(WorkbenchWireError::Io(_)) => {
                failure(FailureKind::TransportLoss).details(stage_details("close-status"))
            }
            WorkbenchWorkspaceCloseFailure::Response(_)
            | WorkbenchWorkspaceCloseFailure::UnexpectedResponse
            | WorkbenchWorkspaceCloseFailure::ActionIncompatible => {
                failure(FailureKind::ProtocolInvalid).details(stage_details("close-status"))
            }
            WorkbenchWorkspaceCloseFailure::CorrelationMismatch { .. } => {
                failure(FailureKind::ProtocolInvalid)
                    .details(stage_reason_details("close-status", "correlation-mismatch"))
            }
            WorkbenchWorkspaceCloseFailure::SessionMismatch { .. }
            | WorkbenchWorkspaceCloseFailure::WorkspaceMismatch { .. }
            | WorkbenchWorkspaceCloseFailure::OperationMismatch { .. }
            | WorkbenchWorkspaceCloseFailure::AcceptedRevisionMismatch { .. }
            | WorkbenchWorkspaceCloseFailure::AttemptGenerationMismatch { .. } => {
                failure(FailureKind::ProtocolInvalid)
                    .details(stage_reason_details("close-status", "scope-mismatch"))
            }
        },
    }
}

/// The typed worktree-list exchange failures, classified under the worktree
/// failure envelope: a deadline-expired response is the command's own
/// timeout, never a transport guess.
fn classify_worktree_read(error: WorktreeListReadError) -> Failure {
    use orchbee_workbench_protocol::WorkbenchWireError;
    let failure = |kind| Failure::new(kind).failure_schema(super::schema::WORKTREE_FAILURE_ID);
    match error {
        WorktreeListReadError::ConnectionUnavailable
        | WorktreeListReadError::RequestEncoding(_) => failure(FailureKind::InternalFailure),
        WorktreeListReadError::RequestNotSubmitted(_) => {
            failure(FailureKind::TransportLoss).details(stage_details("worktree-list"))
        }
        WorktreeListReadError::Failed { reason, .. } => match reason {
            WorktreeListReadFailure::Write(_) => {
                failure(FailureKind::TransportLoss).details(stage_details("worktree-list"))
            }
            WorktreeListReadFailure::ResponseDeadline(_) => {
                failure(FailureKind::Timeout).details(serde_json::Map::from_iter([
                    ("stage".to_owned(), serde_json::json!("worktree-list")),
                    ("timeoutMs".to_owned(), serde_json::json!(5000)),
                ]))
            }
            WorktreeListReadFailure::Response(WorkbenchWireError::Io(_)) => {
                failure(FailureKind::TransportLoss).details(stage_details("worktree-list"))
            }
            WorktreeListReadFailure::Response(_) | WorktreeListReadFailure::UnexpectedResponse => {
                failure(FailureKind::ProtocolInvalid).details(stage_details("worktree-list"))
            }
            WorktreeListReadFailure::CorrelationMismatch { .. } => {
                failure(FailureKind::ProtocolInvalid).details(stage_reason_details(
                    "worktree-list",
                    "correlation-mismatch",
                ))
            }
        },
    }
}

/// `launch-profile list`/`get`: one typed catalog read on the admitted
/// session. `selector` absent lists the whole per-Session catalog; present
/// resolves exactly one entry under the Server's coordinator lock — the
/// CLI never pre-reads the catalog or resolves a selector locally. Every
/// failure past the session boundary carries the dedicated launch-profile
/// failure envelope: the generic schema's resource and candidate-id
/// grammars are closed without it.
pub(crate) fn launch_profile_catalog(
    selector: Option<LaunchProfileSelector>,
) -> Result<(serde_json::Value, Projection), Failure> {
    super::execution::admit(super::execution::Capability::CanonicalWorkbenchRead)?;
    let deadline = deadline_after(NETWORK_BUDGET)?;
    let (mut client, deadline, snapshot) = snapshot_session_until(deadline)?;
    let source = super::results::snapshot_source(&snapshot);
    let request = WorkbenchLaunchProfileCatalogRead {
        protocol_version: ProtocolVersion,
        request_id: request_id()?,
        selector: selector.clone(),
    };
    let outcome = client
        .submit_workbench_launch_profile_catalog_read_before(request, deadline)
        .map_err(|error| classify_launch_profile_catalog_read(error).source(source.clone()));
    // The Client Connection is fully dropped before the answer is mapped into
    // the bounded document or any output is prepared.
    drop(client);
    let result = outcome?;
    match result.outcome {
        LaunchProfileCatalogOutcome::Catalog {
            session_id,
            catalog_revision,
            entries,
        } => Ok((
            launch_profile_catalog_source(session_id.as_str(), catalog_revision),
            Projection::LaunchProfileCatalog(super::results::LaunchProfileCatalogDoc {
                catalog_revision,
                selection: super::results::LaunchProfileCatalogSelection::List(entries),
            }),
        )),
        LaunchProfileCatalogOutcome::Profile {
            session_id,
            catalog_revision,
            profile,
        } => Ok((
            launch_profile_catalog_source(session_id.as_str(), catalog_revision),
            Projection::LaunchProfileCatalog(super::results::LaunchProfileCatalogDoc {
                catalog_revision,
                selection: super::results::LaunchProfileCatalogSelection::Profile(profile),
            }),
        )),
        outcome => Err(classify_launch_profile_outcome(outcome, selector.as_ref()).source(source)),
    }
}

/// The `launch-profile-catalog` authority source: the same Server
/// resolution the answer came from.
fn launch_profile_catalog_source(
    session_id: &str,
    catalog_revision: orchbee_workbench_protocol::LaunchProfileCatalogRevision,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "launch-profile-catalog",
        "sessionId": session_id,
        "catalogRevision": catalog_revision.to_string(),
    })
}

/// The catalog read's typed outcomes → the CLI failure kinds, all under the
/// launch-profile failure envelope.
fn classify_launch_profile_outcome(
    outcome: LaunchProfileCatalogOutcome,
    selector: Option<&LaunchProfileSelector>,
) -> Failure {
    let failure =
        |kind| Failure::new(kind).failure_schema(super::schema::LAUNCH_PROFILE_FAILURE_ID);
    let target = selector.map_or("", LaunchProfileSelector::as_str);
    match outcome {
        LaunchProfileCatalogOutcome::NotFound {} => {
            failure(FailureKind::NotFound).details(serde_json::Map::from_iter([
                ("resource".to_owned(), serde_json::json!("launch-profile")),
                ("target".to_owned(), serde_json::json!(target)),
            ]))
        }
        LaunchProfileCatalogOutcome::Ambiguous { candidate_ids } => {
            let total = candidate_ids.len();
            let candidates: Vec<&str> = candidate_ids
                .iter()
                .take(8)
                .map(orchbee_workbench_protocol::LaunchProfileId::as_str)
                .collect();
            failure(FailureKind::Ambiguous).details(serde_json::Map::from_iter([
                ("resource".to_owned(), serde_json::json!("launch-profile")),
                ("target".to_owned(), serde_json::json!(target)),
                ("candidates".to_owned(), serde_json::json!(candidates)),
                ("totalCount".to_owned(), serde_json::json!(total)),
            ]))
        }
        LaunchProfileCatalogOutcome::Denied {} => {
            failure(FailureKind::Denied).details(stage_details("launch-profile-catalog"))
        }
        LaunchProfileCatalogOutcome::Unavailable { reason } => {
            let reason = match reason {
                WorkbenchSnapshotUnavailableReason::ServiceUnavailable => "service-unavailable",
            };
            failure(FailureKind::ServiceUnavailable)
                .details(stage_reason_details("launch-profile-catalog", reason))
        }
        LaunchProfileCatalogOutcome::Catalog { .. }
        | LaunchProfileCatalogOutcome::Profile { .. } => {
            unreachable!("catalog and profile answers return before classification")
        }
    }
}

/// The typed catalog exchange failures, classified under the launch-profile
/// failure envelope: a deadline-expired response is the command's own
/// timeout, never a transport guess.
fn classify_launch_profile_catalog_read(error: LaunchProfileCatalogReadError) -> Failure {
    use orchbee_workbench_protocol::WorkbenchWireError;
    let failure =
        |kind| Failure::new(kind).failure_schema(super::schema::LAUNCH_PROFILE_FAILURE_ID);
    match error {
        LaunchProfileCatalogReadError::ConnectionUnavailable
        | LaunchProfileCatalogReadError::RequestEncoding(_) => {
            failure(FailureKind::InternalFailure)
        }
        LaunchProfileCatalogReadError::RequestNotSubmitted(_) => {
            failure(FailureKind::TransportLoss).details(stage_details("launch-profile-catalog"))
        }
        LaunchProfileCatalogReadError::Failed { reason, .. } => match reason {
            LaunchProfileCatalogReadFailure::Write(_) => {
                failure(FailureKind::TransportLoss).details(stage_details("launch-profile-catalog"))
            }
            LaunchProfileCatalogReadFailure::ResponseDeadline(_) => failure(FailureKind::Timeout)
                .details(serde_json::Map::from_iter([
                    (
                        "stage".to_owned(),
                        serde_json::json!("launch-profile-catalog"),
                    ),
                    ("timeoutMs".to_owned(), serde_json::json!(5000)),
                ])),
            LaunchProfileCatalogReadFailure::Response(WorkbenchWireError::Io(_)) => {
                failure(FailureKind::TransportLoss).details(stage_details("launch-profile-catalog"))
            }
            LaunchProfileCatalogReadFailure::Response(_)
            | LaunchProfileCatalogReadFailure::UnexpectedResponse => {
                failure(FailureKind::ProtocolInvalid)
                    .details(stage_details("launch-profile-catalog"))
            }
            LaunchProfileCatalogReadFailure::CorrelationMismatch { .. } => {
                failure(FailureKind::ProtocolInvalid).details(stage_reason_details(
                    "launch-profile-catalog",
                    "correlation-mismatch",
                ))
            }
        },
    }
}

/// The shared preamble for every network query and mutation: cohort,
/// existing-only rendezvous, socket validation, one admission, one snapshot
/// read — all under the single network deadline. On success the caller owns
/// the still admitted session; on any failure the session is already dropped.
pub(crate) fn snapshot_session() -> Result<(LocalClient, Instant, Snapshot), Failure> {
    let cohort_deadline = deadline_after(COHORT_BUDGET)?;
    let (cohort, session) = session_preamble(cohort_deadline, || Ok(()))?;
    // Query and read callers keep their accepted budget: a fresh network
    // deadline sampled only after the preamble completes.
    let deadline = deadline_after(NETWORK_BUDGET)?;
    admit_snapshot(&cohort, &session, deadline, || Ok(()))
}

/// Existing-only stream admission and Snapshot read under one cancellable budget.
pub(crate) fn stream_snapshot(
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<(LocalClient, Snapshot), Failure> {
    let (cohort, session) =
        session_preamble(deadline.min(Instant::now() + COHORT_BUDGET), || Ok(()))?;
    let mut client = LocalClient::connect_before_cancellable(
        session.client_socket_path(),
        &crate::local_client::client_offer(Some(cohort.identity())),
        deadline,
        wake,
    )
    .map_err(classify_connect)?;
    let outcome = crate::agent_runtime::startup::snapshot::read(&mut client, deadline, wake)
        .map_err(classify_snapshot)?;
    match outcome {
        WorkbenchSnapshotOutcome::Available { snapshot } => Ok((client, snapshot)),
        WorkbenchSnapshotOutcome::Denied {} => Err(Failure::new(FailureKind::Denied)),
        WorkbenchSnapshotOutcome::Unavailable { .. } => {
            Err(Failure::new(FailureKind::ServiceUnavailable))
        }
    }
}

/// The same preamble and admission as `snapshot_session`, but every stage
/// shares the caller's absolute deadline — a total command budget sampled
/// once at entry. The cohort check still never exceeds its own budget: it
/// runs under the earlier of `COHORT_BUDGET` and `deadline`, and the
/// admission plus snapshot read share what remains.
pub(crate) fn snapshot_session_until(
    deadline: Instant,
) -> Result<(LocalClient, Instant, Snapshot), Failure> {
    let cohort_deadline = bounded_deadline(Instant::now(), COHORT_BUDGET, deadline)?;
    let (cohort, session) = session_preamble(cohort_deadline, || Ok(()))?;
    admit_snapshot(&cohort, &session, deadline, || Ok(()))
}

/// The same preamble gated by the wait command's clock/cancel control:
/// every step gets the earlier of its own budget and the remaining total,
/// and the gate settles after each step's return — success *and* failure —
/// so a cohort, rendezvous, socket, connect, or snapshot step that
/// outlasted the total reports timeout (or cancelled) rather than its own
/// late classification. Filesystem steps stay cooperative: the gate checks
/// between calls and cannot interrupt one. Expiry surfaces as bare
/// `FailureKind::Timeout`/`Cancelled`; the caller maps it into its
/// detailed report.
fn snapshot_session_before(
    gate: &WaitGate<'_>,
) -> Result<(LocalClient, Instant, Snapshot), Failure> {
    let (cohort, session) = session_preamble(gate.budget(COHORT_BUDGET)?, || gate.settle())?;
    // A slow filesystem step may already have spent the whole wait budget;
    // the network deadline is priced only from what remains.
    admit_snapshot(&cohort, &session, gate.budget(NETWORK_BUDGET)?, || {
        gate.settle()
    })
}

/// Everything before the network exchange: the cohort check under its own
/// deadline, then existing-only rendezvous and socket validation. Under a
/// wait gate every step's return — success *and* failure — is settled
/// against the total deadline and the pending termination signal first.
fn session_preamble(
    cohort_deadline: Instant,
    settle: impl Fn() -> Result<(), Failure>,
) -> Result<(Cohort, ModeCheckedLocalSessionRendezvous), Failure> {
    let cohort = Cohort::current_with_diagnostic(cohort_deadline);
    settle()?;
    let cohort = cohort.map_err(|error| {
        Failure::new(FailureKind::CheckFailed)
            .details(stage_details("cohort"))
            .io_diagnostic(&error)
    })?;
    let session = rendezvous();
    settle()?;
    let session = session?;
    let socket_exists = session.client_socket_exists();
    settle()?;
    match socket_exists {
        Ok(true) => Ok((cohort, session)),
        Ok(false) => {
            Err(Failure::new(FailureKind::ServiceUnavailable).details(stage_details("discovery")))
        }
        Err(_) => Err(Failure::new(FailureKind::CheckFailed).details(stage_details("socket"))),
    }
}

/// One admission and one snapshot read under the shared deadline. On success
/// the caller owns the still admitted session; on any failure the session is
/// already dropped.
fn admit_snapshot(
    cohort: &Cohort,
    session: &ModeCheckedLocalSessionRendezvous,
    deadline: Instant,
    settle: impl Fn() -> Result<(), Failure>,
) -> Result<(LocalClient, Instant, Snapshot), Failure> {
    let connected = LocalClient::connect_before(
        session.client_socket_path(),
        &crate::local_client::client_offer(Some(cohort.identity())),
        deadline,
    );
    if let Err(failure) = settle() {
        drop(connected);
        return Err(failure);
    }
    let mut client = match connected {
        Ok(client) => client,
        Err(error) => return Err(classify_connect(error)),
    };
    let intent = SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: request_id()?,
    };
    let result = client.submit_workbench_snapshot_read_before(intent, deadline);
    // A snapshot answer that arrives past the total deadline — or under a
    // pending cancel — is still settled as timeout/cancelled, never a late
    // success.
    if let Err(failure) = settle() {
        drop(client);
        return Err(failure);
    }
    let result = match result {
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

/// Refresh an existing admitted session without rebinding its Session or replaying a read.
pub(in crate::cli) fn refresh_snapshot(
    client: &mut LocalClient,
    session: &orchbee_workbench_protocol::SessionId,
    minimum: orchbee_workbench_protocol::WorkbenchRevision,
    deadline: Instant,
    signals: &mut impl super::console_interrupt::SignalWatch,
) -> Result<Snapshot, Failure> {
    signals.check_before(deadline)?;
    let result = client.submit_workbench_snapshot_read_before(
        SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: request_id()?,
        },
        deadline.min(Instant::now() + Duration::from_secs(2)),
    );
    signals.check_before(deadline)?;
    match result.map_err(classify_read)?.outcome {
        WorkbenchSnapshotOutcome::Available { snapshot }
            if &snapshot.session_id == session && snapshot.revision >= minimum =>
        {
            Ok(snapshot)
        }
        WorkbenchSnapshotOutcome::Available { .. } => {
            Err(Failure::new(FailureKind::ProtocolInvalid))
        }
        WorkbenchSnapshotOutcome::Denied {} => Err(Failure::new(FailureKind::Denied)),
        WorkbenchSnapshotOutcome::Unavailable { .. } => {
            Err(Failure::new(FailureKind::ServiceUnavailable))
        }
    }
}

/// One checked deadline `budget` out from now.
fn deadline_after(budget: Duration) -> Result<Instant, Failure> {
    Instant::now()
        .checked_add(budget)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))
}

/// A stage's effective deadline under a total ceiling: `budget` from `now`
/// or `ceiling`, whichever lands first. The deterministic pin for the
/// shared-budget contract — stages keep their own bound but can never
/// outlive the command's absolute deadline.
fn bounded_deadline(now: Instant, budget: Duration, ceiling: Instant) -> Result<Instant, Failure> {
    Ok(now
        .checked_add(budget)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?
        .min(ceiling))
}

#[cfg(all(test, any(target_os = "linux", windows)))]
mod tests;

/// What the terminal observation produced: the applied visible frame, or the
/// validated scrollback window behind it.
#[cfg(any(target_os = "linux", windows))]
#[cfg(test)]
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
#[cfg(any(target_os = "linux", windows))]
#[cfg(test)]
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

/// `--source recent`: the same attach → poll preamble pins the frame's retained
/// scrollback count, then one `ReadRows` asks for the rows that have
/// scrolled off the active screen; an aged pin repeats both on the next
/// applied frame. `K = min(requested, scrollback,
/// MAXIMUM_FRAME_CELLS / columns)` keeps the absolute start at the recent
/// end; `scrollback == 0` short-circuits before any Host request. No
/// `RowsApplied` is sent: a buffer read creates no pending anchor.
#[cfg(any(target_os = "linux", windows))]
#[cfg(test)]
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
    let (row, viewer_id, _attachment, _epoch, mut projection) =
        attach_viewer(client, deadline, snapshot, target)?;
    let outcome = read_settled(
        client,
        deadline,
        &viewer_id,
        &mut projection,
        |client, frame, sequence| {
            let window =
                read_history_window(client, deadline, &viewer_id, frame, requested, sequence)?;
            // Only an abandoned pin is acknowledged: the Host publishes the
            // next frame after it, while a consistent read sends no feedback.
            if window.is_err() {
                acknowledge_frame(client, deadline, &viewer_id, frame)?;
            }
            Ok(window)
        },
    );
    detach_viewer(client, deadline, viewer_id);
    outcome.map(|(frame, window)| (row, frame, window))
}

/// Resolve the binding from the snapshot — typed ID or `@terminal:` selector
/// — and attach a read-only viewer to it inside the shared deadline.
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
    attach_viewer_cancellable(client, deadline, snapshot, target, None)
}

pub(super) fn attach_viewer_cancellable(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    target: &Target,
    wake: Option<BorrowedFd<'_>>,
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
    attach_viewer_cancellable_with_host(client, deadline, snapshot, target, wake).map(
        |(row, viewer, attachment, epoch, _host_terminal, projection)| {
            (row, viewer, attachment, epoch, projection)
        },
    )
}

pub(super) fn attach_viewer_cancellable_with_host(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    target: &Target,
    wake: Option<BorrowedFd<'_>>,
) -> Result<
    (
        super::results::TerminalRow,
        ViewerId,
        AttachmentId,
        HostEpoch,
        HostTerminalId,
        TerminalProjection,
    ),
    Failure,
> {
    // Typed IDs and `@terminal:` selectors resolve against the snapshot's
    // complete bound set; the attach request carries the resolved full id.
    let row = super::results::terminal_select(snapshot, target)?;
    let binding = crate::agent_runtime::startup::binding::Binding::parse(
        snapshot.session_id.as_str(),
        &row.worker_id,
        &row.terminal_id,
    )
    .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let attachment =
        crate::agent_runtime::startup::processes::attach(client, deadline, &binding, wake)
            .map_err(classify_process)?;
    Ok((
        row,
        attachment.viewer,
        attachment.id,
        attachment.epoch,
        attachment.terminal,
        TerminalProjection::new(attachment.id, attachment.epoch)
            .with_host_terminal(attachment.host_terminal),
    ))
}

/// The read observation is released no matter how the exchange ended; the
/// detach is best-effort because the session drop that follows already
/// guarantees teardown.
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

/// Issue one Host `ReadRows` behind the validated frame and decode its
/// answer against the frame's own route, which the projection already bound
/// to this attachment and epoch. The effective window is
/// `min(requested, scrollback, capacity)` rows ending at the scrollback
/// boundary; an aged pin answers its retryable reason, any other
/// `RowsRejected` maps to its typed failure, and any other outcome —
/// including an `Empty` stand-in — is protocol-invalid.
#[cfg(any(target_os = "linux", windows))]
#[cfg(test)]
fn read_history_window(
    client: &mut LocalClient,
    deadline: Instant,
    viewer_id: &ViewerId,
    frame: &TerminalTextFrame,
    requested: u16,
    sequence: std::num::NonZeroU64,
) -> Result<
    Result<
        Option<orchbee_terminal_protocol::TerminalTextRowWindow>,
        orchbee_client_server_protocol::ViewerRowsRejectedReason,
    >,
    Failure,
> {
    let scrollback = frame.scrollback_rows;
    if scrollback == 0 {
        return Ok(Ok(None));
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
            detail: orchbee_client_server_protocol::ViewerRowDetail::Text,
            attachment_id: viewer_id.clone(),
            start_row,
            row_count,
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
        TerminalViewerOutcome::Record(record) => decode_history_window(
            record.clone(),
            frame.route.attachment_id,
            frame.route.host_epoch,
            frame,
            start_row,
            count,
            sequence,
        )
        .map(|window| Ok(Some(window))),
        TerminalViewerOutcome::RowsRejected { reason } => aged_pin(*reason),
        TerminalViewerOutcome::Rejected { reason } => Err(classify_viewer_rejected(*reason)),
        TerminalViewerOutcome::Unavailable { .. } => Err(viewer_unavailable()),
        TerminalViewerOutcome::Attached { .. }
        | TerminalViewerOutcome::Empty { .. }
        | TerminalViewerOutcome::FeedbackAccepted { .. }
        | TerminalViewerOutcome::Detached { .. } => Err(viewer_protocol_invalid()),
    }
}

/// Independently validate the complete OBTP response record and its OBRW
/// payload against the request and the frame that priced it: envelope role
/// and encoding, Host epoch, and every window field — attachment, sequence,
/// revision, generation, columns, start, count, and the frame-derived total.
fn decode_history_window(
    record: Vec<u8>,
    attachment: AttachmentId,
    epoch: HostEpoch,
    frame: &TerminalTextFrame,
    start_row: u64,
    count: u64,
    request_sequence: std::num::NonZeroU64,
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
        || window.sequence != request_sequence
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

/// One-shot reads pin a frame, then read rows at its exact revision. A pin
/// that ages in between is abandoned for the next applied frame under the
/// same deadline, with a strictly increasing row sequence; rows from
/// different revisions are never spliced. `read` acknowledges any frame it
/// abandons. After the deadline the last aged answer is the failure.
fn read_settled<T>(
    client: &mut LocalClient,
    deadline: Instant,
    viewer_id: &ViewerId,
    projection: &mut TerminalProjection,
    mut read: impl FnMut(
        &mut LocalClient,
        &TerminalTextFrame,
        std::num::NonZeroU64,
    ) -> Result<
        Result<T, orchbee_client_server_protocol::ViewerRowsRejectedReason>,
        Failure,
    >,
) -> Result<(TerminalTextFrame, T), Failure> {
    let mut sequence = std::num::NonZeroU64::MIN;
    let mut aged = None;
    loop {
        if let Err(failure) = poll_applied_frame(client, deadline, viewer_id, projection, None) {
            return Err(match aged {
                Some(reason) if Instant::now() >= deadline => {
                    classify_rows_rejected(reason, "read-history")
                }
                _ => failure,
            });
        }
        let frame = projection
            .frame()
            .cloned()
            .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
        match read(client, &frame, sequence)? {
            Ok(value) => return Ok((frame, value)),
            Err(reason) if Instant::now() >= deadline => {
                return Err(classify_rows_rejected(reason, "read-history"));
            }
            Err(reason) => {
                aged = Some(reason);
                sequence = sequence
                    .checked_add(1)
                    .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
            }
        }
    }
}

/// Stale revision, stale generation and eviction mean the pinned frame aged
/// before its rows were read; every other rejection is the read's failure.
fn aged_pin<T>(
    reason: orchbee_client_server_protocol::ViewerRowsRejectedReason,
) -> Result<Result<T, orchbee_client_server_protocol::ViewerRowsRejectedReason>, Failure> {
    use orchbee_client_server_protocol::ViewerRowsRejectedReason as Reason;
    match reason {
        Reason::StaleRevision | Reason::StaleGeneration | Reason::HistoryEvicted => Ok(Err(reason)),
        other => Err(classify_rows_rejected(other, "read-history")),
    }
}

/// Confirm an applied frame so the Host may publish the next one.
pub(super) fn acknowledge_frame(
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
        TerminalViewerOutcome::Attached { .. }
        | TerminalViewerOutcome::Empty { .. }
        | TerminalViewerOutcome::Detached { .. }
        | TerminalViewerOutcome::RowsRejected { .. }
        | TerminalViewerOutcome::Record(..) => Err(viewer_protocol_invalid()),
    }
}

/// A typed `rows-rejected` answer maps to its fixed failure class: the local
/// budget overrun is a rejection, Host state answers are unavailability, and
/// reasons our fixed request shape cannot honestly produce are protocol
/// violations.
fn classify_rows_rejected(
    reason: orchbee_client_server_protocol::ViewerRowsRejectedReason,
    operation: &'static str,
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
        Some(reason) => failure.details(rows_details(operation, reason)),
        None => failure.details(stage_details("viewer")),
    }
}

/// Row-read failure details: the typed reason the Host carried, under the
/// viewer stage and the operation the read belongs to.
fn rows_details(operation: &str, reason: &str) -> serde_json::Map<String, serde_json::Value> {
    serde_json::Map::from_iter([
        ("stage".to_owned(), serde_json::json!("viewer")),
        ("operation".to_owned(), serde_json::json!(operation)),
        ("reason".to_owned(), serde_json::json!(reason)),
    ])
}

/// Poll until one record applies to the projection, acknowledging a rejected
/// record with `NeedSnapshot` recovery feedback. `Empty` answers are retried
/// under the shared deadline; there is no second connection, no retry of a
/// failed exchange, and no `FrameApplied` for a frame whose only consumer is
/// this read — `Detach` releases it.
#[cfg(any(target_os = "linux", windows))]
#[cfg(test)]
fn poll_terminal_frame(
    client: &mut LocalClient,
    deadline: Instant,
    viewer_id: &ViewerId,
    projection: TerminalProjection,
) -> Result<TerminalTextFrame, Failure> {
    poll_terminal_frame_cancellable(client, deadline, viewer_id, projection, None)
}

fn poll_terminal_frame_cancellable(
    client: &mut LocalClient,
    deadline: Instant,
    viewer_id: &ViewerId,
    mut projection: TerminalProjection,
    wake: Option<BorrowedFd<'_>>,
) -> Result<TerminalTextFrame, Failure> {
    poll_applied_frame(client, deadline, viewer_id, &mut projection, wake)?;
    projection
        .take_frame()
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))
}

/// Poll until one record applies to the borrowed projection. The projection
/// keeps the applied frame as the base later delta frames require, so a read
/// that abandons an aged pin can acknowledge it and poll for the next one.
pub(super) fn poll_applied_frame(
    client: &mut LocalClient,
    deadline: Instant,
    viewer_id: &ViewerId,
    projection: &mut TerminalProjection,
    wake: Option<BorrowedFd<'_>>,
) -> Result<(), Failure> {
    loop {
        let poll = TerminalViewerRequest::new(
            obcs_request_id()?,
            TerminalViewerAction::Poll {
                attachment_id: viewer_id.clone(),
            },
        );
        let result = client
            .submit_terminal_viewer_cancellable(poll, Some(deadline), wake)
            .map_err(classify_viewer)?;
        match result.outcome() {
            TerminalViewerOutcome::Empty {} => std::thread::sleep(EMPTY_POLL_INTERVAL),
            TerminalViewerOutcome::Record(record) => {
                match projection.receive_for_geometry(record, None) {
                    Ok(Some(feedback)) => {
                        if matches!(feedback, TerminalViewerAction::FrameApplied { .. }) {
                            return Ok(());
                        }
                        let request = TerminalViewerRequest::new(obcs_request_id()?, feedback);
                        let result = client
                            .submit_terminal_viewer_cancellable(request, Some(deadline), wake)
                            .map_err(classify_viewer)?;
                        match result.outcome() {
                            TerminalViewerOutcome::FeedbackAccepted {} => {}
                            TerminalViewerOutcome::Rejected { reason } => {
                                return Err(classify_viewer_rejected(*reason));
                            }
                            TerminalViewerOutcome::Unavailable { .. } => {
                                return Err(viewer_unavailable());
                            }
                            TerminalViewerOutcome::Attached { .. }
                            | TerminalViewerOutcome::Empty { .. }
                            | TerminalViewerOutcome::Detached { .. }
                            | TerminalViewerOutcome::RowsRejected { .. }
                            | TerminalViewerOutcome::Record(..) => {
                                return Err(viewer_protocol_invalid());
                            }
                        }
                    }
                    // A consumed ProjectionBlocked keeps the loop polling for
                    // an applicable frame; a replayed record was already
                    // rejected above as protocol-invalid.
                    Ok(None) => {}
                    Err(_) => return Err(viewer_protocol_invalid()),
                }
            }
            TerminalViewerOutcome::Rejected { reason } => {
                return Err(classify_viewer_rejected(*reason));
            }
            TerminalViewerOutcome::Unavailable { .. } => return Err(viewer_unavailable()),
            TerminalViewerOutcome::Attached { .. }
            | TerminalViewerOutcome::FeedbackAccepted { .. }
            | TerminalViewerOutcome::Detached { .. }
            | TerminalViewerOutcome::RowsRejected { .. } => return Err(viewer_protocol_invalid()),
        }
    }
}

/// The clock `wait-output` consults for deadline evaluation and quiet-poll
/// sleeping. The real command uses `SystemWaitClock`; tests inject fakes so
/// deadline and backoff behavior is deterministic.
trait WaitClock {
    fn now(&self) -> Instant;
    fn sleep(&self, duration: Duration);
}

struct SystemWaitClock;

impl WaitClock for SystemWaitClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// The wait command's controlled clock/cancel gate, shared by the
/// deadline-clamped preamble and the observation loop: a step that finishes
/// after the total deadline — success or failure — settles as timeout, and
/// a pending termination signal settles as cancelled.
struct WaitGate<'a> {
    clock: &'a dyn WaitClock,
    signal: &'a dyn TerminationSource,
    deadline_total: super::wait_deadline::WaitDeadline,
}

impl WaitGate<'_> {
    /// Cancellation wins over a same-instant expiry; both beat any step
    /// classification that completed late.
    fn settle(&self) -> Result<(), Failure> {
        if self.signal.requested() {
            return Err(Failure::new(FailureKind::Cancelled));
        }
        if self.deadline_total.expired(self.clock.now()) {
            return Err(Failure::new(FailureKind::Timeout));
        }
        Ok(())
    }

    /// The earlier of `allowance` and the remaining total, settled first.
    fn budget(&self, allowance: Duration) -> Result<Instant, Failure> {
        self.settle()?;
        self.deadline_total.step(self.clock.now(), allowance)
    }
    fn backoff(&self, allowance: Duration) -> Result<Duration, Failure> {
        self.settle()?;
        Ok(self.deadline_total.backoff(self.clock.now(), allowance))
    }
}

/// One `wait-output` run: the shared gate, the completed-observation count
/// the timeout report carries, the quiet-poll backoff, and the
/// per-attachment `request_sequence` the Host requires to be strictly
/// increasing.
struct WaitRun<'a> {
    gate: WaitGate<'a>,
    timeout_ms: Option<u32>,
    observations: u64,
    next_row_sequence: u64,
    poll_interval: Duration,
    after: Option<crate::invocation::OutputAfter>,
    terminal_id: String,
}

impl WaitRun<'_> {
    /// A gate failure becomes the detailed wait failure with the current
    /// observation count; any other kind passes through.
    fn map_clock_failure(&self, failure: Failure) -> Failure {
        match failure.kind {
            FailureKind::Timeout => self.timeout(),
            FailureKind::Cancelled => self.cancelled(),
            _ => failure,
        }
    }

    /// The earlier of the shared network budget and the absolute wait
    /// deadline; past the deadline there is no further budget.
    fn step_deadline(&self) -> Result<Instant, Failure> {
        self.gate
            .budget(NETWORK_BUDGET)
            .map_err(|failure| self.map_clock_failure(failure))
    }

    /// Settle cancellation and expiry against the wait's own detailed kinds.
    fn check(&self) -> Result<(), Failure> {
        self.gate
            .settle()
            .map_err(|failure| self.map_clock_failure(failure))
    }

    fn timeout(&self) -> Failure {
        self.timeout_ms.map_or_else(
            || Failure::new(FailureKind::InternalFailure),
            |ms| wait_timeout_failure(ms, self.observations),
        )
    }

    fn cancelled(&self) -> Failure {
        wait_cancelled_failure()
    }

    /// `ReadRows.request_sequence`: per attachment, from 1, strictly
    /// increasing on every attempt — including the retries a stale answer
    /// forced — and never related to transport request IDs or frame
    /// sequences. Overflow is a real failure, never a reset.
    fn next_sequence(&mut self) -> Result<std::num::NonZeroU64, Failure> {
        let sequence = self.next_row_sequence;
        self.next_row_sequence = sequence
            .checked_add(1)
            .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
        std::num::NonZeroU64::new(sequence)
            .ok_or_else(|| Failure::new(FailureKind::InternalFailure))
    }

    /// A quiet poll sleeps one clamped interval: never past the deadline,
    /// never a busy loop. Cancellation and expiry are re-checked first.
    fn sleep(&mut self) -> Result<(), Failure> {
        self.check()?;
        let remaining = self
            .gate
            .backoff(self.poll_interval)
            .map_err(|failure| self.map_clock_failure(failure))?;
        self.gate.clock.sleep(remaining);
        self.poll_interval = (self.poll_interval * 2).min(WAIT_POLL_BACKOFF);
        Ok(())
    }

    /// Any real frame record resets the backoff.
    fn note_record(&mut self) {
        self.poll_interval = EMPTY_POLL_INTERVAL;
    }
}

fn wait_output_inner(
    target: &Target,
    pattern: &crate::invocation::WaitPattern,
    window: crate::invocation::OutputSelection,
    timeout_ms: Option<u32>,
    after: Option<crate::invocation::OutputAfter>,
) -> Result<(serde_json::Value, Projection), Failure> {
    // The pattern was already validated — and for `--regex`, compiled —
    // before this command caused any effect; matching reuses it as-is.
    let matcher = match pattern {
        crate::invocation::WaitPattern::Literal(text) => {
            super::results::WaitMatcher::Literal(text.as_str())
        }
        crate::invocation::WaitPattern::Regex(regex) => super::results::WaitMatcher::Regex(regex),
    };
    let started = Instant::now();
    let deadline_total = super::wait_deadline::WaitDeadline::new(started, timeout_ms)?;
    let clock = SystemWaitClock;
    let signal =
        TerminationSignal::install().map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let gate = WaitGate {
        clock: &clock,
        signal: &signal,
        deadline_total,
    };
    let (mut client, _deadline, snapshot) = match snapshot_session_before(&gate) {
        Ok(session) => session,
        Err(failure) => {
            return Err(match failure.kind {
                // The preamble expired or was cancelled before any
                // observation could run.
                FailureKind::Timeout => match timeout_ms {
                    Some(ms) => wait_timeout_failure(ms, 0),
                    None => failure,
                },
                FailureKind::Cancelled => wait_cancelled_failure(),
                _ => failure,
            });
        }
    };
    let source = super::results::snapshot_source(&snapshot);
    let mut run = WaitRun {
        gate,
        timeout_ms,
        observations: 0,
        next_row_sequence: 1,
        poll_interval: EMPTY_POLL_INTERVAL,
        after,
        terminal_id: String::new(),
    };
    let attached = run.check().and_then(|()| {
        let deadline = run.step_deadline()?;
        attach_viewer(&mut client, deadline, &snapshot, target)
    });
    let (row, viewer_id, attachment, epoch, projection) = match attached {
        Ok(attached) => attached,
        // The attach exchange failed — possibly after the total deadline or
        // a pending cancel landed during it. Settle the gate before the
        // step's own classification: a late timeout/cancel outranks it. The
        // session drop is still the whole cleanup — a lost exchange may
        // have attached — so no cleanup field is claimed either way.
        Err(failure) => {
            drop(client);
            return Err(run.check().err().unwrap_or(failure).source(source));
        }
    };
    {
        run.terminal_id = row.terminal_id.clone();
    }
    // From here a real viewer attachment exists: every exit — including a
    // deadline or cancel that lands before the first poll — releases it
    // through the same confirmed-detach discipline, so the cleanup fact is
    // always honest.
    let outcome = run.check().and_then(|()| {
        wait_observe(
            &mut client,
            &mut run,
            &viewer_id,
            projection,
            &matcher,
            window,
        )
    });
    // Every exit releases the viewer the same way: an explicit `Detach`
    // inside the remaining budget, then the session drop. When nothing
    // remains, the drop alone is the cleanup and `requested` says so.
    let detach = match run.step_deadline() {
        Ok(deadline) => wait_detach(&mut client, deadline, &viewer_id),
        Err(failure) => Err(failure),
    };
    let cleanup = match &detach {
        Ok(()) => "confirmed",
        Err(failure) if failure.kind == FailureKind::Timeout => "requested",
        Err(_) => "unconfirmed",
    };
    // Whatever the detach outcome, a cancel or expiry that landed during
    // that exchange still wins the answer — over a confirmed `Detached`
    // and over the detach's own typed failure — while `cleanup` keeps the
    // real outcome instead of an invented one.
    let late = run.check().err();
    drop(client);
    match (outcome, detach) {
        (Ok(sample), Ok(())) if late.is_none() => {
            let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            let mut doc = super::results::terminal_wait_output_doc(
                row,
                attachment,
                epoch,
                sample.observation,
                super::results::WaitReport {
                    observations: run.observations,
                    requested_rows: sample.requested_rows,
                    timeout_ms,
                    elapsed_ms,
                },
            );
            doc.selection = Some(window);
            if let Some(crate::invocation::OutputAfter::Cursor(cursor)) = &run.after {
                doc.after = Some(cursor.encode());
            }
            if let Some(omitted) = sample.capacity_omitted {
                doc.window_truncated = omitted > 0;
            }
            Ok((source.clone(), Projection::TerminalWaitOutput(doc)))
        }
        // A match without a clean exit: the detach's own typed failure — or
        // the late cancel/expiry after a confirmed one — is the answer,
        // still honestly carrying that the match was observed.
        (Ok(_), detach_result) => Err(wait_cleanup_details(
            late.or_else(|| detach_result.err())
                .expect("a detach or late failure is present"),
            cleanup,
            true,
        )
        .source(source)),
        (Err(failure), _) => {
            Err(wait_cleanup_details(late.unwrap_or(failure), cleanup, false).source(source))
        }
    }
}

/// The observation loop: poll on the shared quiet cadence (10 ms backing
/// off to 100 ms on consecutive `Empty` answers, reset by any record),
/// decode every record through the projection, send the real feedback —
/// including `FrameApplied` for every applied frame — and confirm
/// `FeedbackAccepted`. Source-visible and empty selections use the frame;
/// other samples pin one `ReadRows` to that exact revision+generation.
/// A match completes the wait, an aged pin is
/// abandoned for a fresh frame, and every awaited step is followed by a
/// cancel+deadline re-check. No frame is skipped: the Host's single record
/// slot demands full consumption.
fn wait_observe(
    client: &mut LocalClient,
    run: &mut WaitRun<'_>,
    viewer_id: &ViewerId,
    mut projection: TerminalProjection,
    matcher: &super::results::WaitMatcher<'_>,
    window: crate::invocation::OutputSelection,
) -> Result<WaitSample, Failure> {
    loop {
        run.check()?;
        let poll = TerminalViewerRequest::new(
            obcs_request_id()?,
            TerminalViewerAction::Poll {
                attachment_id: viewer_id.clone(),
            },
        );
        let deadline = run.step_deadline()?;
        let result = client.submit_terminal_viewer_before(poll, deadline);
        run.check()?;
        let result = result.map_err(classify_viewer)?;
        match result.outcome() {
            TerminalViewerOutcome::Empty {} => run.sleep()?,
            TerminalViewerOutcome::Record(record) => {
                run.note_record();
                match projection.receive_for_geometry(record, None) {
                    Ok(Some(feedback)) => {
                        let applied = matches!(feedback, TerminalViewerAction::FrameApplied { .. });
                        let request = TerminalViewerRequest::new(obcs_request_id()?, feedback);
                        let deadline = run.step_deadline()?;
                        let result = client.submit_terminal_viewer_before(request, deadline);
                        run.check()?;
                        let result = result.map_err(classify_viewer)?;
                        match result.outcome() {
                            TerminalViewerOutcome::FeedbackAccepted {} => {}
                            TerminalViewerOutcome::Rejected { reason } => {
                                return Err(classify_viewer_rejected(*reason));
                            }
                            TerminalViewerOutcome::Unavailable { .. } => {
                                return Err(viewer_unavailable());
                            }
                            TerminalViewerOutcome::Attached { .. }
                            | TerminalViewerOutcome::Empty { .. }
                            | TerminalViewerOutcome::Detached { .. }
                            | TerminalViewerOutcome::RowsRejected { .. }
                            | TerminalViewerOutcome::Record(..) => {
                                return Err(viewer_protocol_invalid());
                            }
                        }
                        if applied {
                            // The just-applied frame pins this read's exact
                            // revision and generation; nothing from another
                            // revision is ever matched.
                            let frame = projection
                                .frame()
                                .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
                            if let Some(sample) = terminal_output::wait_sample(
                                client, run, viewer_id, frame, window, matcher,
                            )? {
                                return Ok(sample);
                            }
                        }
                    }
                    // Same as the one-shot read: a consumed ProjectionBlocked
                    // keeps polling for an applicable frame.
                    Ok(None) => {}
                    Err(_) => return Err(viewer_protocol_invalid()),
                }
            }
            TerminalViewerOutcome::Rejected { reason } => {
                return Err(classify_viewer_rejected(*reason));
            }
            TerminalViewerOutcome::Unavailable { .. } => return Err(viewer_unavailable()),
            TerminalViewerOutcome::Attached { .. }
            | TerminalViewerOutcome::FeedbackAccepted { .. }
            | TerminalViewerOutcome::Detached { .. }
            | TerminalViewerOutcome::RowsRejected { .. } => return Err(viewer_protocol_invalid()),
        }
    }
}

/// The confirmed-`Detach` exit `wait-output` requires: the real outcome is
/// inspected, unlike the best-effort send a one-shot read uses.
pub(super) fn wait_detach(
    client: &mut LocalClient,
    deadline: Instant,
    viewer_id: &ViewerId,
) -> Result<(), Failure> {
    let request = TerminalViewerRequest::new(
        obcs_request_id()?,
        TerminalViewerAction::Detach {
            attachment_id: viewer_id.clone(),
        },
    );
    let result = client
        .submit_terminal_viewer_before(request, deadline)
        .map_err(classify_viewer)?;
    match result.outcome() {
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

/// The timeout report: the wait stage, the caller's total budget, and the
/// completed-observation count — successful consistent window reads that
/// also completed a match pass, never a frame or revision count. Its
/// diagnostic says the awaited text never appeared.
fn wait_timeout_failure(timeout_ms: u32, observations: u64) -> Failure {
    Failure::new(FailureKind::Timeout)
        .details(serde_json::Map::from_iter([
            ("stage".to_owned(), serde_json::json!("wait")),
            ("timeoutMs".to_owned(), serde_json::json!(timeout_ms)),
            (
                "observed".to_owned(),
                serde_json::json!({ "completedObservations": observations }),
            ),
        ]))
        .awaiting(crate::cli_outcome::Awaited::TerminalText)
}

/// The cancelled report: the wait stage under the fixed kind. Its diagnostic
/// says the wait for the text was cancelled.
fn wait_cancelled_failure() -> Failure {
    Failure::new(FailureKind::Cancelled)
        .details(stage_details("wait"))
        .awaiting(crate::cli_outcome::Awaited::TerminalText)
}

/// Merge the honest cleanup state into a failure's details — plus
/// `matchObserved: true` when the text had already matched but the exit is
/// still a failure.
fn wait_cleanup_details(
    mut failure: Failure,
    cleanup: &'static str,
    match_observed: bool,
) -> Failure {
    if match_observed {
        // The text appeared; a later expiry or interrupt ended the cleanup,
        // not the wait.
        failure.clear_awaited();
    }
    let details = failure.details.get_or_insert_with(serde_json::Map::new);
    if match_observed {
        details.insert("matchObserved".to_owned(), serde_json::json!(true));
    }
    details.insert("cleanup".to_owned(), serde_json::json!(cleanup));
    failure
}

/// A launch read is a pure observation like the viewer paths: pre-submission
/// breakage is clean transport loss and a malformed or mismatched answer is a
/// protocol violation — never `outcome-unknown`, because nothing was mutated.
pub(in crate::cli) fn classify_launch_read(error: SubmissionError) -> Failure {
    match error {
        SubmissionError::ConnectionUnavailable | SubmissionError::RequestEncoding(_) => {
            Failure::new(FailureKind::InternalFailure)
        }
        SubmissionError::RequestNotSubmitted(_) => {
            Failure::new(FailureKind::TransportLoss).details(stage_details("launch-read"))
        }
        SubmissionError::OutcomeUnknown { reason, .. } => match reason {
            OutcomeUnknownReason::Write(_) | OutcomeUnknownReason::Read(_) => {
                Failure::new(FailureKind::TransportLoss).details(stage_details("launch-read"))
            }
            OutcomeUnknownReason::CorrelationMismatch { .. } => {
                Failure::new(FailureKind::ProtocolInvalid)
                    .details(stage_reason_details("launch-read", "correlation-mismatch"))
            }
            OutcomeUnknownReason::InvalidResult(_)
            | OutcomeUnknownReason::UnexpectedResult { .. } => {
                Failure::new(FailureKind::ProtocolInvalid).details(stage_details("launch-read"))
            }
        },
    }
}

pub(super) fn classify_viewer(error: SubmissionError) -> Failure {
    match error {
        SubmissionError::ConnectionUnavailable | SubmissionError::RequestEncoding(_) => {
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
pub(super) fn classify_viewer_rejected(reason: ViewerRejectedReason) -> Failure {
    match reason {
        ViewerRejectedReason::AlreadyAttached | ViewerRejectedReason::TerminalUnavailable => {
            viewer_unavailable()
        }
        ViewerRejectedReason::FeedbackMismatch => viewer_protocol_invalid(),
        ViewerRejectedReason::HostRejected => {
            Failure::new(FailureKind::Rejected).details(stage_details("viewer"))
        }
    }
}

fn viewer_unavailable() -> Failure {
    Failure::new(FailureKind::ServiceUnavailable).details(stage_details("viewer"))
}

pub(super) fn viewer_protocol_invalid() -> Failure {
    Failure::new(FailureKind::ProtocolInvalid).details(stage_details("viewer"))
}

fn rendezvous() -> Result<ModeCheckedLocalSessionRendezvous, Failure> {
    let (base, scope_text) = crate::local_activation::runtime_location()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let scope =
        SessionScope::parse(&scope_text).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let scope_directory = base.join(&scope_text);
    ModeCheckedLocalSessionRendezvous::validate_mode_checked(&base, scope)
        .map_err(|error| classify_rendezvous(error, &scope_directory))
}

/// Split rendezvous failures by the typed path they carried: an absent or
/// unsafe Session scope belongs to `runtime-directory`, while anything rooted
/// above it belongs to `runtime-base`. A scope ENOENT is service-unavailable
/// discovery, never a configuration failure.
pub(crate) fn classify_rendezvous(
    error: RendezvousError,
    scope_directory: &std::path::Path,
) -> Failure {
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

fn rendezvous_path(error: &RendezvousError) -> Option<&std::path::Path> {
    match error {
        RendezvousError::InvalidSessionScope => None,
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

pub(crate) fn classify_connect(error: crate::local_client::LocalClientConnectError) -> Failure {
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
                RejectionReason::CapacityExceeded => Failure::new(FailureKind::ConnectionCapacity)
                    .details(stage_details("admission")),
                RejectionReason::HandshakeTimeout => Failure::new(FailureKind::ServiceUnavailable)
                    .details(stage_details("admission")),
                RejectionReason::Stopping => stopping(),
                RejectionReason::WrongMessage | RejectionReason::MalformedFrame => {
                    Failure::new(FailureKind::ProtocolInvalid).details(stage_details("admission"))
                }
            },
            ConnectClientError::Refused(rejection) => classify_refusal(&rejection),
            // A Session that predates the handshake is older than every Client that offers one.
            ConnectClientError::PredatesHandshake => incompatible("session-too-old", None),
            ConnectClientError::Protocol(ProtocolError::IncompatibleVersion { .. }) => {
                Failure::new(FailureKind::Incompatible).details(stage_details("admission"))
            }
            ConnectClientError::Protocol(_) | ConnectClientError::UnexpectedResponse => {
                Failure::new(FailureKind::ProtocolInvalid).details(stage_details("admission"))
            }
            // Only `connect_before_cancellable` callers can observe these; the
            // current `None` path never produces them. The Agent read's signal
            // owner arbitrates the wake before classification.
            ConnectClientError::CancellationReady
            | ConnectClientError::CancellationSourceFault(_) => {
                Failure::new(FailureKind::InternalFailure)
            }
        },
        crate::local_client::LocalClientConnectError::RequestIoConfiguration(_) => {
            Failure::new(FailureKind::InternalFailure)
        }
    }
}

/// A handshake refusal keeps its reason; a generation refusal also names both product versions.
fn classify_refusal(rejection: &orchbee_connection_handshake::Rejection) -> Failure {
    use orchbee_connection_handshake::RejectionReason;
    match rejection.reason {
        RejectionReason::Capacity => {
            Failure::new(FailureKind::ConnectionCapacity).details(stage_details("admission"))
        }
        RejectionReason::Stopping => stopping(),
        RejectionReason::Timeout => {
            Failure::new(FailureKind::ServiceUnavailable).details(stage_details("admission"))
        }
        RejectionReason::InitiatorTooOld => {
            incompatible("client-too-old", Some(rejection.product_version.as_str()))
        }
        RejectionReason::InitiatorTooNew => {
            incompatible("session-too-old", Some(rejection.product_version.as_str()))
        }
        RejectionReason::EnvironmentMismatch => incompatible("environment-mismatch", None),
        RejectionReason::FingerprintMismatch => incompatible("fingerprint-mismatch", None),
        RejectionReason::ConnectionUnsupported
        | RejectionReason::Malformed
        | RejectionReason::Unknown => {
            Failure::new(FailureKind::ProtocolInvalid).details(stage_details("admission"))
        }
    }
}

/// A Session that is draining, stopping or exiting accepts no new Client.
fn stopping() -> Failure {
    Failure::new(FailureKind::ServiceUnavailable)
        .details(stage_reason_details("admission", "stopping"))
}

/// An incompatible admission, naming this Client's version and the Session's when it stated one.
fn incompatible(reason: &str, session_version: Option<&str>) -> Failure {
    let mut details = stage_reason_details("admission", reason);
    if reason.ends_with("-too-old") {
        details.insert(
            "clientVersion".into(),
            crate::local_client::CLIENT_VERSION.into(),
        );
        if let Some(version) = session_version {
            details.insert("sessionVersion".into(), version.into());
        }
    }
    Failure::new(FailureKind::Incompatible).details(details)
}

/// Preserve startup lane/Snapshot failure classification and its existing CLI schema selection.
pub(in crate::cli) fn classify_startup_admission(
    error: crate::agent_runtime::startup::AdmissionError,
) -> Failure {
    use crate::agent_runtime::startup::{AdmissionError, snapshot};
    match error {
        AdmissionError::Connect(error) => classify_connect(error),
        AdmissionError::Snapshot(snapshot::Error::Identity) => {
            Failure::new(FailureKind::InternalFailure)
        }
        AdmissionError::Snapshot(snapshot::Error::Exchange(error)) => {
            classify_read(error).failure_schema(super::schema::FAILURE_ID)
        }
        AdmissionError::Timeout => {
            Failure::new(FailureKind::Timeout).failure_schema(super::schema::FAILURE_ID)
        }
        AdmissionError::Denied => {
            Failure::new(FailureKind::Denied).failure_schema(super::schema::FAILURE_ID)
        }
        AdmissionError::Unavailable => Failure::new(FailureKind::ServiceUnavailable)
            .details(stage_details("snapshot"))
            .failure_schema(super::schema::FAILURE_ID),
    }
}

/// Translate a shared Snapshot exchange failure without adding a caller-specific schema or retry.
pub(in crate::cli) fn classify_snapshot(
    error: crate::agent_runtime::startup::snapshot::Error,
) -> Failure {
    match error {
        crate::agent_runtime::startup::snapshot::Error::Identity => {
            Failure::new(FailureKind::InternalFailure)
        }
        crate::agent_runtime::startup::snapshot::Error::Exchange(error) => classify_read(error),
    }
}

fn classify_read(error: SnapshotReadError) -> Failure {
    use orchbee_workbench_protocol::WorkbenchWireError;
    match error {
        SnapshotReadError::ConnectionUnavailable | SnapshotReadError::RequestEncoding(_) => {
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
fn fresh_request_id() -> Result<String, Failure> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let mut text = String::with_capacity(32);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(text, "{byte:02x}");
    }
    Ok(text)
}

pub(crate) fn request_id() -> Result<orchbee_workbench_protocol::WorkbenchRequestId, Failure> {
    orchbee_workbench_protocol::WorkbenchRequestId::parse(&fresh_request_id()?)
        .map_err(|_| Failure::new(FailureKind::InternalFailure))
}

pub(crate) fn obcs_request_id() -> Result<ObcsRequestId, Failure> {
    ObcsRequestId::parse(&fresh_request_id()?)
        .map_err(|_| Failure::new(FailureKind::InternalFailure))
}

/// The Agent read failure family a caller must answer with: pre-admission
/// failures keep the generic `orchbee.cli.failure` document, while every
/// post-admission exchange or semantic answer is documented by
/// `orchbee.cli.failure.agent`. A captured signal answers `cancelled` on
/// the agent document — the only schema carrying it for this family — and
/// owes the common emit its noncloneable finalization token.
/// The typed cancellation payload: the settled agent failure plus the owed
/// finalization token, boxed so the common error type stays small on paths
/// that never cancel.
pub(crate) struct AgentCancelled {
    pub(crate) failure: Failure,
    pub(crate) finalization: SourceFinalization,
}

pub(crate) enum AgentQueryError {
    Admission(Failure),
    Read(Failure),
    Cancelled(Box<AgentCancelled>),
}

/// Why the armed span never reached an admitted session — the generic
/// failure family. Preamble failures arrive already classified; the raw
/// connect outcome is held unprojected because it may carry the protocol's
/// provisional wake markers, which only the finish arbitration may weigh.
pub(super) enum AgentAdmissionError {
    Preamble(Failure),
    Connect(crate::local_client::LocalClientConnectError),
}

/// The armed Agent span's retained raw outcome — never projected to a
/// `Failure` before the signal owner's single arbitration, so provisional
/// wake markers survive intact to that point. The variant carries the
/// phase: `Admission` answers the generic document, the rest answer the
/// agent document.
enum AgentAttempt<Response = AgentReadResult> {
    Admission(AgentAdmissionError),
    /// The shared absolute budget expired before or after the exchange.
    Expired,
    /// One OBAP exchange settled to its raw result or typed failure.
    Exchanged(Result<Response, AgentExchangeError>),
    /// Target resolution failed before any session read was submitted.
    Target(Failure),
}

/// The armed Agent interruption owner as one pollable descriptor plus one
/// finish arbitration — the private seam where a test substitutes a
/// recording watch to prove the admitted session is dropped first. Only
/// `AgentSignals` implements it in production.
trait AgentWatch {
    /// The borrowed signal descriptor joined into every transport wait.
    fn poll_fd(&self) -> Option<BorrowedFd<'_>>;
    /// The single arbitration read after the armed span.
    fn finish(self) -> std::io::Result<AgentFinish>;
}

impl AgentWatch for AgentSignals {
    fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        AgentSignals::poll_fd(self)
    }

    fn finish(self) -> std::io::Result<AgentFinish> {
        AgentSignals::finish(self)
    }
}

trait AgentReadExchange {
    type Response;
    fn exchange(
        self,
        client: &mut LocalClient,
        total: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<Self::Response, AgentExchangeError>;
}

impl AgentReadExchange for AgentReadRequest {
    type Response = AgentReadResult;
    fn exchange(
        self,
        client: &mut LocalClient,
        total: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<Self::Response, AgentExchangeError> {
        client.submit_agent_read_before_cancellable(self, total, wake)
    }
}

impl AgentReadExchange for orchbee_agent_observation_protocol::AgentChangesRequest {
    type Response = orchbee_agent_observation_protocol::AgentChangesResult;
    fn exchange(
        self,
        client: &mut LocalClient,
        total: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<Self::Response, AgentExchangeError> {
        client.submit_agent_changes_before_cancellable(self, total, wake)
    }
}

/// One armed Agent read command: after grammar validation and request
/// identity, the interruption guard arms before the synchronous preamble
/// and owns SIGINT/SIGTERM for the whole `T0+5s` span; the borrowed wake
/// descriptor joins every transport wait; the Client Connection is dropped
/// before the single `finish` arbitration; and no early return may bypass
/// that arbitration once armed. `Clear` hands the raw attempt back for the
/// normal classification; `Cancelled` answers the typed 130 family.
fn agent_armed_run<Request: AgentReadExchange>(
    operation: AgentReadOperation,
    build: impl FnOnce(
        &mut LocalClient,
        ObapRequestId,
        Instant,
        Option<BorrowedFd<'_>>,
    ) -> Result<Request, Failure>,
) -> Result<(ObapRequestId, AgentAttempt<Request::Response>), AgentQueryError> {
    let total = deadline_after(AGENT_TOTAL_BUDGET).map_err(AgentQueryError::Admission)?;
    let request_id = obap_request_id().map_err(AgentQueryError::Admission)?;
    let armed = AgentSignals::arm_interrupts()
        .map_err(|_| AgentQueryError::Admission(Failure::new(FailureKind::InternalFailure)))?;
    agent_armed_finish(
        operation,
        build,
        total,
        request_id,
        armed,
        agent_session_until,
    )
}

/// The armed span's finish half, separated so a recording watch can prove
/// the admitted session is already dropped when the arbitration read runs:
/// `agent_span` owns the session and drops it before returning; only then
/// does `armed.finish()` read the signal descriptor. Nothing here
/// classifies.
fn agent_armed_finish<Request: AgentReadExchange>(
    operation: AgentReadOperation,
    build: impl FnOnce(
        &mut LocalClient,
        ObapRequestId,
        Instant,
        Option<BorrowedFd<'_>>,
    ) -> Result<Request, Failure>,
    total: Instant,
    request_id: ObapRequestId,
    armed: impl AgentWatch,
    acquire: impl FnOnce(Instant, Option<BorrowedFd<'_>>) -> Result<LocalClient, AgentAdmissionError>,
) -> Result<(ObapRequestId, AgentAttempt<Request::Response>), AgentQueryError> {
    let attempt = {
        let wake = armed.poll_fd();
        agent_span(total, operation, wake, request_id.clone(), build, acquire)
    };
    match armed.finish() {
        Err(_) => Err(match attempt {
            AgentAttempt::Admission(_) => {
                AgentQueryError::Admission(Failure::new(FailureKind::InternalFailure))
            }
            _ => AgentQueryError::Read(Failure::new(FailureKind::InternalFailure)),
        }),
        Ok(AgentFinish::Cancelled { finalization, .. }) => {
            Err(AgentQueryError::Cancelled(Box::new(AgentCancelled {
                // A cancelled read carries no source: only a trustworthy semantic
                // answer may publish the correlated request identity.
                failure: Failure::new(FailureKind::Cancelled)
                    .details(agent_read_details(operation)),
                finalization,
            })))
        }
        Ok(AgentFinish::Clear) => Ok((request_id, attempt)),
    }
}

/// The armed span body: admission under the borrowed wake descriptor, the
/// pre-submission settle, one OBAP exchange, then the session is dropped —
/// all before the caller's finish arbitration. Nothing here classifies.
fn agent_span<Request: AgentReadExchange>(
    total: Instant,
    operation: AgentReadOperation,
    wake: Option<BorrowedFd<'_>>,
    request_id: ObapRequestId,
    build: impl FnOnce(
        &mut LocalClient,
        ObapRequestId,
        Instant,
        Option<BorrowedFd<'_>>,
    ) -> Result<Request, Failure>,
    acquire: impl FnOnce(Instant, Option<BorrowedFd<'_>>) -> Result<LocalClient, AgentAdmissionError>,
) -> AgentAttempt<Request::Response> {
    let mut client = match acquire(total, wake) {
        Err(error) => return AgentAttempt::Admission(error),
        Ok(client) => client,
    };
    if agent_settle(total, operation).is_err() {
        drop(client);
        return AgentAttempt::Expired;
    }
    let request = match build(&mut client, request_id, total, wake) {
        Ok(request) => request,
        Err(failure) => return AgentAttempt::Target(failure),
    };
    let result = request.exchange(&mut client, total, wake);
    // The Client Connection is fully dropped before the owner arbitrates.
    drop(client);
    if agent_settle(total, operation).is_err() {
        return AgentAttempt::Expired;
    }
    AgentAttempt::Exchanged(result)
}

/// One admitted Client Connection for an Agent observation read under the
/// caller's single absolute budget: the same cohort → discovery → socket →
/// admission preamble as every network query, then one connect — each stage
/// bounded by `total`, never a renewed budget. The wake descriptor joins
/// the admission wait set; nothing here may create state, replay a request,
/// or touch the Workbench.
pub(super) fn agent_session_until(
    total: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<LocalClient, AgentAdmissionError> {
    let cohort_deadline = bounded_deadline(Instant::now(), COHORT_BUDGET, total)
        .map_err(AgentAdmissionError::Preamble)?;
    let (cohort, session) =
        session_preamble(cohort_deadline, || Ok(())).map_err(AgentAdmissionError::Preamble)?;
    LocalClient::connect_before_cancellable(
        session.client_socket_path(),
        &crate::local_client::client_offer(Some(cohort.identity())),
        total,
        wake,
    )
    .map_err(AgentAdmissionError::Connect)
}

/// Post-step settlement of the shared absolute budget: at or past the
/// ceiling the step's outcome is the command's own Agent timeout — never a
/// late success or a stale transport fault.
fn agent_settle(total: Instant, operation: AgentReadOperation) -> Result<(), Failure> {
    if Instant::now() >= total {
        return Err(Failure::new(FailureKind::Timeout).details(agent_timeout_details(operation)));
    }
    Ok(())
}

/// Classify a cleared span outcome the existing way: admission outcomes
/// keep the generic failure family, the expiry marker is the command's own
/// Agent timeout, and a raw exchange result or failure maps through the
/// post-admission classifiers — provisional wake markers left readable
/// without a captured signal land on `internal-failure`.
fn classify_attempt<Response>(
    attempt: AgentAttempt<Response>,
    operation: AgentReadOperation,
) -> Result<Response, AgentQueryError> {
    match attempt {
        AgentAttempt::Admission(AgentAdmissionError::Preamble(failure)) => {
            Err(AgentQueryError::Admission(failure))
        }
        AgentAttempt::Admission(AgentAdmissionError::Connect(error)) => {
            Err(AgentQueryError::Admission(classify_connect(error)))
        }
        AgentAttempt::Target(failure) => Err(AgentQueryError::Read(failure)),
        AgentAttempt::Expired => Err(AgentQueryError::Read(
            Failure::new(FailureKind::Timeout).details(agent_timeout_details(operation)),
        )),
        AgentAttempt::Exchanged(result) => {
            result.map_err(|error| AgentQueryError::Read(classify_agent_read(error, operation)))
        }
    }
}

/// `agent list`: one OBAP list request on one admitted session, then the
/// session is dropped before any output work. The `listed` outcome is the
/// only success path; every semantic refusal maps to its fixed failure kind
/// and carries the correlated request identity as the honest source.
pub(crate) fn agent_list(
    include_ended: bool,
    limit: u64,
    cursor: Option<orchbee_agent_observation_protocol::AgentListCursor>,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    super::execution::admit(super::execution::Capability::CanonicalAgentList)
        .map_err(AgentQueryError::Admission)?;
    if super::execution::admit(super::execution::Capability::CanonicalAgentRead).is_err() {
        return agent_list_unarmed::run(include_ended, limit, cursor);
    }
    let (request_id, attempt) =
        agent_armed_run(AgentReadOperation::List, |_, request_id, _, _| {
            Ok(AgentReadRequest::List {
                request_id,
                include_ended,
                limit: AgentSafeInteger::new(limit),
                cursor,
            })
        })?;
    let AgentReadResult::List { result, .. } = classify_attempt(attempt, AgentReadOperation::List)?
    else {
        return Err(AgentQueryError::Read(agent_protocol_invalid(
            AgentReadOperation::List,
        )));
    };
    let outcome = listed_outcome(result.outcome, &request_id).map_err(AgentQueryError::Read)?;
    Ok((
        super::results::agent_source(&request_id),
        Projection::AgentList(outcome),
    ))
}

fn listed_outcome(
    outcome: AgentListOutcome,
    request_id: &ObapRequestId,
) -> Result<AgentListOutcome, Failure> {
    let source = super::results::agent_source(request_id);
    match outcome {
        AgentListOutcome::Listed { .. } => Ok(outcome),
        AgentListOutcome::Rejected { reason } => Err(Failure::new(
            if reason == orchbee_agent_observation_protocol::AgentListRejectionReason::ReadDenied {
                FailureKind::Denied
            } else {
                FailureKind::Rejected
            },
        )
        .details(agent_rejection_details(
            AgentReadOperation::List,
            rejection_reason_text(&reason),
        ))
        .source(source)),
        AgentListOutcome::Unavailable { .. } => Err(Failure::new(FailureKind::ServiceUnavailable)
            .details(agent_unavailable_details(AgentReadOperation::List))
            .source(source)),
    }
}

/// Bounded list read for compositions that already own admission and signals.
pub(in crate::cli) fn read_agent_list(
    client: &mut LocalClient,
    include_ended: bool,
    limit: u64,
    cursor: Option<orchbee_agent_observation_protocol::AgentListCursor>,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<(ObapRequestId, AgentListOutcome), Failure> {
    let read = || {
        let operation = AgentReadOperation::List;
        let request_id = obap_request_id()?;
        let result = client
            .submit_agent_read_before_cancellable(
                AgentReadRequest::List {
                    request_id: request_id.clone(),
                    include_ended,
                    limit: AgentSafeInteger::new(limit),
                    cursor,
                },
                deadline,
                wake,
            )
            .map_err(|error| classify_agent_read(error, operation))?;
        if Instant::now() >= deadline {
            return Err(
                Failure::new(FailureKind::Timeout).details(agent_timeout_details(operation))
            );
        }
        let AgentReadResult::List { result, .. } = result else {
            return Err(agent_protocol_invalid(operation));
        };
        listed_outcome(result.outcome, &request_id).map(|outcome| (request_id, outcome))
    };
    read().map_err(|failure| failure.failure_schema(super::schema::AGENT_FAILURE_ID))
}

/// `agent get`: one OBAP get request, same admission/exchange discipline.
pub(crate) fn agent_get(
    target: &crate::invocation::AgentTarget,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    super::execution::admit(super::execution::Capability::CanonicalAgentRead)
        .map_err(AgentQueryError::Admission)?;
    agent_session_read(AgentReadOperation::Get, target)
}

/// `agent explain`: resolve once and read on the same admitted session.
pub(crate) fn agent_explain(
    target: &crate::invocation::AgentTarget,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    if super::execution::admit(super::execution::Capability::AgentProjection).is_ok() {
        agent_explanation::agent_explain(target)
    } else {
        super::execution::admit(super::execution::Capability::CanonicalAgentRead)
            .map_err(AgentQueryError::Admission)?;
        agent_session_read(AgentReadOperation::Explain, target)
    }
}

/// Admission, optional target resolution and the read share one deadline.
fn agent_session_read(
    operation: AgentReadOperation,
    target: &crate::invocation::AgentTarget,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    super::execution::admit_agent_target(target).map_err(AgentQueryError::Admission)?;
    let mut resolved = None;
    let (request_id, attempt) = agent_armed_run(operation, |client, request_id, total, wake| {
        let session_id =
            agent_target::resolve_on_session(client, target, None, total, wake)?.session_id;
        resolved = Some(session_id.clone());
        match operation {
            AgentReadOperation::Get => Ok(AgentReadRequest::Get {
                request_id,
                session_id,
            }),
            AgentReadOperation::Explain => Ok(AgentReadRequest::Explain {
                request_id,
                session_id,
            }),
            _ => Err(Failure::new(FailureKind::InternalFailure)),
        }
    })?;
    let result = classify_attempt(attempt, operation)?;
    let Some(session_id) = resolved.as_ref() else {
        return Err(AgentQueryError::Read(Failure::new(
            FailureKind::InternalFailure,
        )));
    };
    let source = super::results::agent_source(&request_id);
    match (operation, result) {
        (
            AgentReadOperation::Get,
            AgentReadResult::Get {
                result: envelope, ..
            },
        ) => match envelope.outcome {
            AgentGetOutcome::Found { ref session, .. }
                if session.snapshot.session_id == *session_id =>
            {
                Ok((source, Projection::AgentGet(envelope.outcome)))
            }
            AgentGetOutcome::Found { .. } => {
                Err(AgentQueryError::Read(agent_protocol_invalid(operation)))
            }
            AgentGetOutcome::Rejected { reason } => Err(AgentQueryError::Read(
                agent_session_rejection(operation, reason, session_id).source(source),
            )),
            AgentGetOutcome::Unavailable { .. } => Err(AgentQueryError::Read(
                Failure::new(FailureKind::ServiceUnavailable)
                    .details(agent_unavailable_details(operation))
                    .source(source),
            )),
        },
        (
            AgentReadOperation::Explain,
            AgentReadResult::Explain {
                result: envelope, ..
            },
        ) => match envelope.outcome {
            AgentExplainOutcome::Found { ref session, .. }
                if session.snapshot.session_id == *session_id =>
            {
                Ok((source, Projection::AgentExplain(envelope.outcome)))
            }
            AgentExplainOutcome::Found { .. } => {
                Err(AgentQueryError::Read(agent_protocol_invalid(operation)))
            }
            AgentExplainOutcome::Rejected { reason } => Err(AgentQueryError::Read(
                agent_session_rejection(operation, reason, session_id).source(source),
            )),
            AgentExplainOutcome::Unavailable { .. } => Err(AgentQueryError::Read(
                Failure::new(FailureKind::ServiceUnavailable)
                    .details(agent_unavailable_details(operation))
                    .source(source),
            )),
        },
        // This query consumes the requested Agent read result; a different read family cannot answer it.
        _ => Err(AgentQueryError::Read(agent_protocol_invalid(operation))),
    }
}

/// The `get`/`explain` refusal mapping: `not-found` names the session, while
/// `read-denied` carries only the typed reason — both under the common
/// `agent-read` stage.
pub(super) fn agent_session_rejection(
    operation: AgentReadOperation,
    reason: orchbee_agent_observation_protocol::AgentSessionRejectionReason,
    session_id: &AgentSessionId,
) -> Failure {
    match reason {
        orchbee_agent_observation_protocol::AgentSessionRejectionReason::NotFound => {
            Failure::new(FailureKind::NotFound)
                .details(agent_session_details(operation, session_id.as_str()))
        }
        orchbee_agent_observation_protocol::AgentSessionRejectionReason::ReadDenied => {
            Failure::new(FailureKind::Denied)
                .details(agent_rejection_details(operation, "read-denied"))
        }
    }
}

/// Common `agent-read` details: the stage plus the operation, as the failure
/// schema closes them. Session-targeted failures add `sessionId`.
pub(super) fn agent_read_details(
    operation: AgentReadOperation,
) -> serde_json::Map<String, serde_json::Value> {
    let mut details = serde_json::Map::new();
    details.insert("stage".to_owned(), serde_json::json!("agent-read"));
    details.insert(
        "operation".to_owned(),
        serde_json::json!(operation.as_str()),
    );
    details
}

pub(super) fn agent_session_details(
    operation: AgentReadOperation,
    session_id: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let mut details = agent_read_details(operation);
    details.insert("sessionId".to_owned(), serde_json::json!(session_id));
    details
}

pub(super) fn agent_rejection_details(
    operation: AgentReadOperation,
    reason: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let mut details = agent_read_details(operation);
    details.insert("reason".to_owned(), serde_json::json!(reason));
    details
}

pub(super) fn agent_unavailable_details(
    operation: AgentReadOperation,
) -> serde_json::Map<String, serde_json::Value> {
    agent_rejection_details(operation, "runtime-unavailable")
}

/// The fixed `timeoutMs` the agent-read failure schema publishes: the shared
/// absolute command budget in milliseconds.
fn agent_timeout_details(
    operation: AgentReadOperation,
) -> serde_json::Map<String, serde_json::Value> {
    let mut details = agent_read_details(operation);
    details.insert(
        "timeoutMs".to_owned(),
        serde_json::json!(AGENT_TOTAL_BUDGET.as_millis() as u64),
    );
    details
}

pub(super) fn agent_protocol_invalid(operation: AgentReadOperation) -> Failure {
    Failure::new(FailureKind::ProtocolInvalid).details(agent_read_details(operation))
}

/// A deadline-elapsed transport error is the command's own timeout, never a
/// transport loss: `TimedOut` only arises from the shared deadline inside the
/// exchange machinery.
pub(super) fn is_deadline_elapsed(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::TimedOut
}

/// A provisional wake marker surviving to `Clear` classification is a
/// contradiction — the descriptor reported readiness or a fault yet the
/// owner read no signal. Both map to `internal-failure`, never cancellation
/// or a transport verdict.
fn wake_marker_failure(error: &std::io::Error) -> Option<Failure> {
    if super::execution::admit(super::execution::Capability::CanonicalAgentRead).is_ok()
        && (exchange_wake_ready(error) || exchange_wake_fault(error))
    {
        Some(Failure::new(FailureKind::InternalFailure))
    } else {
        None
    }
}

/// Post-admission Agent read failures: a semantic answer maps to its fixed
/// kind, an expired deadline to timeout, a transport fault to transport-loss,
/// and any malformed/correlated violation to protocol-invalid — every
/// document under the common `agent-read` stage, none with a source.
pub(in crate::cli) fn classify_agent_read(
    error: AgentExchangeError,
    operation: AgentReadOperation,
) -> Failure {
    match error {
        AgentExchangeError::ConnectionUnavailable | AgentExchangeError::RequestEncoding(_) => {
            Failure::new(FailureKind::InternalFailure)
        }
        AgentExchangeError::RequestNotSubmitted(error) => {
            if let Some(failure) = wake_marker_failure(&error) {
                failure
            } else if is_deadline_elapsed(&error) {
                Failure::new(FailureKind::Timeout).details(agent_timeout_details(operation))
            } else {
                Failure::new(FailureKind::TransportLoss).details(agent_read_details(operation))
            }
        }
        AgentExchangeError::Failed { reason, .. } => classify_agent_failure(reason, operation),
    }
}

fn classify_agent_failure(reason: AgentExchangeFailure, operation: AgentReadOperation) -> Failure {
    match reason {
        AgentExchangeFailure::Write(error)
        | AgentExchangeFailure::ResponseDeadline(error)
        | AgentExchangeFailure::Response(AgentWireError::Io(error)) => {
            if let Some(failure) = wake_marker_failure(&error) {
                failure
            } else if is_deadline_elapsed(&error) {
                Failure::new(FailureKind::Timeout).details(agent_timeout_details(operation))
            } else {
                Failure::new(FailureKind::TransportLoss).details(agent_read_details(operation))
            }
        }
        AgentExchangeFailure::Response(_)
        | AgentExchangeFailure::UnexpectedResponse
        | AgentExchangeFailure::CorrelationMismatch { .. } => agent_protocol_invalid(operation),
    }
}

/// The OBAP request identity minted per command: sixteen fresh bytes as
/// canonical 32-lowercase-hex, never reused.
pub(crate) fn obap_request_id() -> Result<ObapRequestId, Failure> {
    crate::agent_runtime::startup::obap_identity()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))
}

fn rejection_reason_text(
    reason: &orchbee_agent_observation_protocol::AgentListRejectionReason,
) -> &'static str {
    match reason {
        orchbee_agent_observation_protocol::AgentListRejectionReason::CursorInvalid => {
            "cursor-invalid"
        }
        orchbee_agent_observation_protocol::AgentListRejectionReason::CursorStale => "cursor-stale",
        orchbee_agent_observation_protocol::AgentListRejectionReason::ReadDenied => "read-denied",
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
#[cfg(test)]
pub(crate) fn read_terminal(
    _target: &crate::invocation::Target,
    _history: Option<u16>,
    _presentation: super::results::ReadPresentation,
) -> Result<(serde_json::Value, super::results::Projection), super::Failure> {
    Err(
        super::Failure::new(super::FailureKind::ServiceUnavailable).details(
            super::stage_reason_details("platform", "unsupported-platform"),
        ),
    )
}

pub(super) fn names_command(
    command: &crate::invocation::AgentNamesCommand,
) -> agent_names::Execution {
    agent_names::execute(command)
}
pub(crate) fn agent_changes(
    limit: u64,
    cursor: Option<orchbee_agent_observation_protocol::AgentChangesCursor>,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    use orchbee_agent_observation_protocol::{
        AgentChangesOutcome, AgentChangesRejectionReason, AgentChangesRequest, AgentChangesResult,
    };
    super::execution::admit(super::execution::Capability::AgentProjection)
        .map_err(AgentQueryError::Admission)?;
    let operation = AgentReadOperation::Changes;
    let (request_id, attempt) = agent_armed_run(operation, |_, request_id, _, _| {
        Ok(AgentChangesRequest::Read {
            request_id,
            cursor,
            limit: AgentSafeInteger::new(limit),
        })
    })?;
    let AgentChangesResult::Read { result, .. } = classify_attempt(attempt, operation)?;
    let source = super::results::agent_source(&request_id);
    match result.outcome {
        AgentChangesOutcome::Changes { .. }
        | AgentChangesOutcome::Current { .. }
        | AgentChangesOutcome::ResetRequired { .. } => {
            Ok((source, Projection::AgentChanges(result.outcome)))
        }
        AgentChangesOutcome::Rejected { reason } => Err(AgentQueryError::Read(
            Failure::new(if reason == AgentChangesRejectionReason::ReadDenied {
                FailureKind::Denied
            } else {
                FailureKind::Rejected
            })
            .details(agent_rejection_details(
                operation,
                if reason == AgentChangesRejectionReason::ReadDenied {
                    "read-denied"
                } else {
                    "cursor-invalid"
                },
            ))
            .source(source),
        )),
        AgentChangesOutcome::Unavailable { .. } => Err(AgentQueryError::Read(
            Failure::new(FailureKind::ServiceUnavailable)
                .details(agent_unavailable_details(operation))
                .source(source),
        )),
    }
}

pub(in crate::cli) fn workbench_failure(reason: &'static str) -> Failure {
    Failure::new(if reason == "agent-page-invalid" {
        FailureKind::ProtocolInvalid
    } else {
        FailureKind::Rejected
    })
    .failure_schema("orchbee.cli.failure.workbench-query")
    .details(super::stage_reason_details("workbench-query", reason))
}
