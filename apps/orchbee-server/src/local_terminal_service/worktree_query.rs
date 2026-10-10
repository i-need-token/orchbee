//! Source discovery holds no Site/registry gate across the read-only Worker job.
use super::{
    LocalTerminalService,
    worktree_list::{discover_source, parse_worktree_listing},
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchDirectoryPath, WorkbenchResponse, WorkbenchWorktreeListOutcome,
    WorkbenchWorktreeQuery, WorkbenchWorktreeQueryOutcome as Outcome, WorkbenchWorktreeQueryResult,
    WorkerId, WorktreeEntryText, WorktreeListTooLargeOrigin, WorktreeListUnavailableReason,
    WorktreeQueryFailure as Failure, WorktreeQuerySource, WorktreeSource,
    serialize_workbench_response,
};
use std::time::{Duration, Instant};

impl LocalTerminalService {
    pub(crate) fn execute_worktree_query(
        &self,
        request: WorkbenchWorktreeQuery,
    ) -> WorkbenchWorktreeQueryResult {
        self.execute_worktree_query_until(request, Instant::now() + Duration::from_secs(5))
    }

    pub(super) fn execute_worktree_query_until(
        &self,
        request: WorkbenchWorktreeQuery,
        deadline: Instant,
    ) -> WorkbenchWorktreeQueryResult {
        let mut result = WorkbenchWorktreeQueryResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            outcome: self
                .query_worktrees(&request, deadline)
                .unwrap_or_else(|reason| Outcome::Failed { reason }),
        };
        if result.validate().is_err() {
            result.outcome = Outcome::Failed {
                reason: Failure::MalformedListing {},
            };
        } else if serialize_workbench_response(&WorkbenchResponse::WorktreeQuery(result.clone()))
            .is_err()
        {
            result.outcome = Outcome::Failed {
                reason: Failure::TooLarge {
                    origin: WorktreeListTooLargeOrigin::WorkbenchResult,
                },
            };
        }
        result
    }

    fn query_worktrees(
        &self,
        request: &WorkbenchWorktreeQuery,
        deadline: Instant,
    ) -> Result<Outcome, Failure> {
        if !cfg!(windows) {
            return Err(Failure::Unsupported {});
        }
        let (target, handle, base) = {
            let workers = self.workers.lock().map_err(|_| busy())?;
            let mut coordinator = self.coordinator.lock().map_err(|_| busy())?;
            let authority = coordinator.worktree_authority().ok_or_else(busy)?;
            if authority.site_id() != &request.site_id {
                return Err(Failure::Denied {});
            }
            let handle = workers.request_handle().ok_or_else(worker_unavailable)?;
            let worker = WorkerId::parse(&format!(
                "{:032x}",
                u128::from_be_bytes(handle.worker_id().as_bytes())
            ))
            .map_err(|_| worker_unavailable())?;
            let target = authority.worktree_query_target(&request.source, &worker)?;
            let base = coordinator.allocate_identity_block(128).ok_or_else(busy)?;
            (target, handle, base)
        };
        let _permit = self
            .worktree_discovery
            .foreground(deadline)
            .map_err(query_failure)?;
        let generation = handle.generation();
        let worker = handle.worker_id();
        let observed = discover_source(
            handle,
            base,
            target.path.as_str().as_bytes().to_vec(),
            request.trust_repository,
            deadline,
        )
        .map_err(|failure| query_failure(*failure))?;
        if Instant::now() >= deadline {
            return Err(Failure::Timeout {});
        }
        let common_directory = git_path(&observed.common_directory)?;
        let checkout_path = git_path(
            observed
                .source_checkout
                .as_deref()
                .ok_or(Failure::MalformedListing {})?,
        )?;
        let entries =
            parse_worktree_listing(&observed.listing).map_err(|()| Failure::MalformedListing {})?;
        let repository_root = match entries.first().map(|entry| &entry.path) {
            Some(WorktreeEntryText::Utf8(path)) => {
                WorkbenchDirectoryPath::parse(path).map_err(|_| Failure::MalformedListing {})?
            }
            _ => return Err(Failure::MalformedListing {}),
        };
        let workers = self.workers.lock().map_err(|_| busy())?;
        let mut coordinator = self.coordinator.lock().map_err(|_| busy())?;
        let handle = workers.request_handle().ok_or_else(worker_unavailable)?;
        if handle.worker_id() != worker || handle.generation() != generation {
            return Err(worker_unavailable());
        }
        let authority = coordinator.worktree_authority().ok_or_else(busy)?;
        if authority.site_id() != &request.site_id
            || authority
                .worktree_query_target(&request.source, &target.worker_id)
                .as_ref()
                != Ok(&target)
        {
            return Err(Failure::SourceChanged {});
        }
        let entries = authority.worktree_query_entries(
            &target.worker_id,
            &request.source,
            &checkout_path,
            entries,
        )?;
        if Instant::now() >= deadline {
            return Err(Failure::Timeout {});
        }
        Ok(Outcome::Found {
            site_id: request.site_id.clone(),
            workbench_revision: authority.snapshot().revision,
            worker_id: target.worker_id,
            source: WorktreeQuerySource {
                requested_path: target.path,
                repository_root,
                common_directory,
                checkout_path,
                workspace_id: match &request.source {
                    WorktreeSource::Workspace { workspace_id } => Some(workspace_id.clone()),
                    WorktreeSource::Directory { .. } => None,
                },
            },
            entries,
        })
    }
}

fn git_path(bytes: &[u8]) -> Result<WorkbenchDirectoryPath, Failure> {
    let bytes = bytes
        .strip_suffix(b"\n")
        .ok_or(Failure::MalformedListing {})?;
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    if bytes.contains(&b'\n') || bytes.contains(&b'\r') {
        return Err(Failure::MalformedListing {});
    }
    let path = std::str::from_utf8(bytes).map_err(|_| Failure::MalformedListing {})?;
    WorkbenchDirectoryPath::parse(path).map_err(|_| Failure::MalformedListing {})
}
fn busy() -> Failure {
    Failure::Unavailable {
        reason: WorktreeListUnavailableReason::ServerBusy,
    }
}
fn worker_unavailable() -> Failure {
    Failure::Unavailable {
        reason: WorktreeListUnavailableReason::WorkerUnavailable,
    }
}
fn query_failure(outcome: WorkbenchWorktreeListOutcome) -> Failure {
    match outcome {
        WorkbenchWorktreeListOutcome::Found { .. }
        | WorkbenchWorktreeListOutcome::MalformedListing {} => Failure::MalformedListing {},
        WorkbenchWorktreeListOutcome::NotFound {} => Failure::NotFound {},
        WorkbenchWorktreeListOutcome::Denied {} => Failure::Denied {},
        WorkbenchWorktreeListOutcome::Unavailable { reason } => Failure::Unavailable { reason },
        WorkbenchWorktreeListOutcome::GitFailed { stage, status } => {
            Failure::GitFailed { stage, status }
        }
        WorkbenchWorktreeListOutcome::SpawnUnavailable { stage } => {
            Failure::SpawnUnavailable { stage }
        }
        WorkbenchWorktreeListOutcome::SpawnFailed { stage } => Failure::SpawnFailed { stage },
        WorkbenchWorktreeListOutcome::TooLarge { origin } => Failure::TooLarge { origin },
        WorkbenchWorktreeListOutcome::Timeout {} => Failure::Timeout {},
        WorkbenchWorktreeListOutcome::OwnerIo { stage } => Failure::OwnerIo { stage },
        WorkbenchWorktreeListOutcome::TransportLoss {} => Failure::TransportLoss {},
    }
}

#[cfg(all(test, windows))]
mod tests;
