//! Addressed read-only discovery shares the bounded Worktree lane, never the interactive Viewer.
use super::{
    AddressedWorktreeSource, AsLocalIo, Discovery, Duration, Instant, LocalClient,
    LocalOperationClock, LocalStream, ProtocolVersion, SidebarDiscovery, WorkbenchRequestId,
    WorkbenchWorktreeQuery, WorkbenchWorktreeQueryOutcome, WorktreeNotice, listing_failure,
    snapshot,
};
use crate::primary_directory::{RootDirectory, decode_root};
use orchbee_client_server_protocol::{
    RequestId, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
};
use orchbee_workbench_protocol::WorktreeSource;

fn viewer(
    client: &mut LocalClient,
    request: RequestId,
    action: TerminalViewerAction,
    deadline: Instant,
    clock: &LocalOperationClock,
    wake: &LocalStream,
) -> Result<TerminalViewerOutcome, WorktreeNotice> {
    if clock.now() >= deadline {
        return Err(WorktreeNotice::SourceUnavailable);
    }
    client
        .submit_terminal_viewer_cancellable(
            TerminalViewerRequest::new(request, action),
            Some(deadline.min(clock.now() + Duration::from_secs(2))),
            Some(wake.as_local_io()),
        )
        .map(orchbee_client_server_protocol::TerminalViewerResult::into_outcome)
        .map_err(|_| WorktreeNotice::SourceUnavailable)
}

pub(super) struct DiscoveryRequest {
    pub(crate) addressed: AddressedWorktreeSource,
    pub(crate) requests: [WorkbenchRequestId; 3],
    pub(crate) viewers: [RequestId; 4],
    pub(crate) expected: Option<WorktreeSource>,
    pub(crate) configured_root: super::ConfiguredRoot,
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
        viewers,
        expected,
        configured_root,
    } = input;
    let client = slot.as_mut().ok_or(WorktreeNotice::SourceUnavailable)?;
    let [before, query, after] = requests;
    let fresh =
        snapshot(client, before, deadline, clock, wake).ok_or(WorktreeNotice::SourceUnavailable)?;
    if !addressed.unchanged(&fresh) {
        return Err(WorktreeNotice::SourceChanged);
    }
    // Preserve one bounded cleanup reserve even when Git or the process sample exhausts forward work.
    let cutoff = deadline
        .checked_sub(Duration::from_secs(1))
        .ok_or(WorktreeNotice::SourceUnavailable)?;
    let [attach, read, reread, detach] = viewers;
    let attached = if let Some(binding) = &addressed.primary {
        let response = viewer(
            client,
            attach,
            TerminalViewerAction::AttachBinding {
                session_id: orchbee_client_server_protocol::SessionId::parse(
                    addressed.cache.session_id().as_str(),
                )
                .map_err(|_| WorktreeNotice::SourceUnavailable)?,
                worker_id: orchbee_client_server_protocol::WorkerId::parse(
                    binding.worker_id().as_str(),
                )
                .map_err(|_| WorktreeNotice::SourceUnavailable)?,
                terminal_id: orchbee_client_server_protocol::TerminalId::parse(
                    binding.terminal_id().as_str(),
                )
                .map_err(|_| WorktreeNotice::SourceUnavailable)?,
            },
            cutoff,
            clock,
            wake,
        );
        match response {
            Ok(TerminalViewerOutcome::Attached {
                attachment_id,
                host_epoch,
                host_terminal_id,
            }) => Some((attachment_id, host_epoch, host_terminal_id)),
            Ok(
                TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. },
            ) => {
                if addressed
                    .resolve_sample(RootDirectory::Unavailable)
                    .is_none()
                {
                    return Err(WorktreeNotice::SourceUnavailable);
                }
                None
            }
            // Action/result admission belongs to the protocol; this step consumes only its requested viewer record or acknowledgment.
            _ => {
                slot.take();
                return Err(WorktreeNotice::SourceUnavailable);
            }
        }
    } else {
        if addressed.member.is_some()
            && addressed
                .resolve_sample(RootDirectory::Unavailable)
                .is_none()
        {
            return Err(WorktreeNotice::SourceUnavailable);
        }
        None
    };
    let result = (|| {
        let sample = if let Some((id, epoch, terminal)) = &attached {
            decode_root(
                viewer(
                    client,
                    read,
                    TerminalViewerAction::ReadProcessInfo {
                        attachment_id: id.clone(),
                    },
                    cutoff,
                    clock,
                    wake,
                )?,
                id,
                epoch,
                terminal,
            )
            .map_err(|_| WorktreeNotice::SourceUnavailable)?
        } else if addressed.member.is_some() {
            RootDirectory::Unavailable
        } else {
            RootDirectory::Available(None)
        };
        let source = addressed
            .resolve_sample(sample.clone())
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
        let fresh = snapshot(client, after, cutoff, clock, wake)
            .ok_or(WorktreeNotice::SourceUnavailable)?;
        if !addressed.unchanged(&fresh) {
            return Err(WorktreeNotice::SourceChanged);
        }
        if let Some((id, epoch, terminal)) = &attached {
            let current = decode_root(
                viewer(
                    client,
                    reread,
                    TerminalViewerAction::ReadProcessInfo {
                        attachment_id: id.clone(),
                    },
                    cutoff,
                    clock,
                    wake,
                )?,
                id,
                epoch,
                terminal,
            )
            .map_err(|_| WorktreeNotice::SourceUnavailable)?;
            if addressed.resolve_sample(current).as_ref() != Some(&source) {
                return Err(WorktreeNotice::SourceChanged);
            }
        }
        let discovered = SidebarDiscovery {
            addressed,
            sample,
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
    })();
    if let Some((id, _, _)) = attached {
        let released = viewer(
            client,
            detach,
            TerminalViewerAction::Detach { attachment_id: id },
            deadline,
            clock,
            wake,
        );
        if !matches!(released, Ok(TerminalViewerOutcome::Detached {})) {
            // This logical lane owns its auxiliary Viewer. Dropping it cancels that lane and retires any uncertain
            // Viewer; neither the main Viewer nor its controller is borrowed or detached.
            slot.take();
            return Err(WorktreeNotice::SourceUnavailable);
        }
    }
    result
}
