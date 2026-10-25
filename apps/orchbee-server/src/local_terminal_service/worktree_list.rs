//! Serialized read-only discovery owns admission through receipt consumption or cancellation.
use std::num::NonZeroU64;
use std::sync::{Condvar, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

use orchbee_capability_node_protocol::{
    OperationId, WorkerGeneration, WorkerId, WorktreeJobAction, WorktreeJobOutcome,
    WorktreeJobPollOutcome, WorktreeJobRequest, WorktreeJobStage, WorktreeJobStartOutcome,
};
use orchbee_server::WorktreeListResolution;
use orchbee_workbench_protocol::{
    Directory, ProtocolVersion, SiteId, WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision,
    WorkbenchWorktreeListOutcome, WorkbenchWorktreeListRead, WorkbenchWorktreeListResult,
    WorktreeEntry, WorktreeEntryText, WorktreeListStage, WorktreeListTooLargeOrigin,
    WorktreeListUnavailableReason, serialize_workbench_response,
};

use crate::worker_obcn_carrier::{
    ExchangeWait, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
};

use super::LocalTerminalService;

const SERVER_WORKTREE_LIST_BUDGET: Duration = Duration::from_secs(5);
const WORKTREE_LIST_POLL_INTERVAL: Duration = Duration::from_millis(50);
// Starts (including definite Busy refusals) and polls share this bound, leaving
// room in the reserved identity block for exact-job cancellation.
const WORKTREE_LIST_MAXIMUM_REQUESTS: u32 = 101;
const WORKTREE_LIST_IDENTITY_RESERVE: u128 = 128;

#[derive(Default)]
struct DiscoveryAdmission {
    active: bool,
    foreground_waiting: bool,
}

/// One discovery lifetime and one waiting foreground reader per local Worker.
/// Background observations never queue or overtake a registered foreground read.
#[derive(Default)]
pub(super) struct DiscoveryGate {
    admission: Mutex<DiscoveryAdmission>,
    changed: Condvar,
}

/// Releases admission only after the driver's terminal read or nonblocking cancel.
pub(super) struct DiscoveryPermit<'a>(&'a DiscoveryGate);

impl DiscoveryGate {
    pub(super) fn background(&self) -> Option<DiscoveryPermit<'_>> {
        let mut state = self.admission.lock().ok()?;
        if state.active || state.foreground_waiting {
            return None;
        }
        state.active = true;
        Some(DiscoveryPermit(self))
    }

    pub(super) fn foreground(
        &self,
        deadline: Instant,
    ) -> Result<DiscoveryPermit<'_>, WorkbenchWorktreeListOutcome> {
        let mut state = self
            .admission
            .lock()
            .map_err(|_| unavailable(WorktreeListUnavailableReason::ServerBusy))?;
        if Instant::now() >= deadline {
            return Err(WorkbenchWorktreeListOutcome::Timeout {});
        }
        // One waiter bounds retained request state to the physical owner plus
        // one successor. Existing waiters own the next turn, even after release.
        if state.foreground_waiting {
            return Err(unavailable(WorktreeListUnavailableReason::ServerBusy));
        }
        state.foreground_waiting = true;
        while state.active {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                state.foreground_waiting = false;
                return Err(WorkbenchWorktreeListOutcome::Timeout {});
            }
            state = match self.changed.wait_timeout(state, remaining) {
                Ok((state, _)) => state,
                Err(poisoned) => {
                    poisoned.into_inner().0.foreground_waiting = false;
                    return Err(unavailable(WorktreeListUnavailableReason::ServerBusy));
                }
            };
        }
        state.foreground_waiting = false;
        if Instant::now() >= deadline {
            return Err(WorkbenchWorktreeListOutcome::Timeout {});
        }
        state.active = true;
        Ok(DiscoveryPermit(self))
    }
}

impl Drop for DiscoveryPermit<'_> {
    fn drop(&mut self) {
        // No Worker/Site lock, IPC or physical cleanup wait is held here.
        let mut state = self.0.admission.lock().unwrap_or_else(|e| e.into_inner());
        state.active = false;
        self.0.changed.notify_one();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolutionFailure {
    Missing,
    Unavailable(WorktreeListUnavailableReason),
}

#[derive(Debug)]
struct ReadyTarget {
    directory: Directory,
    generation: NonZeroU64,
    operation_base: u128,
}

pub(super) struct ObservedWorktrees {
    pub(super) common_directory: Vec<u8>,
    pub(super) source_checkout: Option<Vec<u8>>,
    pub(super) listing: Vec<u8>,
}

enum JobTerminal {
    Completed(ObservedWorktrees),
    Failed(WorkbenchWorktreeListOutcome),
}

// Internal driver failures never carry a listing payload: only the
// pre-terminal request path can fail, and its entire surface is worker
// unavailability or transport loss. Conversion to the public outcome
// happens once at the execute boundary.
enum JobFailure {
    NotFound,
    Timeout,
    Unavailable(WorktreeListUnavailableReason),
    TransportLoss,
}

impl From<JobFailure> for WorkbenchWorktreeListOutcome {
    fn from(failure: JobFailure) -> Self {
        match failure {
            JobFailure::NotFound => Self::NotFound {},
            JobFailure::Timeout => Self::Timeout {},
            JobFailure::Unavailable(reason) => Self::Unavailable { reason },
            JobFailure::TransportLoss => Self::TransportLoss {},
        }
    }
}

enum PollStep {
    Pending,
    Done(JobTerminal),
}

struct JobDriver {
    handle: WorkerRequestHandle,
    worker_id: WorkerId,
    generation: WorkerGeneration,
    operation_base: u128,
    deadline: Instant,
    directory_path: Vec<u8>,
    requests: u32,
    job: Option<OperationId>,
}

impl JobDriver {
    fn operation_id(&mut self) -> Result<OperationId, JobFailure> {
        self.operation_base = self
            .operation_base
            .checked_add(1)
            .ok_or(JobFailure::Unavailable(
                WorktreeListUnavailableReason::ServerBusy,
            ))?;
        OperationId::from_bytes(self.operation_base.to_be_bytes())
            .map_err(|_| JobFailure::Unavailable(WorktreeListUnavailableReason::ServerBusy))
    }

    fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    fn request(
        &mut self,
        action: WorktreeJobAction,
        operation_id: OperationId,
    ) -> Result<WorkerOperationResult, JobFailure> {
        if self.expired() || self.requests >= WORKTREE_LIST_MAXIMUM_REQUESTS {
            return Err(JobFailure::Timeout);
        }
        self.requests += 1;
        let started = matches!(action, WorktreeJobAction::Start { .. });
        let exchange = self
            .handle
            .admit_worktree_job(WorktreeJobRequest {
                operation_id,
                worker_id: self.worker_id,
                worker_generation: self.generation,
                action,
            })
            .map_err(job_request_failure)?;
        // Retain the admitted start ID even if its Accepted reply is late.
        // The carrier checks every reply against this exact operation and job.
        if started {
            self.job = Some(operation_id);
        }
        match exchange.recv_deadline(self.deadline) {
            ExchangeWait::Ready(result) => result.map_err(job_request_failure),
            ExchangeWait::Elapsed => Err(JobFailure::Timeout),
            ExchangeWait::Lost(error) => Err(job_request_failure(error)),
        }
    }

    fn start(&mut self, trust: Option<bool>) -> Result<Option<OperationId>, JobFailure> {
        let operation_id = self.operation_id()?;
        let result = self.request(
            WorktreeJobAction::Start {
                directory: self.directory_path.clone(),
                trust_repository: trust,
            },
            operation_id,
        )?;
        let WorkerOperationResult::WorktreeJob(result) = result else {
            return Err(JobFailure::Unavailable(
                WorktreeListUnavailableReason::WorkerUnavailable,
            ));
        };
        let WorktreeJobOutcome::Start(outcome) = result.outcome else {
            return Err(JobFailure::Unavailable(
                WorktreeListUnavailableReason::WorkerUnavailable,
            ));
        };
        if outcome != WorktreeJobStartOutcome::Accepted {
            self.job = None;
        }
        match outcome {
            WorktreeJobStartOutcome::Accepted => Ok(Some(result.job_id)),
            // A previous abandoned reader may still be physically cancelling.
            // Only this definite pre-effect refusal permits another fresh start.
            WorktreeJobStartOutcome::Busy => Ok(None),
            WorktreeJobStartOutcome::InvalidDirectory if trust.is_some() => {
                Err(JobFailure::NotFound)
            }
            WorktreeJobStartOutcome::Unsupported
            | WorktreeJobStartOutcome::InvalidDirectory
            | WorktreeJobStartOutcome::SpawnThreadFailed
            | WorktreeJobStartOutcome::RejectedFencing => Err(JobFailure::Unavailable(
                WorktreeListUnavailableReason::WorkerUnavailable,
            )),
            WorktreeJobStartOutcome::Dead => Err(JobFailure::Unavailable(
                WorktreeListUnavailableReason::JobDead,
            )),
        }
    }

    fn execute(
        &mut self,
        trust: Option<bool>,
    ) -> Result<ObservedWorktrees, Box<WorkbenchWorktreeListOutcome>> {
        if self.expired() {
            return Err(Box::new(WorkbenchWorktreeListOutcome::Timeout {}));
        }
        let job_id = loop {
            if let Some(job_id) = self
                .start(trust)
                .map_err(|failure| Box::new(WorkbenchWorktreeListOutcome::from(failure)))?
            {
                break job_id;
            }
            self.pause();
        };
        loop {
            match self
                .poll(job_id)
                .map_err(|failure| Box::new(WorkbenchWorktreeListOutcome::from(failure)))?
            {
                PollStep::Pending => self.pause(),
                PollStep::Done(JobTerminal::Completed(observed)) => {
                    if observed.source_checkout.is_some() != trust.is_some() {
                        return Err(Box::new(WorkbenchWorktreeListOutcome::MalformedListing {}));
                    }
                    return Ok(observed);
                }
                PollStep::Done(JobTerminal::Failed(outcome)) => return Err(Box::new(outcome)),
            }
        }
    }

    fn pause(&self) {
        sleep(
            self.deadline
                .saturating_duration_since(Instant::now())
                .min(WORKTREE_LIST_POLL_INTERVAL),
        );
    }

    fn poll(&mut self, job_id: OperationId) -> Result<PollStep, JobFailure> {
        let operation_id = self.operation_id()?;
        let result = self.request(WorktreeJobAction::Poll { job_id }, operation_id)?;
        let WorkerOperationResult::WorktreeJob(result) = result else {
            return Err(JobFailure::Unavailable(
                WorktreeListUnavailableReason::WorkerUnavailable,
            ));
        };
        let WorktreeJobOutcome::Poll(outcome) = result.outcome else {
            return Err(JobFailure::Unavailable(
                WorktreeListUnavailableReason::WorkerUnavailable,
            ));
        };
        if outcome != WorktreeJobPollOutcome::Pending {
            self.job = None;
        }
        Ok(poll_step(outcome))
    }
}

impl Drop for JobDriver {
    fn drop(&mut self) {
        if let Some(job_id) = self.job.take()
            && let Ok(operation_id) = self.operation_id()
        {
            // Queue cleanup before releasing the discovery permit. The same
            // high-lane FIFO consumes late replies before this exact-job cancel.
            // On failed admission, physical TTL remains the Worker's owner.
            let _admitted = self.handle.admit_worktree_job(WorktreeJobRequest {
                operation_id,
                worker_id: self.worker_id,
                worker_generation: self.generation,
                action: WorktreeJobAction::Cancel { job_id },
            });
        }
    }
}

pub(super) fn discover_source(
    handle: WorkerRequestHandle,
    operation_base: u128,
    directory_path: Vec<u8>,
    trust: bool,
    deadline: Instant,
) -> Result<ObservedWorktrees, Box<WorkbenchWorktreeListOutcome>> {
    let worker_id = WorkerId::from_bytes(handle.worker_id().as_bytes()).map_err(|_| {
        Box::new(unavailable(
            WorktreeListUnavailableReason::WorkerUnavailable,
        ))
    })?;
    JobDriver {
        worker_id,
        generation: WorkerGeneration::new(handle.generation()),
        handle,
        operation_base,
        deadline,
        directory_path,
        requests: 0,
        job: None,
    }
    .execute(Some(trust))
}

impl LocalTerminalService {
    pub(crate) fn execute_worktree_list(
        &self,
        read: WorkbenchWorktreeListRead,
    ) -> Option<WorkbenchWorktreeListResult> {
        let deadline = Instant::now() + SERVER_WORKTREE_LIST_BUDGET;

        // Phase one: resolve the committed global Directory identity and the live
        // Worker carrier under workers -> coordinator lock order, then release every
        // lock before any blocking Worker exchange.
        let (target, handle) = match self.resolve_worktree_target(&read) {
            Ok(resolved) => resolved,
            Err(ResolutionFailure::Missing) => {
                return Some(worktree_result(
                    read.request_id,
                    WorkbenchWorktreeListOutcome::NotFound {},
                ));
            }
            Err(ResolutionFailure::Unavailable(reason)) => {
                return Some(worktree_result(
                    read.request_id,
                    WorkbenchWorktreeListOutcome::Unavailable { reason },
                ));
            }
        };

        let _permit = match self.worktree_discovery.foreground(deadline) {
            Ok(permit) => permit,
            Err(outcome) => return Some(worktree_result(read.request_id, outcome)),
        };

        let worker_id = match capability_worker_id(&target.directory) {
            Ok(worker_id) => worker_id,
            Err(()) => {
                return Some(worktree_result(
                    read.request_id,
                    unavailable(WorktreeListUnavailableReason::WorkerUnavailable),
                ));
            }
        };
        let generation = WorkerGeneration::new(target.generation);
        let mut driver = JobDriver {
            handle,
            worker_id,
            generation,
            operation_base: target.operation_base,
            deadline,
            directory_path: target.directory.path.as_str().as_bytes().to_vec(),
            requests: 0,
            job: None,
        };

        match driver.execute(None) {
            Ok(observed) => self.finish_worktree_list(read, &target, observed.listing, deadline),
            Err(outcome) => Some(worktree_result(read.request_id, *outcome)),
        }
    }

    // The elapsed check at final outcome handling precedes the committed-membership
    // recheck; revision is the value observed at this recheck, not a transaction.
    fn finish_worktree_list(
        &self,
        read: WorkbenchWorktreeListRead,
        target: &ReadyTarget,
        listing: Vec<u8>,
        deadline: Instant,
    ) -> Option<WorkbenchWorktreeListResult> {
        if Instant::now() >= deadline {
            return Some(worktree_result(
                read.request_id,
                WorkbenchWorktreeListOutcome::Timeout {},
            ));
        }
        let (site_id, revision) = match self.recheck_worktree_target(&read, target) {
            Ok(identity) => identity,
            Err(ResolutionFailure::Missing) => {
                return Some(worktree_result(
                    read.request_id,
                    WorkbenchWorktreeListOutcome::NotFound {},
                ));
            }
            Err(ResolutionFailure::Unavailable(reason)) => {
                return Some(worktree_result(
                    read.request_id,
                    WorkbenchWorktreeListOutcome::Unavailable { reason },
                ));
            }
        };
        let entries = match parse_worktree_listing(&listing) {
            Ok(entries) => entries,
            Err(()) => {
                return Some(worktree_result(
                    read.request_id,
                    WorkbenchWorktreeListOutcome::MalformedListing {},
                ));
            }
        };
        let result = WorkbenchWorktreeListResult {
            protocol_version: ProtocolVersion,
            request_id: read.request_id.clone(),
            outcome: WorkbenchWorktreeListOutcome::Found {
                site_id,
                workbench_revision: revision,
                directory_id: read.directory_id,
                path: target.directory.path.clone(),
                entries,
            },
        };
        Some(self.bound_worktree_result(read.request_id, result))
    }

    fn bound_worktree_result(
        &self,
        request_id: WorkbenchRequestId,
        result: WorkbenchWorktreeListResult,
    ) -> WorkbenchWorktreeListResult {
        if serialize_workbench_response(&WorkbenchResponse::WorktreeList(result.clone())).is_ok() {
            return result;
        }
        worktree_result(
            request_id,
            WorkbenchWorktreeListOutcome::TooLarge {
                origin: WorktreeListTooLargeOrigin::WorkbenchResult,
            },
        )
    }

    fn resolve_worktree_target(
        &self,
        read: &WorkbenchWorktreeListRead,
    ) -> Result<(ReadyTarget, WorkerRequestHandle), ResolutionFailure> {
        let workers = self.workers.lock().map_err(|_| {
            ResolutionFailure::Unavailable(WorktreeListUnavailableReason::ServerBusy)
        })?;
        let mut coordinator = self.coordinator.lock().map_err(|_| {
            ResolutionFailure::Unavailable(WorktreeListUnavailableReason::ServerBusy)
        })?;
        let resolution = coordinator.worktree_list_target(&read.directory_id);
        let WorktreeListResolution::Found(target) = resolution else {
            return Err(match resolution {
                WorktreeListResolution::Missing => ResolutionFailure::Missing,
                WorktreeListResolution::Unavailable => {
                    ResolutionFailure::Unavailable(WorktreeListUnavailableReason::ServerBusy)
                }
                WorktreeListResolution::Found(_) => unreachable!(),
            });
        };
        let Some(base) = coordinator.allocate_identity_block(WORKTREE_LIST_IDENTITY_RESERVE) else {
            return Err(ResolutionFailure::Unavailable(
                WorktreeListUnavailableReason::ServerBusy,
            ));
        };
        let Some(handle) = workers.request_handle() else {
            return Err(ResolutionFailure::Unavailable(
                WorktreeListUnavailableReason::WorkerUnavailable,
            ));
        };
        if capability_worker_id(&target.directory).map(|worker| worker.as_bytes())
            != Ok(handle.worker_id().as_bytes())
        {
            return Err(ResolutionFailure::Unavailable(
                WorktreeListUnavailableReason::WorkerUnavailable,
            ));
        }
        Ok((
            ReadyTarget {
                directory: target.directory,
                generation: handle.generation(),
                operation_base: base,
            },
            handle,
        ))
    }

    fn recheck_worktree_target(
        &self,
        read: &WorkbenchWorktreeListRead,
        target: &ReadyTarget,
    ) -> Result<(SiteId, WorkbenchRevision), ResolutionFailure> {
        let workers = self.workers.lock().map_err(|_| {
            ResolutionFailure::Unavailable(WorktreeListUnavailableReason::ServerBusy)
        })?;
        let coordinator = self.coordinator.lock().map_err(|_| {
            ResolutionFailure::Unavailable(WorktreeListUnavailableReason::ServerBusy)
        })?;
        let resolution = coordinator.worktree_list_target(&read.directory_id);
        let WorktreeListResolution::Found(current) = resolution else {
            return Err(match resolution {
                WorktreeListResolution::Missing => ResolutionFailure::Missing,
                WorktreeListResolution::Unavailable => {
                    ResolutionFailure::Unavailable(WorktreeListUnavailableReason::ServerBusy)
                }
                WorktreeListResolution::Found(_) => unreachable!(),
            });
        };
        let Some(handle) = workers.request_handle() else {
            return Err(ResolutionFailure::Unavailable(
                WorktreeListUnavailableReason::WorkerUnavailable,
            ));
        };
        if current.directory != target.directory
            || handle.generation() != target.generation
            || capability_worker_id(&current.directory).map(|worker| worker.as_bytes())
                != Ok(handle.worker_id().as_bytes())
        {
            return Err(ResolutionFailure::Unavailable(
                WorktreeListUnavailableReason::WorkerUnavailable,
            ));
        }
        Ok((current.site_id, current.revision))
    }
}

// Carrier-level request failures classified by origin: a full queue is the
// Server's own backpressure, post-delivery silence is read-only IPC response
// loss (transport-loss, never a guessed job state), and every other carrier
// failure is the Worker being unreachable.
fn job_request_failure(error: WorkerRequestError) -> JobFailure {
    match error {
        WorkerRequestError::Backpressure => {
            JobFailure::Unavailable(WorktreeListUnavailableReason::ServerBusy)
        }
        WorkerRequestError::PostDeliveryOutcomeUnknown(_) => JobFailure::TransportLoss,
        _ => JobFailure::Unavailable(WorktreeListUnavailableReason::WorkerUnavailable),
    }
}

fn poll_step(outcome: WorktreeJobPollOutcome) -> PollStep {
    match outcome {
        WorktreeJobPollOutcome::Pending => PollStep::Pending,
        WorktreeJobPollOutcome::StatusCompleted { .. } => PollStep::Done(JobTerminal::Failed(
            WorkbenchWorktreeListOutcome::MalformedListing {},
        )),
        WorktreeJobPollOutcome::SourceCompleted {
            common_dir,
            source_checkout,
            listing,
            ..
        } => PollStep::Done(JobTerminal::Completed(ObservedWorktrees {
            common_directory: common_dir,
            source_checkout: Some(source_checkout),
            listing,
        })),
        WorktreeJobPollOutcome::Completed {
            common_dir,
            listing,
            ..
        } => PollStep::Done(JobTerminal::Completed(ObservedWorktrees {
            common_directory: common_dir,
            source_checkout: None,
            listing,
        })),
        WorktreeJobPollOutcome::GitFailed { stage, status, .. } => PollStep::Done(
            JobTerminal::Failed(WorkbenchWorktreeListOutcome::GitFailed {
                stage: worktree_list_stage(stage),
                status,
            }),
        ),
        WorktreeJobPollOutcome::SpawnUnavailable { stage } => PollStep::Done(JobTerminal::Failed(
            WorkbenchWorktreeListOutcome::SpawnUnavailable {
                stage: worktree_list_stage(stage),
            },
        )),
        WorktreeJobPollOutcome::SpawnFailed { stage } => PollStep::Done(JobTerminal::Failed(
            WorkbenchWorktreeListOutcome::SpawnFailed {
                stage: worktree_list_stage(stage),
            },
        )),
        WorktreeJobPollOutcome::TooLarge { stage } => PollStep::Done(JobTerminal::Failed(
            WorkbenchWorktreeListOutcome::TooLarge {
                origin: match stage {
                    Some(WorktreeJobStage::Probe) => WorktreeListTooLargeOrigin::GitProbe,
                    Some(WorktreeJobStage::List) => WorktreeListTooLargeOrigin::GitList,
                    None => WorktreeListTooLargeOrigin::WorkerResult,
                },
            },
        )),
        WorktreeJobPollOutcome::Timeout => PollStep::Done(JobTerminal::Failed(
            WorkbenchWorktreeListOutcome::Timeout {},
        )),
        WorktreeJobPollOutcome::OwnerIo { stage } => {
            PollStep::Done(JobTerminal::Failed(WorkbenchWorktreeListOutcome::OwnerIo {
                stage: stage.map(worktree_list_stage),
            }))
        }
        // The backend's physical-unknown is a truthful owner state; the
        // public read surface reports it as worker-unavailable.
        WorktreeJobPollOutcome::PhysicalUnknown => PollStep::Done(JobTerminal::Failed(
            unavailable(WorktreeListUnavailableReason::WorkerUnavailable),
        )),
        WorktreeJobPollOutcome::Unknown => PollStep::Done(JobTerminal::Failed(unavailable(
            WorktreeListUnavailableReason::JobUnknown,
        ))),
        WorktreeJobPollOutcome::Cancelled | WorktreeJobPollOutcome::RejectedFencing => {
            PollStep::Done(JobTerminal::Failed(unavailable(
                WorktreeListUnavailableReason::WorkerUnavailable,
            )))
        }
    }
}

fn worktree_list_stage(stage: WorktreeJobStage) -> WorktreeListStage {
    match stage {
        WorktreeJobStage::Probe => WorktreeListStage::Probe,
        WorktreeJobStage::List => WorktreeListStage::List,
    }
}

fn worktree_result(
    request_id: WorkbenchRequestId,
    outcome: WorkbenchWorktreeListOutcome,
) -> WorkbenchWorktreeListResult {
    WorkbenchWorktreeListResult {
        protocol_version: ProtocolVersion,
        request_id,
        outcome,
    }
}

fn unavailable(reason: WorktreeListUnavailableReason) -> WorkbenchWorktreeListOutcome {
    WorkbenchWorktreeListOutcome::Unavailable { reason }
}

fn capability_worker_id(directory: &Directory) -> Result<WorkerId, ()> {
    let worker = u128::from_str_radix(directory.worker_id.as_str(), 16).map_err(|_| ())?;
    WorkerId::from_bytes(worker.to_be_bytes()).map_err(|_| ())
}

#[derive(Debug, Default)]
struct RecordBuilder {
    path: Option<Vec<u8>>,
    head: Option<Vec<u8>>,
    branch: Option<Vec<u8>>,
    detached: bool,
    bare: bool,
    locked: bool,
    prunable: bool,
}

// `git worktree list --porcelain -z`: NUL-terminated attribute fields, one bare
// NUL record separator after every record, stream ends after the last separator.
pub(super) fn parse_worktree_listing(bytes: &[u8]) -> Result<Vec<WorktreeEntry>, ()> {
    let stripped = bytes.strip_suffix(&[0]).ok_or(())?;
    let mut entries = Vec::new();
    let mut record: Vec<&[u8]> = Vec::new();
    for field in stripped.split(|byte| *byte == 0) {
        if field.is_empty() {
            if record.is_empty() {
                return Err(());
            }
            let entry = finish_record(&record)?;
            entry.validate().map_err(|_| ())?;
            entries.push(entry);
            record.clear();
        } else {
            record.push(field);
        }
    }
    if !record.is_empty() || entries.is_empty() {
        return Err(());
    }
    Ok(entries)
}

fn finish_record(fields: &[&[u8]]) -> Result<WorktreeEntry, ()> {
    let mut builder = RecordBuilder::default();
    let mut seen_head = false;
    let mut seen_branch = false;
    let mut seen_detached = false;
    let mut seen_bare = false;
    let mut seen_locked = false;
    let mut seen_prunable = false;
    for (index, field) in fields.iter().enumerate() {
        if index == 0 {
            let path = field.strip_prefix(b"worktree ").ok_or(())?;
            builder.path = Some(path.to_vec());
            continue;
        }
        let (key, rest) = match field.iter().position(|byte| *byte == b' ') {
            Some(space) => (&field[..space], Some(&field[space + 1..])),
            None => (*field, None),
        };
        match key {
            b"HEAD" => {
                if seen_head || rest.is_none() {
                    return Err(());
                }
                seen_head = true;
                builder.head = rest.map(<[u8]>::to_vec);
            }
            b"branch" => {
                if seen_branch || rest.is_none() {
                    return Err(());
                }
                seen_branch = true;
                builder.branch = rest.map(<[u8]>::to_vec);
            }
            b"detached" => {
                if seen_detached || rest.is_some() {
                    return Err(());
                }
                seen_detached = true;
                builder.detached = true;
            }
            b"bare" => {
                if seen_bare || rest.is_some() {
                    return Err(());
                }
                seen_bare = true;
                builder.bare = true;
            }
            b"locked" => {
                if seen_locked {
                    return Err(());
                }
                seen_locked = true;
                builder.locked = true;
            }
            b"prunable" => {
                if seen_prunable {
                    return Err(());
                }
                seen_prunable = true;
                builder.prunable = true;
            }
            // Unknown attribute keys are ignored byte-wise; no UTF-8 decoding of
            // lock or prunable reason text is attempted.
            _ => {}
        }
    }
    let path = builder.path.ok_or(())?;
    if builder.bare {
        if builder.head.is_some() || builder.branch.is_some() || builder.detached {
            return Err(());
        }
        return Ok(WorktreeEntry {
            path: entry_text(&path),
            head: None,
            branch: None,
            bare: true,
            detached: false,
            locked: builder.locked,
            prunable: builder.prunable,
        });
    }
    let head = builder.head.ok_or(())?;
    if !valid_head(&head) {
        return Err(());
    }
    let head = String::from_utf8(head).map_err(|_| ())?;
    match (builder.branch, builder.detached) {
        (Some(branch), false) => Ok(WorktreeEntry {
            path: entry_text(&path),
            head: Some(head),
            branch: Some(entry_text(&branch)),
            bare: false,
            detached: false,
            locked: builder.locked,
            prunable: builder.prunable,
        }),
        (None, true) => Ok(WorktreeEntry {
            path: entry_text(&path),
            head: Some(head),
            branch: None,
            bare: false,
            detached: true,
            locked: builder.locked,
            prunable: builder.prunable,
        }),
        _ => Err(()),
    }
}

// HEAD is kept verbatim: a 40- or 64-digit lowercase hex value, including the
// all-zero value Git emits for an unborn branch.
fn valid_head(head: &[u8]) -> bool {
    (head.len() == 40 || head.len() == 64)
        && head
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn entry_text(bytes: &[u8]) -> WorktreeEntryText {
    match std::str::from_utf8(bytes) {
        Ok(text) => WorktreeEntryText::Utf8(text.to_owned()),
        Err(_) => WorktreeEntryText::Bytes(bytes.to_vec()),
    }
}

#[cfg(test)]
mod tests;
