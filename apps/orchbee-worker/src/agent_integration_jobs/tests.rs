use super::*;
use orchbee_capability_node_protocol::{WorkerGeneration, WorkerId as NodeWorkerId};

const REPORT: &[u8] = br#"{"kind":"home-unavailable"}"#;

fn id(value: u8) -> OperationId {
    OperationId::from_bytes([value; 16]).unwrap()
}

fn document() -> Vec<u8> {
    br#"{"operation":{"kind":"list"}}"#.to_vec()
}

/// Settles only after cooperative cancellation; it never touches provider configuration.
fn until_cancelled(_: IntegrationRequest, cancel: Arc<CancelFlag>) -> Report {
    while !cancel.requested() {
        thread::sleep(Duration::from_millis(1));
    }
    Some(REPORT.to_vec())
}

fn oversized(_: IntegrationRequest, _: Arc<CancelFlag>) -> Report {
    None
}

fn request(worker: u8, generation: u64, action: Action) -> AgentIntegrationJobRequest {
    AgentIntegrationJobRequest {
        operation_id: id(1),
        worker_id: NodeWorkerId::from_bytes([worker; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
        action,
    }
}

fn execute(jobs: &AgentIntegrationJobs, action: Action) -> Outcome {
    jobs.execute(
        request(2, 3, action),
        WorkerId::from_bytes([2; 16]),
        NonZeroU64::new(3).unwrap(),
    )
    .outcome
}

fn settled(jobs: &AgentIntegrationJobs, job_id: OperationId) -> Poll {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let outcome = jobs.poll(job_id);
        if outcome != Poll::Pending || Instant::now() >= deadline {
            return outcome;
        }
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn stale_worker_and_generation_reject_every_action_before_owner_access() {
    let jobs = AgentIntegrationJobs::with_executor(until_cancelled);
    for (worker, generation) in [(9, 3), (2, 4)] {
        for (action, expected) in [
            (
                Action::Start {
                    request: document(),
                },
                Outcome::Start(Start::RejectedFencing),
            ),
            (
                Action::Poll { job_id: id(1) },
                Outcome::Poll(Poll::RejectedFencing),
            ),
            (
                Action::Cancel { job_id: id(1) },
                Outcome::Cancel(Cancel::RejectedFencing),
            ),
        ] {
            let result = jobs.execute(
                request(worker, generation, action),
                WorkerId::from_bytes([2; 16]),
                NonZeroU64::new(3).unwrap(),
            );
            assert_eq!(result.outcome, expected);
            assert_eq!(result.job_id, id(1));
        }
    }
    assert_eq!(jobs.poll(id(1)), Poll::Unknown);
}

#[test]
fn one_running_job_owns_the_writer_until_cancellation_settles_its_report() {
    let jobs = AgentIntegrationJobs::with_executor(until_cancelled);
    assert_eq!(
        execute(
            &jobs,
            Action::Start {
                request: b"{}".to_vec()
            }
        ),
        Outcome::Start(Start::InvalidRequest)
    );
    assert_eq!(jobs.start(id(1), document()), Start::Accepted);
    assert_eq!(jobs.start(id(1), document()), Start::Accepted);
    assert_eq!(jobs.start(id(2), document()), Start::Busy);
    assert_eq!(jobs.poll(id(1)), Poll::Pending);
    assert_eq!(jobs.cancel(id(1)), Cancel::Registered);
    assert_eq!(
        settled(&jobs, id(1)),
        Poll::Completed {
            report: REPORT.to_vec()
        }
    );
    assert_eq!(jobs.cancel(id(1)), Cancel::AlreadyTerminal);
    assert_eq!(jobs.cancel(id(9)), Cancel::Unknown);
}

#[test]
fn an_unreportable_job_is_abandoned_and_stop_closes_admission() {
    let jobs = AgentIntegrationJobs::with_executor(oversized);
    assert_eq!(jobs.start(id(1), document()), Start::Accepted);
    assert_eq!(settled(&jobs, id(1)), Poll::Abandoned);
    jobs.stop_before(Instant::now() + Duration::from_secs(5))
        .unwrap();
    assert_eq!(jobs.start(id(2), document()), Start::Dead);
}
