//! One bounded Windows Worktree command/result lane with its own Client Session and joined shutdown.
use super::state::worktree_launch::{Discovery, Origin, Plan, WorktreeNotice, WorktreeToken};
use crate::local_client::{LocalClient, LocalClientOrigin, LocalOperationClock};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchDirectoryPath, WorkbenchRequestId,
    WorkbenchSnapshotOutcome, WorkbenchWorktreeAction, WorkbenchWorktreeOutcome,
    WorkbenchWorktreeQuery, WorkbenchWorktreeQueryOutcome, WorkbenchWorktreeRequest,
    WorktreeSource,
};
use std::{
    io,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
mod sidebar;
use super::state::worktree_launch::addressed::{AddressedWorktreeSource, SidebarDiscovery};
/// The single exchange whose receipt is settling; Read never submits a mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Exchange {
    Start,
    Read,
    Resume,
}
pub(super) enum Action {
    SidebarQuery {
        episode: u64,
        source: Box<AddressedWorktreeSource>,
    },
    Query {
        token: WorktreeToken,
        source: WorktreeSource,
        origin: Box<Origin>,
    },
    Start(Box<Plan>),
    Read(Box<Plan>),
    Resume {
        plan: Box<Plan>,
        expected_attempt: u32,
    },
}
pub(super) struct Job {
    pub action: Action,
    pub requests: [WorkbenchRequestId; 5],
    pub viewers: [orchbee_client_server_protocol::RequestId; 4],
    pub deadline: Instant,
}
pub(super) enum Reply {
    SidebarQuery {
        episode: u64,
        result: Result<Box<SidebarDiscovery>, WorktreeNotice>,
    },
    Query {
        token: WorktreeToken,
        result: Result<Discovery, WorktreeNotice>,
    },
    Operation {
        operation: WorkbenchRequestId,
        exchange: Exchange,
        result: Result<WorkbenchWorktreeOutcome, bool>,
        snapshot: Option<Snapshot>,
    },
}
#[derive(Default)]
struct Handoff {
    job: Option<Job>,
    busy: bool,
    reply: Option<Reply>,
    stopped: bool,
    failed: bool,
}
type Shared = Arc<(Mutex<Handoff>, Condvar)>;
pub(super) struct Lane {
    shared: Shared,
    wake: LocalStream,
    task: Option<JoinHandle<()>>,
}
impl Lane {
    pub fn spawn(origin: LocalClientOrigin, clock: LocalOperationClock) -> io::Result<Self> {
        let shared = Arc::new((Mutex::new(Handoff::default()), Condvar::new()));
        let (read, wake) = LocalStream::pair()?;
        let worker = Arc::clone(&shared);
        let task = thread::Builder::new()
            .name("orchbee-worktree".into())
            .spawn(move || {
                let _end = End(Arc::clone(&worker));
                run(&worker, origin, clock, &read);
            })?;
        Ok(Self {
            shared,
            wake,
            task: Some(task),
        })
    }
    pub fn idle(&self) -> bool {
        self.shared
            .0
            .lock()
            .is_ok_and(|h| !h.failed && !h.stopped && !h.busy && h.reply.is_none())
    }
    pub fn submit(&self, job: Job) -> bool {
        let Ok(mut h) = self.shared.0.lock() else {
            return false;
        };
        if h.stopped || h.failed || h.busy || h.reply.is_some() {
            return false;
        }
        h.busy = true;
        h.job = Some(job);
        self.shared.1.notify_one();
        true
    }
    pub fn take(&self) -> Option<Reply> {
        self.shared.0.lock().ok()?.reply.take()
    }
    pub fn failed(&self) -> bool {
        self.shared.0.lock().map_or(true, |h| h.failed)
    }
    pub fn close(&mut self) -> bool {
        if self.task.is_none() {
            return true;
        }
        if let Ok(mut h) = self.shared.0.lock() {
            h.stopped = true;
        }
        self.shared.1.notify_all();
        let _wake_closed = self.wake.shutdown(std::net::Shutdown::Both);
        self.task.take().is_none_or(|task| task.join().is_ok())
    }
}
impl Drop for Lane {
    fn drop(&mut self) {
        let _joined = self.close();
    }
}
struct End(Shared);
impl Drop for End {
    fn drop(&mut self) {
        if let Ok(mut h) = self.0.0.lock() {
            h.failed = true;
        }
        self.0.1.notify_all();
    }
}
fn run(shared: &Shared, origin: LocalClientOrigin, clock: LocalOperationClock, wake: &LocalStream) {
    let mut client = None;
    loop {
        let (lock, signal) = &**shared;
        let Ok(mut h) = lock.lock() else {
            return;
        };
        while !h.stopped && h.job.is_none() {
            let Ok(next) = signal.wait(h) else {
                return;
            };
            h = next;
        }
        if h.stopped {
            return;
        }
        let Some(job) = h.job.take() else {
            return;
        };
        drop(h);
        if client
            .as_ref()
            .is_none_or(|c: &LocalClient| c.generation().is_none())
        {
            client = origin
                .reconnect_before_cancellable(
                    job.deadline.min(clock.now() + Duration::from_secs(2)),
                    clock.clone(),
                    Some(wake.as_local_io()),
                )
                .ok();
        }
        let reply = execute(&mut client, job, &clock, wake);
        let Ok(mut h) = lock.lock() else {
            return;
        };
        h.busy = false;
        if h.stopped {
            return;
        }
        if h.reply.is_some() {
            return;
        }
        h.reply = Some(reply);
        signal.notify_all();
    }
}
fn snapshot(
    client: &mut LocalClient,
    id: WorkbenchRequestId,
    deadline: Instant,
    clock: &LocalOperationClock,
    wake: &LocalStream,
) -> Option<Snapshot> {
    let result = client
        .submit_workbench_snapshot_read_before_cancellable(
            SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: id,
            },
            deadline.min(clock.now() + Duration::from_secs(2)),
            Some(wake.as_local_io()),
        )
        .ok()?;
    match result.outcome {
        WorkbenchSnapshotOutcome::Available { snapshot } => Some(snapshot),
        _ => None,
    }
}
fn settings() -> Result<Option<WorkbenchDirectoryPath>, WorktreeNotice> {
    let settings =
        orchbee_local_site_runtime::read_windows_user_settings(|name| std::env::var_os(name))
            .map_err(|_| WorktreeNotice::ConfigurationUnavailable)?;
    settings
        .worktree_root
        .as_deref()
        .map(WorkbenchDirectoryPath::parse)
        .transpose()
        .map_err(|_| WorktreeNotice::ConfigurationUnavailable)
}
fn execute(
    client: &mut Option<LocalClient>,
    job: Job,
    clock: &LocalOperationClock,
    wake: &LocalStream,
) -> Reply {
    let [first, second, third, fourth, fifth] = job.requests;
    match job.action {
        Action::SidebarQuery { episode, source } => {
            let result = sidebar::discover(
                client,
                sidebar::DiscoveryRequest {
                    addressed: *source,
                    requests: [first, second, third],
                    viewers: job.viewers,
                    expected: None,
                },
                job.deadline,
                clock,
                wake,
            )
            .map(Box::new);
            Reply::SidebarQuery { episode, result }
        }
        Action::Query {
            token,
            source,
            origin,
        } => {
            let result = (|| {
                let client = client.as_mut().ok_or(WorktreeNotice::SourceUnavailable)?;
                let root = settings()?;
                let reply = client
                    .query_worktrees(
                        WorkbenchWorktreeQuery {
                            protocol_version: ProtocolVersion,
                            request_id: first,
                            site_id: origin.cache.site_id().clone(),
                            source: source.clone(),
                            trust_repository: false,
                        },
                        job.deadline.min(clock.now() + Duration::from_secs(5)),
                        Some(wake.as_local_io()),
                    )
                    .map_err(|_| WorktreeNotice::ListingUnavailable)?;
                let WorkbenchWorktreeQueryOutcome::Found {
                    site_id,
                    workbench_revision,
                    worker_id,
                    source,
                    entries,
                } = reply.outcome
                else {
                    return Err(WorktreeNotice::ListingUnavailable);
                };
                if site_id != *origin.cache.site_id()
                    || workbench_revision < origin.cache.revision()
                {
                    return Err(WorktreeNotice::SourceChanged);
                }
                Ok(Discovery {
                    source,
                    worker: worker_id,
                    entries,
                    configured_root: root,
                })
            })();
            Reply::Query { token, result }
        }
        action @ (Action::Start(_) | Action::Read(_) | Action::Resume { .. }) => {
            let (plan, exchange, expected_attempt) = match action {
                Action::Start(p) => (p, Exchange::Start, None),
                Action::Read(p) => (p, Exchange::Read, None),
                Action::Resume {
                    plan,
                    expected_attempt,
                } => (plan, Exchange::Resume, Some(expected_attempt)),
                _ => unreachable!(),
            };
            let start = exchange == Exchange::Start;
            let resume = exchange == Exchange::Resume;
            let operation = plan.operation_id.clone();
            if start && let Some(addressed) = &plan.sidebar {
                let checked = sidebar::discover(
                    client,
                    sidebar::DiscoveryRequest {
                        addressed: addressed.addressed.clone(),
                        requests: [first.clone(), second.clone(), third.clone()],
                        viewers: job.viewers,
                        expected: Some(addressed.sample.clone()),
                    },
                    job.deadline,
                    clock,
                    wake,
                );
                if checked.as_ref().map_or(true, |current| {
                    current.sample != addressed.sample
                        || current.source != addressed.source
                        || current.discovery.source != addressed.discovery.source
                        || current.discovery.worker != addressed.discovery.worker
                        || current.discovery.configured_root != addressed.discovery.configured_root
                        || plan.removal.as_ref().is_some_and(|target| {
                            super::state::worktree_launch::Removal::capture(current).as_ref()
                                != Some(target)
                        })
                }) {
                    return Reply::Operation {
                        operation,
                        exchange,
                        result: Err(false),
                        snapshot: None,
                    };
                }
            }
            let Some(client) = client.as_mut() else {
                return Reply::Operation {
                    operation,
                    exchange,
                    result: Err(!start && !resume),
                    snapshot: None,
                };
            };
            let result = (|| {
                if start && plan.sidebar.is_none() {
                    let fresh = snapshot(client, first, job.deadline, clock, wake).ok_or(false)?;
                    if plan
                        .origin
                        .cache
                        .project_snapshot(&fresh)
                        .ok()
                        .is_none_or(|current| current.directory() != plan.origin.cache.directory())
                        || settings().map_err(|_| false)? != plan.discovery.configured_root
                    {
                        return Err(false);
                    }
                    let observed = client
                        .query_worktrees(
                            WorkbenchWorktreeQuery {
                                protocol_version: ProtocolVersion,
                                request_id: second,
                                site_id: plan.origin.cache.site_id().clone(),
                                source: plan.source.clone(),
                                trust_repository: false,
                            },
                            job.deadline.min(clock.now() + Duration::from_secs(5)),
                            Some(wake.as_local_io()),
                        )
                        .map_err(|_| false)?;
                    let WorkbenchWorktreeQueryOutcome::Found {
                        source, worker_id, ..
                    } = observed.outcome
                    else {
                        return Err(false);
                    };
                    if source != plan.discovery.source || worker_id != plan.discovery.worker {
                        return Err(false);
                    }
                }
                let action = if let Some(expected_attempt) = expected_attempt {
                    WorkbenchWorktreeAction::Resume {
                        operation_id: operation.clone(),
                        expected_attempt,
                    }
                } else if start {
                    WorkbenchWorktreeAction::Start {
                        operation_id: operation.clone(),
                        project_id: plan.project.clone(),
                        intent: plan.intent.clone(),
                    }
                } else {
                    WorkbenchWorktreeAction::Read {
                        operation_id: operation.clone(),
                    }
                };
                client
                    .submit_worktree_operation(
                        WorkbenchWorktreeRequest {
                            protocol_version: ProtocolVersion,
                            request_id: if plan.sidebar.is_some() {
                                fourth.clone()
                            } else {
                                third
                            },
                            site_id: plan.origin.cache.site_id().clone(),
                            action,
                        },
                        job.deadline.min(clock.now() + Duration::from_secs(2)),
                        Some(wake.as_local_io()),
                    )
                    .map(|r| r.outcome)
                    .map_err(|e| if start || resume { e.submitted } else { true })
            })();
            let fresh = if matches!(&result, Ok(WorkbenchWorktreeOutcome::Available { status, .. })
                if matches!(status.as_ref(), orchbee_workbench_protocol::WorktreeOperationStatus::Opened { .. }))
            {
                snapshot(
                    client,
                    if plan.sidebar.is_some() {
                        fifth
                    } else {
                        fourth
                    },
                    job.deadline,
                    clock,
                    wake,
                )
            } else {
                None
            };
            Reply::Operation {
                operation,
                exchange,
                result,
                snapshot: fresh,
            }
        }
    }
}
#[cfg(test)]
mod tests;
