use super::{Stage, UpdateJob, Wake};
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
    assert_eq!(job.answer(Read, false), UpdateCheckOutcome::Idle {});
    assert_eq!(job.answer(Start, false), UpdateCheckOutcome::Checking {});
    assert_eq!(job.answer(Start, false), UpdateCheckOutcome::Checking {});
    let later = Instant::now() + Duration::from_secs(3600);
    assert_eq!(job.wait_until(later), Some(Wake::Requested));
    job.begin();
    assert_eq!(job.answer(Start, false), UpdateCheckOutcome::Checking {});
    job.finish(UpdateCheckFinish::UpToDate);
    assert_eq!(
        job.answer(Read, false),
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
    assert_eq!(job.answer(Start, false), UpdateCheckOutcome::Checking {});
    job.end();
    assert!(job.stopped());
    assert_eq!(job.answer(Read, false), UpdateCheckOutcome::Unavailable {});
    assert_eq!(job.answer(Start, false), UpdateCheckOutcome::Unavailable {});
    assert_eq!(job.wait_until(Instant::now()), None);
}

/// A Client that does not ask for phases reads a running run as Checking and every failure as
/// `failed`, as a Client of `update-check` alone always did; one that asks reads where the run
/// stands and the phase its failure ended in. A report while no run runs changes nothing.
#[test]
fn only_a_client_that_asks_for_phases_reads_them() {
    let job = UpdateJob::default();
    job.advance(Stage::Installing {
        version: "0.1.0-beta.26".into(),
    });
    assert_eq!(job.answer(Read, true), UpdateCheckOutcome::Idle {});
    assert_eq!(job.answer(Start, true), UpdateCheckOutcome::Checking {});
    job.begin();
    assert_eq!(job.answer(Read, true), UpdateCheckOutcome::Checking {});
    job.advance(Stage::Downloading {
        version: "0.1.0-beta.26".into(),
        received_bytes: 1,
        total_bytes: 2,
    });
    assert_eq!(job.answer(Read, false), UpdateCheckOutcome::Checking {});
    assert_eq!(
        job.answer(Read, true),
        UpdateCheckOutcome::Downloading {
            version: "0.1.0-beta.26".into(),
            received_bytes: 1,
            total_bytes: 2,
        }
    );
    job.finish(UpdateCheckFinish::DownloadFailed);
    assert_eq!(
        job.answer(Read, false),
        UpdateCheckOutcome::Finished {
            result: UpdateCheckFinish::Failed
        }
    );
    assert_eq!(
        job.answer(Read, true),
        UpdateCheckOutcome::Finished {
            result: UpdateCheckFinish::DownloadFailed
        }
    );
}
