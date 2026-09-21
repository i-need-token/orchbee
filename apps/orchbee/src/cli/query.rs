//! The single read-only Site query: existing-only rendezvous, one OBLC
//! admission, one snapshot read, then the session is dropped. Nothing here may
//! create directories, acquire locks, start processes, or mutate the Workbench.

#[cfg(target_os = "linux")]
use std::os::fd::BorrowedFd;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};
#[cfg(target_os = "linux")]
use std::{env, path::PathBuf};

#[cfg(target_os = "linux")]
use orchbee_agent_observation_protocol::{
    AgentExplainOutcome, AgentGetOutcome, AgentListOutcome, AgentReadRequest, AgentReadResult,
    AgentSafeInteger, AgentSessionId, AgentWireError, ObapRequestId,
};
#[cfg(target_os = "linux")]
use orchbee_client_server_protocol::{
    RequestId as ObcsRequestId, SiteId as ObcsSiteId, TerminalId as ObcsTerminalId,
    TerminalLaunchReadOutcome, TerminalLaunchReadRequest, TerminalViewerAction,
    TerminalViewerOutcome, TerminalViewerRequest, ViewerId, ViewerRejectedReason,
    WorkerId as ObcsWorkerId,
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
    LaunchProfileCatalogOutcome, LaunchProfileSelector, ProtocolVersion, Snapshot,
    SnapshotReadIntent, WorkbenchDirectoryId, WorkbenchLaunchProfileCatalogRead,
    WorkbenchRequestId, WorkbenchSnapshotOutcome, WorkbenchSnapshotUnavailableReason,
    WorkbenchWorktreeListOutcome, WorkbenchWorktreeListRead, WorktreeListStage,
    WorktreeListTooLargeOrigin, WorktreeListUnavailableReason,
};

#[cfg(target_os = "linux")]
use super::results::Projection;
#[cfg(target_os = "linux")]
use super::source::{ArmedSignals, Finish, SignalWatch};
#[cfg(target_os = "linux")]
use super::{
    Failure, FailureKind, SourceFinalization, stage_details, stage_reason_details, target_details,
};
#[cfg(target_os = "linux")]
use crate::invocation::{Resource, Target};
#[cfg(target_os = "linux")]
use crate::local_client::agent::{AgentReadError, AgentReadFailure, AgentReadOperation};
#[cfg(target_os = "linux")]
use crate::local_client::workbench::{
    LaunchProfileCatalogReadError, LaunchProfileCatalogReadFailure, SnapshotReadError,
    SnapshotReadFailure, WorktreeListReadError, WorktreeListReadFailure,
};
#[cfg(target_os = "linux")]
use crate::local_client::{
    LocalClient, OutcomeUnknownReason, SubmissionError, exchange_wake_fault, exchange_wake_ready,
};
#[cfg(target_os = "linux")]
use crate::terminal_projection::TerminalProjection;
#[cfg(target_os = "linux")]
use crate::termination_signal::{TerminationSignal, TerminationSource};

#[cfg(target_os = "linux")]
mod agent_wait;
#[cfg(target_os = "linux")]
pub(crate) use agent_wait::agent_wait;

#[cfg(target_os = "linux")]
const COHORT_BUDGET: Duration = Duration::from_secs(5);
/// The Agent read's whole-command budget: one absolute deadline sampled
/// once at invocation entry and shared by the preamble, the admission, and
/// the single OBAP exchange — never refreshed between stages.
#[cfg(target_os = "linux")]
const AGENT_TOTAL_BUDGET: Duration = Duration::from_secs(5);
#[cfg(target_os = "linux")]
const NETWORK_BUDGET: Duration = Duration::from_secs(5);
/// An `Empty` poll answer is retried inside the shared network budget; the
/// cadence stays coarse enough that a quiet host is never hammered.
#[cfg(target_os = "linux")]
const EMPTY_POLL_INTERVAL: Duration = Duration::from_millis(10);
/// A quiet wait backs off on consecutive `Empty` answers up to this ceiling;
/// any real frame record resets the cadence to `EMPTY_POLL_INTERVAL`.
#[cfg(target_os = "linux")]
const WAIT_POLL_BACKOFF: Duration = Duration::from_millis(100);

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

/// `terminal get`: the snapshot session admits the transport and supplies the
/// typed target id — nothing else. The answer is the Server's own join under
/// the coordinator lock: the current binding, the committed revision, and the
/// retained launch fact. A stale snapshot route is never echoed back.
#[cfg(target_os = "linux")]
pub(crate) fn terminal_get(target: &Target) -> Result<(serde_json::Value, Projection), Failure> {
    let (mut client, deadline, snapshot) = snapshot_session()?;
    // A typed ID passes through to the Server's launch read unchanged — it
    // answers binding state itself. A `@terminal:` selector resolves against
    // the snapshot's complete bound set first; the request then carries the
    // resolved full id.
    let identity = match target {
        Target::Id(identity) => identity.clone(),
        Target::Selector(_) => super::results::terminal_select(&snapshot, target)
            .map_err(|failure| failure.source(super::results::snapshot_source(&snapshot)))?
            .terminal_id
            .clone(),
        // The parser admits only typed IDs and selectors for terminal targets.
        Target::Label(_) => return Err(Failure::new(FailureKind::InternalFailure)),
    };
    let request = TerminalLaunchReadRequest::new(
        obcs_request_id()?,
        ObcsTerminalId::parse(identity.as_str())
            .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
    );
    let outcome = client
        .submit_terminal_launch_read_before(request, deadline)
        .map_err(classify_launch_read);
    // The Client Session is fully dropped before the answer is mapped into the
    // document or any output is prepared.
    drop(client);
    let result = outcome?;
    match result.outcome() {
        TerminalLaunchReadOutcome::Found {
            site_id,
            workbench_revision,
            terminal,
            launch,
            launch_profile,
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
                super::results::launch_read_source(site_id.as_str(), *workbench_revision),
                Projection::TerminalGet(super::results::TerminalGetDoc {
                    terminal: row,
                    launch: (**launch).clone(),
                    launch_profile: launch_profile.clone(),
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
/// observes the terminal's current projection — or, under `--history`, the
/// retained scrollback window behind it. The attachment is released
/// (best-effort `Detach` plus the session drop) before any output work.
#[cfg(target_os = "linux")]
pub(crate) fn read_terminal(
    target: &Target,
    history: Option<u16>,
    presentation: super::results::ReadPresentation,
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
#[cfg(target_os = "linux")]
pub(crate) fn worktree_list(
    project: &Target,
    directory: &WorkbenchDirectoryId,
) -> Result<(serde_json::Value, Projection), Failure> {
    let deadline = deadline_after(NETWORK_BUDGET)?;
    let (mut client, deadline, snapshot) = snapshot_session_until(deadline)?;
    let source = super::results::snapshot_source(&snapshot);
    let outcome = worktree_list_exchange(&mut client, deadline, &snapshot, project, directory);
    // The Client Session is fully dropped before the answer is mapped into
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
#[cfg(target_os = "linux")]
fn worktree_list_exchange(
    client: &mut LocalClient,
    deadline: Instant,
    snapshot: &Snapshot,
    project: &Target,
    directory: &WorkbenchDirectoryId,
) -> Result<(serde_json::Value, super::results::WorktreeListDoc), Failure> {
    let resolved = super::results::resolve_or_fail(
        snapshot
            .navigation
            .projects
            .iter()
            .map(|project| (project.project_id.as_str(), project.label.as_str(), project)),
        project,
        Resource::Project,
    )?;
    let request = WorkbenchWorktreeListRead {
        protocol_version: ProtocolVersion,
        request_id: request_id()?,
        project_id: resolved.project_id.clone(),
        directory_id: directory.clone(),
    };
    let result = client
        .submit_workbench_worktree_list_read_before(request, deadline)
        .map_err(classify_worktree_read)?;
    match result.outcome {
        WorkbenchWorktreeListOutcome::Found {
            site_id,
            workbench_revision,
            project_id,
            directory_id,
            path,
            entries,
        } => Ok((
            serde_json::json!({
                "kind": "worktree-list",
                "siteId": site_id.as_str(),
                "revision": workbench_revision,
            }),
            super::results::WorktreeListDoc {
                project_id: project_id.as_str().to_owned(),
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
#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
const fn worktree_git_stage(stage: WorktreeListStage) -> &'static str {
    match stage {
        WorktreeListStage::Probe => "probe",
        WorktreeListStage::List => "list",
    }
}

/// The typed worktree-list exchange failures, classified under the worktree
/// failure envelope: a deadline-expired response is the command's own
/// timeout, never a transport guess.
#[cfg(target_os = "linux")]
fn classify_worktree_read(error: WorktreeListReadError) -> Failure {
    use orchbee_workbench_protocol::WorkbenchWireError;
    let failure = |kind| Failure::new(kind).failure_schema(super::schema::WORKTREE_FAILURE_ID);
    match error {
        WorktreeListReadError::SessionUnavailable | WorktreeListReadError::RequestEncoding(_) => {
            failure(FailureKind::InternalFailure)
        }
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
/// session. `selector` absent lists the whole per-Site catalog; present
/// resolves exactly one entry under the Server's coordinator lock — the
/// CLI never pre-reads the catalog or resolves a selector locally. Every
/// failure past the session boundary carries the dedicated launch-profile
/// failure envelope: the generic schema's resource and candidate-id
/// grammars are closed without it.
#[cfg(target_os = "linux")]
pub(crate) fn launch_profile_catalog(
    selector: Option<LaunchProfileSelector>,
) -> Result<(serde_json::Value, Projection), Failure> {
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
    // The Client Session is fully dropped before the answer is mapped into
    // the bounded document or any output is prepared.
    drop(client);
    let result = outcome?;
    match result.outcome {
        LaunchProfileCatalogOutcome::Catalog {
            site_id,
            catalog_revision,
            entries,
        } => Ok((
            launch_profile_catalog_source(site_id.as_str(), catalog_revision),
            Projection::LaunchProfileCatalog(super::results::LaunchProfileCatalogDoc {
                catalog_revision,
                selection: super::results::LaunchProfileCatalogSelection::List(entries),
            }),
        )),
        LaunchProfileCatalogOutcome::Profile {
            site_id,
            catalog_revision,
            profile,
        } => Ok((
            launch_profile_catalog_source(site_id.as_str(), catalog_revision),
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
#[cfg(target_os = "linux")]
fn launch_profile_catalog_source(
    site_id: &str,
    catalog_revision: orchbee_workbench_protocol::LaunchProfileCatalogRevision,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "launch-profile-catalog",
        "siteId": site_id,
        "catalogRevision": catalog_revision.to_string(),
    })
}

/// The catalog read's typed outcomes → the CLI failure kinds, all under the
/// launch-profile failure envelope.
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
fn classify_launch_profile_catalog_read(error: LaunchProfileCatalogReadError) -> Failure {
    use orchbee_workbench_protocol::WorkbenchWireError;
    let failure =
        |kind| Failure::new(kind).failure_schema(super::schema::LAUNCH_PROFILE_FAILURE_ID);
    match error {
        LaunchProfileCatalogReadError::SessionUnavailable
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
#[cfg(target_os = "linux")]
pub(crate) fn snapshot_session() -> Result<(LocalClient, Instant, Snapshot), Failure> {
    let cohort_deadline = deadline_after(COHORT_BUDGET)?;
    let (cohort, site) = session_preamble(cohort_deadline, None)?;
    // Query and read callers keep their accepted budget: a fresh network
    // deadline sampled only after the preamble completes.
    let deadline = deadline_after(NETWORK_BUDGET)?;
    admit_snapshot(&cohort, &site, deadline, None)
}

/// The same preamble and admission as `snapshot_session`, but every stage
/// shares the caller's absolute deadline — a total command budget sampled
/// once at entry. The cohort check still never exceeds its own budget: it
/// runs under the earlier of `COHORT_BUDGET` and `deadline`, and the
/// admission plus snapshot read share what remains.
#[cfg(target_os = "linux")]
pub(crate) fn snapshot_session_until(
    deadline: Instant,
) -> Result<(LocalClient, Instant, Snapshot), Failure> {
    let cohort_deadline = bounded_deadline(Instant::now(), COHORT_BUDGET, deadline)?;
    let (cohort, site) = session_preamble(cohort_deadline, None)?;
    admit_snapshot(&cohort, &site, deadline, None)
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
#[cfg(target_os = "linux")]
fn snapshot_session_before(
    gate: &WaitGate<'_>,
) -> Result<(LocalClient, Instant, Snapshot), Failure> {
    let (cohort, site) = session_preamble(gate.budget(COHORT_BUDGET)?, Some(gate))?;
    // A slow filesystem step may already have spent the whole wait budget;
    // the network deadline is priced only from what remains.
    admit_snapshot(&cohort, &site, gate.budget(NETWORK_BUDGET)?, Some(gate))
}

/// The wait gate's post-step settlement, a no-op for ungated callers.
#[cfg(target_os = "linux")]
fn settle(gate: Option<&WaitGate<'_>>) -> Result<(), Failure> {
    match gate {
        Some(gate) => gate.settle(),
        None => Ok(()),
    }
}

/// Everything before the network exchange: the cohort check under its own
/// deadline, then existing-only rendezvous and socket validation. Under a
/// wait gate every step's return — success *and* failure — is settled
/// against the total deadline and the pending termination signal first.
#[cfg(target_os = "linux")]
fn session_preamble(
    cohort_deadline: Instant,
    gate: Option<&WaitGate<'_>>,
) -> Result<(NativeCohort, ModeCheckedLocalSiteRendezvous), Failure> {
    let cohort = NativeCohort::current(cohort_deadline);
    settle(gate)?;
    let cohort = cohort.map_err(|error| {
        Failure::new(FailureKind::CheckFailed)
            .details(stage_details("cohort"))
            .io_diagnostic(&error)
    })?;
    let site = rendezvous();
    settle(gate)?;
    let site = site?;
    let socket_exists = site.client_socket_exists();
    settle(gate)?;
    match socket_exists {
        Ok(true) => Ok((cohort, site)),
        Ok(false) => {
            Err(Failure::new(FailureKind::ServiceUnavailable).details(stage_details("discovery")))
        }
        Err(_) => Err(Failure::new(FailureKind::CheckFailed).details(stage_details("socket"))),
    }
}

/// One admission and one snapshot read under the shared deadline. On success
/// the caller owns the still admitted session; on any failure the session is
/// already dropped.
#[cfg(target_os = "linux")]
fn admit_snapshot(
    cohort: &NativeCohort,
    site: &ModeCheckedLocalSiteRendezvous,
    deadline: Instant,
    gate: Option<&WaitGate<'_>>,
) -> Result<(LocalClient, Instant, Snapshot), Failure> {
    let connected =
        LocalClient::connect_before(site.client_socket_path(), cohort.identity(), deadline);
    if let Err(failure) = settle(gate) {
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
    if let Err(failure) = settle(gate) {
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

/// One checked deadline `budget` out from now.
#[cfg(target_os = "linux")]
fn deadline_after(budget: Duration) -> Result<Instant, Failure> {
    Instant::now()
        .checked_add(budget)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))
}

/// A stage's effective deadline under a total ceiling: `budget` from `now`
/// or `ceiling`, whichever lands first. The deterministic pin for the
/// shared-budget contract — stages keep their own bound but can never
/// outlive the command's absolute deadline.
#[cfg(target_os = "linux")]
fn bounded_deadline(now: Instant, budget: Duration, ceiling: Instant) -> Result<Instant, Failure> {
    Ok(now
        .checked_add(budget)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?
        .min(ceiling))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    /// The send path's total budget is shared, not additive: the preamble's
    /// stage deadline is the earlier of its own budget and the command's
    /// absolute deadline — pinned without any clock or socket.
    #[test]
    fn bounded_deadline_takes_the_earlier_bound() {
        let now = Instant::now();
        // A nearly spent total budget leaves the preamble almost nothing.
        let ceiling = now + Duration::from_millis(50);
        assert_eq!(
            bounded_deadline(now, COHORT_BUDGET, ceiling).unwrap(),
            ceiling
        );
        // A far ceiling keeps the stage's own budget intact.
        assert_eq!(
            bounded_deadline(now, COHORT_BUDGET, now + Duration::from_secs(60)).unwrap(),
            now + COHORT_BUDGET
        );
    }

    fn worktree_directory() -> WorkbenchDirectoryId {
        WorkbenchDirectoryId::parse("0123456789abcdef0123456789abcdef").unwrap()
    }

    /// Post-delivery response loss is transport-loss (exit 5) under the
    /// dedicated worktree envelope — the read-only family has no
    /// outcome-unknown kind.
    #[test]
    fn worktree_transport_loss_is_exit_five() {
        let failure = classify_worktree_outcome(
            WorkbenchWorktreeListOutcome::TransportLoss {},
            &worktree_directory(),
        );
        assert_eq!(failure.kind, FailureKind::TransportLoss);
        assert_eq!(failure.kind.exit_code(), 5);
        assert_eq!(
            failure.failure_schema,
            Some(crate::cli::schema::WORKTREE_FAILURE_ID)
        );
        let details = failure.details.expect("transport-loss details");
        assert_eq!(details["stage"], serde_json::json!("worktree-list"));
        assert_eq!(details.len(), 1);
    }

    /// Both spawn arms are the Worker's process-start failure:
    /// service-unavailable (exit 4) with the worker-unavailable reason —
    /// never a Git check failure.
    #[test]
    fn worktree_spawn_failures_are_worker_unavailable() {
        for outcome in [
            WorkbenchWorktreeListOutcome::SpawnUnavailable {
                stage: WorktreeListStage::Probe,
            },
            WorkbenchWorktreeListOutcome::SpawnFailed {
                stage: WorktreeListStage::List,
            },
        ] {
            let failure = classify_worktree_outcome(outcome, &worktree_directory());
            assert_eq!(failure.kind, FailureKind::ServiceUnavailable);
            assert_eq!(failure.kind.exit_code(), 4);
            assert_eq!(
                failure.failure_schema,
                Some(crate::cli::schema::WORKTREE_FAILURE_ID)
            );
            let details = failure.details.expect("unavailable details");
            assert_eq!(details["stage"], serde_json::json!("worktree-list"));
            assert_eq!(details["reason"], serde_json::json!("worker-unavailable"));
        }
    }

    /// Git failures always carry the status key: the exact exit integer, or
    /// an explicit JSON null when the process was signaled. Malformed
    /// listings carry no invented status at all.
    #[test]
    fn worktree_git_status_is_always_explicit() {
        let failure = classify_worktree_outcome(
            WorkbenchWorktreeListOutcome::GitFailed {
                stage: WorktreeListStage::Probe,
                status: Some(1),
            },
            &worktree_directory(),
        );
        assert_eq!(failure.kind, FailureKind::CheckFailed);
        assert_eq!(failure.kind.exit_code(), 1);
        let details = failure.details.expect("git details");
        assert_eq!(details["stage"], serde_json::json!("probe"));
        assert_eq!(details["status"], serde_json::json!(1));

        let failure = classify_worktree_outcome(
            WorkbenchWorktreeListOutcome::GitFailed {
                stage: WorktreeListStage::List,
                status: None,
            },
            &worktree_directory(),
        );
        let details = failure.details.expect("signaled git details");
        assert_eq!(details["stage"], serde_json::json!("list"));
        assert_eq!(details["status"], serde_json::Value::Null);
        assert!(details.contains_key("status"));

        let failure = classify_worktree_outcome(
            WorkbenchWorktreeListOutcome::MalformedListing {},
            &worktree_directory(),
        );
        assert_eq!(failure.kind, FailureKind::CheckFailed);
        let details = failure.details.expect("parse details");
        assert_eq!(details["stage"], serde_json::json!("parse"));
        assert!(!details.contains_key("status"));
    }

    /// Worker-side physical uncertainty surfaces as worker-unavailable; the
    /// not-found arm keeps its directory resource identity.
    #[test]
    fn worktree_unavailable_reasons_and_not_found_stay_typed() {
        let failure = classify_worktree_outcome(
            WorkbenchWorktreeListOutcome::Unavailable {
                reason: WorktreeListUnavailableReason::WorkerUnavailable,
            },
            &worktree_directory(),
        );
        assert_eq!(failure.kind, FailureKind::ServiceUnavailable);
        let details = failure.details.expect("unavailable details");
        assert_eq!(details["reason"], serde_json::json!("worker-unavailable"));

        let failure = classify_worktree_outcome(
            WorkbenchWorktreeListOutcome::NotFound {},
            &worktree_directory(),
        );
        assert_eq!(failure.kind, FailureKind::NotFound);
        assert_eq!(failure.kind.exit_code(), 3);
        let details = failure.details.expect("not-found details");
        assert_eq!(details["resource"], serde_json::json!("directory"));
        assert_eq!(
            details["target"],
            serde_json::json!("0123456789abcdef0123456789abcdef")
        );
    }

    /// socket.
    #[test]
    fn agent_settle_reports_timeout_only_at_or_past_total() {
        assert!(
            agent_settle(
                Instant::now() + Duration::from_secs(60),
                AgentReadOperation::List
            )
            .is_ok()
        );
        let failure = agent_settle(Instant::now(), AgentReadOperation::Get)
            .expect_err("an expired total settles as the Agent timeout");
        assert_eq!(failure.kind, FailureKind::Timeout);
        let details = failure.details.expect("agent timeout details");
        assert_eq!(details["stage"], serde_json::json!("agent-read"));
        assert_eq!(details["operation"], serde_json::json!("get"));
        assert_eq!(details["timeoutMs"], serde_json::json!(5000));
    }

    /// An already-expired total fails before any subsequent stage IO: the
    /// preamble never gets a usable window and the connect is never
    /// attempted — the call returns promptly with a failure, deterministic
    /// on any machine.
    #[test]
    fn agent_session_until_expired_total_fails_before_io() {
        let started = Instant::now();
        assert!(agent_session_until(Instant::now(), None).is_err());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "an expired total fails fast, never starting the next IO"
        );
    }

    /// A provisional wake marker held into `Clear` classification is a
    /// contradiction: at every transport I/O position it must land on
    /// `internal-failure`, never a transport-loss or timeout verdict.
    #[test]
    fn agent_markers_classify_internal_at_every_io_position() {
        let request_id =
            ObapRequestId::parse("11111111111111111111111111111111").expect("request id");
        for marker in [
            crate::local_client::wake_ready_io,
            crate::local_client::wake_fault_io,
        ] {
            for operation in [
                AgentReadOperation::List,
                AgentReadOperation::Get,
                AgentReadOperation::Explain,
            ] {
                let not_submitted =
                    classify_agent_read(AgentReadError::RequestNotSubmitted(marker()), operation);
                assert_eq!(not_submitted.kind, FailureKind::InternalFailure);
                for reason in [
                    AgentReadFailure::Write(marker()),
                    AgentReadFailure::ResponseDeadline(marker()),
                    AgentReadFailure::Response(AgentWireError::Io(marker())),
                ] {
                    let failure = classify_agent_read(
                        AgentReadError::Failed {
                            request_id: request_id.clone(),
                            reason,
                        },
                        operation,
                    );
                    assert_eq!(
                        failure.kind,
                        FailureKind::InternalFailure,
                        "{operation:?} marker must never classify as transport/timeout"
                    );
                    assert!(
                        failure.details.is_none(),
                        "internal-failure carries no agent-read details"
                    );
                }
            }
        }
    }

    /// A recording watch wrapping a real armed INT/TERM guard: `finish`
    /// first waits for the peer's disconnect observation, then runs the real
    /// arbitration read — so it only succeeds when the admitted session's
    /// socket was already dropped before the finish read.
    struct OrderWatch {
        inner: ArmedSignals,
        eof: std::sync::mpsc::Receiver<()>,
    }

    impl AgentWatch for OrderWatch {
        fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
            SignalWatch::poll_fd(&self.inner)
        }

        fn finish(self) -> Result<Finish, nix::errno::Errno> {
            self.eof
                .recv_timeout(Duration::from_secs(5))
                .expect("the admitted session must be dropped before finish");
            self.inner.finish()
        }
    }

    /// Owner-model evidence for the drop-before-finish order: driving the
    /// real `agent_armed_finish` against a live admitted session, the watch
    /// observes the peer's EOF at finish entry — the Client Session socket
    /// was closed before the signal arbitration read ran.
    #[test]
    fn session_socket_drops_before_finish_arbitration() {
        use orchbee_agent_observation_protocol::{read_agent_request, write_agent_result};
        use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};

        let (eof, eof_rx) = std::sync::mpsc::channel();
        let server = crate::local_client::test_server::FakeServer::start(move |mut stream| {
            match read_message(&mut stream).map_err(std::io::Error::other)? {
                LocalClientMessage::ClientHello(_) => {}
                _ => {
                    return Err(std::io::Error::other("expected ClientHello"));
                }
            }
            write_message(
                &mut stream,
                &LocalClientMessage::ClientAccepted {
                    generation: std::num::NonZeroU64::new(1).expect("generation"),
                },
            )
            .map_err(std::io::Error::other)?;
            let request = read_agent_request(&mut stream).map_err(std::io::Error::other)?;
            let result: AgentReadResult = serde_json::from_value(serde_json::json!({
                "kind": "agent.session.list.result",
                "requestId": request.request_id().as_str(),
                "result": {"outcome": {"kind": "listed", "items": [], "nextCursor": null, "revision": 7}}
            }))
            .map_err(std::io::Error::other)?;
            write_agent_result(&mut stream, &result).map_err(std::io::Error::other)?;
            // Read until the client session closes — EOF is the drop mark.
            let mut sink = [0_u8; 64];
            let _ = std::io::Read::read(&mut stream, &mut sink);
            let _ = eof.send(());
            Ok(())
        })
        .expect("start fake server");
        let client = LocalClient::connect(server.endpoint(), "test-cohort")
            .expect("connect admitted client");
        let armed = ArmedSignals::arm_interrupts().expect("arm interrupts");
        let watch = OrderWatch {
            inner: armed,
            eof: eof_rx,
        };
        let request_id =
            ObapRequestId::parse("11111111111111111111111111111111").expect("request id");
        let outcome = agent_armed_finish(
            AgentReadOperation::List,
            |request_id| AgentReadRequest::List {
                request_id,
                include_ended: false,
                limit: orchbee_agent_observation_protocol::AgentSafeInteger::new(50),
                cursor: None,
            },
            Instant::now() + Duration::from_secs(10),
            request_id,
            watch,
            move |_total, _wake| Ok(client),
        );
        // The finish read ran only after the peer's EOF — and the cleared
        // span hands back the exchanged result for normal classification.
        let (_id, attempt) =
            outcome.unwrap_or_else(|_| panic!("clear arbitration hands back the attempt"));
        assert!(
            matches!(attempt, AgentAttempt::Exchanged(Ok(_))),
            "the exchange completed before finish"
        );
        server.finish().expect("peer finished");
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

/// Resolve the binding from the snapshot — typed ID or `@terminal:` selector
/// — and attach a read-only viewer to it inside the shared deadline.
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
    // Typed IDs and `@terminal:` selectors resolve against the snapshot's
    // complete bound set; the attach request carries the resolved full id.
    let row = super::results::terminal_select(snapshot, target)?;
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
        TerminalViewerOutcome::Record(record) => decode_history_window(
            record.clone(),
            attachment,
            epoch,
            frame,
            start_row,
            count,
            std::num::NonZeroU64::MIN,
        )
        .map(Some),
        TerminalViewerOutcome::RowsRejected { reason } => {
            Err(classify_rows_rejected(*reason, "read-history"))
        }
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

/// A typed `rows-rejected` answer maps to its fixed failure class: the local
/// budget overrun is a rejection, Host state answers are unavailability, and
/// reasons our fixed request shape cannot honestly produce are protocol
/// violations.
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
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

/// The clock `wait-output` consults for deadline evaluation and quiet-poll
/// sleeping. The real command uses `SystemWaitClock`; tests inject fakes so
/// deadline and backoff behavior is deterministic.
#[cfg(target_os = "linux")]
trait WaitClock {
    fn now(&self) -> Instant;
    fn sleep(&self, duration: Duration);
}

#[cfg(target_os = "linux")]
struct SystemWaitClock;

#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
struct WaitGate<'a> {
    clock: &'a dyn WaitClock,
    signal: &'a dyn TerminationSource,
    deadline_total: Instant,
}

#[cfg(target_os = "linux")]
impl WaitGate<'_> {
    /// Cancellation wins over a same-instant expiry; both beat any step
    /// classification that completed late.
    fn settle(&self) -> Result<(), Failure> {
        if self.signal.requested() {
            return Err(Failure::new(FailureKind::Cancelled));
        }
        if self.clock.now() >= self.deadline_total {
            return Err(Failure::new(FailureKind::Timeout));
        }
        Ok(())
    }

    /// The earlier of `allowance` and the remaining total, settled first.
    fn budget(&self, allowance: Duration) -> Result<Instant, Failure> {
        let now = self.clock.now();
        now.checked_add(self.remaining()?.min(allowance))
            .ok_or_else(|| Failure::new(FailureKind::InternalFailure))
    }

    /// Remaining total time after settling; past the deadline there is none.
    fn remaining(&self) -> Result<Duration, Failure> {
        self.settle()?;
        Ok(self
            .deadline_total
            .saturating_duration_since(self.clock.now()))
    }
}

/// One `wait-output` run: the shared gate, the completed-observation count
/// the timeout report carries, the quiet-poll backoff, and the
/// per-attachment `request_sequence` the Host requires to be strictly
/// increasing.
#[cfg(target_os = "linux")]
struct WaitRun<'a> {
    gate: WaitGate<'a>,
    timeout_ms: u32,
    observations: u64,
    next_row_sequence: u64,
    poll_interval: Duration,
}

#[cfg(target_os = "linux")]
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
        wait_timeout_failure(self.timeout_ms, self.observations)
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
            .remaining()
            .map_err(|failure| self.map_clock_failure(failure))?;
        self.gate.clock.sleep(self.poll_interval.min(remaining));
        self.poll_interval = (self.poll_interval * 2).min(WAIT_POLL_BACKOFF);
        Ok(())
    }

    /// Any real frame record resets the backoff.
    fn note_record(&mut self) {
        self.poll_interval = EMPTY_POLL_INTERVAL;
    }
}

/// `terminal wait-output`: resolve the binding, attach one read-only viewer
/// on the admitted session, then observe — poll → project → `FrameApplied`
/// for every applied frame → one absolute `ReadRows` over the most recent
/// `--rows` retained window → literal match — until the text appears, the
/// total deadline expires, or a termination signal cancels the wait. The
/// deadline starts at command start and is re-checked after every awaited
/// step; cleanup is an explicit `Detach` inside the remaining budget plus
/// the session drop, never a new budget.
#[cfg(target_os = "linux")]
pub(crate) fn wait_output(
    target: &Target,
    pattern: &crate::invocation::WaitPattern,
    rows: u16,
    timeout_ms: u32,
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
    let deadline_total = started
        .checked_add(Duration::from_millis(u64::from(timeout_ms)))
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
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
                FailureKind::Timeout => wait_timeout_failure(timeout_ms, 0),
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
            rows,
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
        (Ok(observed), Ok(())) if late.is_none() => {
            let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            Ok((
                source.clone(),
                Projection::TerminalWaitOutput(super::results::terminal_wait_output_doc(
                    row,
                    attachment,
                    epoch,
                    observed,
                    super::results::WaitReport {
                        observations: run.observations,
                        requested_rows: rows,
                        timeout_ms,
                        elapsed_ms,
                    },
                )),
            ))
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
/// `FeedbackAccepted`. After each applied frame, one `ReadRows` pins that
/// exact revision+generation; a match completes the wait, an aged pin is
/// abandoned for a fresh frame, and every awaited step is followed by a
/// cancel+deadline re-check. No frame is skipped: the Host's single record
/// slot demands full consumption.
#[cfg(target_os = "linux")]
fn wait_observe(
    client: &mut LocalClient,
    run: &mut WaitRun<'_>,
    viewer_id: &ViewerId,
    mut projection: TerminalProjection,
    matcher: &super::results::WaitMatcher<'_>,
    rows: u16,
) -> Result<super::results::WaitObservation, Failure> {
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
                            _ => return Err(viewer_protocol_invalid()),
                        }
                        if applied {
                            // The just-applied frame pins this read's exact
                            // revision and generation; nothing from another
                            // revision is ever matched.
                            let frame = projection
                                .frame()
                                .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
                            if let Some(window) =
                                wait_read_window(client, run, viewer_id, frame, rows)?
                            {
                                run.observations += 1;
                                // One bounded match pass over the pinned
                                // window; a cancel or expiry observed right
                                // after it still wins over continuing.
                                let matched = super::results::match_window(&window, matcher);
                                run.check()?;
                                if let Some(matched) = matched {
                                    return Ok(super::results::WaitObservation {
                                        matched,
                                        revision: window.revision,
                                        generation: window.generation.get(),
                                        window_start: window.start_row,
                                        window_rows: u64::try_from(window.rows.len())
                                            .unwrap_or(u64::MAX),
                                        total_rows: window.total_rows,
                                    });
                                }
                            }
                        }
                    }
                    // Same as the one-shot read: a consumed ProjectionBlocked
                    // keeps polling for an applicable frame.
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

/// One consistent `ReadRows` over the pinned frame's most recent retained
/// window: `[max(0, total − K), total)` in the unified screen+scrollback
/// absolute space, `K = min(requested, total, MAXIMUM_FRAME_CELLS/columns)`.
/// `Some(window)` is a fully validated consistent answer; `None` means the
/// pinned snapshot aged between frame and read — stale revision, stale
/// generation, or history eviction — and the caller resumes polling for a
/// fresh consistent frame instead of splicing half windows.
#[cfg(target_os = "linux")]
fn wait_read_window(
    client: &mut LocalClient,
    run: &mut WaitRun<'_>,
    viewer_id: &ViewerId,
    frame: &TerminalTextFrame,
    requested: u16,
) -> Result<Option<orchbee_terminal_protocol::TerminalTextRowWindow>, Failure> {
    let total_rows = frame
        .scrollback_rows
        .checked_add(u64::from(frame.geometry.rows))
        .ok_or_else(viewer_protocol_invalid)?;
    let capacity = u64::try_from(orchbee_terminal_protocol::MAXIMUM_FRAME_CELLS)
        .unwrap_or(u64::MAX)
        / u64::from(frame.geometry.columns);
    let count = u64::from(requested).min(total_rows).min(capacity);
    let start_row = total_rows - count;
    let row_count = u16::try_from(count).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let request_sequence = run.next_sequence()?;
    let request = TerminalViewerRequest::new(
        obcs_request_id()?,
        TerminalViewerAction::ReadRows {
            attachment_id: viewer_id.clone(),
            start_row,
            row_count,
            revision: frame.route.revision,
            generation: frame.route.generation,
            request_sequence,
            maximum_bytes: orchbee_client_server_protocol::MAXIMUM_READ_ROWS_BYTES,
        },
    );
    let deadline = run.step_deadline()?;
    let result = client.submit_terminal_viewer_before(request, deadline);
    run.check()?;
    let result = result.map_err(classify_viewer)?;
    match result.outcome() {
        TerminalViewerOutcome::Record(record) => decode_history_window(
            record.clone(),
            frame.route.attachment_id,
            frame.route.host_epoch,
            frame,
            start_row,
            count,
            request_sequence,
        )
        .map(Some),
        TerminalViewerOutcome::RowsRejected { reason } => {
            use orchbee_client_server_protocol::ViewerRowsRejectedReason as Reason;
            match reason {
                Reason::StaleRevision | Reason::StaleGeneration | Reason::HistoryEvicted => {
                    Ok(None)
                }
                other => Err(classify_rows_rejected(*other, "wait-output")),
            }
        }
        TerminalViewerOutcome::Rejected { reason } => Err(classify_viewer_rejected(*reason)),
        TerminalViewerOutcome::Unavailable { .. } => Err(viewer_unavailable()),
        _ => Err(viewer_protocol_invalid()),
    }
}

/// The confirmed-`Detach` exit `wait-output` requires: the real outcome is
/// inspected, unlike the best-effort send a one-shot read uses.
#[cfg(target_os = "linux")]
fn wait_detach(
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
        _ => Err(viewer_protocol_invalid()),
    }
}

/// The timeout report: the wait stage, the caller's total budget, and the
/// completed-observation count — successful consistent window reads that
/// also completed a match pass, never a frame or revision count.
#[cfg(target_os = "linux")]
fn wait_timeout_failure(timeout_ms: u32, observations: u64) -> Failure {
    Failure::new(FailureKind::Timeout).details(serde_json::Map::from_iter([
        ("stage".to_owned(), serde_json::json!("wait")),
        ("timeoutMs".to_owned(), serde_json::json!(timeout_ms)),
        (
            "observed".to_owned(),
            serde_json::json!({ "completedObservations": observations }),
        ),
    ]))
}

/// The cancelled report: the wait stage under the fixed kind.
#[cfg(target_os = "linux")]
fn wait_cancelled_failure() -> Failure {
    Failure::new(FailureKind::Cancelled).details(stage_details("wait"))
}

/// Merge the honest cleanup state into a failure's details — plus
/// `matchObserved: true` when the text had already matched but the exit is
/// still a failure.
#[cfg(target_os = "linux")]
fn wait_cleanup_details(
    mut failure: Failure,
    cleanup: &'static str,
    match_observed: bool,
) -> Failure {
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
#[cfg(target_os = "linux")]
fn classify_launch_read(error: SubmissionError) -> Failure {
    match error {
        SubmissionError::SessionUnavailable | SubmissionError::RequestEncoding(_) => {
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
        ViewerRejectedReason::HostRejected => {
            Failure::new(FailureKind::Rejected).details(stage_details("viewer"))
        }
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
#[cfg(target_os = "linux")]
pub(crate) struct AgentCancelled {
    pub(crate) failure: Failure,
    pub(crate) finalization: SourceFinalization,
}

#[cfg(target_os = "linux")]
pub(crate) enum AgentQueryError {
    Admission(Failure),
    Read(Failure),
    Cancelled(Box<AgentCancelled>),
}

/// Why the armed span never reached an admitted session — the generic
/// failure family. Preamble failures arrive already classified; the raw
/// connect outcome is held unprojected because it may carry the protocol's
/// provisional wake markers, which only the finish arbitration may weigh.
#[cfg(target_os = "linux")]
enum AgentAdmissionError {
    Preamble(Failure),
    Connect(crate::local_client::LocalClientConnectError),
}

/// The armed Agent span's retained raw outcome — never projected to a
/// `Failure` before the signal owner's single arbitration, so provisional
/// wake markers survive intact to that point. The variant carries the
/// phase: `Admission` answers the generic document, the rest answer the
/// agent document.
#[cfg(target_os = "linux")]
enum AgentAttempt {
    Admission(AgentAdmissionError),
    /// The shared absolute budget expired before or after the exchange.
    Expired,
    /// One OBAP exchange settled to its raw result or typed failure.
    Exchanged(Result<AgentReadResult, AgentReadError>),
}

/// The armed Agent interruption owner as one pollable descriptor plus one
/// finish arbitration — the private seam where a test substitutes a
/// recording watch to prove the admitted session is dropped first. Only
/// `ArmedSignals` implements it in production.
#[cfg(target_os = "linux")]
trait AgentWatch {
    /// The borrowed signal descriptor joined into every transport wait.
    fn poll_fd(&self) -> Option<BorrowedFd<'_>>;
    /// The single arbitration read after the armed span.
    fn finish(self) -> Result<Finish, nix::errno::Errno>;
}

#[cfg(target_os = "linux")]
impl AgentWatch for ArmedSignals {
    fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        SignalWatch::poll_fd(self)
    }

    fn finish(self) -> Result<Finish, nix::errno::Errno> {
        ArmedSignals::finish(self)
    }
}

/// One armed Agent read command: after grammar validation and request
/// identity, the interruption guard arms before the synchronous preamble
/// and owns SIGINT/SIGTERM for the whole `T0+5s` span; the borrowed wake
/// descriptor joins every transport wait; the Client Session is dropped
/// before the single `finish` arbitration; and no early return may bypass
/// that arbitration once armed. `Clear` hands the raw attempt back for the
/// normal classification; `Cancelled` answers the typed 130 family.
#[cfg(target_os = "linux")]
fn agent_armed_run(
    operation: AgentReadOperation,
    build: impl FnOnce(ObapRequestId) -> AgentReadRequest,
) -> Result<(ObapRequestId, AgentAttempt), AgentQueryError> {
    let total = deadline_after(AGENT_TOTAL_BUDGET).map_err(AgentQueryError::Admission)?;
    let request_id = obap_request_id().map_err(AgentQueryError::Admission)?;
    let armed = ArmedSignals::arm_interrupts()
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
#[cfg(target_os = "linux")]
fn agent_armed_finish(
    operation: AgentReadOperation,
    build: impl FnOnce(ObapRequestId) -> AgentReadRequest,
    total: Instant,
    request_id: ObapRequestId,
    armed: impl AgentWatch,
    acquire: impl FnOnce(Instant, Option<BorrowedFd<'_>>) -> Result<LocalClient, AgentAdmissionError>,
) -> Result<(ObapRequestId, AgentAttempt), AgentQueryError> {
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
        Ok(Finish::Cancelled { finalization, .. }) => {
            Err(AgentQueryError::Cancelled(Box::new(AgentCancelled {
                // A cancelled read carries no source: only a trustworthy semantic
                // answer may publish the correlated request identity.
                failure: Failure::new(FailureKind::Cancelled)
                    .details(agent_read_details(operation)),
                finalization,
            })))
        }
        Ok(Finish::Clear) => Ok((request_id, attempt)),
    }
}

/// The armed span body: admission under the borrowed wake descriptor, the
/// pre-submission settle, one OBAP exchange, then the session is dropped —
/// all before the caller's finish arbitration. Nothing here classifies.
#[cfg(target_os = "linux")]
fn agent_span(
    total: Instant,
    operation: AgentReadOperation,
    wake: Option<BorrowedFd<'_>>,
    request_id: ObapRequestId,
    build: impl FnOnce(ObapRequestId) -> AgentReadRequest,
    acquire: impl FnOnce(Instant, Option<BorrowedFd<'_>>) -> Result<LocalClient, AgentAdmissionError>,
) -> AgentAttempt {
    let mut client = match acquire(total, wake) {
        Err(error) => return AgentAttempt::Admission(error),
        Ok(client) => client,
    };
    if agent_settle(total, operation).is_err() {
        drop(client);
        return AgentAttempt::Expired;
    }
    let result = client.submit_agent_read_before_cancellable(build(request_id), total, wake);
    // The Client Session is fully dropped before the owner arbitrates.
    drop(client);
    if agent_settle(total, operation).is_err() {
        return AgentAttempt::Expired;
    }
    AgentAttempt::Exchanged(result)
}

/// One admitted Client Session for an Agent observation read under the
/// caller's single absolute budget: the same cohort → discovery → socket →
/// admission preamble as every network query, then one connect — each stage
/// bounded by `total`, never a renewed budget. The wake descriptor joins
/// the admission wait set; nothing here may create state, replay a request,
/// or touch the Workbench.
#[cfg(target_os = "linux")]
fn agent_session_until(
    total: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<LocalClient, AgentAdmissionError> {
    let cohort_deadline = bounded_deadline(Instant::now(), COHORT_BUDGET, total)
        .map_err(AgentAdmissionError::Preamble)?;
    let (cohort, site) =
        session_preamble(cohort_deadline, None).map_err(AgentAdmissionError::Preamble)?;
    LocalClient::connect_before_cancellable(
        site.client_socket_path(),
        cohort.identity(),
        total,
        wake,
    )
    .map_err(AgentAdmissionError::Connect)
}

/// Post-step settlement of the shared absolute budget: at or past the
/// ceiling the step's outcome is the command's own Agent timeout — never a
/// late success or a stale transport fault.
#[cfg(target_os = "linux")]
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
#[cfg(target_os = "linux")]
fn classify_attempt(
    attempt: AgentAttempt,
    operation: AgentReadOperation,
) -> Result<AgentReadResult, AgentQueryError> {
    match attempt {
        AgentAttempt::Admission(AgentAdmissionError::Preamble(failure)) => {
            Err(AgentQueryError::Admission(failure))
        }
        AgentAttempt::Admission(AgentAdmissionError::Connect(error)) => {
            Err(AgentQueryError::Admission(classify_connect(error)))
        }
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
#[cfg(target_os = "linux")]
pub(crate) fn agent_list(
    include_ended: bool,
    limit: u64,
    cursor: Option<orchbee_agent_observation_protocol::AgentListCursor>,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    let (request_id, attempt) = agent_armed_run(AgentReadOperation::List, |request_id| {
        AgentReadRequest::List {
            request_id,
            include_ended,
            limit: AgentSafeInteger::new(limit),
            cursor,
        }
    })?;
    let AgentReadResult::List { result, .. } = classify_attempt(attempt, AgentReadOperation::List)?
    else {
        return Err(AgentQueryError::Read(agent_protocol_invalid(
            AgentReadOperation::List,
        )));
    };
    let source = super::results::agent_source(&request_id);
    match result.outcome {
        AgentListOutcome::Listed { .. } => Ok((source, Projection::AgentList(result.outcome))),
        AgentListOutcome::Rejected { reason } => Err(AgentQueryError::Read(
            match reason {
                orchbee_agent_observation_protocol::AgentListRejectionReason::ReadDenied => {
                    Failure::new(FailureKind::Denied)
                }
                _ => Failure::new(FailureKind::Rejected),
            }
            .details(agent_rejection_details(
                AgentReadOperation::List,
                rejection_reason_text(&reason),
            ))
            .source(source),
        )),
        AgentListOutcome::Unavailable { .. } => Err(AgentQueryError::Read(
            Failure::new(FailureKind::ServiceUnavailable)
                .details(agent_unavailable_details(AgentReadOperation::List))
                .source(source),
        )),
    }
}

/// `agent get`: one OBAP get request, same admission/exchange discipline.
#[cfg(target_os = "linux")]
pub(crate) fn agent_get(
    session_id: &AgentSessionId,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    agent_session_read(
        AgentReadOperation::Get,
        |request_id| AgentReadRequest::Get {
            request_id,
            session_id: session_id.clone(),
        },
        session_id,
    )
}

/// `agent explain`: one OBAP explain request, same admission/exchange
/// discipline.
#[cfg(target_os = "linux")]
pub(crate) fn agent_explain(
    session_id: &AgentSessionId,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    agent_session_read(
        AgentReadOperation::Explain,
        |request_id| AgentReadRequest::Explain {
            request_id,
            session_id: session_id.clone(),
        },
        session_id,
    )
}

/// The shared single-session read behind `agent get` and `agent explain`.
#[cfg(target_os = "linux")]
fn agent_session_read(
    operation: AgentReadOperation,
    request: impl FnOnce(ObapRequestId) -> AgentReadRequest,
    session_id: &AgentSessionId,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    let (request_id, attempt) = agent_armed_run(operation, request)?;
    let result = classify_attempt(attempt, operation)?;
    let source = super::results::agent_source(&request_id);
    match (operation, result) {
        (
            AgentReadOperation::Get,
            AgentReadResult::Get {
                result: envelope, ..
            },
        ) => match envelope.outcome {
            AgentGetOutcome::Found { .. } => Ok((source, Projection::AgentGet(envelope.outcome))),
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
            AgentExplainOutcome::Found { .. } => {
                Ok((source, Projection::AgentExplain(envelope.outcome)))
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
        _ => Err(AgentQueryError::Read(agent_protocol_invalid(operation))),
    }
}

/// The `get`/`explain` refusal mapping: `not-found` names the session, while
/// `read-denied` carries only the typed reason — both under the common
/// `agent-read` stage.
#[cfg(target_os = "linux")]
fn agent_session_rejection(
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
#[cfg(target_os = "linux")]
fn agent_read_details(operation: AgentReadOperation) -> serde_json::Map<String, serde_json::Value> {
    let mut details = serde_json::Map::new();
    details.insert("stage".to_owned(), serde_json::json!("agent-read"));
    details.insert(
        "operation".to_owned(),
        serde_json::json!(operation.as_str()),
    );
    details
}

#[cfg(target_os = "linux")]
fn agent_session_details(
    operation: AgentReadOperation,
    session_id: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let mut details = agent_read_details(operation);
    details.insert("sessionId".to_owned(), serde_json::json!(session_id));
    details
}

#[cfg(target_os = "linux")]
fn agent_rejection_details(
    operation: AgentReadOperation,
    reason: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let mut details = agent_read_details(operation);
    details.insert("reason".to_owned(), serde_json::json!(reason));
    details
}

#[cfg(target_os = "linux")]
fn agent_unavailable_details(
    operation: AgentReadOperation,
) -> serde_json::Map<String, serde_json::Value> {
    agent_rejection_details(operation, "runtime-unavailable")
}

/// The fixed `timeoutMs` the agent-read failure schema publishes: the shared
/// absolute command budget in milliseconds.
#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
fn agent_protocol_invalid(operation: AgentReadOperation) -> Failure {
    Failure::new(FailureKind::ProtocolInvalid).details(agent_read_details(operation))
}

/// A deadline-elapsed transport error is the command's own timeout, never a
/// transport loss: `TimedOut` only arises from the shared deadline inside the
/// exchange machinery.
#[cfg(target_os = "linux")]
fn is_deadline_elapsed(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::TimedOut
}

/// A provisional wake marker surviving to `Clear` classification is a
/// contradiction — the descriptor reported readiness or a fault yet the
/// owner read no signal. Both map to `internal-failure`, never cancellation
/// or a transport verdict.
#[cfg(target_os = "linux")]
fn wake_marker_failure(error: &std::io::Error) -> Option<Failure> {
    if exchange_wake_ready(error) || exchange_wake_fault(error) {
        Some(Failure::new(FailureKind::InternalFailure))
    } else {
        None
    }
}

/// Post-admission Agent read failures: a semantic answer maps to its fixed
/// kind, an expired deadline to timeout, a transport fault to transport-loss,
/// and any malformed/correlated violation to protocol-invalid — every
/// document under the common `agent-read` stage, none with a source.
#[cfg(target_os = "linux")]
fn classify_agent_read(error: AgentReadError, operation: AgentReadOperation) -> Failure {
    match error {
        AgentReadError::SessionUnavailable | AgentReadError::RequestEncoding(_) => {
            Failure::new(FailureKind::InternalFailure)
        }
        AgentReadError::RequestNotSubmitted(error) => {
            if let Some(failure) = wake_marker_failure(&error) {
                failure
            } else if is_deadline_elapsed(&error) {
                Failure::new(FailureKind::Timeout).details(agent_timeout_details(operation))
            } else {
                Failure::new(FailureKind::TransportLoss).details(agent_read_details(operation))
            }
        }
        AgentReadError::Failed { reason, .. } => classify_agent_failure(reason, operation),
    }
}

#[cfg(target_os = "linux")]
fn classify_agent_failure(reason: AgentReadFailure, operation: AgentReadOperation) -> Failure {
    match reason {
        AgentReadFailure::Write(error)
        | AgentReadFailure::ResponseDeadline(error)
        | AgentReadFailure::Response(AgentWireError::Io(error)) => {
            if let Some(failure) = wake_marker_failure(&error) {
                failure
            } else if is_deadline_elapsed(&error) {
                Failure::new(FailureKind::Timeout).details(agent_timeout_details(operation))
            } else {
                Failure::new(FailureKind::TransportLoss).details(agent_read_details(operation))
            }
        }
        AgentReadFailure::Response(_)
        | AgentReadFailure::UnexpectedResponse
        | AgentReadFailure::CorrelationMismatch { .. } => agent_protocol_invalid(operation),
    }
}

/// The OBAP request identity minted per command: sixteen fresh bytes as
/// canonical 32-lowercase-hex, never reused.
#[cfg(target_os = "linux")]
fn obap_request_id() -> Result<ObapRequestId, Failure> {
    ObapRequestId::parse(&fresh_request_id()?)
        .map_err(|_| Failure::new(FailureKind::InternalFailure))
}

#[cfg(target_os = "linux")]
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

/// What the terminal observation produced: the applied visible frame, or the
/// validated scrollback window behind it.

#[cfg(not(target_os = "linux"))]
pub(crate) fn read_snapshot() -> Result<orchbee_workbench_protocol::Snapshot, super::Failure> {
    Err(
        super::Failure::new(super::FailureKind::ServiceUnavailable).details(
            super::stage_reason_details("platform", "unsupported-platform"),
        ),
    )
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn terminal_get(
    _target: &crate::invocation::Target,
) -> Result<(serde_json::Value, super::results::Projection), super::Failure> {
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
    _presentation: super::results::ReadPresentation,
) -> Result<(serde_json::Value, super::results::Projection), super::Failure> {
    Err(
        super::Failure::new(super::FailureKind::ServiceUnavailable).details(
            super::stage_reason_details("platform", "unsupported-platform"),
        ),
    )
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn worktree_list(
    _project: &crate::invocation::Target,
    _directory: &orchbee_workbench_protocol::WorkbenchDirectoryId,
) -> Result<(serde_json::Value, super::results::Projection), super::Failure> {
    Err(
        super::Failure::new(super::FailureKind::ServiceUnavailable).details(
            super::stage_reason_details("platform", "unsupported-platform"),
        ),
    )
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn launch_profile_catalog(
    _selector: Option<orchbee_workbench_protocol::LaunchProfileSelector>,
) -> Result<(serde_json::Value, super::results::Projection), super::Failure> {
    Err(
        super::Failure::new(super::FailureKind::ServiceUnavailable).details(
            super::stage_reason_details("platform", "unsupported-platform"),
        ),
    )
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn wait_output(
    _target: &crate::invocation::Target,
    _pattern: &crate::invocation::WaitPattern,
    _rows: u16,
    _timeout_ms: u32,
) -> Result<(serde_json::Value, super::results::Projection), super::Failure> {
    Err(
        super::Failure::new(super::FailureKind::ServiceUnavailable).details(
            super::stage_reason_details("platform", "unsupported-platform"),
        ),
    )
}

/// The Agent read failure family a caller must answer with; on platforms
/// without a local Client Session every read is a pre-admission platform
/// failure under the generic document.
#[cfg(not(target_os = "linux"))]
pub(crate) enum AgentQueryError {
    Admission(super::Failure),
    Read(super::Failure),
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn agent_list(
    _include_ended: bool,
    _limit: u64,
    _cursor: Option<orchbee_agent_observation_protocol::AgentListCursor>,
) -> Result<(serde_json::Value, super::results::Projection), AgentQueryError> {
    Err(AgentQueryError::Admission(
        super::Failure::new(super::FailureKind::ServiceUnavailable).details(
            super::stage_reason_details("platform", "unsupported-platform"),
        ),
    ))
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn agent_get(
    _session_id: &orchbee_agent_observation_protocol::AgentSessionId,
) -> Result<(serde_json::Value, super::results::Projection), AgentQueryError> {
    Err(AgentQueryError::Admission(
        super::Failure::new(super::FailureKind::ServiceUnavailable).details(
            super::stage_reason_details("platform", "unsupported-platform"),
        ),
    ))
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn agent_explain(
    _session_id: &orchbee_agent_observation_protocol::AgentSessionId,
) -> Result<(serde_json::Value, super::results::Projection), AgentQueryError> {
    Err(AgentQueryError::Admission(
        super::Failure::new(super::FailureKind::ServiceUnavailable).details(
            super::stage_reason_details("platform", "unsupported-platform"),
        ),
    ))
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn agent_wait(
    _session_id: &orchbee_agent_observation_protocol::AgentSessionId,
    _until: &[crate::invocation::AgentWaitCondition],
    _after_revision: Option<orchbee_agent_observation_protocol::AgentSafeInteger>,
    _timeout_ms: u32,
) -> Result<(serde_json::Value, super::results::Projection), AgentQueryError> {
    Err(AgentQueryError::Admission(
        super::Failure::new(super::FailureKind::ServiceUnavailable).details(
            super::stage_reason_details("platform", "unsupported-platform"),
        ),
    ))
}

/// Deterministic wait-path evidence: an injectable clock and flag make the
/// deadline, cancel, backoff, and per-attachment sequence behavior exact.
#[cfg(all(test, target_os = "linux"))]
mod wait_tests {
    use super::*;
    use std::cell::Cell;

    /// A `WaitClock` whose `now` only moves when the test says so; `sleep`
    /// records and advances the requested duration instead of blocking.
    struct FakeWaitClock {
        now: Cell<Instant>,
        slept: Cell<Duration>,
    }

    impl FakeWaitClock {
        fn new() -> Self {
            Self {
                now: Cell::new(Instant::now()),
                slept: Cell::new(Duration::ZERO),
            }
        }
        fn advance(&self, duration: Duration) {
            self.now.set(self.now.get() + duration);
        }
    }

    impl WaitClock for FakeWaitClock {
        fn now(&self) -> Instant {
            self.now.get()
        }
        fn sleep(&self, duration: Duration) {
            self.slept.set(self.slept.get() + duration);
            self.now.set(self.now.get() + duration);
        }
    }

    struct FlaggedTermination(Cell<bool>);

    impl TerminationSource for FlaggedTermination {
        fn requested(&self) -> bool {
            self.0.get()
        }
    }

    fn fake_run<'a>(
        clock: &'a FakeWaitClock,
        flag: &'a FlaggedTermination,
        deadline_total: Instant,
    ) -> WaitRun<'a> {
        WaitRun {
            gate: WaitGate {
                clock,
                signal: flag,
                deadline_total,
            },
            timeout_ms: 30_000,
            observations: 0,
            next_row_sequence: 1,
            poll_interval: EMPTY_POLL_INTERVAL,
        }
    }

    #[test]
    fn settle_reports_cancel_before_expiry() {
        let clock = FakeWaitClock::new();
        let flag = FlaggedTermination(Cell::new(false));
        let deadline = clock.now() + Duration::from_secs(1);
        let gate = WaitGate {
            clock: &clock,
            signal: &flag,
            deadline_total: deadline,
        };
        assert!(gate.settle().is_ok());
        flag.0.set(true);
        // A pending cancel wins even while budget remains — and also when
        // the deadline already passed at the same instant.
        assert_eq!(gate.settle().unwrap_err().kind, FailureKind::Cancelled);
        clock.advance(Duration::from_secs(2));
        assert_eq!(gate.settle().unwrap_err().kind, FailureKind::Cancelled);
    }

    #[test]
    fn settle_reports_timeout_only_after_deadline() {
        let clock = FakeWaitClock::new();
        let flag = FlaggedTermination(Cell::new(false));
        let deadline = clock.now() + Duration::from_millis(50);
        let gate = WaitGate {
            clock: &clock,
            signal: &flag,
            deadline_total: deadline,
        };
        assert!(gate.settle().is_ok());
        clock.advance(Duration::from_millis(50));
        assert_eq!(gate.settle().unwrap_err().kind, FailureKind::Timeout);
    }

    #[test]
    fn budget_is_the_earlier_of_allowance_and_remaining() {
        let clock = FakeWaitClock::new();
        let flag = FlaggedTermination(Cell::new(false));
        let deadline = clock.now() + Duration::from_millis(100);
        let gate = WaitGate {
            clock: &clock,
            signal: &flag,
            deadline_total: deadline,
        };
        let step = gate.budget(NETWORK_BUDGET).expect("budget");
        assert_eq!(step, clock.now() + Duration::from_millis(100));
        clock.advance(Duration::from_millis(100));
        assert_eq!(
            gate.budget(NETWORK_BUDGET).unwrap_err().kind,
            FailureKind::Timeout
        );
    }

    #[test]
    fn row_sequences_increase_strictly_and_overflow_is_real() {
        let clock = FakeWaitClock::new();
        let flag = FlaggedTermination(Cell::new(false));
        let deadline = clock.now() + Duration::from_secs(60);
        let mut run = fake_run(&clock, &flag, deadline);
        assert_eq!(run.next_sequence().unwrap().get(), 1);
        assert_eq!(run.next_sequence().unwrap().get(), 2);
        assert_eq!(run.next_sequence().unwrap().get(), 3);
        run.next_row_sequence = u64::MAX;
        assert_eq!(
            run.next_sequence().unwrap_err().kind,
            FailureKind::InternalFailure
        );
        // Overflow never resets: the counter stays where it failed.
        assert_eq!(run.next_row_sequence, u64::MAX);
    }

    #[test]
    fn sleep_backs_off_to_ceiling_and_clamps_to_deadline() {
        let clock = FakeWaitClock::new();
        let flag = FlaggedTermination(Cell::new(false));
        let deadline = clock.now() + Duration::from_millis(290);
        let mut run = fake_run(&clock, &flag, deadline);
        // 10 → 20 → 40 → 80 → 100 (cap) → clamped 60 to the deadline.
        run.sleep().expect("sleep 1");
        run.sleep().expect("sleep 2");
        run.sleep().expect("sleep 3");
        run.sleep().expect("sleep 4");
        run.sleep().expect("sleep 5");
        run.sleep().expect("sleep 6");
        assert_eq!(
            clock.slept.get(),
            Duration::from_millis(10 + 20 + 40 + 80 + 100 + 40)
        );
        // 40ms of headroom remained after the 100ms-cap sleep; the last
        // sleep consumed it, so the next sleep sees an expired deadline.
        assert_eq!(run.sleep().unwrap_err().kind, FailureKind::Timeout);
    }

    #[test]
    fn sleep_honors_cancel_before_any_sleep() {
        let clock = FakeWaitClock::new();
        let flag = FlaggedTermination(Cell::new(true));
        let deadline = clock.now() + Duration::from_secs(60);
        let mut run = fake_run(&clock, &flag, deadline);
        assert_eq!(run.sleep().unwrap_err().kind, FailureKind::Cancelled);
        assert_eq!(clock.slept.get(), Duration::ZERO);
    }

    #[test]
    fn check_maps_expiry_to_detailed_timeout() {
        let clock = FakeWaitClock::new();
        let flag = FlaggedTermination(Cell::new(false));
        let deadline = clock.now() + Duration::ZERO;
        let mut run = fake_run(&clock, &flag, deadline);
        run.observations = 7;
        let failure = run.check().unwrap_err();
        assert_eq!(failure.kind, FailureKind::Timeout);
        let details = failure.details.expect("timeout details");
        assert_eq!(details["stage"], serde_json::json!("wait"));
        assert_eq!(details["timeoutMs"], serde_json::json!(30_000));
        assert_eq!(
            details["observed"]["completedObservations"],
            serde_json::json!(7)
        );
    }
}
