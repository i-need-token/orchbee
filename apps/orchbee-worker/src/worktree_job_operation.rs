use std::{num::NonZeroU64, path::Path};

use orchbee_capability_node_protocol::{
    WorktreeJobAction, WorktreeJobCancelOutcome, WorktreeJobOutcome, WorktreeJobPollOutcome,
    WorktreeJobRequest, WorktreeJobResult, WorktreeJobStage, WorktreeJobStartOutcome,
};
use orchbee_local_site_protocol::WorkerId;

use crate::worktree_discovery_job::{
    CancelOutcome, JobId, JobOutcome, PollOutcome, Stage, StartOutcome, WorktreeDiscoverySlot,
};

// Fencing is a typed outcome, never a connection-fatal error: a wrong worker
// or stale generation only rejects this one request.
pub(crate) fn execute(
    request: WorktreeJobRequest,
    worker_id: WorkerId,
    generation: NonZeroU64,
    slot: &WorktreeDiscoverySlot,
) -> WorktreeJobResult {
    let fenced = request.worker_id.as_bytes() != worker_id.as_bytes()
        || request.worker_generation.get() != generation.get();
    let (job_id, outcome) = match request.action {
        WorktreeJobAction::StartSource {
            directory,
            trust_repository,
        } => {
            let job_id = JobId(request.operation_id.as_bytes());
            let outcome = if fenced {
                WorktreeJobStartOutcome::RejectedFencing
            } else {
                #[cfg(windows)]
                {
                    match directory_path(&directory) {
                        Some(path) => map_start(slot.start_source(job_id, path, trust_repository)),
                        None => WorktreeJobStartOutcome::InvalidDirectory,
                    }
                }
                #[cfg(not(windows))]
                {
                    let _ = (directory, trust_repository);
                    WorktreeJobStartOutcome::Unsupported
                }
            };
            (job_id, WorktreeJobOutcome::Start(outcome))
        }
        WorktreeJobAction::StartStatus { directory } => {
            let job_id = JobId(request.operation_id.as_bytes());
            let outcome = if fenced {
                WorktreeJobStartOutcome::RejectedFencing
            } else {
                #[cfg(windows)]
                {
                    match directory_path(&directory) {
                        Some(path) => map_start(slot.start_status(job_id, path)),
                        None => WorktreeJobStartOutcome::InvalidDirectory,
                    }
                }
                #[cfg(not(windows))]
                {
                    let _ = directory;
                    WorktreeJobStartOutcome::Unsupported
                }
            };
            (job_id, WorktreeJobOutcome::Start(outcome))
        }
        WorktreeJobAction::Start { directory } => {
            let job_id = JobId(request.operation_id.as_bytes());
            let outcome = if fenced {
                WorktreeJobOutcome::Start(WorktreeJobStartOutcome::RejectedFencing)
            } else {
                WorktreeJobOutcome::Start(match directory_path(&directory) {
                    Some(path) => map_start(slot.start(job_id, path)),
                    None => WorktreeJobStartOutcome::InvalidDirectory,
                })
            };
            (job_id, outcome)
        }
        WorktreeJobAction::Poll { job_id } => {
            let job_id = JobId(job_id.as_bytes());
            let outcome = if fenced {
                WorktreeJobOutcome::Poll(WorktreeJobPollOutcome::RejectedFencing)
            } else {
                WorktreeJobOutcome::Poll(map_poll(slot.poll(job_id)))
            };
            (job_id, outcome)
        }
        WorktreeJobAction::Cancel { job_id } => {
            let job_id = JobId(job_id.as_bytes());
            let outcome = if fenced {
                WorktreeJobOutcome::Cancel(WorktreeJobCancelOutcome::RejectedFencing)
            } else {
                WorktreeJobOutcome::Cancel(map_cancel(slot.cancel(job_id)))
            };
            (job_id, outcome)
        }
    };
    WorktreeJobResult {
        operation_id: request.operation_id,
        job_id: orchbee_capability_node_protocol::OperationId::from_bytes(job_id.0)
            .expect("job identities are nonzero"),
        outcome,
    }
}

fn map_start(outcome: StartOutcome) -> WorktreeJobStartOutcome {
    match outcome {
        StartOutcome::Accepted => WorktreeJobStartOutcome::Accepted,
        StartOutcome::Busy => WorktreeJobStartOutcome::Busy,
        StartOutcome::InvalidDirectory => WorktreeJobStartOutcome::InvalidDirectory,
        StartOutcome::SpawnFailed => WorktreeJobStartOutcome::SpawnThreadFailed,
        StartOutcome::Dead => WorktreeJobStartOutcome::Dead,
    }
}

fn map_poll(outcome: PollOutcome) -> WorktreeJobPollOutcome {
    match outcome {
        PollOutcome::Pending => WorktreeJobPollOutcome::Pending,
        PollOutcome::PhysicalUnknown => WorktreeJobPollOutcome::PhysicalUnknown,
        PollOutcome::Unknown => WorktreeJobPollOutcome::Unknown,
        PollOutcome::Completed(job) => map_job(job),
    }
}

fn map_cancel(outcome: CancelOutcome) -> WorktreeJobCancelOutcome {
    match outcome {
        CancelOutcome::Registered => WorktreeJobCancelOutcome::Registered,
        // Summary-only: the retained terminal outcome stays re-readable via Poll.
        CancelOutcome::AlreadyTerminal(_) => WorktreeJobCancelOutcome::AlreadyTerminal,
        CancelOutcome::PhysicalUnknown => WorktreeJobCancelOutcome::PhysicalUnknown,
        CancelOutcome::Unknown => WorktreeJobCancelOutcome::Unknown,
    }
}

fn map_stage(stage: Stage) -> WorktreeJobStage {
    match stage {
        Stage::Probe => WorktreeJobStage::Probe,
        Stage::List => WorktreeJobStage::List,
    }
}

fn map_job(outcome: JobOutcome) -> WorktreeJobPollOutcome {
    match outcome {
        JobOutcome::StatusCompleted { status, stderr } => {
            WorktreeJobPollOutcome::status_completed(status, stderr)
                .unwrap_or(WorktreeJobPollOutcome::TooLarge { stage: None })
        }
        JobOutcome::SourceCompleted {
            common_dir,
            source_checkout,
            listing,
            stderr,
        } => WorktreeJobPollOutcome::source_completed(common_dir, source_checkout, listing, stderr)
            .unwrap_or(WorktreeJobPollOutcome::TooLarge { stage: None }),
        JobOutcome::Completed {
            common_dir,
            listing,
            stderr,
        } => WorktreeJobPollOutcome::completed(common_dir, listing, stderr)
            .unwrap_or(WorktreeJobPollOutcome::TooLarge { stage: None }),
        JobOutcome::GitFailed {
            stage,
            status,
            stderr,
        } => WorktreeJobPollOutcome::git_failed(map_stage(stage), status, stderr)
            .unwrap_or(WorktreeJobPollOutcome::TooLarge { stage: None }),
        JobOutcome::SpawnUnavailable { stage } => WorktreeJobPollOutcome::SpawnUnavailable {
            stage: map_stage(stage),
        },
        JobOutcome::SpawnFailed { stage } => WorktreeJobPollOutcome::SpawnFailed {
            stage: map_stage(stage),
        },
        JobOutcome::TooLarge { stage } => WorktreeJobPollOutcome::TooLarge {
            stage: Some(map_stage(stage)),
        },
        JobOutcome::Timeout => WorktreeJobPollOutcome::Timeout,
        JobOutcome::Cancelled => WorktreeJobPollOutcome::Cancelled,
        JobOutcome::OwnerIo { stage } => WorktreeJobPollOutcome::OwnerIo {
            stage: stage.map(map_stage),
        },
    }
}

#[cfg(unix)]
fn directory_path(bytes: &[u8]) -> Option<&Path> {
    use std::os::unix::ffi::OsStrExt;
    Some(Path::new(std::ffi::OsStr::from_bytes(bytes)))
}
#[cfg(windows)]
fn directory_path(bytes: &[u8]) -> Option<&Path> {
    std::str::from_utf8(bytes).ok().map(Path::new)
}

#[cfg(test)]
mod tests;
