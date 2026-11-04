//! Addressed read-only discovery shares the bounded Worktree lane: the Server says where the first
//! terminal stands, then Git lists the repository there. Nothing attaches to a terminal.
use super::{
    AddressedWorktreeSource, AsLocalIo, Discovery, Duration, Instant, LocalClient,
    LocalOperationClock, LocalStream, ProtocolVersion, SidebarDiscovery, WorkbenchRequestId,
    WorkbenchWorktreeQuery, WorkbenchWorktreeQueryOutcome, WorktreeNotice, listing_failure,
    snapshot,
};
use orchbee_workbench_protocol::{
    CurrentDirectoryOutcome, CurrentDirectoryRequest, WorkbenchDirectoryPath, WorktreeSource,
};

pub(super) struct DiscoveryRequest {
    pub(crate) addressed: AddressedWorktreeSource,
    pub(crate) requests: [WorkbenchRequestId; 4],
    pub(crate) expected: Option<WorktreeSource>,
    pub(crate) configured_root: super::ConfiguredRoot,
}

/// The Server's `workbench.current-directory` answer for the first Workspace, the read the Web and
/// Desktop Clients make before their Worktree discovery. A refused read is no answer; a lost or
/// malformed exchange has already retired this lane's connection.
fn current_directory(
    client: &mut LocalClient,
    addressed: &AddressedWorktreeSource,
    request_id: WorkbenchRequestId,
    deadline: Instant,
    clock: &LocalOperationClock,
    wake: &LocalStream,
) -> Result<Option<WorkbenchDirectoryPath>, WorktreeNotice> {
    let workspace_id = addressed
        .first_workspace()
        .ok_or(WorktreeNotice::SourceUnavailable)?;
    let result = client
        .current_directory(
            CurrentDirectoryRequest {
                protocol_version: ProtocolVersion,
                request_id,
                session_id: addressed.cache.session_id().clone(),
                workspace_id: workspace_id.clone(),
            },
            deadline.min(clock.now() + Duration::from_secs(5)),
            Some(wake.as_local_io()),
        )
        .map_err(|_| WorktreeNotice::SourceUnavailable)?;
    Ok(match result.outcome {
        CurrentDirectoryOutcome::Observed { directory } => Some(directory),
        CurrentDirectoryOutcome::Failed { .. } => None,
    })
}

pub(super) fn discover(
    slot: &mut Option<LocalClient>,
    input: DiscoveryRequest,
    deadline: Instant,
    clock: &LocalOperationClock,
    wake: &LocalStream,
) -> Result<SidebarDiscovery, WorktreeNotice> {
    let DiscoveryRequest {
        addressed,
        requests,
        expected,
        configured_root,
    } = input;
    let client = slot.as_mut().ok_or(WorktreeNotice::SourceUnavailable)?;
    let [before, directory, query, after] = requests;
    // Discovery ends a second early, so a Start that follows it keeps time to submit.
    let cutoff = deadline
        .checked_sub(Duration::from_secs(1))
        .ok_or(WorktreeNotice::SourceUnavailable)?;
    let fresh =
        snapshot(client, before, cutoff, clock, wake).ok_or(WorktreeNotice::SourceUnavailable)?;
    if !addressed.unchanged(&fresh) {
        return Err(WorktreeNotice::SourceChanged);
    }
    let observed = current_directory(client, &addressed, directory, cutoff, clock, wake)?;
    let source = addressed
        .resolve(observed.as_ref())
        .ok_or(WorktreeNotice::SourceUnavailable)?;
    if expected
        .as_ref()
        .is_some_and(|expected| expected != &source)
    {
        return Err(WorktreeNotice::SourceChanged);
    }
    let root = configured_root?;
    let reply = client
        .query_worktrees(
            WorkbenchWorktreeQuery {
                protocol_version: ProtocolVersion,
                request_id: query,
                session_id: addressed.cache.session_id().clone(),
                source: source.clone(),
                trust_repository: false,
            },
            cutoff.min(clock.now() + Duration::from_secs(5)),
            Some(wake.as_local_io()),
        )
        .map_err(|_| WorktreeNotice::ListingUnavailable)?;
    let failure = listing_failure(&reply.outcome);
    let WorkbenchWorktreeQueryOutcome::Found {
        session_id,
        workbench_revision,
        worker_id,
        source: found,
        entries,
    } = reply.outcome
    else {
        return Err(failure);
    };
    if session_id != *addressed.cache.session_id()
        || workbench_revision < addressed.cache.revision()
    {
        return Err(WorktreeNotice::SourceChanged);
    }
    let fresh =
        snapshot(client, after, cutoff, clock, wake).ok_or(WorktreeNotice::SourceUnavailable)?;
    if !addressed.unchanged(&fresh) {
        return Err(WorktreeNotice::SourceChanged);
    }
    let discovered = SidebarDiscovery {
        addressed,
        observed,
        source,
        discovery: Discovery {
            source: found,
            worker: worker_id,
            entries,
            configured_root: root,
        },
    };
    discovered
        .valid(&fresh)
        .then_some(discovered)
        .ok_or(WorktreeNotice::SourceChanged)
}
