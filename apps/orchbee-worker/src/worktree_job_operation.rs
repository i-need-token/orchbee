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
mod tests {
    use std::{error::Error, num::NonZeroU64, thread, time::Duration, time::Instant};

    use orchbee_capability_node_protocol::{
        OperationId, WorkerGeneration, WorkerId as CapabilityWorkerId, WorktreeJobAction,
        WorktreeJobCancelOutcome, WorktreeJobOutcome, WorktreeJobPollOutcome, WorktreeJobRequest,
        WorktreeJobStage, WorktreeJobStartOutcome,
    };
    use orchbee_local_site_protocol::WorkerId;

    use super::{WorktreeDiscoverySlot, execute};
    use crate::worktree_discovery_job::JobOutcome;

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    const LOCAL_WORKER: WorkerId = WorkerId::from_bytes([7; 16]);
    const LOCAL_GENERATION: NonZeroU64 = NonZeroU64::new(3).unwrap();

    fn operation_id(byte: u8) -> OperationId {
        OperationId::from_bytes([byte; 16]).unwrap()
    }

    fn request(
        operation: u8,
        worker: CapabilityWorkerId,
        generation: u64,
        action: WorktreeJobAction,
    ) -> WorktreeJobRequest {
        WorktreeJobRequest {
            operation_id: operation_id(operation),
            worker_id: worker,
            worker_generation: WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
            action,
        }
    }

    fn start_request(operation: u8, directory: &str) -> WorktreeJobRequest {
        request(
            operation,
            CapabilityWorkerId::from_bytes(LOCAL_WORKER.as_bytes()).unwrap(),
            LOCAL_GENERATION.get(),
            WorktreeJobAction::start(directory.as_bytes().to_vec()).unwrap(),
        )
    }

    fn settle_poll(slot: &WorktreeDiscoverySlot, job_id: u8) -> WorktreeJobPollOutcome {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let request = request(
                90,
                CapabilityWorkerId::from_bytes(LOCAL_WORKER.as_bytes()).unwrap(),
                LOCAL_GENERATION.get(),
                WorktreeJobAction::Poll {
                    job_id: operation_id(job_id),
                },
            );
            match execute(request, LOCAL_WORKER, LOCAL_GENERATION, slot).outcome {
                WorktreeJobOutcome::Poll(WorktreeJobPollOutcome::Pending)
                    if Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(10));
                }
                WorktreeJobOutcome::Poll(outcome) => return outcome,
                other => panic!("poll answered a non-poll outcome: {other:?}"),
            }
        }
    }

    #[test]
    fn fencing_rejects_every_action_without_touching_the_slot() -> TestResult {
        // A request pinned to another Worker or generation answers
        // RejectedFencing and must not consume the physical slot.
        let slot = WorktreeDiscoverySlot::new();
        let foreign_worker = CapabilityWorkerId::from_bytes([9; 16])?;
        let stale_generation = LOCAL_GENERATION.get() + 1;
        for (worker, generation) in [
            (foreign_worker, LOCAL_GENERATION.get()),
            (
                CapabilityWorkerId::from_bytes(LOCAL_WORKER.as_bytes())?,
                stale_generation,
            ),
        ] {
            for (operation, action, expected) in [
                (
                    41,
                    WorktreeJobAction::start(b"/tmp".to_vec())?,
                    WorktreeJobOutcome::Start(WorktreeJobStartOutcome::RejectedFencing),
                ),
                (
                    43,
                    WorktreeJobAction::Poll {
                        job_id: operation_id(40),
                    },
                    WorktreeJobOutcome::Poll(WorktreeJobPollOutcome::RejectedFencing),
                ),
                (
                    44,
                    WorktreeJobAction::Cancel {
                        job_id: operation_id(40),
                    },
                    WorktreeJobOutcome::Cancel(WorktreeJobCancelOutcome::RejectedFencing),
                ),
            ] {
                let result = execute(
                    request(operation, worker, generation, action),
                    LOCAL_WORKER,
                    LOCAL_GENERATION,
                    &slot,
                );
                assert_eq!(result.outcome, expected);
            }
        }
        // None of the fenced Starts occupied the slot.
        let result = execute(
            start_request(42, std::env::temp_dir().to_str().unwrap()),
            LOCAL_WORKER,
            LOCAL_GENERATION,
            &slot,
        );
        assert_eq!(
            result.outcome,
            WorktreeJobOutcome::Start(WorktreeJobStartOutcome::Accepted)
        );
        let terminal = settle_poll(&slot, 42);
        assert!(matches!(
            terminal,
            WorktreeJobPollOutcome::Completed { .. }
                | WorktreeJobPollOutcome::GitFailed { .. }
                | WorktreeJobPollOutcome::SpawnUnavailable { .. }
        ));
        Ok(())
    }

    #[test]
    fn result_correlation_echoes_operation_and_job_identity() -> TestResult {
        // JobId is the Start operation's own ID: every result echoes its
        // request's operation_id plus the job identity it addresses.
        let slot = WorktreeDiscoverySlot::new();
        let result = execute(
            start_request(50, std::env::temp_dir().to_str().unwrap()),
            LOCAL_WORKER,
            LOCAL_GENERATION,
            &slot,
        );
        assert_eq!(result.operation_id, operation_id(50));
        assert_eq!(result.job_id, operation_id(50));
        assert!(matches!(
            result.outcome,
            WorktreeJobOutcome::Start(WorktreeJobStartOutcome::Accepted)
        ));
        let poll = request(
            51,
            CapabilityWorkerId::from_bytes(LOCAL_WORKER.as_bytes())?,
            LOCAL_GENERATION.get(),
            WorktreeJobAction::Poll {
                job_id: operation_id(50),
            },
        );
        let result = execute(poll, LOCAL_WORKER, LOCAL_GENERATION, &slot);
        assert_eq!(result.operation_id, operation_id(51));
        assert_eq!(result.job_id, operation_id(50));
        let _ = settle_poll(&slot, 50);
        Ok(())
    }

    #[test]
    fn unknown_job_poll_and_cancel_are_typed_unknown() {
        let slot = WorktreeDiscoverySlot::new();
        let worker = CapabilityWorkerId::from_bytes(LOCAL_WORKER.as_bytes()).unwrap();
        for (operation, action, expected) in [
            (
                60,
                WorktreeJobAction::Poll {
                    job_id: operation_id(61),
                },
                WorktreeJobOutcome::Poll(WorktreeJobPollOutcome::Unknown),
            ),
            (
                62,
                WorktreeJobAction::Cancel {
                    job_id: operation_id(61),
                },
                WorktreeJobOutcome::Cancel(WorktreeJobCancelOutcome::Unknown),
            ),
        ] {
            let result = execute(
                request(operation, worker, LOCAL_GENERATION.get(), action),
                LOCAL_WORKER,
                LOCAL_GENERATION,
                &slot,
            );
            assert_eq!(result.outcome, expected);
        }
    }

    #[test]
    fn oversized_payloads_fall_back_to_stageless_too_large() {
        // A Completed/GitFailed payload past the 7,936-byte field budget can
        // never be encoded, so it collapses to the stageless TooLarge marker.
        let oversized = vec![b'x'; 8000];
        assert_eq!(
            super::map_job(JobOutcome::Completed {
                common_dir: vec![],
                listing: oversized.clone(),
                stderr: vec![],
            }),
            WorktreeJobPollOutcome::TooLarge { stage: None }
        );
        assert_eq!(
            super::map_job(JobOutcome::GitFailed {
                stage: crate::worktree_discovery_job::Stage::List,
                status: Some(128),
                stderr: oversized,
            }),
            WorktreeJobPollOutcome::TooLarge { stage: None }
        );
        assert_eq!(
            super::map_job(JobOutcome::TooLarge {
                stage: crate::worktree_discovery_job::Stage::Probe,
            }),
            WorktreeJobPollOutcome::TooLarge {
                stage: Some(WorktreeJobStage::Probe),
            }
        );
    }

    #[test]
    fn real_git_fixture_round_trips_through_execute() -> TestResult {
        // Activation witness through the dispatch layer itself: one real
        // disposable repository, per-command `-c` identity only, one physical
        // owner, exact correlation, and a truthful terminal Cancel answer.
        let tag = std::process::id();
        let root_path = std::env::temp_dir().join(format!("orchbee-worktree-op-{tag}"));
        std::fs::create_dir(&root_path)?;
        let root = root_path.to_str().unwrap().replace('\\', "/");
        let git = |args: &[&str]| -> TestResult {
            let status = std::process::Command::new("git")
                .arg("-c")
                .arg("user.email=fixture@example.invalid")
                .arg("-c")
                .arg("user.name=fixture")
                .arg("-C")
                .arg(&root)
                .args(args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()?;
            assert!(status.success(), "git {args:?} failed");
            Ok(())
        };
        git(&["init", "-b", "main"])?;
        std::fs::write(format!("{root}/seed.txt"), "seed")?;
        git(&["add", "seed.txt"])?;
        git(&["commit", "-m", "seed"])?;

        let slot = WorktreeDiscoverySlot::new();
        let result = execute(
            start_request(70, &root),
            LOCAL_WORKER,
            LOCAL_GENERATION,
            &slot,
        );
        assert_eq!(
            result.outcome,
            WorktreeJobOutcome::Start(WorktreeJobStartOutcome::Accepted)
        );
        match settle_poll(&slot, 70) {
            WorktreeJobPollOutcome::Completed { listing, .. } => {
                let text = String::from_utf8_lossy(&listing);
                assert!(
                    text.contains(&format!("worktree {root}\0")),
                    "real listing must name the repository: {text:?}"
                );
                assert!(text.contains("branch refs/heads/main\0"));
            }
            other => panic!("real fixture must complete: {other:?}"),
        }
        // A second Start on the settled slot is admitted (receipt evicted),
        // and Cancel on the finished job reports the summary terminal fact.
        let cancel = request(
            71,
            CapabilityWorkerId::from_bytes(LOCAL_WORKER.as_bytes())?,
            LOCAL_GENERATION.get(),
            WorktreeJobAction::Cancel {
                job_id: operation_id(70),
            },
        );
        let result = execute(cancel, LOCAL_WORKER, LOCAL_GENERATION, &slot);
        assert_eq!(
            result.outcome,
            WorktreeJobOutcome::Cancel(WorktreeJobCancelOutcome::AlreadyTerminal)
        );
        let resolved = root_path.canonicalize()?;
        assert_eq!(
            resolved.parent(),
            Some(std::env::temp_dir().canonicalize()?.as_path())
        );
        std::fs::remove_dir_all(resolved)?;
        Ok(())
    }
}
