//! The deadline contract under real threads: one answer per job, a timed-out helper abandoned
//! and its late result dropped, refusal while too many helpers are stuck, and recovery once they
//! return.

use std::sync::mpsc::{self, Receiver, Sender};

use super::*;

/// Long enough for a helper to take a job; short enough to keep the suite fast.
const SHORT: Duration = Duration::from_millis(200);
const WAIT: Duration = Duration::from_secs(10);

fn pool() -> Blocking {
    Blocking::new(Arc::new(Timer::start().unwrap()))
}

/// A submitted job that reports when a helper takes it and then waits for its gate.
struct Gated {
    gate: Sender<()>,
    started: Receiver<()>,
    answers: Receiver<Result<u32, Interrupted>>,
}

fn gated(pool: &Blocking, deadline: Duration, value: u32) -> Gated {
    let (gate, wait) = mpsc::channel::<()>();
    let (start, started) = mpsc::channel();
    let (answer, answers) = mpsc::channel();
    pool.run_within(
        deadline,
        move || {
            let _ = start.send(());
            let _ = wait.recv();
            value
        },
        move |result| answer.send(result).unwrap(),
    );
    Gated {
        gate,
        started,
        answers,
    }
}

#[test]
fn a_job_answers_once_with_its_result_or_its_timeout() {
    let pool = pool();
    let prompt = gated(&pool, WAIT, 7);
    prompt.gate.send(()).unwrap();
    assert_eq!(prompt.answers.recv_timeout(WAIT), Ok(Ok(7)));

    let late = gated(&pool, SHORT, 8);
    late.started.recv_timeout(WAIT).unwrap();
    assert_eq!(
        late.answers.recv_timeout(WAIT),
        Ok(Err(Interrupted::Timeout))
    );
    late.gate.send(()).unwrap();
    // The timeout consumed the only answer, so the late result has nowhere to go.
    assert!(late.answers.recv_timeout(WAIT).is_err());
}

#[test]
fn eight_stuck_helpers_refuse_new_work_until_one_returns() {
    let pool = pool();
    let mut stuck = Vec::new();
    for value in 0..8 {
        let job = gated(&pool, SHORT, value);
        job.started.recv_timeout(WAIT).unwrap();
        assert_eq!(
            job.answers.recv_timeout(WAIT),
            Ok(Err(Interrupted::Timeout))
        );
        stuck.push(job.gate);
    }
    let refused = gated(&pool, WAIT, 100);
    assert_eq!(
        refused.answers.recv_timeout(WAIT),
        Ok(Err(Interrupted::StuckFilesystem))
    );

    // One helper returns; work is accepted and served again once it has counted itself out.
    stuck.pop().unwrap().send(()).unwrap();
    let deadline = Instant::now() + WAIT;
    loop {
        let job = gated(&pool, WAIT, 101);
        let _ = job.gate.send(());
        match job.answers.recv_timeout(WAIT) {
            Ok(Ok(value)) => {
                assert_eq!(value, 101);
                break;
            }
            Ok(Err(Interrupted::StuckFilesystem)) => {
                assert!(
                    Instant::now() < deadline,
                    "the returned helper never counted out"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            other => panic!("unexpected answer {other:?}"),
        }
    }
    for gate in stuck {
        gate.send(()).unwrap();
    }
}

#[test]
fn a_job_waits_for_a_free_helper() {
    let pool = pool();
    let busy: Vec<_> = (0..MAXIMUM_HELPERS)
        .map(|value| gated(&pool, WAIT, u32::try_from(value).unwrap()))
        .collect();
    for job in &busy {
        job.started.recv_timeout(WAIT).unwrap();
    }
    let queued = gated(&pool, WAIT, 50);
    queued.gate.send(()).unwrap();
    assert!(queued.started.recv_timeout(SHORT).is_err());
    busy[0].gate.send(()).unwrap();
    assert_eq!(queued.answers.recv_timeout(WAIT), Ok(Ok(50)));
    for job in busy {
        let _ = job.gate.send(());
        assert!(job.answers.recv_timeout(WAIT).unwrap().is_ok());
    }
}

#[test]
fn a_dropped_pool_answers_waiting_jobs_as_stopping() {
    let pool = pool();
    let busy: Vec<_> = (0..MAXIMUM_HELPERS)
        .map(|value| gated(&pool, WAIT, u32::try_from(value).unwrap()))
        .collect();
    for job in &busy {
        job.started.recv_timeout(WAIT).unwrap();
    }
    let queued = gated(&pool, WAIT, 9);
    drop(pool);
    assert_eq!(
        queued.answers.recv_timeout(WAIT),
        Ok(Err(Interrupted::Stopping))
    );
    for job in busy {
        job.gate.send(()).unwrap();
        assert!(job.answers.recv_timeout(WAIT).unwrap().is_ok());
    }
}
