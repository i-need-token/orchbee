//! Canonical Workspace Git reads fenced around a settled Worker discovery job.
use super::LocalTerminalService;
use crate::worker_obcn_carrier::{ExchangeWait, WorkerOperationResult, WorkerRequestHandle};
use orchbee_capability_node_protocol as node;
use orchbee_workbench_protocol::{
    GitDivergence, ProtocolVersion, WorkbenchResponse, WorkerId, WorkspaceGitFailure as Failure,
    WorkspaceGitOutcome as Outcome, WorkspaceGitRequest, WorkspaceGitResult, WorktreeSource,
    serialize_workbench_response,
};
use std::time::{Duration, Instant};

pub(super) fn parse_status(
    bytes: &[u8],
) -> Result<(Option<String>, Option<GitDivergence>), Failure> {
    if bytes.len() > 4096 || !bytes.ends_with(b"\n") {
        return Err(Failure::Malformed);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Failure::Malformed)?;
    let (mut oid, mut head, mut upstream, mut divergence) = (None, None, None, None);
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("# branch.oid ") {
            if oid.replace(value).is_some()
                || (value != "(initial)"
                    && (!matches!(value.len(), 40 | 64)
                        || !value
                            .bytes()
                            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())))
            {
                return Err(Failure::Malformed);
            }
        } else if let Some(value) = line.strip_prefix("# branch.head ") {
            if head.replace(value).is_some()
                || value.is_empty()
                || value.len() > 1024
                || value.chars().any(char::is_control)
            {
                return Err(Failure::Malformed);
            }
        } else if let Some(value) = line.strip_prefix("# branch.upstream ") {
            if upstream.replace(value).is_some()
                || value.is_empty()
                || value.chars().any(char::is_control)
            {
                return Err(Failure::Malformed);
            }
        } else if let Some(value) = line.strip_prefix("# branch.ab ") {
            let (a, b) = value.split_once(' ').ok_or(Failure::Malformed)?;
            let number = |s: &str, sign| -> Result<u32, Failure> {
                let n = s.strip_prefix(sign).ok_or(Failure::Malformed)?;
                let value = n.parse::<u32>().map_err(|_| Failure::Malformed)?;
                if value.to_string() != n {
                    return Err(Failure::Malformed);
                }
                Ok(value)
            };
            let counts = GitDivergence {
                ahead: number(a, '+')?,
                behind: number(b, '-')?,
            };
            if divergence.replace(counts).is_some() {
                return Err(Failure::Malformed);
            }
        } else {
            return Err(Failure::Malformed);
        }
    }
    let oid = oid.ok_or(Failure::Malformed)?;
    let head = head.ok_or(Failure::Malformed)?;
    let branch = (head != "(detached)").then(|| head.to_owned());
    if divergence.is_some() && (branch.is_none() || upstream.is_none() || oid == "(initial)") {
        return Err(Failure::Malformed);
    }
    Ok((branch, divergence))
}

struct Driver {
    handle: WorkerRequestHandle,
    worker: node::WorkerId,
    generation: node::WorkerGeneration,
    base: u128,
    job: Option<node::OperationId>,
}
impl Driver {
    fn request(
        &mut self,
        action: node::WorktreeJobAction,
    ) -> Result<node::WorktreeJobRequest, Failure> {
        self.base = self.base.checked_add(1).ok_or(Failure::Unavailable)?;
        let operation_id = node::OperationId::from_bytes(self.base.to_be_bytes())
            .map_err(|_| Failure::Unavailable)?;
        Ok(node::WorktreeJobRequest {
            operation_id,
            worker_id: self.worker,
            worker_generation: self.generation,
            action,
        })
    }
    fn call(
        &mut self,
        action: node::WorktreeJobAction,
        deadline: Instant,
    ) -> Result<node::WorktreeJobResult, Failure> {
        if Instant::now() >= deadline {
            return Err(Failure::Timeout);
        }
        let request = self.request(action)?;
        let started = matches!(request.action, node::WorktreeJobAction::StartStatus { .. })
            .then_some(request.operation_id);
        let exchange = self
            .handle
            .admit_worktree_job(request)
            .map_err(|_| Failure::Unavailable)?;
        // Job identity is the start operation identity even if the accepted
        // reply arrives after our deadline. Retain it only after admission.
        if let Some(job) = started {
            self.job = Some(job);
        }
        let result = match exchange.recv_deadline(deadline) {
            ExchangeWait::Ready(result) => result.map_err(|_| Failure::Unavailable)?,
            ExchangeWait::Elapsed => return Err(Failure::Timeout),
            ExchangeWait::Lost(_) => return Err(Failure::Unavailable),
        };
        match result {
            WorkerOperationResult::WorktreeJob(result) => Ok(result),
            _ => Err(Failure::Unavailable),
        }
    }
    fn read(
        &mut self,
        path: &str,
        deadline: Instant,
    ) -> Result<(Option<String>, Option<GitDivergence>), Failure> {
        let request = node::WorktreeJobAction::start_status(path.as_bytes().to_vec())
            .map_err(|_| Failure::Unavailable)?;
        let result = self.call(request, deadline)?;
        if result.outcome
            != node::WorktreeJobOutcome::Start(node::WorktreeJobStartOutcome::Accepted)
        {
            self.job = None;
            return Err(Failure::Unavailable);
        }
        let job = result.job_id;
        self.job = Some(job);
        for _ in 0..100 {
            if Instant::now() >= deadline {
                return Err(Failure::Timeout);
            }
            let result = self.call(node::WorktreeJobAction::Poll { job_id: job }, deadline)?;
            match result.outcome {
                node::WorktreeJobOutcome::Poll(node::WorktreeJobPollOutcome::Pending) => {
                    std::thread::sleep(
                        deadline
                            .saturating_duration_since(Instant::now())
                            .min(Duration::from_millis(50)),
                    )
                }
                node::WorktreeJobOutcome::Poll(node::WorktreeJobPollOutcome::StatusCompleted {
                    status,
                    ..
                }) => {
                    self.job = None;
                    return parse_status(&status);
                }
                node::WorktreeJobOutcome::Poll(node::WorktreeJobPollOutcome::Timeout) => {
                    self.job = None;
                    return Err(Failure::Timeout);
                }
                node::WorktreeJobOutcome::Poll(node::WorktreeJobPollOutcome::GitFailed {
                    ..
                }) => {
                    self.job = None;
                    return Err(Failure::Unavailable);
                }
                _ => return Err(Failure::Unavailable),
            }
        }
        Err(Failure::Timeout)
    }
}
impl Drop for Driver {
    fn drop(&mut self) {
        if let Some(job_id) = self.job.take()
            && let Ok(request) = self.request(node::WorktreeJobAction::Cancel { job_id })
        {
            // Same high-lane FIFO as the admitted start/polls; no blocking
            // admission or reply wait in Drop. A lost/full carrier leaves
            // the existing Worker TTL as the physical cleanup owner.
            let _admitted = self.handle.admit_worktree_job(request);
        }
    }
}
impl LocalTerminalService {
    pub(crate) fn execute_workspace_git(&self, request: WorkspaceGitRequest) -> WorkspaceGitResult {
        self.execute_workspace_git_until(request, Instant::now() + Duration::from_secs(5))
    }
    pub(super) fn execute_workspace_git_until(
        &self,
        request: WorkspaceGitRequest,
        deadline: Instant,
    ) -> WorkspaceGitResult {
        let result = WorkspaceGitResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            site_id: request.site_id.clone(),
            workspace_id: request.workspace_id.clone(),
            outcome: self
                .workspace_git(&request, deadline)
                .unwrap_or_else(|reason| Outcome::Failed { reason }),
        };
        if result.validate().is_err()
            || serialize_workbench_response(&WorkbenchResponse::WorkspaceGit(result.clone()))
                .is_err()
        {
            WorkspaceGitResult {
                outcome: Outcome::Failed {
                    reason: Failure::Malformed,
                },
                ..result
            }
        } else {
            result
        }
    }
    fn workspace_git(
        &self,
        request: &WorkspaceGitRequest,
        deadline: Instant,
    ) -> Result<Outcome, Failure> {
        let source = WorktreeSource::Workspace {
            workspace_id: request.workspace_id.clone(),
        };
        let (target, handle, base, revision) = {
            let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
            let mut coordinator = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
            let authority = coordinator
                .worktree_authority()
                .ok_or(Failure::Unavailable)?;
            if authority.site_id() != &request.site_id {
                return Err(Failure::Denied);
            }
            let handle = workers
                .request_handle()
                .filter(|h| h.is_active())
                .ok_or(Failure::Unavailable)?;
            let worker = WorkerId::parse(&format!(
                "{:032x}",
                u128::from_be_bytes(handle.worker_id().as_bytes())
            ))
            .map_err(|_| Failure::Unavailable)?;
            let target = authority
                .worktree_query_target(&source, &worker)
                .map_err(|reason| match reason {
                    orchbee_workbench_protocol::WorktreeQueryFailure::NotFound {} => {
                        Failure::NotFound
                    }
                    _ => Failure::Unavailable,
                })?;
            let revision = authority.snapshot().revision;
            let base = coordinator
                .allocate_identity_block(128)
                .ok_or(Failure::Unavailable)?;
            (target, handle, base, revision)
        };
        let worker = handle.worker_id();
        let generation = handle.generation();
        let mut driver = Driver {
            worker: node::WorkerId::from_bytes(worker.as_bytes())
                .map_err(|_| Failure::Unavailable)?,
            generation: node::WorkerGeneration::new(generation),
            handle,
            base,
            job: None,
        };
        let (branch, divergence) = driver.read(target.path.as_str(), deadline)?;
        if Instant::now() >= deadline {
            return Err(Failure::Timeout);
        }
        let workers = self.workers.lock().map_err(|_| Failure::Unavailable)?;
        let mut coordinator = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
        let handle = workers
            .request_handle()
            .filter(|h| h.is_active())
            .ok_or(Failure::Unavailable)?;
        if handle.worker_id() != worker || handle.generation() != generation {
            return Err(Failure::SourceChanged);
        }
        let authority = coordinator
            .worktree_authority()
            .ok_or(Failure::Unavailable)?;
        if authority.site_id() != &request.site_id
            || authority.snapshot().revision != revision
            || authority
                .worktree_query_target(&source, &target.worker_id)
                .as_ref()
                != Ok(&target)
        {
            return Err(Failure::SourceChanged);
        }
        Ok(Outcome::Observed {
            workbench_revision: revision,
            branch,
            divergence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn oid() -> String {
        "a".repeat(40)
    }
    #[test]
    fn branch_counts_are_exact_local_upstream_values_and_absence_is_not_zero() {
        let data = format!(
            "# branch.oid {}\n# branch.head worktree/测试\n# branch.upstream origin/main\n# branch.ab +12 -3\n",
            oid()
        );
        assert_eq!(
            parse_status(data.as_bytes()),
            Ok((
                Some("worktree/测试".into()),
                Some(GitDivergence {
                    ahead: 12,
                    behind: 3
                })
            ))
        );
        let clean = data.replace("+12 -3", "+0 -0");
        assert_eq!(
            parse_status(clean.as_bytes()).unwrap().1,
            Some(GitDivergence {
                ahead: 0,
                behind: 0
            })
        );
        let missing = format!("# branch.oid {}\n# branch.head main\n", oid());
        assert_eq!(
            parse_status(missing.as_bytes()),
            Ok((Some("main".into()), None))
        );
    }
    #[test]
    fn detached_unborn_and_malformed_statuses_do_not_fabricate_branch_or_counts() {
        assert_eq!(
            parse_status(format!("# branch.oid {}\n# branch.head (detached)\n", oid()).as_bytes()),
            Ok((None, None))
        );
        assert_eq!(
            parse_status(b"# branch.oid (initial)\n# branch.head first\n"),
            Ok((Some("first".into()), None))
        );
        let prefix = format!(
            "# branch.oid {}\n# branch.head main\n# branch.upstream origin/main\n",
            oid()
        );
        for suffix in [
            "# branch.ab +01 -0\n",
            "# branch.ab +4294967296 -0\n",
            "# branch.ab +1 -2 extra\n",
            "# branch.ab +1 -2\n# branch.ab +1 -2\n",
            "# branch.head duplicate\n",
            "1 .M N... arbitrary file\n",
            "# branch.ab +1 -2",
        ] {
            assert!(
                parse_status(format!("{prefix}{suffix}").as_bytes()).is_err(),
                "{suffix}"
            );
        }
        assert!(
            parse_status(b"# branch.oid (initial)\n# branch.head first\n# branch.ab +0 -0\n")
                .is_err()
        );
        assert!(parse_status(&[0xff, b'\n']).is_err());
    }
}
