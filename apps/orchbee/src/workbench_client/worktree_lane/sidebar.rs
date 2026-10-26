//! Addressed read-only discovery shares the bounded Worktree lane, never the interactive Viewer.
use super::{
    AddressedWorktreeSource, AsLocalIo, Discovery, Duration, Instant, LocalClient,
    LocalOperationClock, LocalStream, ProtocolVersion, SidebarDiscovery, WorkbenchRequestId,
    WorkbenchWorktreeQuery, WorkbenchWorktreeQueryOutcome, WorktreeNotice, settings, snapshot,
};
use crate::workbench_client::primary_directory::{RootDirectory, decode_root};
use orchbee_client_server_protocol::{
    RequestId, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
};

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
    pub addressed: AddressedWorktreeSource,
    pub requests: [WorkbenchRequestId; 3],
    pub viewers: [RequestId; 4],
    pub expected: Option<RootDirectory>,
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
                site_id: orchbee_client_server_protocol::SiteId::parse(
                    addressed.cache.site_id().as_str(),
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
                return Err(WorktreeNotice::SourceUnavailable);
            }
            _ => {
                slot.take();
                return Err(WorktreeNotice::SourceUnavailable);
            }
        }
    } else {
        if addressed.member.is_some() {
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
        } else {
            RootDirectory::Available(None)
        };
        if sample == RootDirectory::Unavailable {
            return Err(WorktreeNotice::SourceUnavailable);
        }
        if expected
            .as_ref()
            .is_some_and(|expected| expected != &sample)
        {
            return Err(WorktreeNotice::SourceChanged);
        }
        let source = addressed
            .resolve_sample(sample.clone())
            .ok_or(WorktreeNotice::SourceUnavailable)?;
        let root = settings()?;
        let reply = client
            .query_worktrees(
                WorkbenchWorktreeQuery {
                    protocol_version: ProtocolVersion,
                    request_id: query,
                    site_id: addressed.cache.site_id().clone(),
                    source: source.clone(),
                    trust_repository: false,
                },
                cutoff.min(clock.now() + Duration::from_secs(5)),
                Some(wake.as_local_io()),
            )
            .map_err(|_| WorktreeNotice::ListingUnavailable)?;
        let WorkbenchWorktreeQueryOutcome::Found {
            site_id,
            workbench_revision,
            worker_id,
            source: found,
            entries,
        } = reply.outcome
        else {
            return Err(WorktreeNotice::ListingUnavailable);
        };
        if site_id != *addressed.cache.site_id() || workbench_revision < addressed.cache.revision()
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
            if current != sample {
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
            // This lane owns the entire auxiliary Client Connection. Dropping it retires any uncertain
            // Viewer; neither the main Viewer nor its controller is borrowed or detached.
            slot.take();
            return Err(WorktreeNotice::SourceUnavailable);
        }
    }
    result
}
