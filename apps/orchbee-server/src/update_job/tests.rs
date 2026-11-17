use super::{UpdateJob, Wake};
use orchbee_workbench_protocol::{
    UpdateCheckAction::{Read, Start},
    UpdateCheckFinish, UpdateCheckOutcome,
};
use std::time::{Duration, Instant};

/// A request made before the schedule starts waits for it, a run serves every request queued
/// before it, and a request during a run adds no second run.
#[test]
fn requests_join_the_queued_or_running_run_and_never_add_another() {
    let job = UpdateJob::default();
    assert_eq!(job.answer(Read), UpdateCheckOutcome::Idle {});
    assert_eq!(job.answer(Start), UpdateCheckOutcome::Checking {});
    assert_eq!(job.answer(Start), UpdateCheckOutcome::Checking {});
    let later = Instant::now() + Duration::from_secs(3600);
    assert_eq!(job.wait_until(later), Some(Wake::Requested));
    job.begin();
    assert_eq!(job.answer(Start), UpdateCheckOutcome::Checking {});
    job.finish(UpdateCheckFinish::UpToDate);
    assert_eq!(
        job.answer(Read),
        UpdateCheckOutcome::Finished {
            result: UpdateCheckFinish::UpToDate
        }
    );
    // The schedule's next wait sees no request: the run served every one.
    assert_eq!(job.wait_until(Instant::now()), Some(Wake::Due));
}

#[test]
fn an_ended_job_answers_unavailable_and_drops_what_was_queued() {
    let job = UpdateJob::default();
    assert_eq!(job.answer(Start), UpdateCheckOutcome::Checking {});
    job.end();
    assert!(job.stopped());
    assert_eq!(job.answer(Read), UpdateCheckOutcome::Unavailable {});
    assert_eq!(job.answer(Start), UpdateCheckOutcome::Unavailable {});
    assert_eq!(job.wait_until(Instant::now()), None);
}
